import {
  core,
  isCoreError,
  type TelemetryIndicator,
  type TelemetryItem,
  type TelemetryProblem,
  type VerifyTelemetryResult,
} from "../core-api";
import { el, numberText, setText, utc } from "./dom";

const SVG_NS = "http://www.w3.org/2000/svg";
const MAX_POINTS = 200;
const RECENT_LIST_LIMIT = 10;
const SELECTOR_DEBOUNCE_MS = 250;
const VERIFY_COOLDOWN_MS = 60_000;
const TRACE_HELP_TEXT = "Detects edits and missing records on this device.";

const PROBLEM_LABELS: Record<TelemetryProblem, string> = {
  hash_mismatch: "hash mismatch",
  broken_link: "broken link",
  missing_tail: "missing tail",
  missing_series: "missing series",
  count_mismatch: "count mismatch",
};

interface IndicatorOption {
  key: TelemetryIndicator;
  label: string;
  unit: string;
}

/** Catálogo cerrado de indicadores de telemetría local y sus etiquetas en inglés. */
const TELEMETRY_INDICATORS: readonly IndicatorOption[] = [
  { key: "heart_rate", label: "Heart rate", unit: "bpm" },
  { key: "oxygen_in_blood", label: "Blood oxygen · SpO₂", unit: "%" },
  { key: "blood_pressure", label: "Blood pressure", unit: "mmHg" },
  { key: "glucose", label: "Blood glucose", unit: "mg/dL" },
  { key: "body_temperature", label: "Body temperature", unit: "°C" },
  { key: "sleep", label: "Sleep", unit: "h" },
  { key: "nutrition", label: "Nutrition", unit: "kcal" },
  { key: "space_radiation", label: "Space radiation", unit: "µSv/h" },
  { key: "distance_from_earth", label: "Distance from Earth", unit: "km" },
  { key: "mood", label: "Mood", unit: "" },
  { key: "hrv", label: "HRV", unit: "ms" },
  { key: "body_battery", label: "Body battery", unit: "" },
];

const SCALAR_KEYS = [
  "bpm",
  "percentage",
  "mg_dl",
  "celsius",
  "hours",
  "kcal",
  "usv_h",
  "km",
  "ms",
  "value",
  "score",
] as const;

let instanceCounter = 0;

export interface CrewChoice {
  id: string;
  name: string;
}

export type TelemetryPersistStateKind = "ok" | "slow" | "error";

export interface TelemetryPersistStatus {
  status?: TelemetryPersistStateKind;
  ok: boolean;
  stored: number;
  skipped: number;
  rejected?: number;
  lastRejection?: string | null;
  lastError: string | null;
}

export interface TelemetryHistoryHandle {
  refresh(): Promise<void>;
  setPersistStatus(persist: TelemetryPersistStatus | null): void;
  destroy(): void;
}

function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && Boolean((window as unknown as Record<string, unknown>).__TAURI_INTERNALS__);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function finiteNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function formatError(error: unknown): string {
  if (isCoreError(error)) return error.message;
  if (error instanceof Error) return error.message;
  return String(error);
}

interface ExtractedReading {
  primary: number | null;
  secondary: number | null;
  text: string;
}

/** Extrae valores numéricos y texto legible desde el JSON genérico `TelemetryItem.value`. */
function extractValue(item: TelemetryItem): ExtractedReading {
  const raw = item.value;
  if (typeof raw === "number" && Number.isFinite(raw)) {
    return { primary: raw, secondary: null, text: numberText(raw) };
  }
  if (!isRecord(raw)) {
    return { primary: null, secondary: null, text: typeof raw === "string" && raw.trim() ? raw.trim() : "--" };
  }
  if ("systolic" in raw && "diastolic" in raw) {
    const sys = finiteNumber(raw.systolic);
    const dia = finiteNumber(raw.diastolic);
    return {
      primary: sys,
      secondary: dia,
      text: `${numberText(sys)} / ${numberText(dia)}`,
    };
  }
  if ("label" in raw && typeof raw.label === "string") {
    const label = raw.label.trim();
    return { primary: null, secondary: null, text: label || "--" };
  }
  for (const key of SCALAR_KEYS) {
    if (key in raw) {
      const num = finiteNumber(raw[key]);
      if (num !== null) {
        return { primary: num, secondary: null, text: numberText(num) };
      }
    }
  }
  return { primary: null, secondary: null, text: "--" };
}

