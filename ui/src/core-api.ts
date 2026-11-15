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
  | "voice_unavailable"
  | "model_missing"
  | "text_invalid"
  | "text_unsupported"
  | "synthesis_failed"
  | "audio_invalid"
  | "transcription_failed"
  | "busy"
  | "model_unavailable"
  | "cancelled"
  | "timeout"
  | "slm_error"
  | "invalid_value"
  | "invalid_schema"
  | "not_found"
  | "conflict"
  | "real_data_present"
  | "storage_error"
  // Solo en simListAstronauts/simImport (API de simulación):
  | "missing_token"
  | "unauthorized"
  | "network"
  | "http_error"
  | "bad_response"
  | "too_much_data";

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

// ---------- API de simulación (datos inventados) ----------

/** Snapshot already saved on this device; reading it never contacts the backend. */
export interface CachedCrew { received_at: string; crew: Record<string, unknown>[] }

export interface SimAstronaut { astronaut_id: string; display_name: string }

export interface SimImportInput {
  astronaut_id: string;
  /** Días hacia atrás (1–31). Por defecto 14, la ventana de historial del núcleo. */
  days?: number;
  /** Token de la API de simulación. Si falta, Rust usa la variable MERKEN_SIM_TOKEN. */
  token?: string;
}

/** Historial de un miembro ya guardado en la telemetría local (SQLite). */
export interface CrewImportInput {
  /** Clave de la telemetría local (el ID del miembro en la base de tierra). */
  astronaut_key: string;
  display_name: string;
}

export interface SimImportResult {
  astronaut_id: string;
  display_name: string;
  imported: number;
  /** Registros descartados: duplicados, de otro astronauta, con fecha futura
   *  o que no pasan la validación del contrato o del núcleo. */
  rejected: number;
  first_measured_at: Iso | null;
  last_measured_at: Iso | null;
}

// ---------- Telemetría local con trazabilidad ----------

export type TelemetryIndicator =
  | "heart_rate"
  | "oxygen_in_blood"
  | "blood_pressure"
  | "glucose"
  | "body_temperature"
  | "sleep"
  | "nutrition"
  | "space_radiation"
  | "distance_from_earth"
  | "mood"
  | "hrv"
  | "body_battery";

export type TelemetryValue =
  | { bpm: number }
  | { percentage: number }
  | { systolic: number; diastolic: number }
  | { mg_dl: number }
  | { celsius: number }
  | { hours: number }
  | { kcal: number }
  | { usv_h: number }
  | { km: number }
  | { label: string }
  | { ms: number }
  | { value: number };

export interface TelemetryReadingInput {
  astronaut_key: string;
  astronaut_name: string;
  indicator: TelemetryIndicator;
  value: TelemetryValue;
  unit: string;
}

export interface RecordTelemetryInput {
  /** Entre 1 y 64 lecturas por lote. */
  readings: TelemetryReadingInput[];
}

export interface RecordTelemetryResult {
  stored: number;
  skipped: number;
  batch_id: string;
}

export interface ListTelemetryInput {
  astronaut_key: string;
  indicator?: TelemetryIndicator;
  from?: Iso;
  to?: Iso;
  /** Por defecto 200, máximo 1000. */
  limit?: number;
}

export interface TelemetryItem {
  reading_id: number;
  astronaut_key: string;
  astronaut_name: string;
  indicator: string;
  value: unknown;
  unit: string;
  observed_at: Iso;
  batch_id: string;
  source: string;
  app_version: string;
  prev_hash: string;
  row_hash: string;
}

export interface VerifyTelemetryInput {
  astronaut_key?: string;
}

export type TelemetryProblem =
  | "hash_mismatch"
  | "broken_link"
  | "missing_tail"
  | "missing_series"
  | "count_mismatch";

export interface VerifyTelemetryResult {
  ok: boolean;
  checked: number;
  series_checked: number;
  first_bad_reading_id: number | null;
  problem: TelemetryProblem | null;
  /** Mayor `reading_id` comprobado en la verificación; `null` si no hay referencia o cadena vacía. */
  checked_through_reading_id: number | null;
}


