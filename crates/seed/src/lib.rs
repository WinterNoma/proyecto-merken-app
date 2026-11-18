//! Datos sintéticos, reproducibles y sin información de personas reales.

use pulsys_core::{catalog::Indicator, time::Timestamp};
use serde_json::{Value, json};

const DAY_MS: i64 = 86_400_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    Routine,
    PersistentChange,
    AlarmEvent,
}

impl Scenario {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "routine" => Some(Self::Routine),
            "persistent_change" => Some(Self::PersistentChange),
            "alarm_event" => Some(Self::AlarmEvent),
            _ => None,
        }
    }
}

pub struct DemoRecord {
    pub indicator: Indicator,
    pub value: Value,
    pub measured_at: Timestamp,
    pub note: Option<String>,
}

pub struct DemoEvent {
    pub kind: &'static str,
    pub message: Option<String>,
    pub body_zone: Option<&'static str>,
    pub occurred_at: Timestamp,
}

pub struct DemoData {
    pub records: Vec<DemoRecord>,
    pub events: Vec<DemoEvent>,
}

/// Perfil completamente ficticio y marcado como DEMO.
pub fn demo_profile(now: Timestamp) -> Value {
    let epoch = now
        .millis()
        .saturating_sub(12 * DAY_MS)
        .div_euclid(60_000)
        .saturating_mul(60_000);
    json!({
        "astronaut": {"astronaut_id": "demo-astronaut", "username": "demo", "display_name": "DEMO Astronaut",
            "mission": {"mission_id": "demo-mission", "name": "DEMO Mission", "epoch_utc": Timestamp::from_millis(epoch)}},
        "medical_profile": {"height_cm": 175.0, "weight_kg": 72.0, "blood_type": "O+", "sex": null,
            "date_of_birth": null, "notes": "DEMO synthetic profile", "verified_by": null,
            "verified_at": null, "profile_version": 1, "updated_at": now},
        "medical_conditions": [{"condition_id": "demo-condition", "name": "DEMO: Seasonal allergy",
            "icd10_code": null, "status": "resolved", "since": null, "notes": null}]
    })
}

