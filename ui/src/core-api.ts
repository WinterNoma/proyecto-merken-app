// Tipado de los comandos del núcleo en Rust (src-tauri).
// Referencia: docs/api/comandos-core.md. La interfaz solo dibuja lo que
// devuelve el núcleo: no calcula estados, rangos ni tendencias.

import { invoke } from "@tauri-apps/api/core";

// ---------- Catálogo (docs/api/modelo-datos-backend.md §5) ----------

export type Indicator =
  | "heart_rate"
  | "blood_pressure"
  | "body_temperature"
  | "glucose"
  | "sleep"
  | "mood"
  | "oxygen_in_blood"
  | "space_radiation"     // contexto: no se evalúa
  | "distance_from_earth" // contexto: no se evalúa
  | "nutrition"           // contexto: no se evalúa
  | "symptom";

export type BodyZone =
  | "head" | "neck" | "chest" | "abdomen" | "back"
  | "left_arm" | "right_arm" | "left_hand" | "right_hand"
  | "left_leg" | "right_leg" | "left_foot" | "right_foot" | "other";

export type SleepQuality = "poor" | "fair" | "good";

/** ISO 8601 en UTC con milisegundos, p. ej. "2026-11-14T21:05:33.120Z". */
export type Iso = string;

export type RecordValue =
  | { bpm: number }                                   // heart_rate, unit "bpm"
  | { systolic: number; diastolic: number }           // blood_pressure, "mmHg"
  | { celsius: number }                               // body_temperature, "°C"
  | { mg_dl: number }                                 // glucose, "mg/dL"
  | { hours: number; quality: SleepQuality | null; window_start: Iso; window_end: Iso } // sleep, "h" (ventana obligatoria, ≤ 24 h)
  | { score: number }                                 // mood, "score_0_10"
  | { percentage: number }                            // oxygen_in_blood, "%" (0 < x ≤ 100)
  | { usv_h: number }                                 // space_radiation, "µSv/h" (≥ 0)
  | { km: number }                                    // distance_from_earth, "km" (≥ 0)
  | { kcal: number }                                  // nutrition, "kcal" (≥ 0)
  | { body_zone: BodyZone; score: number; description?: string | null }; // symptom, "score_0_10"

// ---------- Evaluación ----------

export type Status = "not_evaluated" | "stable" | "change" | "alarm";
/** `not_applicable`: indicador de contexto (radiación, distancia, nutrición).
 *  Sale en `indicators` solo si hay datos, con `latest_value`; no cuenta para `overall`. */
export type NotEvaluatedReason = "no_data" | "too_few_data" | "stale" | "not_applicable";
export type Direction = "up" | "down";
export type Escalation = "continue" | "self_care" | "flag_for_ground" | "urgent";
export type SourceKind = "published" | "approved_protocol" | "demo_heuristic";

export interface Evaluation {
  /** Indicador del catálogo, o el tipo de evento ("need_help") en las alertas. */
  indicator: string;
  body_zone: BodyZone | null;
  status: Status;
  reason: NotEvaluatedReason | null;
  direction: Direction | null;
  persistent: boolean;
  escalation: Escalation;
  rule_id: string;
  rule_version: string;
  source_kind: SourceKind;
  source_ref: string | null;
  input_record_ids: string[];
  input_event_ids: string[];
  core_version: string;
  evaluated_at: Iso;
  latest_value: RecordValue | null;
  latest_measured_at: Iso | null;
  /** Mediana del historial reciente (valor principal; en presión, la sistólica). */
  baseline: number | null;
  /** Lecturas ANTERIORES a la última dentro de 14 días (la última no cuenta). */
  history_count: number;
  history_needed: number;
  /** Frase corta en inglés para mostrar tal cual. */
  summary: string;
}

// ---------- Errores ----------

export type ErrorCode =
  | "invalid_value"
  | "invalid_schema"
  | "not_found"
  | "conflict"
  | "real_data_present"
  | "storage_error";

/** Lo que rechaza la promesa de `invoke` cuando el comando falla. */
export interface CoreError {
  code: ErrorCode;
  field: string | null;
  message: string; // en inglés, se puede mostrar
}

export function isCoreError(e: unknown): e is CoreError {
  return typeof e === "object" && e !== null && "code" in e && "message" in e;
}

// ---------- Perfil ----------

export interface Mission { mission_id: string; name: string; epoch_utc: Iso }

export interface Profile {
  astronaut: {
    astronaut_id: string;
    username: string;
    display_name: string;
    mission: Mission | null;
  };
  medical_profile: {
    height_cm: number | null;
    weight_kg: number | null;
    blood_type: "A+" | "A-" | "B+" | "B-" | "AB+" | "AB-" | "O+" | "O-" | null;
    sex: "female" | "male" | "other" | null;
    date_of_birth: string | null;
    notes: string | null;
    verified_by: string | null;
    verified_at: Iso | null;
    profile_version: number;
    updated_at: Iso | null;
  };
  medical_conditions: {
    condition_id: string;
    name: string;
    icd10_code: string | null;
    status: "active" | "resolved";
    since: string | null;
    notes: string | null;
  }[];
}

export interface ProfileResponse {
  /** null si aún no se ha descargado ningún perfil (sin login ni demo). */
  profile: Profile | null;
  is_demo: boolean;
  core_version: string;
  rules_version: string;
}

