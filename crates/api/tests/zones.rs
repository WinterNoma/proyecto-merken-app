use pulsys_api::{
    BodyZone, Direction, Escalation, PulseEvaluation, SourceKind, Status, SymptomEvaluation,
    Timestamp, ZoneSnapshot, ZoneState, close_store, open_store, zone_status,
};
use pulsys_core::{RULES_VERSION, catalog::Indicator, rules::MINUTE};
use pulsys_store::{
    Store,
    api::{LogRecordInput, LogResult},
};
use serde_json::{Value, json};
use std::error::Error;

const NOW_MS: i64 = 1_800_000_000_000;
const APP_VERSION: &str = "0.1.0";

fn log_fixture(
    store: &mut Store,
    indicator: Indicator,
    value: Value,
    offset_ms: i64,
) -> Result<String, Box<dyn Error>> {
    let result = store.log_record(
        LogRecordInput {
            indicator,
            value,
            measured_at: Some(Timestamp::from_millis(NOW_MS + offset_ms)),
            source: None,
            note: None,
            confirm_warnings: false,
        },
        Timestamp::from_millis(NOW_MS),
        APP_VERSION,
    )?;
    match result {
        LogResult::Saved { record, .. } => Ok(record.record_id),
        LogResult::NeedsConfirmation { .. } => Err("Fixture must not need confirmation".into()),
    }
}

fn expected_zones(symptom_ids: Vec<String>) -> Vec<ZoneState> {
    let left_arm = ZoneState {
        zone_id: BodyZone::LeftArm,
        status: Status::Change,
        symptoms: vec![SymptomEvaluation {
            status: Status::Change,
            reason: None,
            evaluated_at_ms: NOW_MS,
            rule_id: "symptom.attention_high".into(),
            rule_version: RULES_VERSION.into(),
            source_kind: SourceKind::DemoHeuristic,
            source_ref: None,
            input_record_ids: symptom_ids,
            input_event_ids: Vec::new(),
        }],
    };
    BodyZone::ALL
        .into_iter()
        .map(|zone_id| {
            if zone_id == left_arm.zone_id {
                left_arm.clone()
            } else {
                ZoneState {
                    zone_id,
                    status: Status::NotEvaluated,
                    symptoms: Vec::new(),
                }
            }
        })
        .collect()
}

fn expected_pulse(pulse_ids: Vec<String>) -> PulseEvaluation {
    PulseEvaluation {
        status: Status::Change,
        reason: None,
        direction: Some(Direction::Up),
        persistent: false,
        escalation: Escalation::SelfCare,
        evaluated_at_ms: NOW_MS,
        latest_measured_at_ms: Some(NOW_MS),
        rule_id: "heart_rate.attention_high".into(),
        rule_version: RULES_VERSION.into(),
        source_kind: SourceKind::Published,
        source_ref: Some("AHA: resting tachycardia > 100 bpm".into()),
        input_record_ids: pulse_ids,
        history_count: 3,
        history_needed: 3,
        baseline_bpm: Some(74.0),
    }
}

#[test]
fn zones_project_symptom_evidence_and_independent_pulse_evaluation() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let mut store = Store::open(&dir.path().join("pulsys.sqlite3"))?;
    let older = log_fixture(
        &mut store,
        Indicator::Symptom,
        json!({"body_zone": "left_arm", "score": 3}),
        -2 * MINUTE,
    )?;
    let latest = log_fixture(
        &mut store,
        Indicator::Symptom,
        json!({"body_zone": "left_arm", "score": 5}),
        -MINUTE,
    )?;
    let mut pulse_ids = Vec::new();
    for minutes in [-3, -2, -1] {
        pulse_ids.push(log_fixture(
            &mut store,
            Indicator::HeartRate,
            json!({"bpm": 74}),
            minutes * MINUTE,
        )?);
    }
    pulse_ids.push(log_fixture(
        &mut store,
        Indicator::HeartRate,
        json!({"bpm": 110}),
        0,
    )?);
    pulse_ids.reverse();
    store.close()?;

    let mut app = open_store(
        dir.path(),
        APP_VERSION.into(),
        Box::new(|| Ok(Timestamp::from_millis(NOW_MS))),
    )?;
    let actual = zone_status(&mut app)?;
    assert_eq!(
        actual,
        ZoneSnapshot {
            evaluated_at_ms: NOW_MS,
            is_demo: false,
            zones: expected_zones(vec![latest, older]),
            pulse: expected_pulse(pulse_ids),
        },
    );
    close_store(app)?;
    Ok(())
}
