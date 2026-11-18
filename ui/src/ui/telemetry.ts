import {
  core,
  isCoreError,
  type TelemetryIndicator,
  type TelemetryReadingInput,
  type TelemetryValue,
} from "../core-api";
import type { AstronautProfile, AstronautTelemetry, BloodPressure } from "../types";
import { readingDomainError } from "../vital-status";

export type TelemetryPersistStatusKind = "ok" | "slow" | "error";

export interface TelemetryPersistState {
  status: TelemetryPersistStatusKind;
  ok: boolean;
  stored: number;
  skipped: number;
  rejected: number;
  lastRejection: string | null;
  lastError: string | null;
  lastBatchId: string | null;
}

export type ReadingValidationResult =
  | { ok: true; reason: null }
  | { ok: false; reason: string };

export interface ReadingBatchValidation {
  readings: TelemetryReadingInput[];
  rejected: number;
  lastRejection: string | null;
}

export interface FeedState {
  /** Saved backend replay, not a live sensor observation. */
  cached?: boolean;
  /** Dev-only fixed synthetic readings, never a live feed. */
  preview?: boolean;
  crew: AstronautProfile[];
  loading: boolean;
  live: boolean;
  lastSuccess: Date | null;
  persist?: TelemetryPersistState;
}

const MAX_TELEMETRY_BATCH = 64;
const PERSIST_IPC_TIMEOUT_MS = 10_000;
const MAX_ASTRONAUT_KEY_CHARS = 64;
const MAX_ASTRONAUT_NAME_CHARS = 100;
const MAX_UNIT_CHARS = 16;
const MAX_MOOD_LABEL_CHARS = 64;
const MAX_VALUE_JSON_BYTES = 2048;
const CONTROL_CHAR_RE = /\p{Cc}/u;
const textEncoder = new TextEncoder();

/** Mapeo uniforme de indicadores escalares del feed hacia el contrato de telemetría local. */
const NUMERIC_INDICATORS: ReadonlyArray<{
  indicator: TelemetryIndicator;
  unit: string;
  pick: (telemetry: AstronautTelemetry) => number | null | undefined;
  wrap: (value: number) => TelemetryValue;
}> = [
  { indicator: "heart_rate", unit: "bpm", pick: t => t.heart_rate, wrap: bpm => ({ bpm }) },
  { indicator: "oxygen_in_blood", unit: "%", pick: t => t.oxygen_in_blood, wrap: percentage => ({ percentage }) },
  { indicator: "glucose", unit: "mg/dL", pick: t => t.blood_sugar, wrap: mg_dl => ({ mg_dl }) },
  { indicator: "body_temperature", unit: "°C", pick: t => t.body_temperature, wrap: celsius => ({ celsius }) },
  { indicator: "sleep", unit: "h", pick: t => t.sleep_hours, wrap: hours => ({ hours }) },
  { indicator: "nutrition", unit: "kcal", pick: t => t.nutrition, wrap: kcal => ({ kcal }) },
  { indicator: "space_radiation", unit: "µSv/h", pick: t => t.space_radiation, wrap: usv_h => ({ usv_h }) },
  { indicator: "distance_from_earth", unit: "km", pick: t => t.distance_from_earth, wrap: km => ({ km }) },
  { indicator: "hrv", unit: "ms", pick: t => t.hrv, wrap: ms => ({ ms }) },
  { indicator: "body_battery", unit: "", pick: t => t.body_battery, wrap: value => ({ value }) },
];

