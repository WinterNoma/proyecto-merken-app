/** MySQL readings are nullable: absence must never become a plausible value. */
export interface BloodPressure {
  systolic: number | null;
  diastolic: number | null;
}
export interface AstronautTelemetry {
  /** Present but malformed feed values, kept distinct from absent readings. */
  invalidIndicators?: string[];
  astronaut_name: string;
  space_radiation: number | null;
  distance_from_earth: number | null;
  blood_pressure: BloodPressure;
  heart_rate: number | null;
  blood_sugar: number | null;
  body_temperature: number | null;
  sleep_hours: number | null;
  mood: string | null;
  nutrition: number | null;
  oxygen_in_blood: number | null;
  hrv?: number | null;
  body_battery?: number | null;
}
export interface AstronautProfile {
  id: string;
  name: string;
  telemetry: AstronautTelemetry;
  invalid?: { indicator: string; reason: string }[];
}