function formatWithUnit(item: TelemetryItem, fallbackUnit: string): string {
  const { text } = extractValue(item);
  if (text === "--") return "--";
  const unit = item.unit.trim() || fallbackUnit;
  return unit ? `${text} ${unit}` : text;
}

interface ObservedIndicatorStats {
  count: number;
  maxReadingId: number | null;
}

type TraceStaleNotice = "newer" | "unknown" | null;

interface TraceSnapshot {
  result: VerifyTelemetryResult;
  checkedAtMs: number;
}

/** Ordena una copia cronológicamente (antiguo → reciente) solo para la gráfica SVG, sin tocar la lista. */
function sortChronologicalForChart(items: TelemetryItem[]): TelemetryItem[] {
  return [...items].sort((a, b) => {
    const ta = Date.parse(a.observed_at);
    const tb = Date.parse(b.observed_at);
    const aValid = Number.isFinite(ta);
    const bValid = Number.isFinite(tb);
    if (aValid && bValid && ta !== tb) return ta - tb;
    return a.reading_id - b.reading_id;
  });
}

function buildTraceabilityText(snapshot: TraceSnapshot, staleNotice: TraceStaleNotice): string {
  const { result, checkedAtMs } = snapshot;
  const timeUtc = utc(new Date(checkedAtMs), true);
  const base = `Checked ${timeUtc} · ${result.checked} readings in chain`;
  const statusText = result.ok
    ? "consistent ✓"
    : (() => {
        const problemText = result.problem ? PROBLEM_LABELS[result.problem] : "integrity issue";
        return result.first_bad_reading_id !== null
          ? `chain problem: ${problemText} at #${result.first_bad_reading_id}`
          : `chain problem: ${problemText}`;
      })();
  const staleSuffix =
    staleNotice === "newer"
      ? " · newer readings not yet checked"
      : staleNotice === "unknown"
        ? " · scope unknown"
        : "";
  return `${base} · ${statusText}${staleSuffix}`;
}