export type TimeHint = { kind: "unspecified" } | { kind: "ambiguous"; quote: string };
export interface ProposedField { key: string; value: string | number | boolean | null; quote: string | null }
export interface ProposedRecord { indicator: Indicator; fields: ProposedField[]; time: TimeHint; quote: string }
export interface Proposal { schema_version: number; interpreter: "rules" | "slm"; interpreter_version: string; records: ProposedRecord[]; negated: string[]; uninterpreted: string[] }
export type Layer2Status = "not_needed" | "unavailable" | "used" | "model_unavailable" | "cancelled" | "timeout" | "discarded" | "slm_error";
export interface InterpretResponse { proposal: Proposal; layer2_proposal: Proposal | null; unresolved: string[]; layer2: Layer2Status }
export type FactId = "indicator" | "status" | "reason" | "direction" | "latest_reading" | "baseline" | "history" | "escalation" | "source";
export interface FactLine { id: FactId; text: string }
/** Tauri event slm-progress; it never contains input or generated text. */
export interface SlmProgress { request_id: string; phase: "loading" | "generating" | "validating" | "done"; tokens: number; max_tokens: number }
export interface AiNote { statements: { ficha_id: string; statement_id: string; text: string }[]; label: string }
export type NoteStatus = "available" | "no_fichas" | "model_unavailable" | "cancelled" | "timeout" | "discarded" | "slm_error";
export interface ExplainResponse { facts: FactLine[]; note: AiNote | null; note_status: NoteStatus; model_id: string | null }
export interface SlmModelStatus { model_id: string; installed: boolean; verified: boolean; loaded: boolean; available: boolean }
export interface SlmStatus { interpreter: SlmModelStatus; explainer: SlmModelStatus }

export interface SpeakResponse { request_id: string; chunks: number; duration_ms: number }
/** Each event contains one complete PCM16 mono WAV file in base64. */
export interface VoiceChunkEvent { request_id: string; index: number; wav_base64: string }
export type DictationLanguage = "es" | "en" | "auto";
export interface TranscribeResponse { request_id: string; text: string; duration_ms: number }

export interface ExportJsonInput {
  include_telemetry?: boolean;
  astronaut_key?: string;
}

export interface ExportJsonResult {
  path: string;
  file_name: string;
  bytes: number;
  exported_at: Iso;
  counts: { records: number; events: number; evaluations: number; telemetry_readings: number; outbox_pending: number };
}

export const core = {
  cachedCrew: () => invoke<CachedCrew | null>("recommendations_cached_crew"),
  speakText: (input: { request_id: string; text: string; voice?: string | null; speed?: number | null }) => invoke<SpeakResponse>("speak_text", { input }),
  speakCancel: (requestId: string) => invoke<boolean>("speak_cancel", { request_id: requestId }),
  voiceRelease: () => invoke<boolean>("voice_release"),
  interpretText: (input: { request_id: string; text: string; source?: "text" | "voice" | null }) => invoke<InterpretResponse>("interpret_text", { input }),
  explainIndicator: (input: { request_id: string; indicator: Indicator; body_zone?: BodyZone | null }) => invoke<ExplainResponse>("explain_indicator", { input }),
  slmStatus: () => invoke<SlmStatus>("slm_status"),
  slmCancel: (requestId: string) => invoke<boolean>("slm_cancel", { request_id: requestId }),
  slmRelease: () => invoke<boolean>("slm_release"),
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
  /** Astronautas de la API de simulación. */
  simListAstronauts: (token?: string) =>
    invoke<{ astronauts: SimAstronaut[] }>("sim_list_astronauts", { input: { token } }),
  /** Descarga los registros de un astronauta simulado y los guarda como DEMO
   *  (sustituye la DEMO anterior, nunca se suben). Después llama a getStatus(). */
  simImport: (input: SimImportInput) => invoke<SimImportResult>("sim_import", { input }),
  /** Lleva las últimas lecturas de un miembro de MySQL al núcleo como DEMO (sustituye la DEMO anterior). */
  importCrewMember: (input: CrewImportInput) => invoke<SimImportResult>("import_crew_member", { input }),
  /** Registra un lote (1..=64) de lecturas de telemetría en la cadena local. */
  recordTelemetry: (input: RecordTelemetryInput) =>
    invoke<RecordTelemetryResult>("record_telemetry", { input }),
  /** Lista lecturas de telemetría local de la más reciente a la más antigua. */
  listTelemetry: (input: ListTelemetryInput) =>
    invoke<TelemetryItem[]>("list_telemetry", { input }),
  /** Exporta los datos locales a un archivo JSON. */
  exportJson: (input?: ExportJsonInput) =>
    input === undefined ? invoke<ExportJsonResult>("export_json") : invoke<ExportJsonResult>("export_json", { input }),
  /** Verifica la integridad de la cadena de hashes de telemetría local. */
  verifyTelemetry: (input: VerifyTelemetryInput = {}) =>
    invoke<VerifyTelemetryResult>("verify_telemetry", { input }),
  /** Transcribe audio PCM float32 little-endian en base64 sin conexión. */
  transcribeAudio: (input: {
    request_id: string;
    pcm_f32le_base64: string;
    sample_rate: number;
    language?: DictationLanguage | null;
  }) => invoke<TranscribeResponse>("transcribe_audio", { input }),
  /** Libera el modelo STT cargado en memoria. */
  sttRelease: () => invoke<boolean>("stt_release"),
};

