import {
  core,
  isCoreError,
  type TelemetryIndicator,
  type TelemetryReadingInput,
  type TelemetryValue,
} from "../core-api";
import type { AstronautProfile, AstronautTelemetry, BloodPressure } from "../types";

export interface TelemetryPersistState {
  ok: boolean;
  stored: number;
  skipped: number;
  lastError: string | null;
  lastBatchId: string | null;
}

export interface FeedState {
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
    const id = String(row.id).trim();
    if (!id) throw new Error("Invalid crew identity");
    if (seen.has(id)) throw new Error("Duplicate crew identity");
    seen.add(id);
    const name = typeof row.astronaut_name === "string" && row.astronaut_name.trim() ? row.astronaut_name.trim() : "Unnamed crew member";
    const telemetry: AstronautTelemetry = {
      astronaut_name: name, blood_pressure: parsePressure(row.blood_pressure),
      heart_rate: reading(row.heart_rate), oxygen_in_blood: reading(row.oxygen_in_blood),
      blood_sugar: reading(row.blood_sugar), body_temperature: reading(row.body_temperature),
      sleep_hours: reading(row.sleep_hours), nutrition: reading(row.nutrition),
      space_radiation: reading(row.space_radiation), distance_from_earth: reading(row.distance_from_earth),
      mood: typeof row.mood === "string" && row.mood.trim() ? row.mood.trim() : null,
    };
    if ("hrv" in row) telemetry.hrv = reading(row.hrv);
    if ("body_battery" in row) telemetry.body_battery = reading(row.body_battery);
    return { id, name, telemetry };
  });
}

/** Convierte una lista saneada de tripulación en lecturas individuales para telemetría local. */
export function crewToReadings(crew: AstronautProfile[]): TelemetryReadingInput[] {
  const readings: TelemetryReadingInput[] = [];
  for (const member of crew) {
    // Reutilizamos exactamente la clave normalizada en parseCrew.
    const astronautKey = member.id;
    if (!astronautKey) continue;
    const astronautName = member.name.trim() || member.telemetry.astronaut_name.trim() || "Unnamed crew member";
    const t = member.telemetry;

    for (const item of NUMERIC_INDICATORS) {
      const value = item.pick(t);
      if (typeof value === "number" && Number.isFinite(value)) {
        readings.push({
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
    if (
      typeof systolic === "number" && Number.isFinite(systolic) &&
      typeof diastolic === "number" && Number.isFinite(diastolic)
    ) {
      readings.push({
        astronaut_key: astronautKey,
        astronaut_name: astronautName,
        indicator: "blood_pressure",
        value: { systolic, diastolic },
        unit: "mmHg",
      });
    }

    // Ánimo: solo si hay texto no vacío.
    if (typeof t.mood === "string" && t.mood.trim() !== "") {
      readings.push({
        astronaut_key: astronautKey,
        astronaut_name: astronautName,
        indicator: "mood",
        value: { label: t.mood.trim() },
        unit: "",
      });
    }
  }
  return readings;
}

function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && Boolean((window as unknown as Record<string, unknown>).__TAURI_INTERNALS__);
}

function formatError(error: unknown): string {
  if (isCoreError(error)) return error.message;
  if (error instanceof Error) return error.message;
  return String(error);
}

export function createTelemetryFeed(onUpdate: (state: FeedState) => void) {
  let state: FeedState = {
    crew: [],
    loading: true,
    live: false,
    lastSuccess: null,
    persist: { ok: true, stored: 0, skipped: 0, lastError: null, lastBatchId: null },
  };
  let inFlight = false;
  let persistInFlight = false;
  let pendingReadings: TelemetryReadingInput[] | null = null;
  let cancelActiveIpc: (() => void) | null = null;
  let stopped = false;
  let controller: AbortController | null = null;

  /** Limita una llamada IPC con Promise.race a 10 s y permite abortar la espera al hacer stop(). */
  function runWithIpcTimeout<T>(promise: Promise<T>): Promise<T> {
    let timerId: number | undefined;
    let cancelWait: (() => void) | undefined;
    const timeoutPromise = new Promise<never>((_, reject) => {
      timerId = window.setTimeout(() => {
        reject(new Error("Telemetry save timed out"));
      }, PERSIST_IPC_TIMEOUT_MS);
    });
    const stopPromise = new Promise<never>((_, reject) => {
      cancelWait = () => reject(new Error("Telemetry feed stopped"));
    });
    cancelActiveIpc = cancelWait ?? null;
    return Promise.race([promise, timeoutPromise, stopPromise]).finally(() => {
      if (timerId !== undefined) window.clearTimeout(timerId);
      if (cancelActiveIpc === cancelWait) cancelActiveIpc = null;
    });
  }

  /** Envía un lote a Rust en bloques de hasta 64 lecturas de forma secuencial. */
  async function persistBatch(readings: TelemetryReadingInput[]): Promise<void> {
    let stored = 0;
    let skipped = 0;
    let lastBatchId = state.persist?.lastBatchId ?? null;

    try {
      for (let offset = 0; offset < readings.length; offset += MAX_TELEMETRY_BATCH) {
        if (stopped) return;
        const batch = readings.slice(offset, offset + MAX_TELEMETRY_BATCH);
        const result = await runWithIpcTimeout(core.recordTelemetry({ readings: batch }));
        if (stopped) return;
        stored += result.stored;
        skipped += result.skipped;
        lastBatchId = result.batch_id;
      }
      if (stopped) return;
      state = {
        ...state,
        persist: { ok: true, stored, skipped, lastError: null, lastBatchId },
      };
    } catch (error: unknown) {
      if (stopped) return;
      state = {
        ...state,
        persist: { ok: false, stored, skipped, lastError: formatError(error), lastBatchId },
      };
    }
  }

  /** Drena en serie la cola de persistencia manteniendo solo el último lote pendiente. */
  async function drainPersistQueue(): Promise<void> {
    if (persistInFlight || stopped || !isTauriRuntime()) return;
    persistInFlight = true;
    try {
      while (!stopped && pendingReadings !== null) {
        const readings = pendingReadings;
        pendingReadings = null;
        if (readings.length === 0) continue;
        await persistBatch(readings);
        if (!stopped) onUpdate(state);
      }
    } finally {
      persistInFlight = false;
    }
  }

  /** Cola de un solo hueco con política "gana la última" separada del refresco HTTP. */
  function enqueuePersist(crew: AstronautProfile[]): void {
    if (stopped || !isTauriRuntime()) return;
    const readings = crewToReadings(crew);
    if (readings.length === 0) {
      pendingReadings = null;
      return;
    }
    pendingReadings = readings;
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
  const timer = window.setInterval(() => { void refresh(); }, 2000);
  return {
    refresh,
    stop(): void {
      stopped = true;
      pendingReadings = null;
      window.clearInterval(timer);
      controller?.abort();
      cancelActiveIpc?.();
    },
  };
}