function object(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function charCount(value: string): number {
  return [...value].length;
}

function trimPreservingControls(value: string): string {
  return CONTROL_CHAR_RE.test(value) ? value.replace(/^ +| +$/g, "") : value.trim();
}

function hasNonFiniteNumber(value: unknown): boolean {
  if (typeof value === "number") return !Number.isFinite(value);
  if (Array.isArray(value)) return value.some(hasNonFiniteNumber);
  if (object(value)) return Object.values(value).some(hasNonFiniteNumber);
  return false;
}

function invalid(reason: string): ReadingValidationResult {
  return { ok: false, reason };
}

/** Validate the IPC envelope and physical/domain limits before local persistence. */
export function validateReading(input: TelemetryReadingInput): ReadingValidationResult {
  if (typeof input.astronaut_key !== "string") return invalid("astronaut_key: invalid");
  if (CONTROL_CHAR_RE.test(input.astronaut_key)) return invalid("astronaut_key: control characters");
  if (input.astronaut_key.trim().length === 0) return invalid("astronaut_key: empty");
  if (charCount(input.astronaut_key) > MAX_ASTRONAUT_KEY_CHARS) return invalid("astronaut_key: too long");

  if (typeof input.astronaut_name !== "string") return invalid("astronaut_name: invalid");
  if (CONTROL_CHAR_RE.test(input.astronaut_name)) return invalid("astronaut_name: control characters");
  if (input.astronaut_name.trim().length === 0) return invalid("astronaut_name: empty");
  if (charCount(input.astronaut_name) > MAX_ASTRONAUT_NAME_CHARS) return invalid("astronaut_name: too long");

  if (typeof input.unit !== "string") return invalid("unit: invalid");
  if (charCount(input.unit) > MAX_UNIT_CHARS) return invalid("unit: too long");

  if (!object(input.value)) return invalid("value: invalid");
  const rawValue: Record<string, unknown> = input.value;
  if (hasNonFiniteNumber(rawValue)) return invalid("value: non-finite number");

  if (input.indicator === "mood") {
    const label = "label" in rawValue ? rawValue.label : undefined;
    if (typeof label !== "string") return invalid("mood: invalid");
    if (CONTROL_CHAR_RE.test(label)) return invalid("mood: control characters");
    if (label.trim().length === 0) return invalid("mood: empty");
    if (charCount(label) > MAX_MOOD_LABEL_CHARS) return invalid("mood: too long");
  } else if (input.indicator === "blood_pressure") {
    const sys = rawValue.systolic;
    const dia = rawValue.diastolic;
    if (typeof sys !== "number" || !Number.isFinite(sys) || typeof dia !== "number" || !Number.isFinite(dia)) {
      return invalid("blood_pressure: non-finite number");
    }
  } else {
    const hasFiniteNumeric = Object.values(rawValue).some(
      val => typeof val === "number" && Number.isFinite(val),
    );
    if (!hasFiniteNumeric) return invalid("value: non-finite number");
  }

  const domainError = readingDomainError(input.indicator, input.value);
  if (domainError) return invalid(domainError);

  let serialized: string;
  try {
    serialized = JSON.stringify(input.value);
  } catch {
    return invalid("value: invalid");
  }
  if (typeof serialized !== "string" || textEncoder.encode(serialized).length > MAX_VALUE_JSON_BYTES) {
    return invalid("value: too large");
  }

  return { ok: true, reason: null };
}

export function reading(value: unknown): number | null {
  if (typeof value !== "number" && typeof value !== "string") return null;
  if (typeof value === "string" && !value.trim()) return null;
  const number = Number(value);
  return Number.isFinite(number) ? number : null;
}
export function parsePressure(value: unknown): BloodPressure {
  if (typeof value !== "string") return { systolic: null, diastolic: null };
  const parts = value.split("/");
  if (parts.length !== 2) return { systolic: null, diastolic: null };
  return { systolic: reading(parts[0]), diastolic: reading(parts[1]) };
}
export function parseCrew(payload: unknown): AstronautProfile[] {
  if (!Array.isArray(payload)) throw new Error("Invalid crew response");
  const seen = new Set<string>();
  return payload.map((row: unknown) => {
    if (!object(row) || (typeof row.id !== "number" && typeof row.id !== "string")) {
      throw new Error("Invalid crew identity");
    }
    // Normalizamos la identidad una sola vez y comprobamos duplicados tras normalizar.
    const id = trimPreservingControls(String(row.id));
    if (!id) throw new Error("Invalid crew identity");
    if (seen.has(id)) throw new Error("Duplicate crew identity");
    seen.add(id);
    const trimmedName = typeof row.astronaut_name === "string" ? trimPreservingControls(row.astronaut_name) : "";
    const name = trimmedName || "Unnamed crew member";
    const trimmedMood = typeof row.mood === "string" ? trimPreservingControls(row.mood) : "";
    const invalidReadings: { indicator: string; reason: string }[] = [];
    const checkNumber = (value: unknown, indicator: string): void => {
      if (value !== null && value !== undefined && !(typeof value === "string" && !value.trim()) && reading(value) === null) {
        invalidReadings.push({ indicator, reason: "not a finite number" });
      }
    };
    const numericFields: [string, string][] = [
      ["heart_rate", "heart_rate"], ["oxygen_in_blood", "oxygen_in_blood"],
      ["blood_sugar", "glucose"], ["body_temperature", "body_temperature"],
      ["sleep_hours", "sleep"], ["nutrition", "nutrition"],
      ["space_radiation", "space_radiation"], ["distance_from_earth", "distance_from_earth"],
      ["hrv", "hrv"], ["body_battery", "body_battery"],
    ];
    for (const [field, indicator] of numericFields) checkNumber(row[field], indicator);
    const pressure = row.blood_pressure;
    if (typeof pressure === "string" && pressure.split("/").length === 2) {
      for (const part of pressure.split("/")) checkNumber(part, "blood_pressure");
    } else {
      checkNumber(pressure, "blood_pressure");
    }
    const telemetry: AstronautTelemetry = {
      astronaut_name: name, blood_pressure: parsePressure(row.blood_pressure),
      heart_rate: reading(row.heart_rate), oxygen_in_blood: reading(row.oxygen_in_blood),
      blood_sugar: reading(row.blood_sugar), body_temperature: reading(row.body_temperature),
      sleep_hours: reading(row.sleep_hours), nutrition: reading(row.nutrition),
      space_radiation: reading(row.space_radiation), distance_from_earth: reading(row.distance_from_earth),
      mood: trimmedMood || null,
      invalidIndicators: [...new Set(invalidReadings.map(entry => entry.indicator))],
    };
    if ("hrv" in row) telemetry.hrv = reading(row.hrv);
    if ("body_battery" in row) telemetry.body_battery = reading(row.body_battery);
    return invalidReadings.length ? { id, name, telemetry, invalid: invalidReadings } : { id, name, telemetry };
  });
}

/** Map backend names without inventing sensor timestamps or converting dose to rate. */
export function backendRowsForReplay(rows: Record<string, unknown>[]): Record<string, unknown>[] {
  return rows.map(row => ({
    id: row.id, astronaut_name: row.astronautName, heart_rate: row.heartRate,
    blood_pressure: row.bloodPressure, oxygen_in_blood: row.oxygenInBlood,
    blood_sugar: row.bloodSugar, body_temperature: row.bodyTemperature,
    sleep_hours: row.sleepHours, hrv: row.hrv, body_battery: row.bodyBattery, mood: row.mood,
    // Backend mSv is accumulated dose, not the app's µSv/h. Intake window is unverified.
    space_radiation: null, nutrition: null, distance_from_earth: row.distanceFromEarth,
  }));
}

/** Construye y valida cada lectura por separado: excluye las inválidas sin afectar al resto del lote. */
export function partitionCrewReadings(crew: AstronautProfile[]): ReadingBatchValidation {
  const readings: TelemetryReadingInput[] = [];
  let rejected = 0;
  let lastRejection: string | null = null;

  const pushValidated = (candidate: TelemetryReadingInput): void => {
    const check = validateReading(candidate);
    if (check.ok) {
      readings.push(candidate);
    } else {
      rejected += 1;
      lastRejection = check.reason;
    }
  };

  for (const member of crew) {
    // Normalizamos sin borrar caracteres de control para que validateReading rechace los inválidos.
    const astronautKey = trimPreservingControls(member.id);
    const rawName = trimPreservingControls(member.name);
    const rawTelemetryName = trimPreservingControls(member.telemetry.astronaut_name);
    const astronautName = rawName || rawTelemetryName || "Unnamed crew member";
    const t = member.telemetry;
    const rejectNumber = (indicator: string): void => {
      rejected += 1;
      lastRejection = indicator + ": not a finite number";
    };
    for (const entry of member.invalid ?? []) {
      rejected += 1;
      lastRejection = entry.indicator + ": " + entry.reason;
    }

    for (const item of NUMERIC_INDICATORS) {
      const value = item.pick(t);
      if (typeof value === "number" && !Number.isFinite(value)) {
        rejectNumber(item.indicator);
      } else if (typeof value === "number") {
        pushValidated({
          astronaut_key: astronautKey,
          astronaut_name: astronautName,
          indicator: item.indicator,
          value: item.wrap(value),
          unit: item.unit,
        });
      }
    }

    // Presión arterial: solo si existen ambos componentes válidos.
    const { systolic, diastolic } = t.blood_pressure;
    for (const value of [systolic, diastolic]) {
      if (typeof value === "number" && !Number.isFinite(value)) rejectNumber("blood_pressure");
    }
    if (
      typeof systolic === "number" && Number.isFinite(systolic) &&
      typeof diastolic === "number" && Number.isFinite(diastolic)
    ) {
      pushValidated({
        astronaut_key: astronautKey,
        astronaut_name: astronautName,
        indicator: "blood_pressure",
        value: { systolic, diastolic },
        unit: "mmHg",
      });
    }

    // Ánimo: solo si hay texto no vacío (los saltos/controles siguen visibles para rechazarse).
    if (typeof t.mood === "string") {
      const label = trimPreservingControls(t.mood);
      if (label !== "") {
        pushValidated({
          astronaut_key: astronautKey,
          astronaut_name: astronautName,
          indicator: "mood",
          value: { label },
          unit: "",
        });
      }
    }
  }
  return { readings, rejected, lastRejection };
}

/** Convierte una lista saneada de tripulación en lecturas válidas para telemetría local. */
export function crewToReadings(crew: AstronautProfile[]): TelemetryReadingInput[] {
  return partitionCrewReadings(crew).readings;
}

function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && Boolean((window as unknown as Record<string, unknown>).__TAURI_INTERNALS__);
}