// ---------- Dictado por voz (solo voz -> texto; no interpreta ni guarda) ----------

const DICTATION_TARGET_SAMPLE_RATE = 16000;
const DICTATION_MAX_SECONDS = 30;
const DICTATION_MAX_TARGET_SAMPLES = DICTATION_TARGET_SAMPLE_RATE * DICTATION_MAX_SECONDS;

const DICTATION_WORKLET_SOURCE = `
class MerkenDictationProcessor extends AudioWorkletProcessor {
  process(inputs) {
    const input = inputs[0];
    if (input && input.length > 0 && input[0] && input[0].length > 0) {
      const frameCount = input[0].length;
      const channelCount = input.length;
      const mono = new Float32Array(frameCount);
      if (channelCount === 1) {
        mono.set(input[0]);
      } else {
        for (let i = 0; i < frameCount; i++) {
          let sum = 0;
          for (let ch = 0; ch < channelCount; ch++) {
            sum += input[ch][i] || 0;
          }
          mono[i] = sum / channelCount;
        }
      }
      this.port.postMessage(mono, [mono.buffer]);
    }
    return true;
  }
}
registerProcessor("merken-dictation-processor", MerkenDictationProcessor);
`;

export interface DictationAudio {
  samples: Float32Array;
  sampleRate: number;
}

export interface DictationRecording {
  /** Resolves once the microphone is active; rejects if permission or audio setup fails. */
  ready: Promise<void>;
  /**
   * Resolves once the 30-second audio capture limit is reached and capture stops automatically;
   * never rejects. Accumulated audio remains available until `stop()` or `cancel()`.
   */
  limitReached: Promise<void>;
  /** Stops capture, releases audio resources, and returns mono 16 kHz Float32 samples (max 30 s). */
  stop(): Promise<DictationAudio>;
  /** Aborts capture and immediately releases all microphone and Web Audio resources. */
  cancel(): void;
}

function concatFloat32Chunks(chunks: Float32Array[], totalLength: number): Float32Array {
  const merged = new Float32Array(totalLength);
  let offset = 0;
  for (const chunk of chunks) {
    merged.set(chunk, offset);
    offset += chunk.length;
  }
  return merged;
}

function clampAudioSample(value: number): number {
  if (value > 1) return 1;
  if (value < -1) return -1;
  return value;
}