pub fn scenario(scenario: Scenario, now: Timestamp) -> DemoData {
    let mut records = Vec::new();
    for day in (0_i64..7).rev() {
        let measured_at = now.saturating_add_ms(-day * DAY_MS);
        let changed = scenario == Scenario::PersistentChange && day < 3;
        let variation = (day % 3) as f64 - 1.0;
        for (indicator, value) in [
            (
                Indicator::HeartRate,
                json!({"bpm": 70.0 + variation + if changed { 20.0 } else { 0.0 }}),
            ),
            (
                Indicator::BloodPressure,
                json!({"systolic": 118.0 + variation, "diastolic": 76.0 + variation}),
            ),
            (
                Indicator::BodyTemperature,
                json!({"celsius": 36.7 + variation * 0.1}),
            ),
            (
                Indicator::Sleep,
                json!({
                    "hours": if changed { 4.5 } else { 7.5 + variation * 0.1 },
                    "quality": if changed { "poor" } else { "good" },
                    // Ventana de 9 h que termina al medir (contrato §5).
                    "window_start": measured_at.saturating_add_ms(-9 * 3_600_000),
                    "window_end": measured_at,
                }),
            ),
            (Indicator::Mood, json!({"score": 7})),
            (
                Indicator::OxygenInBlood,
                json!({"percentage": 97.5 + variation * 0.1}),
            ),
            (
                Indicator::SpaceRadiation,
                json!({"usv_h": 15.0 + variation * 0.5}),
            ),
        ] {
            records.push(DemoRecord {
                indicator,
                value,
                measured_at,
                note: Some("DEMO synthetic data".to_owned()),
            });
        }
        if day == 0 || day == 3 {
            records.push(DemoRecord {
                indicator: Indicator::Glucose,
                value: json!({"mg_dl": 95.0 + variation}),
                measured_at,
                note: Some("DEMO synthetic data".to_owned()),
            });
        }
    }
    let mut events = Vec::new();
    if scenario == Scenario::AlarmEvent {
        records.push(DemoRecord {
            indicator: Indicator::Symptom,
            value: json!({"body_zone": "left_foot", "score": 7, "description": "DEMO symptom"}),
            measured_at: now.saturating_add_ms(-1_800_000),
            note: Some("DEMO synthetic data".to_owned()),
        });
        events.push(DemoEvent {
            kind: "need_help",
            message: Some("DEMO help request".to_owned()),
            body_zone: None,
            occurred_at: now.saturating_add_ms(-1_800_000),
        });
    }
    DemoData { records, events }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulsys_core::{measurement::Measurement, plausibility};

    #[test]
    fn demo_values_are_valid_and_plausible() -> Result<(), Box<dyn std::error::Error>> {
        let now = Timestamp::from_millis(1_800_000_000_000);
        for scenario_kind in [
            Scenario::Routine,
            Scenario::PersistentChange,
            Scenario::AlarmEvent,
        ] {
            for record in scenario(scenario_kind, now).records {
                let measurement = Measurement::validate(record.indicator, &record.value, None)
                    .map_err(|error| format!("{}: {}", error.field, error.message))?;
                assert!(plausibility::check(&measurement, record.measured_at, now).is_empty());
            }
        }
        Ok(())
    }

    #[test]
    fn scenarios_produce_expected_evaluations() -> Result<(), Box<dyn std::error::Error>> {
        use pulsys_core::effective::RecordView;
        use pulsys_core::evaluate::{EventView, evaluate_all};
        use pulsys_core::record::EventKind;
        let now = Timestamp::from_millis(1_800_000_000_000);
        for scenario_kind in [
            Scenario::Routine,
            Scenario::PersistentChange,
            Scenario::AlarmEvent,
        ] {
            let data = scenario(scenario_kind, now);
            let mut records = Vec::new();
            for (order, record) in data.records.into_iter().enumerate() {
                let measurement = Measurement::validate(record.indicator, &record.value, None)
                    .map_err(|error| format!("{}: {}", error.field, error.message))?;
                records.push(RecordView {
                    record_id: format!("demo-{order}"),
                    indicator: record.indicator,
                    body_zone: measurement.body_zone(),
                    measurement,
                    measured_at: record.measured_at,
                    logged_at: now,
                    received_at: None,
                    seq: None,
                    local_order: i64::try_from(order)?,
                    replaces: None,
                });
            }
            let events = data
                .events
                .into_iter()
                .enumerate()
                .map(|(order, event)| EventView {
                    event_id: format!("demo-event-{order}"),
                    kind: EventKind::NeedHelp,
                    occurred_at: event.occurred_at,
                    priority: EventKind::NeedHelp.default_priority(),
                })
                .collect::<Vec<_>>();
            let evaluations = serde_json::to_value(evaluate_all(&records, &events, now))?;
            let evaluations = evaluations.as_array().ok_or("expected evaluations array")?;
            let find = |indicator: &str| {
                evaluations
                    .iter()
                    .find(|item| item.get("indicator").and_then(Value::as_str) == Some(indicator))
            };
            let pulse = find("heart_rate").ok_or("missing heart rate")?;
            let sleep = find("sleep").ok_or("missing sleep")?;
            if scenario_kind == Scenario::PersistentChange {
                for item in [pulse, sleep] {
                    assert_eq!(item.get("status").and_then(Value::as_str), Some("change"));
                    assert_eq!(item.get("persistent").and_then(Value::as_bool), Some(true));
                    assert_eq!(
                        item.get("escalation").and_then(Value::as_str),
                        Some("flag_for_ground")
                    );
                }
            } else {
                assert_eq!(pulse.get("status").and_then(Value::as_str), Some("stable"));
                assert_eq!(sleep.get("status").and_then(Value::as_str), Some("stable"));
            }
            if scenario_kind == Scenario::AlarmEvent {
                let alarm = find("need_help").ok_or("missing help alarm")?;
                assert_eq!(alarm.get("status").and_then(Value::as_str), Some("alarm"));
            }
        }
        Ok(())
    }
}
