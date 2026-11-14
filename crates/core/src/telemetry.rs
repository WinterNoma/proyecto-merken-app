//! Modelo canónico de telemetría médica para astronautas (11 indicadores).
//! Excluye cualquier indicador descartado (sin cefalea/headache ni CO₂).

use serde::{Deserialize, Serialize};

/// Presión arterial con sistólica y diastólica en mmHg.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BloodPressure {
    pub systolic: u16,
    pub diastolic: u16,
}

/// Registro unificado de telemetría médica del astronauta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AstronautTelemetry {
    /// Nombre o identificador del tripulante.
    #[serde(alias = "astronaut_name", alias = "astronautName", alias = "AstronautName")]
    pub astronaut_name: String,

    /// Tasa de radiación espacial en µSv/h (o dosis).
    #[serde(alias = "space_radiation", alias = "spaceRadiation", alias = "SpaceRadiation")]
    pub space_radiation: f64,

    /// Distancia desde la Tierra en kilómetros.
    #[serde(alias = "distance_from_earth", alias = "distanceFromEarth", alias = "DistanceFromEarth")]
    pub distance_from_earth: f64,

    /// Presión arterial (sistólica/diastólica mmHg).
    #[serde(alias = "blood_pressure", alias = "bloodPressure", alias = "BloodPressure")]
    pub blood_pressure: BloodPressure,

    /// Frecuencia cardíaca / pulso en latidos por minuto (bpm).
    #[serde(alias = "heart_rate", alias = "heartRate", alias = "HeartRate")]
    pub heart_rate: u16,

    /// Nivel de glucosa en sangre en mg/dL.
    #[serde(alias = "blood_sugar", alias = "bloodSugar", alias = "BloodSugar")]
    pub blood_sugar: f64,

    /// Temperatura corporal en grados Celsius (°C).
    #[serde(alias = "body_temperature", alias = "bodyTemperature", alias = "BodyTemperature")]
    pub body_temperature: f64,

    /// Horas de sueño registradas (0.0 - 24.0).
    #[serde(alias = "sleep_hours", alias = "sleepHours", alias = "SleepHours")]
    pub sleep_hours: f64,

    /// Estado de ánimo o bienestar psicológico.
    #[serde(alias = "mood", alias = "Mood")]
    pub mood: String,

    /// Ingesta nutricional / energía en kilocalorías (kcal).
    #[serde(alias = "nutrition", alias = "Nutrition")]
    pub nutrition: f64,

    /// Saturación de oxígeno en sangre (SpO₂) en porcentaje (0.0 - 100.0).
    #[serde(alias = "oxygen_in_blood", alias = "oxygenInBlood", alias = "OxygenInBlood")]
    pub oxygen_in_blood: f64,
}