// ---------- Registros ----------

export type SyncState = "pending" | "synced" | "failed" | "local_only";
export type RecordSource = "form" | "text" | "voice" | "earth" | "garmin";

export interface RecordItem {
  record_id: string;
  indicator: Indicator;
  body_zone: BodyZone | null;
  value: RecordValue;
  unit: string;
  measured_at: Iso;
  logged_at: Iso;
  source: RecordSource;
  note: string | null;
  replaces: string | null;
  root_id: string;
  /** false si una corrección posterior lo sustituye. */
  effective: boolean;
  superseded_by: string | null;
  /** Medido más de 10 min en el futuro: no cuenta para el estado. */
  future_timestamp: boolean;
  sync: SyncState;
  is_demo: boolean;
}

export interface PlausibilityWarning {
  code: string;
  rule_id: string;
  message: string; // en inglés
}

export interface LogRecordInput {
  indicator: Indicator;
  value: RecordValue;
  /** Por defecto, ahora. */
  measured_at?: Iso;
  /** Por defecto "form". */
  source?: "form" | "text" | "voice";
  note?: string;
  /** true tras mostrar los avisos y que el usuario confirme. */
  confirm_warnings?: boolean;
}

export interface CorrectRecordInput {
  /** record_id del registro que se corrige (mismo indicador; en síntomas, misma zona). */
  replaces: string;
  value: RecordValue;
  measured_at?: Iso;
  note?: string;
  confirm_warnings?: boolean;
}

export type LogResult =
  | { status: "needs_confirmation"; warnings: PlausibilityWarning[] }
  | { status: "saved"; record: RecordItem; warnings: PlausibilityWarning[]; evaluation: Evaluation };

export interface ListRecordsInput {
  indicator?: Indicator;
  body_zone?: BodyZone;
  from?: Iso;
  to?: Iso;
  /** Por defecto 100, máximo 500. */
  limit?: number;
  /** Por defecto false: solo los registros efectivos. */
  include_superseded?: boolean;
}

// ---------- Estado ----------

export interface StatusResponse {
  evaluated_at: Iso;
  /** "T+12d 03:04:05" si hay misión en el perfil. */
  met: string | null;
  overall: { status: Status; escalation: Escalation; worst: string[] };
  /** Siempre los 7 indicadores médicos (sin symptom), aunque no haya datos, más
   *  hasta 3 de contexto (space_radiation, distance_from_earth, nutrition) solo si
   *  tienen datos: esos llevan `reason: "not_applicable"` y no tienen color de estado. */
  indicators: Evaluation[];
  /** Una por zona del cuerpo con síntomas registrados. */
  symptoms: Evaluation[];
  /** Alarmas por eventos (need_help…). No caducan: siguen hasta que exista una resolución en el contrato. */
  alerts: Evaluation[];
  /** Las 14 zonas, con el peor estado de sus síntomas. */
  body_zones: { body_zone: BodyZone; status: Status }[];
  sync: { pending: number; failed: number; last_push_at: Iso | null; last_pull_at: Iso | null };
  is_demo: boolean;
}

// ---------- Eventos ----------

export type EventKind = "need_help" | "syncope" | "visual_field_loss" | "suspected_clot" | "feeling_report";

export interface RaiseEventInput {
  kind: "need_help" | "feeling_report";
  /** Texto libre, máximo 2000 caracteres. */
  message?: string;
  body_zone?: BodyZone;
  /** Por defecto, ahora. */
  occurred_at?: Iso;
}

export interface EventItem {
  event_id: string;
  kind: EventKind;
  certainty: "symptom" | "suspicion";
  message: string | null;
  body_zone: BodyZone | null;
  priority: "urgent" | "normal";
  occurred_at: Iso;
  logged_at: Iso;
  sync: SyncState;
  is_demo: boolean;
}

export interface RaiseEventResult {
  event: EventItem;
  /** Alarma generada (need_help); null en feeling_report. */
  evaluation: Evaluation | null;
}

// ---------- Demo ----------

export type DemoScenario = "routine" | "persistent_change" | "alarm_event";

// ---------- Comandos ----------

export const core = {
  getProfile: () => invoke<ProfileResponse>("get_profile"),
  logRecord: (input: LogRecordInput) => invoke<LogResult>("log_record", { input }),
  correctRecord: (input: CorrectRecordInput) => invoke<LogResult>("correct_record", { input }),
  listRecords: (input: ListRecordsInput = {}) =>
    invoke<{ records: RecordItem[] }>("list_records", { input }),
  getStatus: () => invoke<StatusResponse>("get_status"),
  raiseEvent: (input: RaiseEventInput) => invoke<RaiseEventResult>("raise_event", { input }),
  listEvents: (input: { limit?: number } = {}) => invoke<{ events: EventItem[] }>("list_events", { input }),
  /** Carga datos DEMO deterministas. Falla con real_data_present si hay datos reales.
   *  El primer dato real (registro, evento, perfil o descarga) borra todo lo DEMO. */
  loadDemo: (scenario: DemoScenario) => invoke<StatusResponse>("load_demo", { input: { scenario } }),
  /** Borra solo los datos DEMO. */
  clearDemo: () => invoke<void>("clear_demo"),
};
