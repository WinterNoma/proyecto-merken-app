/**
 * Modelo canónico de telemetría médica para astronautas (11 indicadores).
 * Excluye headache/cefalea y cabinCO2.
 */

export interface BloodPressure {
  systolic: number; // mmHg
  diastolic: number; // mmHg
}

export interface AstronautTelemetry {
  astronaut_name: string;
  space_radiation: number; // µSv/h
  distance_from_earth: number; // km
  blood_pressure: BloodPressure;
  heart_rate: number; // bpm
  blood_sugar: number; // mg/dL
  body_temperature: number; // °C
  sleep_hours: number; // h (0.0 - 24.0)
  mood: string;
  nutrition: number; // kcal
  oxygen_in_blood: number; // SpO2 %
}
