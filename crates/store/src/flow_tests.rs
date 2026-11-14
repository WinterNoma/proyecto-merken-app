//! Pruebas de flujo completo de `merken-store`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use crate::{
    Scenario, Store,
    api::{CorrectRecordInput, ListRecordsInput, LogRecordInput, LogResult, RaiseEventInput},
    records::PulledChange,
};
use merken_core::{
    catalog::{BodyZone, Indicator},
    evaluate::{NotEvaluatedReason, Status},
    record::EventKind,
    time::Timestamp,
};
use serde_json::{Value, json};

const V: &str = "0.1.0";
const HOUR: i64 = 3_600_000;

fn now() -> Timestamp {
    Timestamp::from_millis(1_800_000_000_000)
}

fn log(store: &mut Store, indicator: Indicator, value: Value, at: Timestamp) -> LogResult {
    store
        .log_record(
            LogRecordInput {
                indicator,
                value,
                measured_at: Some(at),
                source: None,
                note: None,
                confirm_warnings: false,
            },
            now(),
            V,
        )
        .unwrap()
}

fn saved_id(r: &LogResult) -> String {
    match r {
        LogResult::Saved { record, .. } => record.record_id.clone(),
        LogResult::NeedsConfirmation { .. } => panic!("expected saved"),
    }
}

fn outbox_count(store: &Store, entity: &str) -> usize {
    store
        .pending_outbox(1000)
        .unwrap()
        .iter()
        .filter(|o| o.entity == entity)
        .count()
}

#[test]
fn log_record_queues_contract_payload() {
    let mut store = Store::open_in_memory().unwrap();
    let r = log(&mut store, Indicator::HeartRate, json!({"bpm": 72}), now());
    let id = saved_id(&r);
    let out = store.pending_outbox(10).unwrap();
    let rec = out.iter().find(|o| o.entity == "record").unwrap();
    assert_eq!(rec.entity_id, id);
    assert_eq!(rec.payload["unit"], "bpm");
    assert_eq!(rec.payload["value"], json!({"bpm": 72.0}));
    assert_eq!(rec.payload["source"], "form");
    assert!(rec.payload.get("replaces").is_some());
    // Un solo dato: nunca "stable".
    if let LogResult::Saved { evaluation, .. } = r {
        assert_eq!(evaluation.status, Status::NotEvaluated);
        assert_eq!(evaluation.reason, Some(NotEvaluatedReason::TooFewData));
    }
}

#[test]
fn unusual_value_needs_confirmation_and_saves_nothing() {
    let mut store = Store::open_in_memory().unwrap();
    let r = log(&mut store, Indicator::HeartRate, json!({"bpm": 300}), now());
    assert!(matches!(r, LogResult::NeedsConfirmation { ref warnings } if !warnings.is_empty()));
    assert!(
        store
            .list_records(ListRecordsInput::default())
            .unwrap()
            .is_empty()
    );
    assert_eq!(store.pending_outbox(10).unwrap().len(), 0);
    let r = store
        .log_record(
            LogRecordInput {
                indicator: Indicator::HeartRate,
                value: json!({"bpm": 300}),
                measured_at: None,
                source: None,
                note: None,
                confirm_warnings: true,
            },
            now(),
            V,
        )
        .unwrap();
    match r {
        LogResult::Saved { evaluation, .. } => assert_eq!(evaluation.status, Status::Alarm),
        LogResult::NeedsConfirmation { .. } => panic!("expected saved"),
    }
}

#[test]
fn invalid_values_are_rejected_with_field() {
    let mut store = Store::open_in_memory().unwrap();
    let e = store
        .log_record(
            LogRecordInput {
                indicator: Indicator::Mood,
                value: json!({"score": 11}),
                measured_at: None,
                source: None,
                note: None,
                confirm_warnings: true,
            },
            now(),
            V,
        )
        .unwrap_err();
    assert_eq!(e.code, "invalid_value");
    assert_eq!(e.field.as_deref(), Some("value.score"));
    let e = store
        .log_record(
            LogRecordInput {
                indicator: Indicator::HeartRate,
                value: json!({"bpm": 70}),
                measured_at: None,
                source: Some(merken_core::record::RecordSource::Earth),
                note: None,
                confirm_warnings: true,
            },
            now(),
            V,
        )
        .unwrap_err();
    assert_eq!(e.code, "invalid_value");
}

