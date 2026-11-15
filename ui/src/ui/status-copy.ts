import {
  bloodPressureStatus, glucoseStatus, heartRateStatus, oxygenStatus,
  sleepStatus, temperatureStatus, worstLevel, readingDomainError, type Level, type VitalStatus,
} from "../vital-status";
import type { AstronautTelemetry } from "../types";
import type { Evaluation, Status, Escalation } from "../core-api";
import { el } from "./dom";
import { icon, type IconName } from "./icons";

export type Tone = Level | "neutral";
export interface StatusCopy { tone: Tone; label: string; icon: IconName }
export type VitalKey = "heart" | "oxygen" | "pressure" | "glucose" | "temperature" | "sleep";
const noData = (): StatusCopy => ({ tone: "neutral", label: "No data", icon: "no_data" });

const invalidData = (): StatusCopy => ({ tone: "neutral", label: "Invalid reading", icon: "neutral" });

// Translate the function result, never its numeric thresholds. The source detail
// encodes direction; sleep has its own wording and no directional glyph.
function translate(source: VitalStatus, kind: VitalKey): StatusCopy {
  if (source.level === "invalid") return invalidData();
  if (source.level === "alarm") return { tone: source.level, label: "Notify flight surgeons", icon: "alarm" };
  if (source.level === "ok") return { tone: source.level, label: kind === "sleep" ? "Adequate rest" : "In range", icon: "ok" };
  const label = kind === "sleep"
    ? (source.detail === "Descanso muy insuficiente" ? "Very insufficient rest" : "Insufficient rest")
    : source.detail.startsWith("▼") ? "Below range"
    : source.detail.startsWith("▲") ? "Above range" : "Outside range";
  return { tone: source.level, label, icon: "warn" };
}
export function mysqlStatuses(t: AstronautTelemetry): Record<VitalKey, StatusCopy> {
  const bp = t.blood_pressure;
  const states: Record<VitalKey, StatusCopy> & Partial<Record<"nutrition" | "radiation" | "distance", StatusCopy>> = {
    heart: t.heart_rate === null ? noData() : translate(heartRateStatus(t.heart_rate), "heart"),
    oxygen: t.oxygen_in_blood === null ? noData() : translate(oxygenStatus(t.oxygen_in_blood), "oxygen"),
    pressure: bp.systolic === null || bp.diastolic === null ? noData() : translate(bloodPressureStatus(bp.systolic, bp.diastolic), "pressure"),
    glucose: t.blood_sugar === null ? noData() : translate(glucoseStatus(t.blood_sugar), "glucose"),
    temperature: t.body_temperature === null ? noData() : translate(temperatureStatus(t.body_temperature), "temperature"),
    sleep: t.sleep_hours === null ? noData() : translate(sleepStatus(t.sleep_hours), "sleep"),
  };
  const fields: [string, VitalKey | "nutrition" | "radiation" | "distance"][] = [
    ["heart_rate", "heart"], ["oxygen_in_blood", "oxygen"], ["blood_pressure", "pressure"],
    ["glucose", "glucose"], ["body_temperature", "temperature"], ["sleep", "sleep"],
    ["nutrition", "nutrition"], ["space_radiation", "radiation"], ["distance_from_earth", "distance"],
  ];
  for (const [indicator, key] of fields) if (t.invalidIndicators?.includes(indicator)) states[key] = invalidData();
  for (const [indicator, key, value] of [
    ["nutrition", "nutrition", { kcal: t.nutrition }],
    ["space_radiation", "radiation", { usv_h: t.space_radiation }],
    ["distance_from_earth", "distance", { km: t.distance_from_earth }],
  ] as const) {
    if (Object.values(value).some(number => number !== null) && readingDomainError(indicator, value)) states[key] = invalidData();
  }
  return states;
}
export function crewStatus(t: AstronautTelemetry): StatusCopy {
  const states = Object.values(mysqlStatuses(t));
  const levels = states.map(s => s.tone).filter((level): level is Level => level !== "neutral");
  const invalid = states.some(state => state.label === "Invalid reading")
    || (t.hrv != null && Boolean(readingDomainError("hrv", { ms: t.hrv })))
    || (t.body_battery != null && Boolean(readingDomainError("body_battery", { value: t.body_battery })))
    || Boolean(t.invalidIndicators?.length);
  if (!levels.length) return invalid ? invalidData() : noData();
  const worst = worstLevel(levels);
  const partial = levels.length !== states.length || invalid;
  return {
    tone: worst === "ok" && partial ? "neutral" : worst,
    icon: worst === "ok" && partial ? "neutral" : worst,
    label: (worst === "alarm" ? "Notify flight surgeons" : worst === "warn" ? "Outside range" : partial ? invalid ? "Invalid reading · Not evaluated" : "Incomplete readings" : "In range")
      + (partial && worst !== "ok" ? invalid ? " · Some readings invalid or missing" : " · Some readings missing" : ""),
  };
}
export function statusCopy(status: Status): StatusCopy {
  switch (status) {
    case "stable": return { tone: "ok", label: "Stable", icon: "ok" };
    case "change": return { tone: "warn", label: "Change detected", icon: "warn" };
    case "alarm": return { tone: "alarm", label: "Alarm", icon: "alarm" };
    default: return { tone: "neutral", label: "Not evaluated", icon: "neutral" };
  }
}
export function evaluationCopy(evaluation: Evaluation): StatusCopy {
  if (evaluation.latest_value !== null && readingDomainError(evaluation.indicator, evaluation.latest_value)) return invalidData();
  const copy = statusCopy(evaluation.status);
  if (evaluation.status === "not_evaluated") {
    switch (evaluation.reason) {
      case "no_data": return noData();
      case "stale": return { tone: "neutral", label: "Stale reading", icon: "stale" };
      case "too_few_data": return { tone: "neutral", label: "Insufficient history (" + evaluation.history_count + "/" + evaluation.history_needed + ")", icon: "history" };
      case "not_applicable": return { tone: "neutral", label: "Context only", icon: "info" };
    }
  }
  if (evaluation.status === "change") {
    if (evaluation.direction) copy.label += evaluation.direction === "up" ? " · ↑ Increasing" : " · ↓ Decreasing";
    if (evaluation.persistent) copy.label += " · Persistent";
  }
  return copy;
}
export function escalationCopy(value: Escalation): string {
  return { continue: "No additional escalation", self_care: "Core guidance", flag_for_ground: "Flagged for ground review", urgent: "Urgent" }[value];
}
export function badge(copy: StatusCopy): HTMLElement {
  const node = el("span", "status-chip");
  paintBadge(node, copy);
  return node;
}
export function paintBadge(node: HTMLElement, copy: StatusCopy): void {
  if (node.dataset.tone === copy.tone && node.dataset.label === copy.label && node.dataset.icon === copy.icon) return;
  node.dataset.tone = copy.tone;
  node.dataset.label = copy.label;
  node.dataset.icon = copy.icon;
  node.replaceChildren(icon(copy.icon), el("span", "", copy.label));
}