function buildLineChart(items: TelemetryItem[], option: IndicatorOption): HTMLElement {
  const figure = el("figure", "tele-hist-figure");
  if (option.key === "mood") {
    figure.append(el("p", "caption tele-hist-note", "Mood is stored as text labels. Recent entries are listed below."));
    return figure;
  }

  // Tomamos hasta 200 puntos y ordenamos aparte cronológicamente de izquierda (antiguo) a derecha (reciente).
  const chronological = sortChronologicalForChart(items.slice(0, MAX_POINTS));
  const points = chronological
    .map(item => ({ item, extracted: extractValue(item) }))
    .filter((entry): entry is { item: TelemetryItem; extracted: ExtractedReading & { primary: number } } =>
      entry.extracted.primary !== null,
    );

  if (points.length === 0) {
    figure.append(el("p", "caption tele-hist-note", "No numeric readings available to plot."));
    return figure;
  }

  const allValues: number[] = [];
  for (const point of points) {
    allValues.push(point.extracted.primary);
    if (point.extracted.secondary !== null) allValues.push(point.extracted.secondary);
  }
  const min = Math.min(...allValues);
  const max = Math.max(...allValues);
  const unitSuffix = option.unit ? ` ${option.unit}` : "";

  const width = 340;
  const height = 116;
  const padX = 12;
  const padY = 12;
  const plotW = width - padX * 2;
  const plotH = height - padY * 2;

  const xAt = (index: number): number =>
    points.length === 1 ? width / 2 : padX + (index / (points.length - 1)) * plotW;
  const yAt = (value: number): number =>
    min === max ? height / 2 : padY + plotH - ((value - min) / (max - min)) * plotH;

  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", `0 0 ${width} ${height}`);
  svg.setAttribute("class", "tele-hist-chart");
  svg.setAttribute("role", "img");
  const ariaSummary =
    `${option.label} history: ${points.length} readings ranging from ` +
    `${numberText(min)} to ${numberText(max)}${unitSuffix}.`;
  svg.setAttribute("aria-label", ariaSummary);

  const title = document.createElementNS(SVG_NS, "title");
  title.textContent = ariaSummary;
  svg.append(title);

  // Líneas guía horizontales para facilitar la lectura visual sin sobrecargar el SVG.
  for (const guideY of [padY, height / 2, height - padY]) {
    const guide = document.createElementNS(SVG_NS, "line");
    guide.setAttribute("x1", String(padX));
    guide.setAttribute("x2", String(width - padX));
    guide.setAttribute("y1", String(guideY));
    guide.setAttribute("y2", String(guideY));
    guide.setAttribute("class", "tele-hist-grid");
    svg.append(guide);
  }

  // Serie secundaria (presión diastólica) si aplica.
  const secondaryCoords = points
    .map((point, index) =>
      point.extracted.secondary !== null ? `${xAt(index).toFixed(1)},${yAt(point.extracted.secondary).toFixed(1)}` : null,
    )
    .filter((coord): coord is string => coord !== null);

  if (option.key === "blood_pressure" && secondaryCoords.length >= 2) {
    const secondaryLine = document.createElementNS(SVG_NS, "polyline");
    secondaryLine.setAttribute("points", secondaryCoords.join(" "));
    secondaryLine.setAttribute("class", "tele-hist-line-secondary");
    svg.append(secondaryLine);
  }

  if (points.length >= 2) {
    const line = document.createElementNS(SVG_NS, "polyline");
    line.setAttribute(
      "points",
      points.map((point, index) => `${xAt(index).toFixed(1)},${yAt(point.extracted.primary).toFixed(1)}`).join(" "),
    );
    line.setAttribute("class", "tele-hist-line");
    svg.append(line);
  }

  const dotRadius = points.length > 40 ? 1.6 : 2.5;
  points.forEach((point, index) => {
    const dot = document.createElementNS(SVG_NS, "circle");
    dot.setAttribute("cx", xAt(index).toFixed(1));
    dot.setAttribute("cy", yAt(point.extracted.primary).toFixed(1));
    dot.setAttribute("r", index === points.length - 1 ? "3.2" : String(dotRadius));
    dot.setAttribute("class", index === points.length - 1 ? "tele-hist-dot is-latest" : "tele-hist-dot");
    svg.append(dot);
  });

  const firstItem = points[0].item;
  const lastItem = points[points.length - 1].item;
  const figcaption = el(
    "figcaption",
    "tele-hist-meta caption",
    `Range ${numberText(min)}–${numberText(max)}${unitSuffix} · ${points.length} pts` +
      (option.key === "blood_pressure" ? " · Systolic (solid) / Diastolic (dashed)" : ""),
  );
  const windowText = el(
    "p",
    "tele-hist-window caption",
    `${utc(firstItem.observed_at, true)} — ${utc(lastItem.observed_at, true)}`,
  );

  figure.append(svg, figcaption, windowText);
  return figure;
}

function buildRecentList(itemsReceivedOrder: TelemetryItem[], option: IndicatorOption): HTMLElement {
  const wrapper = el("div", "tele-hist-list-wrap");
  wrapper.append(el("h4", "tele-hist-subtitle", "Latest 10 readings"));

  const list = el("ul", "tele-hist-list");
  // Conservamos el orden recibido del backend (reading_id DESC).
  for (const item of itemsReceivedOrder.slice(0, RECENT_LIST_LIMIT)) {
    const row = el("li", "tele-hist-item");
    const time = el("time", "tele-hist-time", utc(item.observed_at, true));
    if (Number.isFinite(Date.parse(item.observed_at))) {
      time.dateTime = item.observed_at;
    }
    const val = el("span", "tele-hist-value data", formatWithUnit(item, option.unit));
    row.append(time, val);
    list.append(row);
  }
  wrapper.append(list);
  return wrapper;
}