function resampleTo16k(mono: Float32Array, sourceRate: number): Float32Array {
  if (mono.length === 0 || sourceRate <= 0) {
    return new Float32Array(0);
  }
  if (sourceRate === DICTATION_TARGET_SAMPLE_RATE) {
    const capped = mono.length > DICTATION_MAX_TARGET_SAMPLES
      ? mono.subarray(0, DICTATION_MAX_TARGET_SAMPLES)
      : mono;
    for (let i = 0; i < capped.length; i++) {
      capped[i] = clampAudioSample(capped[i]);
    }
    return capped;
  }
  const ratio = sourceRate / DICTATION_TARGET_SAMPLE_RATE;
  const outLen = Math.min(DICTATION_MAX_TARGET_SAMPLES, Math.floor(mono.length / ratio));
  const out = new Float32Array(outLen);
  if (sourceRate > DICTATION_TARGET_SAMPLE_RATE) {
    for (let i = 0; i < outLen; i++) {
      const winStart = i * ratio;
      const winEnd = (i + 1) * ratio;
      const firstIdx = Math.floor(winStart);
      const lastIdx = Math.min(mono.length - 1, Math.floor(winEnd));
      let weightedSum = 0;
      let totalWeight = 0;
      for (let j = firstIdx; j <= lastIdx; j++) {
        const overlapStart = Math.max(winStart, j);
        const overlapEnd = Math.min(winEnd, j + 1);
        const weight = overlapEnd - overlapStart;
        if (weight > 0) {
          weightedSum += mono[j] * weight;
          totalWeight += weight;
        }
      }
      out[i] = clampAudioSample(totalWeight > 0 ? weightedSum / totalWeight : 0);
    }
    return out;
  }
  for (let i = 0; i < outLen; i++) {
    const pos = i * ratio;
    const idx = Math.floor(pos);
    const frac = pos - idx;
    const s0 = mono[idx] ?? 0;
    const s1 = mono[Math.min(idx + 1, mono.length - 1)] ?? s0;
    out[i] = clampAudioSample(s0 + (s1 - s0) * frac);
  }
  return out;
}

function encodeFloat32LeBase64(samples: Float32Array): string {
  const bytes = new Uint8Array(samples.byteLength);
  const view = new DataView(bytes.buffer);
  for (let i = 0; i < samples.length; i++) {
    view.setFloat32(i * 4, samples[i], true);
  }
  const CHUNK_SIZE = 0x8000;
  const parts: string[] = [];
  for (let offset = 0; offset < bytes.length; offset += CHUNK_SIZE) {
    const chunk = bytes.subarray(offset, offset + CHUNK_SIZE);
    parts.push(String.fromCharCode(...chunk));
  }
  return btoa(parts.join(""));
}

function makeDictationRequestId(): string {
  const ts = Date.now().toString(36);
  const bytes = new Uint8Array(6);
  if (typeof crypto !== "undefined" && typeof crypto.getRandomValues === "function") {
    crypto.getRandomValues(bytes);
  } else {
    for (let i = 0; i < bytes.length; i++) {
      bytes[i] = Math.floor(Math.random() * 256);
    }
  }
  const rand = Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
  return `dict_${ts}_${rand}`.slice(0, 64);
}

/**
 * Starts an offline microphone recording session for voice dictation.
 *
 * Uses `AudioWorkletNode` when available (with a `ScriptProcessorNode` fallback),
 * mixes input to mono, resamples to 16 kHz, caps accumulation at 30 seconds,
 * and always releases microphone tracks and Web Audio resources on `stop()`, `cancel()`,
 * or automatically when the 30-second limit is reached.
 *
 * `ready` resolves once microphone capture is active and rejects if permission or audio setup fails;
 * `limitReached` resolves when the 30-second limit is reached (stopping microphone tracks, the audio
 * processor, and the `AudioContext`, while preserving accumulated samples for `stop()`) and never rejects;
 * `stop()` also rejects if capture failed to initialize or was cancelled.
 */
