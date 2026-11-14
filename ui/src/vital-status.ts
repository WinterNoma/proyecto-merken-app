/**
 * Estado de cada lectura para pintar las etiquetas y colores de las tarjetas.
 *
 * Los cortes son los mismos que las reglas absolutas del núcleo Rust
 * (`crates/core/src/rules.rs`, `ABSOLUTE`), que ya tienen fuente y revisión:
 * NEWS2 (RCP 2017), AHA 2020, ACC/AHA 2017/2018, ADA 2026, BTS 2017, AASM/SRS.
 * Si cambia un umbral allí, hay que cambiarlo aquí también.
 *
 * Solo mira la lectura actual: no calcula tendencias (para eso hace falta
 * historial, que es lo que evalúa el núcleo).
 */

export type Level = "ok" | "warn" | "alarm";

export interface VitalStatus {
  level: Level;
  /** Texto corto de la etiqueta (p. ej. "NORMAL", "FIEBRE"). */
  label: string;
  /** Línea inferior de la tarjeta. */
  detail: string;
}

const IN_RANGE = "● En rango";
const ABOVE = "▲ Sobre el rango";
const BELOW = "▼ Bajo el rango";
const NOTIFY = "⚠ Avisar a médicos de vuelo";

const ok = (label: string, detail = IN_RANGE): VitalStatus => ({ level: "ok", label, detail });
const warn = (label: string, detail: string): VitalStatus => ({ level: "warn", label, detail });
const alarm = (label: string): VitalStatus => ({ level: "alarm", label, detail: NOTIFY });

/** Pulso en reposo (lpm). Alarma ≥150 o ≤40; atención >100 o <50. */
export function heartRateStatus(bpm: number): VitalStatus {
  if (bpm >= 150 || bpm <= 40) return alarm("CRÍTICO");
  if (bpm > 100) return warn("ELEVADO", ABOVE);
  if (bpm < 50) return warn("BAJO", BELOW);
  return ok("NORMAL");
}

/** SpO₂ (%). Alarma ≤91; atención <94. */
export function oxygenStatus(pct: number): VitalStatus {
  if (pct <= 91) return alarm("CRÍTICO");
  if (pct < 94) return warn("BAJO", BELOW);
  return ok("ÓPTIMO");
}

/**
 * Presión arterial (mmHg). Alarma: sistólica >180 o diastólica >120, o
 * sistólica <80. Atención: ≥140/90 (alta) o <90/60 (baja).
 */
export function bloodPressureStatus(sys: number, dia: number): VitalStatus {
  if (sys > 180 || dia > 120 || sys < 80) return alarm("CRÍTICA");
  if (sys >= 140 || dia >= 90) return warn("ELEVADA", "▲ Hipertensión");
  if (sys < 90 || dia < 60) return warn("BAJA", "▼ Hipotensión");
  return ok("NORMAL", "● Normotenso");
}

/** Glucosa (mg/dL). Alarma <54 o >250; atención <70 o >180. */
export function glucoseStatus(mgdl: number): VitalStatus {
  if (mgdl < 54 || mgdl > 250) return alarm("CRÍTICA");
  if (mgdl < 70) return warn("BAJA", BELOW);
  if (mgdl > 180) return warn("ALTA", ABOVE);
  return ok("NORMAL");
}

/** Temperatura corporal (°C). Alarma >40 o <35; atención ≥38.1 o ≤36.0. */
export function temperatureStatus(celsius: number): VitalStatus {
  if (celsius > 40) return alarm("HIPERTERMIA");
  if (celsius < 35) return alarm("HIPOTERMIA");
  if (celsius >= 38.1) return warn("FIEBRE", "▲ Febril");
  if (celsius <= 36.0) return warn("BAJA", "▼ Bajo el rango");
  return ok("AFEBRIL", "● Normotermia");
}

/** Sueño (h). Atención <6 h; por debajo de 5 h el texto es más fuerte. */
export function sleepStatus(hours: number): VitalStatus {
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