/** Monta el panel compacto de historial y trazabilidad de telemetría local dentro de `host`. */
export function mountTelemetryHistory(
  host: HTMLElement,
  getCrew: () => CrewChoice[],
): TelemetryHistoryHandle {
  const uid = ++instanceCounter;
  const crewSelectId = `tele-hist-crew-${uid}`;
  const indicatorSelectId = `tele-hist-ind-${uid}`;

  const panel = el("section", "tele-hist-panel");
  panel.setAttribute("aria-label", "Local telemetry history");

  const header = el("div", "tele-hist-header");
  const titleWrap = el("div", "tele-hist-title-wrap");
  titleWrap.append(
    el("p", "eyebrow tele-hist-eyebrow", "Local telemetry"),
    el("h3", "tele-hist-title", "Traceable history"),
  );
  const refreshButton = el("button", "button secondary tele-hist-refresh", "Refresh");
  refreshButton.type = "button";
  header.append(titleWrap, refreshButton);

  const controls = el("div", "tele-hist-controls");

  const crewField = el("div", "tele-hist-field");
  const crewLabel = el("label", "tele-hist-label", "Astronaut");
  crewLabel.htmlFor = crewSelectId;
  const crewSelect = el("select", "tele-hist-select");
  crewSelect.id = crewSelectId;
  crewField.append(crewLabel, crewSelect);

  const indicatorField = el("div", "tele-hist-field");
  const indicatorLabel = el("label", "tele-hist-label", "Indicator");
  indicatorLabel.htmlFor = indicatorSelectId;
  const indicatorSelect = el("select", "tele-hist-select");
  indicatorSelect.id = indicatorSelectId;
  for (const opt of TELEMETRY_INDICATORS) {
    const optionEl = el("option", "", opt.label);
    optionEl.value = opt.key;
    indicatorSelect.append(optionEl);
  }
  indicatorField.append(indicatorLabel, indicatorSelect);

  controls.append(crewField, indicatorField);

  const statusLine = el("p", "tele-hist-status caption", "");
  statusLine.setAttribute("role", "status");
  statusLine.setAttribute("aria-live", "polite");

  const persistLine = el("p", "tele-hist-persist caption", "");
  persistLine.setAttribute("role", "status");
  persistLine.setAttribute("aria-live", "polite");
  persistLine.hidden = true;

  const traceBox = el("div", "tele-hist-trace-box");
  const traceRow = el("div", "tele-hist-trace-row");

  const traceLine = el("p", "tele-hist-trace caption", "");
  traceLine.title = TRACE_HELP_TEXT;
  traceLine.hidden = true;

  const verifyButton = el("button", "button secondary tele-hist-verify", "Verify chain");
  verifyButton.type = "button";
  verifyButton.title = TRACE_HELP_TEXT;

  traceRow.append(traceLine, verifyButton);

  const helpLine = el("p", "caption tele-hist-help", TRACE_HELP_TEXT);
  helpLine.title = TRACE_HELP_TEXT;

  traceBox.append(traceRow, helpLine);

  const body = el("div", "tele-hist-body");

  panel.append(header, controls, statusLine, persistLine, traceBox, body);
  host.replaceChildren(panel);

  let selectedCrewId = "";
  let selectedIndicator: TelemetryIndicator = TELEMETRY_INDICATORS[0].key;
  let crewSignature = "";
  let requestSeq = 0;
  let activeLoadSeq: number | null = null;
  let listInFlight = false;
  let pendingRefresh = false;
  let pendingRefreshPromise: Promise<void> | null = null;
  let resolvePendingRefresh: (() => void) | null = null;
  let verifyInFlight = false;
  let pendingVerifyCrewId: string | null = null;
  let lastTrackedCrewId = "";
  let selectorTimer: number | null = null;
  const lastVerifiedAt = new Map<string, number>();
  const lastTraceSnapshot = new Map<string, TraceSnapshot>();
  const observedByCrew = new Map<string, Map<TelemetryIndicator, ObservedIndicatorStats>>();
  let destroyed = false;

  function clearLoadingBusy(): void {
    activeLoadSeq = null;
    body.removeAttribute("aria-busy");
  }

  function finishPendingWaiters(): void {
    if (resolvePendingRefresh) {
      const resolve = resolvePendingRefresh;
      resolvePendingRefresh = null;
      pendingRefreshPromise = null;
      resolve();
    }
  }

  function updateVerifyButtonState(): void {
    verifyButton.disabled = destroyed || !isTauriRuntime() || !selectedCrewId || verifyInFlight;
  }

  function setPersistStatus(persist: TelemetryPersistStatus | null): void {
    if (destroyed) return;
    if (!persist) {
      persistLine.hidden = true;
      delete persistLine.dataset.tone;
      delete persistLine.dataset.state;
      setText(persistLine, "");
      return;
    }
    persistLine.hidden = false;
    const phase: TelemetryPersistStateKind = persist.status ?? (persist.ok ? "ok" : "error");
    persistLine.dataset.state = phase;
    const rejectedSuffix =
      persist.rejected && persist.rejected > 0
        ? ` · ${persist.rejected} excluded (${persist.lastRejection ?? "invalid reading"})`
        : "";

    if (phase === "slow") {
      persistLine.dataset.tone = "warn";
      setText(
        persistLine,
        `Saving to device is taking longer than usual; waiting for it to finish.${rejectedSuffix}`,
      );
    } else if (phase === "ok") {
      persistLine.dataset.tone = "ok";
      setText(persistLine, `Saving to device: OK · last batch stored ${persist.stored}${rejectedSuffix}`);
    } else {
      persistLine.dataset.tone = "alarm";
      const reason = persist.lastError?.trim() || "unknown error";
      setText(persistLine, `Saving to device failed: ${reason}${rejectedSuffix}`);
    }
  }

  function recordObservedReadings(
    crewId: string,
    indicator: TelemetryIndicator,
    items: TelemetryItem[],
  ): void {
    let maxReadingId: number | null = null;
    for (const item of items) {
      if (Number.isFinite(item.reading_id) && (maxReadingId === null || item.reading_id > maxReadingId)) {
        maxReadingId = item.reading_id;
      }
    }
    const stats: ObservedIndicatorStats = { count: items.length, maxReadingId };
    let crewObs = observedByCrew.get(crewId);
    if (!crewObs) {
      crewObs = new Map();
      observedByCrew.set(crewId, crewObs);
    }
    crewObs.set(indicator, stats);
  }

  function getMaxObservedReadingId(crewId: string): number | null {
    const crewObs = observedByCrew.get(crewId);
    if (!crewObs) return null;
    let maxId: number | null = null;
    for (const stats of crewObs.values()) {
      if (stats.maxReadingId !== null && (maxId === null || stats.maxReadingId > maxId)) {
        maxId = stats.maxReadingId;
      }
    }
    return maxId;
  }

  function getTraceStaleNotice(crewId: string, result: VerifyTelemetryResult): TraceStaleNotice {
    const maxObserved = getMaxObservedReadingId(crewId);
    if (maxObserved === null) return null;
    const checkedThrough = result.checked_through_reading_id ?? null;
    if (checkedThrough === null) return "unknown";
    return maxObserved > checkedThrough ? "newer" : null;
  }

  function applyCachedTrace(crewId: string): void {
    const snapshot = lastTraceSnapshot.get(crewId);
    if (snapshot) {
      traceLine.hidden = false;
      traceLine.dataset.tone = snapshot.result.ok ? "ok" : "alarm";
      setText(traceLine, buildTraceabilityText(snapshot, getTraceStaleNotice(crewId, snapshot.result)));
      return;
    }
    if (!verifyInFlight) {
      traceLine.hidden = true;
      delete traceLine.dataset.tone;
      setText(traceLine, "");
    }
  }

  /** Ejecuta la verificación de cadena de un astronauta evitando llamadas simultáneas. */
  async function runVerify(crewId: string): Promise<void> {
    if (destroyed || !isTauriRuntime() || !crewId || verifyInFlight) return;
    verifyInFlight = true;
    updateVerifyButtonState();

    if (selectedCrewId === crewId) {
      traceLine.hidden = false;
      delete traceLine.dataset.tone;
      setText(traceLine, "Checking chain…");
    }

    try {
      const verify = await core.verifyTelemetry({ astronaut_key: crewId });
      if (destroyed) return;
      const checkedAtMs = Date.now();
      lastVerifiedAt.set(crewId, checkedAtMs);
      lastTraceSnapshot.set(crewId, {
        result: verify,
        checkedAtMs,
      });
      if (selectedCrewId === crewId) {
        applyCachedTrace(crewId);
      }
    } catch (error: unknown) {
      if (destroyed) return;
      if (selectedCrewId === crewId) {
        traceLine.hidden = false;
        traceLine.dataset.tone = "alarm";
        setText(traceLine, "Chain check failed: " + formatError(error));
      }
    } finally {
      verifyInFlight = false;
      updateVerifyButtonState();
      if (!destroyed && pendingVerifyCrewId && pendingVerifyCrewId !== crewId && pendingVerifyCrewId === selectedCrewId) {
        const nextCrewId = pendingVerifyCrewId;
        pendingVerifyCrewId = null;
        maybeVerifyCrew(nextCrewId);
      } else {
        pendingVerifyCrewId = null;
      }
    }
  }

  /** Lanza verifyTelemetry al montar o cambiar de astronauta respetando el intervalo de 60 s. */
  function maybeVerifyCrew(crewId: string): void {
    if (destroyed || !isTauriRuntime() || !crewId) return;
    const prevTimestamp = lastVerifiedAt.get(crewId);
    if (prevTimestamp !== undefined && Date.now() - prevTimestamp < VERIFY_COOLDOWN_MS) {
      applyCachedTrace(crewId);
      return;
    }
    if (verifyInFlight) {
      pendingVerifyCrewId = crewId;
      return;
    }
    void runVerify(crewId);
  }

  function syncCrewOptions(): CrewChoice[] {
    const crew = getCrew();
    const signature = JSON.stringify(crew.map(member => [member.id, member.name]));
    if (signature !== crewSignature) {
      crewSignature = signature;
      crewSelect.replaceChildren();
      if (crew.length === 0) {
        const emptyOpt = el("option", "", "No crew available");
        emptyOpt.value = "";
        crewSelect.append(emptyOpt);
      } else {
        for (const member of crew) {
          const opt = el("option", "", member.name || member.id);
          opt.value = member.id;
          crewSelect.append(opt);
        }
      }
    }
    if (crew.length === 0) {
      selectedCrewId = "";
      crewSelect.value = "";
    } else if (!crew.some(member => member.id === selectedCrewId)) {
      selectedCrewId = crew[0].id;
      crewSelect.value = selectedCrewId;
    } else {
      crewSelect.value = selectedCrewId;
    }
    return crew;
  }

  function renderOutsideTauri(): void {
    clearLoadingBusy();
    crewSelect.disabled = true;
    indicatorSelect.disabled = true;
    refreshButton.disabled = true;
    updateVerifyButtonState();
    delete statusLine.dataset.tone;
    setText(statusLine, "History is available in the app");
    traceLine.hidden = true;
    helpLine.hidden = true;
    body.replaceChildren(el("p", "tele-hist-empty", "History is available in the app"));
  }

  async function executeListPass(): Promise<void> {
    if (destroyed) return;
    const seq = ++requestSeq;
    const crew = syncCrewOptions();

    if (!isTauriRuntime()) {
      lastTrackedCrewId = "";
      renderOutsideTauri();
      return;
    }

    indicatorSelect.disabled = false;
    refreshButton.disabled = false;
    crewSelect.disabled = crew.length === 0;
    helpLine.hidden = false;
    updateVerifyButtonState();

    if (!selectedCrewId) {
      clearLoadingBusy();
      lastTrackedCrewId = "";
      delete statusLine.dataset.tone;
      setText(statusLine, "No readings stored yet");
      traceLine.hidden = true;
      body.replaceChildren(el("p", "tele-hist-empty", "No readings stored yet"));
      return;
    }

    // Solo verificamos al montar o cambiar de astronauta (máximo 1 vez cada 60 s);
    // el refresco habitual únicamente llama a listTelemetry.
    if (selectedCrewId !== lastTrackedCrewId) {
      lastTrackedCrewId = selectedCrewId;
      applyCachedTrace(selectedCrewId);
      maybeVerifyCrew(selectedCrewId);
    }

    const indicatorOption =
      TELEMETRY_INDICATORS.find(item => item.key === selectedIndicator) ?? TELEMETRY_INDICATORS[0];
    // Capturamos astronauta e indicador al lanzar la consulta para descartar respuestas obsoletas.
    const queryCrewId = selectedCrewId;
    const queryIndicator = indicatorOption.key;

    delete statusLine.dataset.tone;
    setText(statusLine, "Loading history…");
    activeLoadSeq = seq;
    body.setAttribute("aria-busy", "true");
    listInFlight = true;

    try {
      const rawItems = await core.listTelemetry({
        astronaut_key: queryCrewId,
        indicator: queryIndicator,
        limit: MAX_POINTS,
      });

      if (destroyed) {
        if (activeLoadSeq === seq) clearLoadingBusy();
        return;
      }
      if (
        seq !== requestSeq ||
        queryCrewId !== selectedCrewId ||
        queryIndicator !== selectedIndicator
      ) {
        if (activeLoadSeq === seq) clearLoadingBusy();
        return;
      }
      if (activeLoadSeq === seq) clearLoadingBusy();

      recordObservedReadings(queryCrewId, queryIndicator, rawItems);
      applyCachedTrace(queryCrewId);

      if (rawItems.length === 0) {
        setText(statusLine, "No readings stored yet");
        body.replaceChildren(el("p", "tele-hist-empty", "No readings stored yet"));
        return;
      }

      setText(statusLine, `${rawItems.length} readings loaded · ${indicatorOption.label}`);
      body.replaceChildren(
        buildLineChart(rawItems, indicatorOption),
        buildRecentList(rawItems, indicatorOption),
      );
    } catch (error: unknown) {
      if (destroyed) {
        if (activeLoadSeq === seq) clearLoadingBusy();
        return;
      }
      if (
        seq !== requestSeq ||
        queryCrewId !== selectedCrewId ||
        queryIndicator !== selectedIndicator
      ) {
        if (activeLoadSeq === seq) clearLoadingBusy();
        return;
      }
      if (activeLoadSeq === seq) clearLoadingBusy();
      statusLine.dataset.tone = "alarm";
      const message = "Could not load history: " + formatError(error);
      setText(statusLine, message);
      body.replaceChildren(el("p", "tele-hist-empty", message));
    } finally {
      listInFlight = false;
    }
  }

  /** Ejecuta el listado evitando llamadas simultáneas: 1 en vuelo y como máximo 1 pendiente. */
  function refresh(): Promise<void> {
    if (destroyed) return Promise.resolve();
    syncCrewOptions();
    if (!isTauriRuntime() || !selectedCrewId) {
      pendingRefresh = false;
      finishPendingWaiters();
      return executeListPass();
    }
    if (listInFlight) {
      pendingRefresh = true;
      if (!pendingRefreshPromise) {
        pendingRefreshPromise = new Promise<void>(resolve => {
          resolvePendingRefresh = resolve;
        });
      }
      return pendingRefreshPromise;
    }

    return (async () => {
      try {
        await executeListPass();
        while (!destroyed && pendingRefresh) {
          pendingRefresh = false;
          const resolveWaiters = resolvePendingRefresh;
          resolvePendingRefresh = null;
          pendingRefreshPromise = null;
          if (!isTauriRuntime() || !selectedCrewId) {
            await executeListPass();
            resolveWaiters?.();
            break;
          }
          await executeListPass();
          resolveWaiters?.();
        }
      } finally {
        finishPendingWaiters();
      }
    })();
  }

  /** Agrupa cambios rápidos de selector con debounce de 250 ms. */
  function scheduleSelectorRefresh(): void {
    if (selectorTimer !== null) {
      window.clearTimeout(selectorTimer);
    }
    selectorTimer = window.setTimeout(() => {
      selectorTimer = null;
      void refresh();
    }, SELECTOR_DEBOUNCE_MS);
  }

  function onCrewChange(): void {
    selectedCrewId = crewSelect.value;
    // Invalidamos cualquier consulta activa en vuelo al cambiar de astronauta.
    requestSeq += 1;
    updateVerifyButtonState();
    scheduleSelectorRefresh();
  }

  function onIndicatorChange(): void {
    selectedIndicator = indicatorSelect.value as TelemetryIndicator;
    // Invalidamos cualquier consulta activa en vuelo al cambiar de indicador.
    requestSeq += 1;
    scheduleSelectorRefresh();
  }

  function onRefreshClick(): void {
    if (selectorTimer !== null) {
      window.clearTimeout(selectorTimer);
      selectorTimer = null;
    }
    void refresh();
  }

  function onVerifyClick(): void {
    if (destroyed || !isTauriRuntime() || !selectedCrewId || verifyInFlight) return;
    pendingVerifyCrewId = null;
    void runVerify(selectedCrewId);
  }

  crewSelect.addEventListener("change", onCrewChange);
  indicatorSelect.addEventListener("change", onIndicatorChange);
  refreshButton.addEventListener("click", onRefreshClick);
  verifyButton.addEventListener("click", onVerifyClick);

  void refresh();

  return {
    refresh(): Promise<void> {
      if (selectorTimer !== null) {
        window.clearTimeout(selectorTimer);
        selectorTimer = null;
      }
      return refresh();
    },
    setPersistStatus,
    destroy(): void {
      destroyed = true;
      pendingRefresh = false;
      finishPendingWaiters();
      requestSeq += 1;
      clearLoadingBusy();
      if (selectorTimer !== null) {
        window.clearTimeout(selectorTimer);
        selectorTimer = null;
      }
      crewSelect.removeEventListener("change", onCrewChange);
      indicatorSelect.removeEventListener("change", onIndicatorChange);
      refreshButton.removeEventListener("click", onRefreshClick);
      verifyButton.removeEventListener("click", onVerifyClick);
      host.replaceChildren();
    },
  };
}