#[test]
fn correction_rules() {
    let mut store = Store::open_in_memory().unwrap();
    let first = saved_id(&log(
        &mut store,
        Indicator::HeartRate,
        json!({"bpm": 72}),
        now(),
    ));
    let corr = |store: &mut Store, replaces: &str, value: Value| {
        store.correct_record(
            CorrectRecordInput {
                replaces: replaces.into(),
                value,
                measured_at: Some(now()),
                note: None,
                confirm_warnings: true,
            },
            now(),
            V,
        )
    };
    assert_eq!(
        corr(&mut store, "missing", json!({"bpm": 70}))
            .unwrap_err()
            .code,
        "not_found"
    );
    // Otro indicador: el valor no encaja con heart_rate.
    assert_eq!(
        corr(&mut store, &first, json!({"score": 3}))
            .unwrap_err()
            .code,
        "invalid_schema"
    );
    let second = saved_id(&corr(&mut store, &first, json!({"bpm": 75})).unwrap());
    let third = saved_id(&corr(&mut store, &second, json!({"bpm": 76})).unwrap());
    let all = store
        .list_records(ListRecordsInput {
            include_superseded: true,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(all.len(), 3);
    assert!(all.iter().all(|r| r.root_id == first));
    let old = all.iter().find(|r| r.record_id == first).unwrap();
    assert!(!old.effective);
    assert_eq!(old.superseded_by.as_deref(), Some(third.as_str()));
    let effective = store.list_records(ListRecordsInput::default()).unwrap();
    assert_eq!(effective.len(), 1);
    assert_eq!(effective[0].record_id, third);
    assert_eq!(effective[0].replaces.as_deref(), Some(second.as_str()));

    // Síntomas: la corrección conserva la zona.
    let s = saved_id(&log(
        &mut store,
        Indicator::Symptom,
        json!({"body_zone": "left_foot", "score": 4}),
        now(),
    ));
    let e = corr(
        &mut store,
        &s,
        json!({"body_zone": "right_foot", "score": 4}),
    )
    .unwrap_err();
    assert_eq!(e.field.as_deref(), Some("value.body_zone"));
}

#[test]
fn list_records_filters_and_orders() {
    let mut store = Store::open_in_memory().unwrap();
    for (i, bpm) in [70, 71, 72].iter().enumerate() {
        let at = now().saturating_add_ms(-(i64::try_from(i).unwrap() + 1) * HOUR);
        log(&mut store, Indicator::HeartRate, json!({"bpm": bpm}), at);
    }
    log(
        &mut store,
        Indicator::Symptom,
        json!({"body_zone": "head", "score": 2}),
        now(),
    );
    let hr = store
        .list_records(ListRecordsInput {
            indicator: Some(Indicator::HeartRate),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(hr.len(), 3);
    assert!(hr.windows(2).all(|w| w[0].measured_at >= w[1].measured_at));
    let head = store
        .list_records(ListRecordsInput {
            body_zone: Some(BodyZone::Head),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(head.len(), 1);
    let limited = store
        .list_records(ListRecordsInput {
            limit: Some(2),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(limited.len(), 2);
    let from = store
        .list_records(ListRecordsInput {
            from: Some(now().saturating_add_ms(-90 * 60_000)),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(from.len(), 2);
}

#[test]
fn need_help_alarms_and_feeling_report_does_not() {
    let mut store = Store::open_in_memory().unwrap();
    let r = store
        .raise_event(
            RaiseEventInput {
                kind: EventKind::FeelingReport,
                message: Some("Tired".into()),
                body_zone: None,
                occurred_at: None,
            },
            now(),
            V,
        )
        .unwrap();
    assert!(r.evaluation.is_none());
    assert!(store.status(now(), V).unwrap().alerts.is_empty());
    store
        .raise_event(
            RaiseEventInput {
                kind: EventKind::NeedHelp,
                message: None,
                body_zone: Some(BodyZone::Chest),
                occurred_at: None,
            },
            now(),
            V,
        )
        .unwrap();
    let st = store.status(now(), V).unwrap();
    assert_eq!(st.alerts.len(), 1);
    assert_eq!(st.overall.status, Status::Alarm);
    let e = store
        .raise_event(
            RaiseEventInput {
                kind: EventKind::Syncope,
                message: None,
                body_zone: None,
                occurred_at: None,
            },
            now(),
            V,
        )
        .unwrap_err();
    assert_eq!(e.code, "invalid_value");
}

#[test]
fn evaluations_are_not_duplicated() {
    let mut store = Store::open_in_memory().unwrap();
    log(&mut store, Indicator::HeartRate, json!({"bpm": 72}), now());
    store.status(now(), V).unwrap();
    let n = outbox_count(&store, "evaluation");
    store.status(now(), V).unwrap();
    store.status(now(), V).unwrap();
    assert_eq!(outbox_count(&store, "evaluation"), n);
    assert!(n >= 1);
}

#[test]
fn demo_never_reaches_outbox_and_blocks_on_real_data() {
    let mut store = Store::open_in_memory().unwrap();
    for s in [
        Scenario::Routine,
        Scenario::PersistentChange,
        Scenario::AlarmEvent,
    ] {
        let st = store.load_demo(s, now(), V).unwrap();
        assert!(st.is_demo);
        assert_eq!(store.pending_outbox(1000).unwrap().len(), 0);
        assert_eq!(st.sync.pending, 0);
    }
    let recs = store.list_records(ListRecordsInput::default()).unwrap();
    assert!(!recs.is_empty());
    assert!(recs.iter().all(|r| r.is_demo && r.sync == "local_only"));
    assert!(store.get_profile().unwrap().is_demo);

    // B1: el primer dato real borra lo DEMO en la misma transacción.
    let id = saved_id(&log(
        &mut store,
        Indicator::HeartRate,
        json!({"bpm": 72}),
        now(),
    ));
    let left = store
        .list_records(ListRecordsInput {
            include_superseded: true,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].record_id, id);
    assert!(!left[0].is_demo);
    assert!(store.get_profile().unwrap().profile.is_none());
    assert!(store.list_events(10).unwrap().is_empty());
    let st = store.status(now(), V).unwrap();
    assert!(!st.is_demo);
    assert!(st.alerts.is_empty());
    let hr = st
        .indicators
        .iter()
        .find(|e| e.indicator == "heart_rate")
        .unwrap();
    assert_eq!(hr.reason, Some(NotEvaluatedReason::TooFewData));
    assert!(
        store
            .pending_outbox(1000)
            .unwrap()
            .iter()
            .all(|o| o.entity_id == id || o.entity == "evaluation")
    );
    assert_eq!(
        store
            .load_demo(Scenario::Routine, now(), V)
            .unwrap_err()
            .code,
        "real_data_present"
    );
}

#[test]
fn need_help_during_demo_is_saved_as_real() {
    let mut store = Store::open_in_memory().unwrap();
    store.load_demo(Scenario::AlarmEvent, now(), V).unwrap();
    store
        .raise_event(
            RaiseEventInput {
                kind: EventKind::NeedHelp,
                message: None,
                body_zone: None,
                occurred_at: None,
            },
            now(),
            V,
        )
        .unwrap();
    let events = store.list_events(10).unwrap();
    assert_eq!(events.len(), 1);
    assert!(!events[0].is_demo);
    assert_eq!(events[0].sync, "pending");
    assert_eq!(store.status(now(), V).unwrap().alerts.len(), 1);
}

#[test]
fn future_first_symptom_returns_saved() {
    let mut store = Store::open_in_memory().unwrap();
    let r = store
        .log_record(
            LogRecordInput {
                indicator: Indicator::Symptom,
                value: json!({"body_zone": "back", "score": 5}),
                measured_at: Some(now().saturating_add_ms(2 * HOUR)),
                source: None,
                note: None,
                confirm_warnings: true,
            },
            now(),
            V,
        )
        .unwrap();
    match r {
        LogResult::Saved {
            record, evaluation, ..
        } => {
            assert!(record.future_timestamp);
            assert_eq!(evaluation.reason, Some(NotEvaluatedReason::NoData));
        }
        LogResult::NeedsConfirmation { .. } => panic!("expected saved"),
    }
}

#[test]
fn demo_scenarios_give_expected_status() {
    let mut store = Store::open_in_memory().unwrap();
    let st = store.load_demo(Scenario::Routine, now(), V).unwrap();
    assert!(st.alerts.is_empty());
    assert!(st.met.is_some_and(|m| m.starts_with("T+12d")));
    let st = store
        .load_demo(Scenario::PersistentChange, now(), V)
        .unwrap();
    let sleep = st
        .indicators
        .iter()
        .find(|e| e.indicator == "sleep")
        .unwrap();
    assert_eq!(sleep.status, Status::Change);
    let st = store.load_demo(Scenario::AlarmEvent, now(), V).unwrap();
    assert_eq!(st.overall.status, Status::Alarm);
    let foot = st
        .body_zones
        .iter()
        .find(|z| z.body_zone == BodyZone::LeftFoot)
        .unwrap();
    assert_eq!(foot.status, Status::Change);
}

#[test]
fn sync_bookkeeping() {
    let mut store = Store::open_in_memory().unwrap();
    log(&mut store, Indicator::HeartRate, json!({"bpm": 72}), now());
    let items = store.pending_outbox(10).unwrap();
    let first = items.first().unwrap().outbox_id;
    store.mark_synced(first, 129, now()).unwrap();
    if let Some(second) = items.get(1) {
        store
            .mark_failed(second.outbox_id, "invalid_value", false)
            .unwrap();
    }
    let st = store.status(now(), V).unwrap();
    assert!(st.sync.last_push_at.is_some());
    assert_eq!(st.sync.failed, i64::from(items.len() > 1));
    let synced = store.list_records(ListRecordsInput::default()).unwrap();
    assert_eq!(synced[0].sync, "synced");
}

fn pulled_record(id: &str, seq: i64, bpm: f64, replaces: Option<&str>) -> PulledChange {
    PulledChange {
        seq,
        entity: "record".into(),
        id: id.into(),
        payload: json!({
            "record_id": id, "indicator": "heart_rate", "value": {"bpm": bpm}, "unit": "bpm",
            "measured_at": "2026-11-14T10:00:00.000Z", "logged_at": "2026-11-14T10:00:00.000Z",
            "source": "earth", "interpreted_by": null, "replaces": replaces, "external_id": null,
            "note": null, "schema_version": 1, "app_version": null, "seq": seq,
            "received_at": "2026-11-14T10:01:00.000Z"
        }),
    }
}

#[test]
fn pull_page_keeps_everything_and_is_idempotent() {
    let mut store = Store::open_in_memory().unwrap();
    let page = vec![
        pulled_record("0193aaaa-0000-7000-8000-000000000001", 5, 99.0, None),
        PulledChange {
            seq: 6,
            entity: "analysis".into(),
            id: "y".into(),
            payload: json!({"summary": "ok", "seq": 6}),
        },
    ];
    assert_eq!(store.apply_pull_page(&page, "c1", now()).unwrap(), 1);
    assert_eq!(store.apply_pull_page(&page, "c2", now()).unwrap(), 0);
    assert_eq!(store.get_cursor().unwrap().as_deref(), Some("c2"));
    let kept: i64 = store
        .conn
        .query_row("SELECT COUNT(*) FROM pulled_changes", [], |r| r.get(0))
        .unwrap();
    assert_eq!(kept, 2);
    let recs = store.list_records(ListRecordsInput::default()).unwrap();
    assert_eq!(recs.len(), 1);
    assert_eq!(recs[0].sync, "synced");
    assert_eq!(outbox_count(&store, "record"), 0);
}

#[test]
fn invalid_pull_page_rolls_back_and_keeps_cursor() {
    let mut store = Store::open_in_memory().unwrap();
    let mut bad = pulled_record("0193aaaa-0000-7000-8000-000000000002", 8, 70.0, None);
    bad.payload["value"] = json!({"bpm": "fast"});
    let page = vec![
        pulled_record("0193aaaa-0000-7000-8000-000000000001", 7, 70.0, None),
        bad,
    ];
    assert!(store.apply_pull_page(&page, "c9", now()).is_err());
    assert!(store.get_cursor().unwrap().is_none());
    assert!(
        store
            .list_records(ListRecordsInput::default())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn lost_response_then_earth_correction_alarms() {
    let mut store = Store::open_in_memory().unwrap();
    // A se guarda en local y se sube, pero se pierde la respuesta (sigue sin seq).
    let a = saved_id(
        &store
            .log_record(
                LogRecordInput {
                    indicator: Indicator::HeartRate,
                    value: json!({"bpm": 70}),
                    measured_at: Some(Timestamp::parse_rfc3339("2026-11-14T10:00:00Z").unwrap()),
                    source: None,
                    note: None,
                    confirm_warnings: true,
                },
                now(),
                V,
            )
            .unwrap(),
    );
    let mut a_change = pulled_record(&a, 10, 70.0, None);
    a_change.payload["source"] = json!("form");
    a_change.payload["logged_at"] = json!(now());
    a_change.payload["app_version"] = json!(V);
    let b = pulled_record("0193bbbb-0000-7000-8000-000000000001", 11, 30.0, Some(&a));
    store
        .apply_pull_page(&[a_change.clone(), b.clone()], "c1", now())
        .unwrap();
    let st = store.status(now(), V).unwrap();
    let hr = st
        .indicators
        .iter()
        .find(|e| e.indicator == "heart_rate")
        .unwrap();
    assert_eq!(hr.status, Status::Alarm);
    assert_eq!(outbox_count(&store, "record"), 0);
    // Repetir la página es idempotente.
    store.apply_pull_page(&[a_change, b], "c2", now()).unwrap();
    assert_eq!(
        store
            .list_records(ListRecordsInput::default())
            .unwrap()
            .len(),
        1
    );
    // Mismo ID con otro contenido → conflicto y cursor intacto.
    let clash = pulled_record(&a, 12, 99.0, None);
    assert_eq!(
        store
            .apply_pull_page(&[clash], "c3", now())
            .unwrap_err()
            .code,
        "conflict"
    );
    assert_eq!(store.get_cursor().unwrap().as_deref(), Some("c2"));
}

#[test]
fn status_json_matches_core_api_keys() {
    let mut store = Store::open_in_memory().unwrap();
    store.load_demo(Scenario::AlarmEvent, now(), V).unwrap();
    let v = serde_json::to_value(store.status(now(), V).unwrap()).unwrap();
    let keys = |v: &Value| {
        let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
        k.sort();
        k
    };
    assert_eq!(
        keys(&v),
        [
            "alerts",
            "body_zones",
            "evaluated_at",
            "indicators",
            "is_demo",
            "met",
            "overall",
            "symptoms",
            "sync"
        ]
    );
    assert_eq!(keys(&v["overall"]), ["escalation", "status", "worst"]);
    assert_eq!(
        keys(&v["sync"]),
        ["failed", "last_pull_at", "last_push_at", "pending"]
    );
    assert_eq!(keys(&v["body_zones"][0]), ["body_zone", "status"]);
    assert_eq!(
        keys(&v["indicators"][0]),
        [
            "baseline",
            "body_zone",
            "core_version",
            "direction",
            "escalation",
            "evaluated_at",
            "history_count",
            "history_needed",
            "indicator",
            "input_event_ids",
            "input_record_ids",
            "latest_measured_at",
            "latest_value",
            "persistent",
            "reason",
            "rule_id",
            "rule_version",
            "source_kind",
            "source_ref",
            "status",
            "summary"
        ]
    );
    // 7 evaluados + `space_radiation` (contexto con datos en la DEMO).
    assert_eq!(v["indicators"].as_array().unwrap().len(), 8);
    assert_eq!(v["body_zones"].as_array().unwrap().len(), 14);
    let rec =
        serde_json::to_value(&store.list_records(ListRecordsInput::default()).unwrap()[0]).unwrap();
    assert_eq!(
        keys(&rec),
        [
            "body_zone",
            "effective",
            "future_timestamp",
            "indicator",
            "is_demo",
            "logged_at",
            "measured_at",
            "note",
            "record_id",
            "replaces",
            "root_id",
            "source",
            "superseded_by",
            "sync",
            "unit",
            "value"
        ]
    );
}

#[test]
fn pull_rejects_incoherent_journal() {
    let mut store = Store::open_in_memory().unwrap();
    let a = pulled_record("0193cccc-0000-7000-8000-000000000001", 20, 70.0, None);
    store
        .apply_pull_page(std::slice::from_ref(&a), "c1", now())
        .unwrap();
    // Mismo registro ya sincronizado con otro seq → conflicto (no se reescribe).
    let mut moved = pulled_record("0193cccc-0000-7000-8000-000000000001", 21, 70.0, None);
    moved.payload["seq"] = json!(21);
    assert_eq!(
        store
            .apply_pull_page(&[moved], "c2", now())
            .unwrap_err()
            .code,
        "conflict"
    );
    // Mismo seq con otro contenido → conflicto.
    let other = pulled_record("0193cccc-0000-7000-8000-000000000002", 20, 80.0, None);
    assert_eq!(
        store
            .apply_pull_page(&[other], "c3", now())
            .unwrap_err()
            .code,
        "conflict"
    );
    // seq ausente en el payload → inválido.
    let mut no_seq = pulled_record("0193cccc-0000-7000-8000-000000000003", 22, 70.0, None);
    no_seq.payload.as_object_mut().unwrap().remove("seq");
    assert_eq!(
        store
            .apply_pull_page(&[no_seq], "c4", now())
            .unwrap_err()
            .code,
        "invalid_schema"
    );
    // received_at ausente → inválido.
    let mut no_rx = pulled_record("0193cccc-0000-7000-8000-000000000004", 23, 70.0, None);
    no_rx.payload.as_object_mut().unwrap().remove("received_at");
    assert!(store.apply_pull_page(&[no_rx], "c5", now()).is_err());
    assert_eq!(store.get_cursor().unwrap().as_deref(), Some("c1"));
}

#[test]
fn context_indicators_are_stored_but_not_evaluated() {
    let mut store = Store::open_in_memory().unwrap();
    for (indicator, value) in [
        (Indicator::SpaceRadiation, json!({"usv_h": 14.2})),
        (Indicator::DistanceFromEarth, json!({"km": 384_400.0})),
        (Indicator::Nutrition, json!({"kcal": 2400.0})),
    ] {
        let r = log(&mut store, indicator, value, now());
        let LogResult::Saved { evaluation, .. } = r else {
            panic!("expected saved");
        };
        assert_eq!(evaluation.status, Status::NotEvaluated);
        assert_eq!(evaluation.reason, Some(NotEvaluatedReason::NotApplicable));
        assert!(evaluation.latest_value.is_some());
    }
    assert_eq!(outbox_count(&store, "record"), 3);
    let status = store.status(now(), V).unwrap();
    let radiation = status
        .indicators
        .iter()
        .find(|e| e.indicator == "space_radiation")
        .unwrap();
    assert_eq!(radiation.reason, Some(NotEvaluatedReason::NotApplicable));
    assert_eq!(radiation.latest_value, Some(json!({"usv_h": 14.2})));
    assert!(!status.overall.worst.iter().any(|w| w == "space_radiation"));
}

#[test]
fn oxygen_in_blood_low_raises_alarm_and_rejects_over_100() {
    let mut store = Store::open_in_memory().unwrap();
    let r = log(
        &mut store,
        Indicator::OxygenInBlood,
        json!({"percentage": 90}),
        now(),
    );
    let LogResult::Saved { evaluation, .. } = r else {
        panic!("expected saved");
    };
    assert_eq!(evaluation.status, Status::Alarm);
    assert_eq!(evaluation.rule_id, "oxygen_in_blood.alarm_low");
    let err = store
        .log_record(
            LogRecordInput {
                indicator: Indicator::OxygenInBlood,
                value: json!({"percentage": 101}),
                measured_at: Some(now()),
                source: None,
                note: None,
                confirm_warnings: true,
            },
            now(),
            V,
        )
        .unwrap_err();
    assert_eq!(err.field.as_deref(), Some("value.percentage"), "{err:?}");
}

#[test]
fn pull_page_with_retired_headache_advances_and_keeps_it() {
    let mut store = Store::open_in_memory().unwrap();
    let headache = PulledChange {
        seq: 7,
        entity: "record".into(),
        id: "0193aaaa-0000-7000-8000-0000000000aa".into(),
        payload: json!({
            "record_id": "0193aaaa-0000-7000-8000-0000000000aa", "indicator": "headache",
            "value": {"score": 6}, "unit": "score_0_10",
            "measured_at": "2026-11-13T10:00:00.000Z", "logged_at": "2026-11-13T10:00:00.000Z",
            "source": "form", "interpreted_by": null, "replaces": null, "external_id": null,
            "note": null, "schema_version": 1, "app_version": "0.1.0", "seq": 7,
            "received_at": "2026-11-13T10:01:00.000Z"
        }),
    };
    let page = vec![
        headache.clone(),
        pulled_record("0193aaaa-0000-7000-8000-000000000008", 8, 72.0, None),
    ];
    assert_eq!(store.apply_pull_page(&page, "c1", now()).unwrap(), 1);
    assert_eq!(store.apply_pull_page(&page, "c2", now()).unwrap(), 0);
    assert_eq!(store.get_cursor().unwrap().as_deref(), Some("c2"));
    let kept: i64 = store
        .conn
        .query_row(
            "SELECT COUNT(*) FROM pulled_changes WHERE entity_id = ?1",
            [&headache.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(kept, 1);
    let recs = store.list_records(ListRecordsInput::default()).unwrap();
    assert_eq!(recs.len(), 1);
    // Un `headache` mal formado sigue rechazándose.
    let mut bad = headache;
    bad.seq = 9;
    bad.payload["value"] = json!({"score": 11});
    bad.payload["seq"] = json!(9);
    assert!(store.apply_pull_page(&[bad], "c3", now()).is_err());
    assert_eq!(store.get_cursor().unwrap().as_deref(), Some("c2"));
}

#[test]
fn context_evaluations_are_never_queued() {
    let mut store = Store::open_in_memory().unwrap();
    log(
        &mut store,
        Indicator::Nutrition,
        json!({"kcal": 2400}),
        now(),
    );
    log(&mut store, Indicator::HeartRate, json!({"bpm": 72}), now());
    let evals: Vec<_> = store
        .pending_outbox(100)
        .unwrap()
        .into_iter()
        .filter(|o| o.entity == "evaluation")
        .collect();
    assert!(!evals.is_empty());
    assert!(
        evals
            .iter()
            .all(|o| o.payload["reason"] != "not_applicable")
    );
    assert!(evals.iter().all(|o| o.payload["indicator"] != "nutrition"));
}

#[test]
fn migration_v3_parks_pending_headache_outbox() {
    let store = Store::open_in_memory().unwrap();
    let Store { conn, .. } = store;
    conn.execute(
        "INSERT INTO outbox(entity, entity_id, payload_json, state, created_at)
         VALUES ('record', 'h1', '{\"indicator\":\"headache\",\"value\":{\"score\":5}}', 'pending', 0),
                ('record', 'r1', '{\"indicator\":\"heart_rate\",\"value\":{\"bpm\":70}}', 'pending', 0)",
        [],
    )
    .unwrap();
    conn.pragma_update(None, "user_version", 2).unwrap();
    let store = Store::initialize(conn).unwrap();
    let pending: Vec<String> = store
        .pending_outbox(10)
        .unwrap()
        .into_iter()
        .map(|o| o.entity_id)
        .collect();
    assert_eq!(pending, ["r1"]);
    let (state, err): (String, String) = store
        .conn
        .query_row(
            "SELECT state, last_error FROM outbox WHERE entity_id = 'h1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(state, "failed");
    assert!(err.starts_with("retired_indicator"));
}

const SIM: &str = "a9c7929e-f810-5b0f-ae07-7aeb92e4252c";

fn sim_record(id: &str, astronaut: &str, minutes_ago: i64, bpm: f64) -> Value {
    let at = Timestamp::from_millis(now().millis() - minutes_ago * 60_000).to_rfc3339();
    json!({
        "astronaut_id": astronaut, "record_id": id, "indicator": "heart_rate",
        "value": {"bpm": bpm}, "unit": "bpm", "measured_at": at, "logged_at": at,
        "source": "garmin", "external_id": format!("garmin:{id}"), "schema_version": 1,
        "app_version": null, "interpreted_by": null, "replaces": null,
        "note": "Synthetic simulated data; not a medical measurement."
    })
}

#[test]
fn import_synthetic_stores_demo_only_and_counts_rejections() {
    let mut store = Store::open_in_memory().unwrap();
    let mut records: Vec<Value> = (0..5)
        .map(|i| sim_record(&format!("rec-{i}"), SIM, 60 * (5 - i), 72.0 + i as f64))
        .collect();
    records.push(sim_record("rec-0", SIM, 10, 80.0)); // duplicado
    records.push(sim_record("other", "otro-astronauta", 10, 80.0)); // otro astronauta
    let mut bad = sim_record("bad", SIM, 10, 80.0);
    bad["unit"] = json!("mmHg");
    records.push(bad);
    let r = store
        .import_synthetic(SIM, "Alex Morgan", &records, now())
        .unwrap();
    assert_eq!((r.imported, r.rejected), (5, 3));
    assert!(r.first_measured_at < r.last_measured_at);
    assert!(store.pending_outbox(100).unwrap().is_empty());
    let status = store.status(now(), V).unwrap();
    assert!(status.is_demo);
    let hr = status
        .indicators
        .iter()
        .find(|e| e.indicator == "heart_rate")
        .unwrap();
    assert_ne!(hr.reason, Some(NotEvaluatedReason::NoData));
    // Las evaluaciones de datos sintéticos tampoco se suben.
    assert!(store.pending_outbox(100).unwrap().is_empty());
    let profile = serde_json::to_value(store.get_profile().unwrap()).unwrap();
    assert!(profile.to_string().contains("SIMULATION Alex Morgan"));
    // Reimportar sustituye, no duplica.
    let again = store
        .import_synthetic(SIM, "Alex Morgan", &records, now())
        .unwrap();
    assert_eq!(again.imported, 5);
    assert_eq!(
        store
            .list_records(ListRecordsInput::default())
            .unwrap()
            .len(),
        5
    );
}

#[test]
fn import_synthetic_refuses_when_real_data_exists_and_real_write_clears_it() {
    let mut store = Store::open_in_memory().unwrap();
    let records = vec![sim_record("rec-1", SIM, 30, 70.0)];
    store
        .import_synthetic(SIM, "Alex Morgan", &records, now())
        .unwrap();
    log(&mut store, Indicator::HeartRate, json!({"bpm": 66}), now());
    let recs = store.list_records(ListRecordsInput::default()).unwrap();
    assert_eq!(recs.len(), 1);
    let err = store
        .import_synthetic(SIM, "Alex Morgan", &records, now())
        .unwrap_err();
    assert_eq!(err.code, "real_data_present");
    let mut fresh = Store::open_in_memory().unwrap();
    let blank = fresh
        .import_synthetic(SIM, "  ", &records, now())
        .unwrap_err();
    assert_eq!(blank.field.as_deref(), Some("display_name"));
}

#[test]
fn import_synthetic_accepts_api_shapes() {
    // Misma forma que la respuesta de la API de simulación (°C, sueño con
    // ventana), con fechas del día de `now()` en las pruebas (15-ene-2027).
    let records = vec![
        json!({"astronaut_id": SIM, "record_id": "35746106-96c2-5579-b77a-f0b511d31f6b",
            "indicator": "body_temperature", "value": {"celsius": 36.4}, "unit": "°C",
            "measured_at": "2027-01-15T07:00:00.000Z", "logged_at": "2027-01-15T07:00:00.000Z",
            "source": "garmin", "external_id": "garmin:sim-01-body_temperature-2027-01-15T07:00:00.000Z",
            "schema_version": 1, "app_version": null, "interpreted_by": null, "replaces": null,
            "note": "Synthetic simulated data; not a medical measurement."}),
        json!({"astronaut_id": SIM, "record_id": "f0d1c1a2-0000-5000-8000-000000000001",
            "indicator": "sleep", "value": {"hours": 7.4, "quality": "good",
                "window_start": "2027-01-15T00:00:00.000Z", "window_end": "2027-01-15T08:00:00.000Z"},
            "unit": "h", "measured_at": "2027-01-15T08:00:00.000Z", "logged_at": "2027-01-15T08:00:00.000Z",
            "source": "garmin", "external_id": "garmin:sim-01-sleep-2027-01-15T08:00:00.000Z",
            "schema_version": 1, "app_version": null, "interpreted_by": null, "replaces": null,
            "note": "Synthetic simulated data; not a medical measurement."}),
    ];
    let r = store_with_import(&records);
    assert_eq!((r.imported, r.rejected), (2, 0));
    // Con el reloj real del 14-nov-2026 esas mismas lecturas serían futuras:
    // no se cargan, para que el núcleo no las evalúe como recientes (Sol, I2).
    let nov_14 = Timestamp::parse_rfc3339("2026-11-14T12:00:00.000Z").unwrap();
    let mut store = Store::open_in_memory().unwrap();
    let r = store
        .import_synthetic(SIM, "Alex Morgan", &records, nov_14)
        .unwrap();
    assert_eq!((r.imported, r.rejected), (0, 2));
}

#[test]
fn import_synthetic_rejects_records_outside_the_contract() {
    let mut records = vec![sim_record("ok", SIM, 10, 70.0)];
    let mut v99 = sim_record("v99", SIM, 10, 70.0);
    v99["schema_version"] = json!(99);
    let mut no_id = sim_record("x", SIM, 10, 70.0);
    no_id["record_id"] = json!(" ");
    let mut long_note = sim_record("note", SIM, 10, 70.0);
    long_note["note"] = json!("a".repeat(2001));
    let future = sim_record("future", SIM, -11, 70.0);
    records.extend([v99, no_id, long_note, future]);
    let r = store_with_import(&records);
    assert_eq!((r.imported, r.rejected), (1, 4));
}

#[test]
fn import_synthetic_failure_keeps_the_previous_demo() {
    // B1 de Sol: si falla una inserción, ni la DEMO anterior ni el perfil
    // cambian (todo va en una sola transacción).
    let mut store = Store::open_in_memory().unwrap();
    let first = vec![
        sim_record("old-1", SIM, 30, 70.0),
        sim_record("old-2", SIM, 20, 71.0),
    ];
    store
        .import_synthetic(SIM, "Alex Morgan", &first, now())
        .unwrap();
    store
        .conn
        .execute_batch(
            "CREATE TRIGGER fail BEFORE INSERT ON records WHEN NEW.record_id = 'boom'              BEGIN SELECT RAISE(ABORT, 'boom'); END;",
        )
        .unwrap();
    let second = vec![
        sim_record("new-1", SIM, 15, 72.0),
        sim_record("boom", SIM, 10, 73.0),
    ];
    assert!(
        store
            .import_synthetic(SIM, "Jordan Lee", &second, now())
            .is_err()
    );
    let ids: Vec<String> = store
        .list_records(ListRecordsInput::default())
        .unwrap()
        .into_iter()
        .map(|r| r.record_id)
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.iter().all(|id| id.starts_with("old-")));
    let profile = serde_json::to_value(store.get_profile().unwrap()).unwrap();
    assert!(profile.to_string().contains("SIMULATION Alex Morgan"));
    assert!(store.status(now(), V).unwrap().is_demo);
}

fn store_with_import(records: &[Value]) -> crate::api::ImportResult {
    let mut store = Store::open_in_memory().unwrap();
    store
        .import_synthetic(SIM, "Alex Morgan", records, now())
        .unwrap()
}
