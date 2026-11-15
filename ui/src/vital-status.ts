/**
 * Estado de cada lectura para pintar las etiquetas y colores de las tarjetas.
 *
 * Los cortes son los mismos que las reglas absolutas del núcleo Rust
 * (`crates/core/src/rules.rs`, `ABSOLUTE`), que ya tienen fuente y revisión:
 * NEWS2 (RCP 2017), AHA 2020, ACC/AHA 2017/2018, ADA 2026, BTS 2017, AASM/SRS.
 * Si cambia un umbral allí, hay que cambiarlo aquí también, y en los textos
 * «Ref:» de `ui/index.html` (muestran el rango que da estado normal).
 *
 * Solo mira la lectura actual: no calcula tendencias (para eso hace falta
 * historial, que es lo que evalúa el núcleo).
 */

export type Level = "ok" | "warn" | "alarm";

export interface VitalStatus {
  level: Level | "invalid";
  /** Texto corto de la etiqueta (p. ej. "NORMAL", "FIEBRE"). */
  label: string;
  /** Línea inferior de la tarjeta. */
  detail: string;
}


/** Domain constraints only, separate from the unchanged clinical thresholds below.
 * Positive scalars and nonnegative context follow core-api.ts / comandos-core.md.
 * Feed SpO2 accepts 0..100; the manual core command retains its stricter >0 contract.
 * HRV is nonnegative; body battery is a percentage. No clinical maximum is invented.
 */
const domains: Record<string, { key: string; min: number; max?: number; positive?: boolean }[]> = {
  heart_rate: [{ key: "bpm", min: 0, positive: true }],
  oxygen_in_blood: [{ key: "percentage", min: 0, max: 100 }],
  blood_pressure: [{ key: "systolic", min: 0, positive: true }, { key: "diastolic", min: 0, positive: true }],
  glucose: [{ key: "mg_dl", min: 0, positive: true }],
  body_temperature: [{ key: "celsius", min: 0, positive: true }],
  sleep: [{ key: "hours", min: 0, max: 24 }],
  nutrition: [{ key: "kcal", min: 0 }],
  space_radiation: [{ key: "usv_h", min: 0 }],
  distance_from_earth: [{ key: "km", min: 0 }],
  hrv: [{ key: "ms", min: 0 }],
  body_battery: [{ key: "value", min: 0, max: 100 }],
};
export function readingDomainError(indicator: string, value: unknown): string | null {
  const rules = domains[indicator];
  if (!rules) return null; // Text/score and event contracts are validated separately.
  if (typeof value !== "object" || value === null || Array.isArray(value)) return indicator + ": invalid value";
  const data = value as Record<string, unknown>;
  for (const rule of rules) {
    const number = data[rule.key];
    if (typeof number !== "number" || !Number.isFinite(number)) return indicator + ": not a finite number";
    if (number < rule.min || (rule.positive && number === rule.min) || (rule.max !== undefined && number > rule.max)) {
      return indicator + ": outside valid domain";
    }
  }
  return null;
}
const invalidReading = (): VitalStatus => ({ level: "invalid", label: "Invalid reading", detail: "Not evaluated" });
const IN_RANGE = "● En rango";
const ABOVE = "▲ Sobre el rango";
const BELOW = "▼ Bajo el rango";
const NOTIFY = "⚠ Avisar a médicos de vuelo";

const ok = (label: string, detail = IN_RANGE): VitalStatus => ({ level: "ok", label, detail });
const warn = (label: string, detail: string): VitalStatus => ({ level: "warn", label, detail });
const alarm = (label: string): VitalStatus => ({ level: "alarm", label, detail: NOTIFY });

/** Pulso en reposo (lpm). Alarma ≥150 o ≤40; atención >100 o <50. */
export function heartRateStatus(bpm: number): VitalStatus {
  if (readingDomainError("heart_rate", { bpm })) return invalidReading();
  if (bpm >= 150 || bpm <= 40) return alarm("CRÍTICO");
  if (bpm > 100) return warn("ELEVADO", ABOVE);
  if (bpm < 50) return warn("BAJO", BELOW);
  return ok("NORMAL");
}

/** SpO₂ (%). Alarma ≤91; atención <94. */
export function oxygenStatus(pct: number): VitalStatus {
  if (readingDomainError("oxygen_in_blood", { percentage: pct })) return invalidReading();
  if (pct <= 91) return alarm("CRÍTICO");
  if (pct < 94) return warn("BAJO", BELOW);
  return ok("ÓPTIMO");
}

/**
 * Presión arterial (mmHg). Alarma: sistólica >180 o diastólica >120, o
 * sistólica <80. Atención: ≥140/90 (alta) o <90/60 (baja).
 */
export function bloodPressureStatus(sys: number, dia: number): VitalStatus {
  if (readingDomainError("blood_pressure", { systolic: sys, diastolic: dia })) return invalidReading();
  if (sys > 180 || dia > 120 || sys < 80) return alarm("CRÍTICA");
  if (sys >= 140 || dia >= 90) return warn("ELEVADA", "▲ Hipertensión");
  if (sys < 90 || dia < 60) return warn("BAJA", "▼ Hipotensión");
  return ok("NORMAL", "● Normotenso");
}

/** Glucosa (mg/dL). Alarma <54 o >250; atención <70 o >180. */
export function glucoseStatus(mgdl: number): VitalStatus {
  if (readingDomainError("glucose", { mg_dl: mgdl })) return invalidReading();
  if (mgdl < 54 || mgdl > 250) return alarm("CRÍTICA");
  if (mgdl < 70) return warn("BAJA", BELOW);
  if (mgdl > 180) return warn("ALTA", ABOVE);
  return ok("NORMAL");
}

/** Temperatura corporal (°C). Alarma >40 o <35; atención ≥38.1 o ≤36.0. */
export function temperatureStatus(celsius: number): VitalStatus {
  if (readingDomainError("body_temperature", { celsius })) return invalidReading();
  if (celsius > 40) return alarm("HIPERTERMIA");
  if (celsius < 35) return alarm("HIPOTERMIA");
  if (celsius >= 38.1) return warn("FIEBRE", "▲ Febril");
  if (celsius <= 36.0) return warn("BAJA", "▼ Bajo el rango");
  return ok("AFEBRIL", "● Normotermia");
}

/** Sueño (h). Atención <6 h; por debajo de 5 h el texto es más fuerte. */
export function sleepStatus(hours: number): VitalStatus {
  if (readingDomainError("sleep", { hours })) return invalidReading();
  if (hours < 5) return warn("SUEÑO", "Descanso muy insuficiente");
  if (hours < 6) return warn("SUEÑO", "Descanso insuficiente");
  return ok("SUEÑO", "Descanso adecuado");
}

/** El peor nivel de la lista. */
export function worstLevel(levels: Level[]): Level {
  if (levels.includes("alarm")) return "alarm";
  if (levels.includes("warn")) return "warn";
  return "ok";
}

export const OVERALL_LABEL: Record<Level, string> = {
  ok: "● ESTADO GENERAL: ESTABLE",
  warn: "● ESTADO GENERAL: ATENCIÓN",
  alarm: "⚠ ESTADO GENERAL: ALERTA",
};