export function recordDictation(): DictationRecording {
  const chunks: Float32Array[] = [];
  let accumulatedSamples = 0;
  let sourceRate = DICTATION_TARGET_SAMPLE_RATE;
  let maxInputSamples = DICTATION_MAX_TARGET_SAMPLES;
  let stopped = false;
  let cancelled = false;
  let limitReachedResolved = false;
  let stopResult: Promise<DictationAudio> | null = null;

  let stream: MediaStream | null = null;
  let ctx: AudioContext | null = null;
  let sourceNode: MediaStreamAudioSourceNode | null = null;
  let workletNode: AudioWorkletNode | null = null;
  let scriptNode: ScriptProcessorNode | null = null;
  let silentGain: GainNode | null = null;
  let blobUrl: string | null = null;
  const cancelRef: { reject: ((err: Error) => void) | null } = { reject: null };
  let resolveLimitReached: (() => void) | null = null;
  const limitReached = new Promise<void>((resolve) => {
    resolveLimitReached = resolve;
  });

  const clearBuffer = () => {
    chunks.length = 0;
    accumulatedSamples = 0;
  };

  const cleanup = () => {
    if (workletNode) {
      workletNode.port.onmessage = null;
      try { workletNode.disconnect(); } catch { /* ignore */ }
      workletNode = null;
    }
    if (scriptNode) {
      scriptNode.onaudioprocess = null;
      try { scriptNode.disconnect(); } catch { /* ignore */ }
      scriptNode = null;
    }
    if (sourceNode) {
      try { sourceNode.disconnect(); } catch { /* ignore */ }
      sourceNode = null;
    }
    if (silentGain) {
      try { silentGain.disconnect(); } catch { /* ignore */ }
      silentGain = null;
    }
    if (stream) {
      for (const track of stream.getTracks()) {
        track.stop();
      }
      stream = null;
    }
    if (blobUrl) {
      URL.revokeObjectURL(blobUrl);
      blobUrl = null;
    }
    if (ctx && ctx.state !== "closed") {
      void ctx.close().catch(() => {});
    }
    ctx = null;
  };

  const pushMonoBlock = (block: Float32Array) => {
    if (stopped || cancelled || accumulatedSamples >= maxInputSamples || block.length === 0) {
      return;
    }
    const remaining = maxInputSamples - accumulatedSamples;
    const slice = block.length > remaining ? block.subarray(0, remaining) : block;
    chunks.push(slice);
    accumulatedSamples += slice.length;
    if (accumulatedSamples >= maxInputSamples && !limitReachedResolved) {
      limitReachedResolved = true;
      cleanup();
      resolveLimitReached?.();
    }
  };

  const startPromise = (async () => {
    if (typeof navigator === "undefined" || !navigator.mediaDevices?.getUserMedia) {
      throw new Error("Microphone capture is unavailable in this environment");
    }
    const globalAudio = globalThis as typeof globalThis & {
      AudioContext?: typeof AudioContext;
      webkitAudioContext?: typeof AudioContext;
    };
    const AudioCtx = globalAudio.AudioContext ?? globalAudio.webkitAudioContext;
    if (!AudioCtx) {
      throw new Error("Web Audio is unavailable in this environment");
    }

    const media = await navigator.mediaDevices.getUserMedia({
      audio: {
        channelCount: 1,
        echoCancellation: true,
        noiseSuppression: true,
      },
    });
    if (cancelled) {
      for (const track of media.getTracks()) {
        track.stop();
      }
      throw new Error("Dictation recording was cancelled");
    }
    stream = media;

    ctx = new AudioCtx();
    sourceRate = ctx.sampleRate || DICTATION_TARGET_SAMPLE_RATE;
    maxInputSamples = Math.ceil(DICTATION_MAX_SECONDS * sourceRate);

    sourceNode = ctx.createMediaStreamSource(media);
    silentGain = ctx.createGain();
    silentGain.gain.value = 0;

    let attachedWorklet = false;
    if (
      typeof AudioWorkletNode !== "undefined" &&
      Boolean(ctx.audioWorklet) &&
      typeof Blob !== "undefined" &&
      typeof URL !== "undefined"
    ) {
      try {
        const blob = new Blob([DICTATION_WORKLET_SOURCE], { type: "application/javascript" });
        blobUrl = URL.createObjectURL(blob);
        await ctx.audioWorklet.addModule(blobUrl);
        if (cancelled) {
          cleanup();
          throw new Error("Dictation recording was cancelled");
        }
        workletNode = new AudioWorkletNode(ctx, "merken-dictation-processor", {
          numberOfInputs: 1,
          numberOfOutputs: 1,
          channelCount: 1,
        });
        workletNode.port.onmessage = (event: MessageEvent) => {
          if (event.data instanceof Float32Array) {
            pushMonoBlock(event.data);
          }
        };
        sourceNode.connect(workletNode);
        workletNode.connect(silentGain);
        silentGain.connect(ctx.destination);
        attachedWorklet = true;
      } catch (err) {
        if (cancelled) {
          cleanup();
          throw err;
        }
        if (blobUrl) {
          URL.revokeObjectURL(blobUrl);
          blobUrl = null;
        }
        if (workletNode) {
          workletNode.port.onmessage = null;
          try { workletNode.disconnect(); } catch { /* ignore */ }
          workletNode = null;
        }
      }
    }

    if (!attachedWorklet) {
      if (typeof ctx.createScriptProcessor !== "function") {
        cleanup();
        throw new Error("Audio processor is unavailable in this environment");
      }
      const channels = Math.max(1, sourceNode.channelCount || 1);
      scriptNode = ctx.createScriptProcessor(4096, channels, 1);
      scriptNode.onaudioprocess = (event: AudioProcessingEvent) => {
        const inputBuffer = event.inputBuffer;
        const frames = inputBuffer.length;
        const numChannels = inputBuffer.numberOfChannels;
        if (frames === 0 || numChannels === 0) return;
        const mono = new Float32Array(frames);
        if (numChannels === 1) {
          mono.set(inputBuffer.getChannelData(0));
        } else {
          for (let ch = 0; ch < numChannels; ch++) {
            const data = inputBuffer.getChannelData(ch);
            for (let i = 0; i < frames; i++) {
              mono[i] += data[i];
            }
          }
          for (let i = 0; i < frames; i++) {
            mono[i] /= numChannels;
          }
        }
        pushMonoBlock(mono);
      };
      sourceNode.connect(scriptNode);
      scriptNode.connect(silentGain);
      silentGain.connect(ctx.destination);
    }

    if (ctx.state === "suspended") {
      await ctx.resume();
    }
    if (cancelled) {
      cleanup();
      throw new Error("Dictation recording was cancelled");
    }
  })().catch((err) => {
    cleanup();
    throw err;
  });

  const cancelPromise = new Promise<never>((_resolve, reject) => {
    cancelRef.reject = reject;
  });
  const ready = Promise.race([startPromise, cancelPromise]);
  ready.catch(() => {});

  return {
    ready,
    limitReached,
    stop(): Promise<DictationAudio> {
      if (stopResult) {
        return stopResult;
      }
      stopResult = (async () => {
        await ready;
        if (cancelled) {
          throw new Error("Dictation recording was cancelled");
        }
        stopped = true;
        cleanup();
        let samples: Float32Array;
        try {
          const merged = concatFloat32Chunks(chunks, accumulatedSamples);
          samples = resampleTo16k(merged, sourceRate);
        } finally {
          clearBuffer();
        }
        return { samples, sampleRate: DICTATION_TARGET_SAMPLE_RATE };
      })();
      return stopResult;
    },
    cancel(): void {
      if (stopped || cancelled) {
        return;
      }
      cancelled = true;
      cleanup();
      clearBuffer();
      cancelRef.reject?.(new Error("Dictation recording was cancelled"));
    },
  };
}

/**
 * Stops a dictation recording and transcribes the captured audio offline via `core.transcribeAudio`.
 *
 * There is no transcription cancellation in Rust: once dispatched, the STT reservation remains held
 * until the native call completes, and any new transcription request receives `"busy"`.
 *
 * Voice-to-text only: this function NEVER interprets or saves health data.
 * Callers must pass the resulting `text` through `core.interpretText({ ..., source: "voice" })`
 * and require field-by-field user confirmation before calling `core.logRecord`.
 */
export async function dictate(
  recording: Pick<DictationRecording, "stop">,
  language?: DictationLanguage | null,
): Promise<TranscribeResponse> {
  const { samples, sampleRate } = await recording.stop();
  const request_id = makeDictationRequestId();
  const pcm_f32le_base64 = encodeFloat32LeBase64(samples);
  return core.transcribeAudio({
    request_id,
    pcm_f32le_base64,
    sample_rate: sampleRate,
    language: language ?? null,
  });
}
