import type { RecordValue, Indicator } from "../core-api";
import { numberText } from "./dom";
const labels: Record<string, string> = {
  heart_rate: "Heart rate", blood_pressure: "Blood pressure", body_temperature: "Body temperature",
  glucose: "Blood glucose", sleep: "Sleep", mood: "Mood", oxygen_in_blood: "Blood oxygen · SpO₂",
  space_radiation: "Space radiation", distance_from_earth: "Distance from Earth", nutrition: "Nutrition",
  symptom: "Symptom", need_help: "Help request", syncope: "Syncope", visual_field_loss: "Visual field loss",
  suspected_clot: "Suspected clot", feeling_report: "Feeling report",
};
export function indicatorName(key: string): string {
  return labels[key] ?? key.replace(/_/g, " ").replace(/^./, letter => letter.toUpperCase());
}
export const units: Record<Indicator, string> = {
  heart_rate: "bpm", blood_pressure: "mmHg", body_temperature: "°C", glucose: "mg/dL",
  sleep: "h", mood: "/ 10", oxygen_in_blood: "%", space_radiation: "µSv/h",
  distance_from_earth: "km", nutrition: "kcal", symptom: "/ 10",
};
export function scalar(value: RecordValue): number | null {
  for (const key of ["bpm", "celsius", "mg_dl", "hours", "score", "percentage", "usv_h", "km", "kcal"] as const) {
    if (key in value) {
      const number = (value as unknown as Record<string, unknown>)[key];
      if (typeof number === "number" && Number.isFinite(number)) return number;
    }
  }
  return null;
}
export function recordValue(value: RecordValue | null, indicator: string): string {
  if (!value) return "--";
  const text = "systolic" in value
    ? numberText(value.systolic) + " / " + numberText(value.diastolic)
    : numberText(scalar(value));
  const unit = units[indicator as Indicator] ?? "";
  return text + (unit ? " " + unit : "");
}