function formatError(error: unknown): string {
  if (isCoreError(error)) return error.message;
  if (error instanceof Error) return error.message;
  return String(error);
}

export function createTelemetryFeed(onUpdate: (state: FeedState) => void, cacheOnly = isTauriRuntime()) {
  let state: FeedState = {
    crew: [],
    loading: true,
    live: false,
    lastSuccess: null,
    persist: {
      status: "ok",
      ok: true,
      stored: 0,
      skipped: 0,
      rejected: 0,
      lastRejection: null,
      lastError: null,
      lastBatchId: null,
    },
  };
  let inFlight = false;
  let persistInFlight = false;
  let pendingBatch: ReadingBatchValidation | null = null;
  let clearActiveIpcTimer: (() => void) | null = null;
  let stopped = false;
  let controller: AbortController | null = null;

  /** Espera la promesa IPC real manteniendo el candado de cola y avisando si supera 10 s. */
  async function awaitIpcWithSlowWarning<T>(
    promise: Promise<T>,
    onSlow: () => void,
  ): Promise<T> {
    let timerId: number | undefined = window.setTimeout(() => {
      timerId = undefined;
      if (!stopped) onSlow();
    }, PERSIST_IPC_TIMEOUT_MS);

    const clearTimer = (): void => {
      if (timerId !== undefined) {
        window.clearTimeout(timerId);
        timerId = undefined;
      }
      if (clearActiveIpcTimer === clearTimer) {
        clearActiveIpcTimer = null;
      }
    };
    clearActiveIpcTimer = clearTimer;

    try {
      return await promise;
    } finally {
      clearTimer();
    }
  }

  /** Envía un lote a Rust en bloques de hasta 64 lecturas de forma secuencial. */
  async function persistBatch(batchData: ReadingBatchValidation): Promise<void> {
    const { readings, rejected, lastRejection } = batchData;
    let stored = 0;
    let skipped = 0;
    let lastBatchId = state.persist?.lastBatchId ?? null;

    try {
      for (let offset = 0; offset < readings.length; offset += MAX_TELEMETRY_BATCH) {
        if (stopped) return;
        const batch = readings.slice(offset, offset + MAX_TELEMETRY_BATCH);
        const result = await awaitIpcWithSlowWarning(
          core.recordTelemetry({ readings: batch }),
          () => {
            if (stopped) return;
            // El aviso de lentitud mantiene ok: true (lento != fallo) y no libera la cola hasta terminar el invoke.
            state = {
              ...state,
              persist: {
                status: "slow",
                ok: true,
                stored,
                skipped,
                rejected,
                lastRejection,
                lastError: null,
                lastBatchId,
              },
            };
            onUpdate(state);
          },
        );
        if (stopped) return;
        stored += result.stored;
        skipped += result.skipped;
        lastBatchId = result.batch_id;
      }
      if (stopped) return;
      state = {
        ...state,
        persist: {
          status: "ok",
          ok: true,
          stored,
          skipped,
          rejected,
          lastRejection,
          lastError: null,
          lastBatchId,
        },
      };
    } catch (error: unknown) {
      if (stopped) return;
      state = {
        ...state,
        persist: {
          status: "error",
          ok: false,
          stored,
          skipped,
          rejected,
          lastRejection,
          lastError: formatError(error),
          lastBatchId,
        },
      };
    }
  }

  /** Drena en serie la cola de persistencia manteniendo solo el último lote pendiente. */
  async function drainPersistQueue(): Promise<void> {
    if (persistInFlight || stopped || !isTauriRuntime()) return;
    persistInFlight = true;
    try {
      while (!stopped && pendingBatch !== null) {
        const batch = pendingBatch;
        pendingBatch = null;
        if (batch.readings.length === 0) {
          if (batch.rejected > 0) {
            const prev = state.persist ?? {
              status: "ok" as const,
              ok: true,
              stored: 0,
              skipped: 0,
              rejected: 0,
              lastRejection: null,
              lastError: null,
              lastBatchId: null,
            };
            state = {
              ...state,
              persist: {
                ...prev,
                rejected: batch.rejected,
                lastRejection: batch.lastRejection,
              },
            };
            onUpdate(state);
          }
          continue;
        }
        await persistBatch(batch);
        if (!stopped) onUpdate(state);
      }
    } finally {
      persistInFlight = false;
    }
  }

  /** Cola de un solo hueco con política "gana la última" separada del refresco HTTP. */
  function enqueuePersist(crew: AstronautProfile[]): void {
    if (stopped || !isTauriRuntime()) return;
    const batch = partitionCrewReadings(crew);
    if (batch.readings.length === 0 && batch.rejected === 0) {
      pendingBatch = null;
      return;
    }
    pendingBatch = batch;
    if (!persistInFlight) {
      void drainPersistQueue();
    }
  }

  async function refresh(): Promise<void> {
    if (inFlight || stopped) return;
    inFlight = true;
    controller = new AbortController();
    const timeout = window.setTimeout(() => controller?.abort(), 8000);
    let crewToPersist: AstronautProfile[] | null = null;
    try {
      if (cacheOnly) {
        const cache = await core.cachedCrew();
        if (stopped) return;
        const crew = cache ? parseCrew(backendRowsForReplay(cache.crew)) : [];
        state = {...state, crew, loading: false, live: false, cached: true, lastSuccess: cache ? new Date(cache.received_at) : null};
        return; // Never re-record cached values as newly observed telemetry.
      }
      const response = await fetch("/api/astronauts", { signal: controller.signal, cache: "no-store" });
      if (!response.ok) throw new Error("Crew request failed");
      const crew = parseCrew(await response.json());
      if (stopped) return;
      state = { ...state, crew, loading: false, live: true, lastSuccess: new Date() };
      crewToPersist = crew;
    } catch {
      if (!stopped) {
        state = { ...state, loading: false, live: false };
      }
    } finally {
      window.clearTimeout(timeout);
      inFlight = false;
      if (!stopped) {
        onUpdate(state);
        if (crewToPersist) enqueuePersist(crewToPersist);
      }
    }
  }
  onUpdate(state);
  void refresh();
  const timer = cacheOnly ? null : window.setInterval(() => { void refresh(); }, 2000);
  const saved = (): void => { void refresh(); };
  if (cacheOnly) window.addEventListener("pulsys:backend-snapshot", saved);
  return {
    refresh,
    stop(): void {
      stopped = true;
      pendingBatch = null;
      if (timer !== null) window.clearInterval(timer);
      if (cacheOnly) window.removeEventListener("pulsys:backend-snapshot", saved);
      controller?.abort();
      clearActiveIpcTimer?.();
    },
  };
}
