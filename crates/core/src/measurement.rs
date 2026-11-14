//! Validación de mediciones según el contrato.
use crate::{
    ErrorCode, ValidationError,
    catalog::{BodyZone, Indicator, SleepQuality},
    time::Timestamp,
    units::{Bpm, Celsius, Hours, MgDl, MmHg, Score},
    validate_text,
};
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq)]
pub enum Measurement {
    HeartRate {
        bpm: Bpm,
    },
    BloodPressure {
        systolic: MmHg,
        diastolic: MmHg,
    },
    BodyTemperature {
        celsius: Celsius,
    },
    Glucose {
        mg_dl: MgDl,
    },
    Sleep {
        hours: Hours,
        quality: Option<SleepQuality>,
        window: Option<(Timestamp, Timestamp)>,
    },
    Mood {
        score: Score,
    },
    Headache {
        score: Score,
    },
    Symptom {
        body_zone: BodyZone,
        score: Score,
        description: Option<String>,
    },
}
fn err(field: &str, code: ErrorCode, msg: &str) -> ValidationError {
    ValidationError {
        code,
        field: field.into(),
        message: msg.into(),
    }
}
fn number(v: &Value, field: &str) -> Result<f64, ValidationError> {
    v.get(field)
        .and_then(Value::as_f64)
        .filter(|n| n.is_finite())
        .ok_or_else(|| err(field, ErrorCode::InvalidValue, "Expected a finite number"))
}
fn score(v: &Value) -> Result<Score, ValidationError> {
    let n = v
        .get("score")
        .and_then(Value::as_f64)
        .filter(|n| n.is_finite() && (0.0..=10.0).contains(n) && n.fract() == 0.0)
        .ok_or_else(|| {
            err(
                "score",
                ErrorCode::InvalidValue,
                "Score must be an integer between 0 and 10",
            )
        })?;
    // La conversión se limita a las once representaciones exactas permitidas.
    let n = (0_u8..=10)
        .find(|x| f64::from(*x) == n)
        .ok_or_else(|| err("score", ErrorCode::InvalidValue, "Invalid score"))?;
    Score::try_from(n)
}
fn optional_string(v: &Value, field: &str) -> Result<Option<String>, ValidationError> {
    match v.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        _ => Err(err(
            field,
            ErrorCode::InvalidValue,
            "Expected a string or null",
        )),
    }
}
impl Measurement {
    pub fn validate(
        indicator: Indicator,
        value: &Value,
        unit: Option<&str>,
    ) -> Result<Self, ValidationError> {
        if unit.is_some_and(|u| u != indicator.unit()) {
            return Err(err("unit", ErrorCode::InvalidValue, "Incorrect unit"));
        }
        let obj = value
            .as_object()
            .ok_or_else(|| err("value", ErrorCode::InvalidValue, "Expected an object"))?;
        let allowed: &[&str] = match indicator {
            Indicator::HeartRate => &["bpm"],
            Indicator::BloodPressure => &["systolic", "diastolic"],
            Indicator::BodyTemperature => &["celsius"],
            Indicator::Glucose => &["mg_dl"],
            Indicator::Sleep => &["hours", "quality", "window_start", "window_end"],
            Indicator::Mood | Indicator::Headache => &["score"],
            Indicator::Symptom => &["body_zone", "score", "description"],
        };
        if let Some(key) = obj.keys().find(|k| !allowed.contains(&k.as_str())) {
            return Err(err(key, ErrorCode::InvalidSchema, "Unknown field"));
        }
        Ok(match indicator {
            Indicator::HeartRate => Self::HeartRate {
                bpm: Bpm::try_from(number(value, "bpm")?)?,
            },
            Indicator::BloodPressure => Self::BloodPressure {
                systolic: MmHg::try_from(number(value, "systolic")?).map_err(|mut e| {
                    e.field = "systolic".into();
                    e
                })?,
                diastolic: MmHg::try_from(number(value, "diastolic")?).map_err(|mut e| {
                    e.field = "diastolic".into();
                    e
                })?,
            },
            Indicator::BodyTemperature => Self::BodyTemperature {
                celsius: Celsius::try_from(number(value, "celsius")?)?,
            },
            Indicator::Glucose => Self::Glucose {
                mg_dl: MgDl::try_from(number(value, "mg_dl")?)?,
            },
            Indicator::Sleep => {
                let hours = Hours::try_from(number(value, "hours")?)?;
                let quality = optional_string(value, "quality")?
                    .map(|s| SleepQuality::parse(&s))
                    .transpose()?;
                let endpoint = |field: &str| -> Result<Option<String>, ValidationError> {
                    match value.get(field) {
                        None => Ok(None),
                        Some(Value::String(s)) => Ok(Some(s.clone())),
                        _ => Err(err(
                            field,
                            ErrorCode::InvalidValue,
                            "Expected a timestamp string",
                        )),
                    }
                };
                let start = endpoint("window_start")?;
                let end = endpoint("window_end")?;
                // Contrato §5: la ventana es obligatoria para poder comprobar `hours`.
                let window = match (start, end) {
                    (Some(a), Some(b)) => {
                        let a = Timestamp::parse_rfc3339(&a).map_err(|mut e| {
                            e.field = "window_start".into();
                            e
                        })?;
                        let b = Timestamp::parse_rfc3339(&b).map_err(|mut e| {
                            e.field = "window_end".into();
                            e
                        })?;
                        let duration = i128::from(b.millis()) - i128::from(a.millis());
                        if duration <= 0
                            || duration > 86_400_000
                            || hours.value() * 3_600_000.0
                                > f64::from(i32::try_from(duration).unwrap_or(i32::MAX))
                        {
                            return Err(err(
                                "window_end",
                                ErrorCode::InvalidValue,
                                "Invalid sleep window or duration",
                            ));
                        }
                        Some((a, b))
                    }
                    _ => {
                        return Err(err(
                            "window_start",
                            ErrorCode::InvalidValue,
                            "Sleep needs window_start and window_end",
                        ));
                    }
                };
                Self::Sleep {
                    hours,
                    quality,
                    window,
                }
            }
            Indicator::Mood => Self::Mood {
                score: score(value)?,
            },
            Indicator::Headache => Self::Headache {
                score: score(value)?,
            },
            Indicator::Symptom => {
                let zone = value
                    .get("body_zone")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        err("body_zone", ErrorCode::InvalidValue, "Expected a body zone")
                    })?;
                let description = optional_string(value, "description")?;
                if let Some(s) = &description {
                    validate_text("description", s, 2000)?;
                }
                Self::Symptom {
                    body_zone: BodyZone::parse(zone)?,
                    score: score(value)?,
                    description,
                }
            }
        })
    }
    pub fn indicator(&self) -> Indicator {
        match self {
            Self::HeartRate { .. } => Indicator::HeartRate,
            Self::BloodPressure { .. } => Indicator::BloodPressure,
            Self::BodyTemperature { .. } => Indicator::BodyTemperature,
            Self::Glucose { .. } => Indicator::Glucose,
            Self::Sleep { .. } => Indicator::Sleep,
            Self::Mood { .. } => Indicator::Mood,
            Self::Headache { .. } => Indicator::Headache,
            Self::Symptom { .. } => Indicator::Symptom,
        }
    }
    pub fn unit(&self) -> &'static str {
        self.indicator().unit()
    }
    pub fn body_zone(&self) -> Option<BodyZone> {
        match self {
            Self::Symptom { body_zone, .. } => Some(*body_zone),
            _ => None,
        }
    }
    pub fn primary(&self) -> f64 {
        match self {
            Self::HeartRate { bpm } => bpm.value(),
            Self::BloodPressure { systolic, .. } => systolic.value(),
            Self::BodyTemperature { celsius } => celsius.value(),
            Self::Glucose { mg_dl } => mg_dl.value(),
            Self::Sleep { hours, .. } => hours.value(),
            Self::Mood { score } | Self::Headache { score } | Self::Symptom { score, .. } => {
                f64::from(score.value())
            }
        }
    }
    pub fn to_value(&self) -> Value {
        match self {
            Self::HeartRate { bpm } => json!({"bpm": bpm.value()}),
            Self::BloodPressure {
                systolic,
                diastolic,
            } => json!({"systolic":systolic.value(),"diastolic":diastolic.value()}),
            Self::BodyTemperature { celsius } => json!({"celsius":celsius.value()}),
            Self::Glucose { mg_dl } => json!({"mg_dl":mg_dl.value()}),
            Self::Sleep {
                hours,
                quality,
                window,
            } => {
                let mut v = json!({"hours":hours.value(),"quality":quality});
                if let (Some((start, end)), Some(obj)) = (window, v.as_object_mut()) {
                    obj.insert("window_start".into(), json!(start));
                    obj.insert("window_end".into(), json!(end));
                }
                v
            }
            Self::Mood { score } | Self::Headache { score } => json!({"score":score.value()}),
            Self::Symptom {
                body_zone,
                score,
                description,
            } => json!({"body_zone":body_zone,"score":score.value(),"description":description}),
        }
    }
}
