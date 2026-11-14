//! Tabla central de umbrales y procedencia.
use crate::{
    catalog::Indicator,
    evaluate::{Direction, Escalation, SourceKind, Status},
};
#[derive(Clone, Copy, Debug)]
pub struct Rule {
    pub id: &'static str,
    pub indicator: Indicator,
    pub status: Status,
    pub direction: Direction,
    pub floor: Escalation,
    pub source: SourceKind,
    pub source_ref: Option<&'static str>,
    pub primary: (Compare, f64),
    pub secondary: Option<(Compare, f64)>,
}
#[derive(Clone, Copy, Debug)]
pub enum Compare {
    Lt,
    Le,
    Gt,
    Ge,
}
impl Compare {
    pub fn matches(self, value: f64, threshold: f64) -> bool {
        match self {
            Self::Lt => value < threshold,
            Self::Le => value <= threshold,
            Self::Gt => value > threshold,
            Self::Ge => value >= threshold,
        }
    }
}
/// Ventanas temporales en milisegundos.
pub const MINUTE: i64 = 60_000;
pub const HOUR: i64 = 60 * MINUTE;
pub const DAY: i64 = 24 * HOUR;
pub const FUTURE: i64 = 10 * MINUTE;
pub const OLD: i64 = 7 * DAY;
pub const HISTORY: i64 = 14 * DAY;
/// Rango plausible `(indicador, mínimo, máximo, rango secundario)`; fuera de él se pide confirmar.
pub type PlausibleRange = (Indicator, f64, f64, Option<(f64, f64)>);
pub const PLAUSIBILITY: &[PlausibleRange] = &[
    (Indicator::HeartRate, 30.0, 200.0, None),
    (Indicator::BloodPressure, 70.0, 230.0, Some((35.0, 140.0))),
    (Indicator::BodyTemperature, 34.0, 41.5, None),
    (Indicator::Glucose, 40.0, 450.0, None),
    (Indicator::Sleep, 0.0, 14.0, None),
];
pub const ABSOLUTE: &[Rule] = &[
    Rule {
        id: "heart_rate.alarm_high",
        indicator: Indicator::HeartRate,
        status: Status::Alarm,
        direction: Direction::Up,
        floor: Escalation::Urgent,
        source: SourceKind::Published,
        source_ref: Some(
            "AHA 2020 Guidelines for CPR and ECC, Part 3, Adult Tachycardia With a Pulse Algorithm (>= 150/min). Circulation 2020;142:S366-S468",
        ),
        primary: (Compare::Ge, 150.0),
        secondary: None,
    },
    Rule {
        id: "heart_rate.alarm_low",
        indicator: Indicator::HeartRate,
        status: Status::Alarm,
        direction: Direction::Down,
        floor: Escalation::Urgent,
        source: SourceKind::Published,
        source_ref: Some(
            "Royal College of Physicians, NEWS2 (2017), Chart 1: pulse <= 40 scores 3",
        ),
        primary: (Compare::Le, 40.0),
        secondary: None,
    },
    Rule {
        id: "heart_rate.attention_high",
        indicator: Indicator::HeartRate,
        status: Status::Change,
        direction: Direction::Up,
        floor: Escalation::SelfCare,
        source: SourceKind::Published,
        source_ref: Some("AHA: resting tachycardia > 100 bpm"),
        primary: (Compare::Gt, 100.0),
        secondary: None,
    },
    Rule {
        id: "heart_rate.attention_low",
        indicator: Indicator::HeartRate,
        status: Status::Change,
        direction: Direction::Down,
        floor: Escalation::SelfCare,
        source: SourceKind::Published,
        source_ref: Some(
            "2018 ACC/AHA/HRS Bradycardia Guideline, Section 3 (< 50 bpm while awake). Circulation 2019;140:e382-e482",
        ),
        primary: (Compare::Lt, 50.0),
        secondary: None,
    },
    Rule {
        id: "blood_pressure.alarm_high",
        indicator: Indicator::BloodPressure,
        status: Status::Alarm,
        direction: Direction::Up,
        floor: Escalation::Urgent,
        source: SourceKind::Published,
        source_ref: Some(
            "2025 AHA/ACC High Blood Pressure Guideline: severe hypertension > 180/120 mm Hg. Circulation 2025;152:e114-e218",
        ),
        primary: (Compare::Gt, 180.0),
        secondary: Some((Compare::Gt, 120.0)),
    },
    Rule {
        id: "blood_pressure.alarm_low",
        indicator: Indicator::BloodPressure,
        status: Status::Alarm,
        direction: Direction::Down,
        floor: Escalation::Urgent,
        source: SourceKind::DemoHeuristic,
        source_ref: None,
        primary: (Compare::Lt, 80.0),
        secondary: None,
    },
    Rule {
        id: "blood_pressure.attention_high",
        indicator: Indicator::BloodPressure,
        status: Status::Change,
        direction: Direction::Up,
        floor: Escalation::SelfCare,
        source: SourceKind::Published,
        source_ref: Some(
            "2017 ACC/AHA Guideline, stage 2 hypertension >= 140/90; NASA OCHMO-STD-100.1A",
        ),
        primary: (Compare::Ge, 140.0),
        secondary: Some((Compare::Ge, 90.0)),
    },
    Rule {
        id: "blood_pressure.attention_low",
        indicator: Indicator::BloodPressure,
        status: Status::Change,
        direction: Direction::Down,
        floor: Escalation::SelfCare,
        source: SourceKind::Published,
        source_ref: Some("NHLBI: low blood pressure < 90/60 mm Hg"),
        primary: (Compare::Lt, 90.0),
        secondary: Some((Compare::Lt, 60.0)),
    },
    Rule {
        id: "body_temperature.alarm_high",
        indicator: Indicator::BodyTemperature,
        status: Status::Alarm,
        direction: Direction::Up,
        floor: Escalation::Urgent,
        source: SourceKind::Published,
        source_ref: Some(
            "Bouchama & Knochel, Heat Stroke, N Engl J Med 2002;346:1978-1988 (> 40 C)",
        ),
        primary: (Compare::Gt, 40.0),
        secondary: None,
    },
    Rule {
        id: "body_temperature.alarm_low",
        indicator: Indicator::BodyTemperature,
        status: Status::Alarm,
        direction: Direction::Down,
        floor: Escalation::Urgent,
        source: SourceKind::Published,
        source_ref: Some("Hypothermia < 35 C; NASA-STD-3001 Vol. 1 Rev. C, Appendix C"),
        primary: (Compare::Lt, 35.0),
        secondary: None,
    },
    Rule {
        id: "body_temperature.attention_high",
        indicator: Indicator::BodyTemperature,
        status: Status::Change,
        direction: Direction::Up,
        floor: Escalation::SelfCare,
        source: SourceKind::Published,
        source_ref: Some(
            "Royal College of Physicians, NEWS2 (2017), Chart 1: 38.1-39.0 C scores 1",
        ),
        // Se compara el valor tal cual se anotó (termómetros de 0,1 °C): corte en 38.1.
        primary: (Compare::Ge, 38.1),
        secondary: None,
    },
    Rule {
        id: "body_temperature.attention_low",
        indicator: Indicator::BodyTemperature,
        status: Status::Change,
        direction: Direction::Down,
        floor: Escalation::SelfCare,
        source: SourceKind::Published,
        source_ref: Some(
            "Royal College of Physicians, NEWS2 (2017), Chart 1: 35.1-36.0 C scores 1",
        ),
        primary: (Compare::Le, 36.0),
        secondary: None,
    },
    Rule {
        id: "glucose.alarm_low",
        indicator: Indicator::Glucose,
        status: Status::Alarm,
        direction: Direction::Down,
        floor: Escalation::Urgent,
        source: SourceKind::Published,
        source_ref: Some(
            "ADA Standards of Care 2026, Section 6, Table 6.2: level 2 hypoglycemia < 54 mg/dL",
        ),
        primary: (Compare::Lt, 54.0),
        secondary: None,
    },
    Rule {
        id: "glucose.alarm_high",
        indicator: Indicator::Glucose,
        status: Status::Alarm,
        direction: Direction::Up,
        floor: Escalation::Urgent,
        source: SourceKind::Published,
        source_ref: Some(
            "ADA Standards of Care 2026, Section 6, Table 6.2: level 2 hyperglycemia > 250 mg/dL",
        ),
        primary: (Compare::Gt, 250.0),
        secondary: None,
    },
    Rule {
        id: "glucose.attention_low",
        indicator: Indicator::Glucose,
        status: Status::Change,
        direction: Direction::Down,
        floor: Escalation::SelfCare,
        source: SourceKind::Published,
        source_ref: Some(
            "ADA Standards of Care 2026, Section 6, Table 6.2: level 1 hypoglycemia < 70 mg/dL",
        ),
        primary: (Compare::Lt, 70.0),
        secondary: None,
    },
    Rule {
        id: "glucose.attention_high",
        indicator: Indicator::Glucose,
        status: Status::Change,
        direction: Direction::Up,
        floor: Escalation::SelfCare,
        source: SourceKind::Published,
        source_ref: Some(
            "ADA Standards of Care 2026, Section 6: upper limit of target range 70-180 mg/dL",
        ),
        primary: (Compare::Gt, 180.0),
        secondary: None,
    },
    Rule {
        id: "sleep.attention_low",
        indicator: Indicator::Sleep,
        status: Status::Change,
        direction: Direction::Down,
        floor: Escalation::SelfCare,
        source: SourceKind::DemoHeuristic,
        source_ref: Some(
            "Adults >= 7 h (AASM/SRS, Watson 2015); NASA-STD-3001 Vol. 1 Rev. C [V1 6001] plans 8.5 h; ISS crews averaged 6.09 h (Barger 2014)",
        ),
        primary: (Compare::Lt, 6.0),
        secondary: None,
    },
    Rule {
        id: "sleep.severe_low",
        indicator: Indicator::Sleep,
        status: Status::Change,
        direction: Direction::Down,
        floor: Escalation::FlagForGround,
        source: SourceKind::DemoHeuristic,
        source_ref: Some(
            "ISS: < 5 h sleep associated with somatic symptoms (Jones et al., Sleep 2022)",
        ),
        primary: (Compare::Lt, 5.0),
        secondary: None,
    },
    Rule {
        id: "mood.attention_low",
        indicator: Indicator::Mood,
        status: Status::Change,
        direction: Direction::Down,
        floor: Escalation::SelfCare,
        source: SourceKind::DemoHeuristic,
        source_ref: None,
        primary: (Compare::Le, 3.0),
        secondary: None,
    },
    Rule {
        id: "mood.severe_low",
        indicator: Indicator::Mood,
        status: Status::Change,
        direction: Direction::Down,
        floor: Escalation::FlagForGround,
        source: SourceKind::DemoHeuristic,
        source_ref: None,
        primary: (Compare::Le, 1.0),
        secondary: None,
    },
    Rule {
        id: "headache.attention_high",
        indicator: Indicator::Headache,
        status: Status::Change,
        direction: Direction::Up,
        floor: Escalation::SelfCare,
        source: SourceKind::Published,
        source_ref: Some(
            "Numeric Rating Scale 0-10: 4-6 moderate pain (Krebs et al., J Gen Intern Med 2007)",
        ),
        primary: (Compare::Ge, 4.0),
        secondary: None,
    },
    Rule {
        id: "headache.severe_high",
        indicator: Indicator::Headache,
        status: Status::Change,
        direction: Direction::Up,
        floor: Escalation::FlagForGround,
        source: SourceKind::Published,
        source_ref: Some("Numeric Rating Scale 0-10: 7-10 severe pain (Serlin et al., Pain 1995)"),
        primary: (Compare::Ge, 7.0),
        secondary: None,
    },
    Rule {
        id: "symptom.attention_high",
        indicator: Indicator::Symptom,
        status: Status::Change,
        direction: Direction::Up,
        floor: Escalation::SelfCare,
        source: SourceKind::DemoHeuristic,
        source_ref: None,
        primary: (Compare::Ge, 4.0),
        secondary: None,
    },
    Rule {
        id: "symptom.severe_high",
        indicator: Indicator::Symptom,
        status: Status::Change,
        direction: Direction::Up,
        floor: Escalation::FlagForGround,
        source: SourceKind::DemoHeuristic,
        source_ref: None,
        primary: (Compare::Ge, 7.0),
        secondary: None,
    },
];
/// Parámetros de historial por indicador: diferencia, dirección y caducidad.
pub fn history(ind: Indicator) -> (f64, Option<Direction>, i64) {
    match ind {
        Indicator::HeartRate => (15.0, None, 48 * HOUR),
        Indicator::BloodPressure => (20.0, None, 48 * HOUR),
        Indicator::BodyTemperature => (0.8, None, 48 * HOUR),
        Indicator::Glucose => (30.0, None, 72 * HOUR),
        Indicator::Sleep => (1.5, Some(Direction::Down), 48 * HOUR),
        Indicator::Mood => (3.0, Some(Direction::Down), 48 * HOUR),
        Indicator::Headache => (3.0, Some(Direction::Up), 48 * HOUR),
        Indicator::Symptom => (3.0, Some(Direction::Up), 72 * HOUR),
    }
}
pub const BP_DIASTOLIC_DELTA: f64 = 10.0;

/// Identificadores y procedencia de reglas no numéricas.
pub const EVENT_ALARM_ID: &str = "event.alarm";
pub const EVENT_ALARM_SOURCE: &str = "Merken requirements RF-10: every alarm event raises an alarm";
pub const BP_ORDER_ID: &str = "plaus.bp_order";
pub const FUTURE_ID: &str = "plaus.future_timestamp";
pub const FUTURE_SOURCE: &str = "Merken backend contract v0.3, section 6";
pub const OLD_ID: &str = "plaus.old_measurement";
