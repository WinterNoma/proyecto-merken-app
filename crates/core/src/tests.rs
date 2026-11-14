#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
use crate::{
    ErrorCode,
    catalog::{BodyZone, Indicator},
    effective::{EffectiveSet, RecordView, SeriesKey},
    evaluate::{self, Direction, Escalation, EventView, NotEvaluatedReason, Status},
    measurement::Measurement,
    plausibility,
    record::{EventKind, Priority},
    time::{Timestamp, mission_elapsed},
    units::Score,
};
use proptest::prelude::*;
use serde_json::{Value, json};
fn t(n: i64) -> Timestamp {
    Timestamp::from_millis(n)
}
fn measurement(i: Indicator, v: Value) -> Measurement {
    Measurement::validate(i, &v, None).unwrap()
}
fn record(id: &str, value: f64, at: i64, replaces: Option<&str>) -> RecordView {
    RecordView {
        record_id: id.into(),
        indicator: Indicator::HeartRate,
        body_zone: None,
        measurement: measurement(Indicator::HeartRate, json!({"bpm":value})),
        measured_at: t(at),
        logged_at: t(at),
        received_at: None,
        seq: None,
        local_order: at,
        replaces: replaces.map(str::to_owned),
    }
}
fn eval(values: &[f64]) -> evaluate::Evaluation {
    let records: Vec<_> = values
        .iter()
        .enumerate()
        .map(|(i, v)| record(&i.to_string(), *v, i64::try_from(i).unwrap() * 1000, None))
        .collect();
    let refs: Vec<_> = records.iter().collect();
    evaluate::evaluate_series(
        SeriesKey {
            indicator: Indicator::HeartRate,
            body_zone: None,
        },
        &refs,
        t(i64::try_from(values.len()).unwrap() * 1000),
    )
}
#[test]
fn timestamps() {
    let ts = Timestamp::parse_rfc3339("2026-11-14T23:05:33.120+02:00").unwrap();
    assert_eq!(ts.to_rfc3339(), "2026-11-14T21:05:33.120Z");
    assert_eq!(
        Timestamp::from_millis(-1).to_rfc3339(),
        "1969-12-31T23:59:59.999Z"
    );
    assert_eq!(mission_elapsed(t(3_600_000), t(0)), "T-00d 01:00:00");
    assert_eq!(
        serde_json::from_str::<Timestamp>(&serde_json::to_string(&ts).unwrap()).unwrap(),
        ts
    );
}
#[test]
fn validation_contract() {
    for (i, v) in [
        (Indicator::HeartRate, json!({"bpm":300})),
        (
            Indicator::BloodPressure,
            json!({"systolic":120,"diastolic":80}),
        ),
        (Indicator::BodyTemperature, json!({"celsius":14})),
        (Indicator::Glucose, json!({"mg_dl":2375})),
        (
            Indicator::Sleep,
            json!({"hours":5,"quality":null,"window_start":"2026-01-01T00:00:00Z","window_end":"2026-01-01T08:00:00Z"}),
        ),
        (Indicator::Mood, json!({"score":0})),
        (Indicator::OxygenInBlood, json!({"percentage":62})),
        (Indicator::SpaceRadiation, json!({"usv_h":0})),
        (Indicator::DistanceFromEarth, json!({"km":401_000_000.0})),
        (Indicator::Nutrition, json!({"kcal":0})),
        (
            Indicator::Symptom,
            json!({"body_zone":"left_foot","score":4,"description":"pain"}),
        ),
    ] {
        let m = measurement(i, v);
        assert_eq!(
            Measurement::validate(i, &m.to_value(), Some(i.unit())).unwrap(),
            m
        );
    }
    for (i, v) in [
        (Indicator::HeartRate, json!({"bpm":0})),
        (
            Indicator::BloodPressure,
            json!({"systolic":0,"diastolic":3}),
        ),
        (
            Indicator::BloodPressure,
            json!({"systolic":3,"diastolic":-1}),
        ),
        (Indicator::BodyTemperature, json!({"celsius":0})),
        (Indicator::Glucose, json!({"mg_dl":0})),
        (Indicator::Sleep, json!({"hours":25})),
        (Indicator::Sleep, json!({"hours":2,"quality":"bad"})),
        (
            Indicator::Sleep,
            json!({"hours":2,"window_start":"2026-01-01T00:00:00Z"}),
        ),
        (
            Indicator::Sleep,
            json!({"hours":2,"window_start":"2026-01-01T00:00:00Z","window_end":"2026-01-01T01:00:00Z"}),
        ),
        (
            Indicator::Sleep,
            json!({"hours":1,"window_start":"2026-01-01T00:00:00Z","window_end":"2026-01-02T01:00:00Z"}),
        ),
        (
            Indicator::Sleep,
            json!({"hours":0,"window_start":"2026-01-01T00:00:00Z","window_end":"2026-01-01T00:00:00Z"}),
        ),
        (Indicator::Mood, json!({"score":2.5})),
        (Indicator::Symptom, json!({"body_zone":"unknown","score":1})),
        (Indicator::HeartRate, json!({"bpm":"72"})),
    ] {
        assert!(Measurement::validate(i, &v, None).is_err(), "{i:?} {v}");
    }
    assert_eq!(
        Measurement::validate(Indicator::HeartRate, &json!({"bpm":72,"extra":1}), None)
            .unwrap_err()
            .code,
        ErrorCode::InvalidSchema
    );
    assert!(Measurement::validate(Indicator::HeartRate, &json!({"bpm":72}), Some("Hz")).is_err());
    assert!(
        Measurement::validate(
            Indicator::Symptom,
            &json!({"body_zone":"head","score":1,"description":"x".repeat(2001)}),
            None
        )
        .is_err()
    );
    assert!(crate::validate_text("note", &"é".repeat(2000), 2000).is_ok());
    assert!(crate::validate_text("message", &"é".repeat(2001), 2000).is_err());
}
#[test]
fn warnings() {
    let m = measurement(
        Indicator::BloodPressure,
        json!({"systolic":60,"diastolic":80}),
    );
    let w = plausibility::check(&m, t(0), t(8 * crate::rules::DAY));
    assert!(w.iter().any(|w| w.code == "diastolic_not_below_systolic"));
    assert!(w.iter().any(|w| w.code == "old_measurement"));
    assert!(
        plausibility::check(&m, t(crate::rules::FUTURE + 1), t(0))
            .iter()
            .any(|w| w.code == "future_timestamp")
    );
}
#[test]
fn effective_chains() {
    let mut a = record("a", 70.0, 0, None);
    a.seq = Some(1);
    let mut b = record("b", 71.0, 1, Some("a"));
    b.seq = Some(2);
    let c = record("c", 72.0, 2, Some("b"));
    let x = record("x", 70.0, 3, Some("y"));
    let y = record("y", 70.0, 4, Some("x"));
    let z = record("z", 70.0, 5, Some("missing"));
    let mut future = record("future", 70.0, crate::rules::FUTURE + 1, None);
    future.logged_at = t(0);
    let records = vec![a, b, c, x, y, z, future];
    let set = EffectiveSet::build(&records);
    assert_eq!(set.root_of("c"), Some("a"));
    assert_eq!(set.superseded_by("a"), Some("c"));
    assert!(set.is_effective("c"));
    assert_eq!(set.root_of("x"), Some("x"));
    assert_eq!(set.root_of("y"), Some("y"));
    assert_eq!(set.root_of("z"), Some("z"));
    assert!(set.is_future("future"));
    assert!(!set.is_effective("future"));
}
#[test]
fn states_and_escalation() {
    assert_eq!(eval(&[]).reason, Some(NotEvaluatedReason::NoData));
    assert_eq!(eval(&[70.0]).reason, Some(NotEvaluatedReason::TooFewData));
    assert_eq!(eval(&[70.0, 71.0, 69.0, 70.0]).status, Status::Stable);
    let up = eval(&[70.0, 70.0, 70.0, 85.0]);
    assert_eq!(
        (up.status, up.direction),
        (Status::Change, Some(Direction::Up))
    );
    let down = eval(&[80.0, 80.0, 80.0, 65.0]);
    assert_eq!(down.direction, Some(Direction::Down));
    let persistent = eval(&[70.0, 70.0, 70.0, 86.0, 86.0, 86.0]);
    assert!(persistent.persistent);
    assert_eq!(persistent.escalation, Escalation::FlagForGround);
    assert_eq!(eval(&[150.0]).status, Status::Alarm);
    let r = record("old", 70.0, 0, None);
    let e = evaluate::evaluate_series(
        SeriesKey {
            indicator: Indicator::HeartRate,
            body_zone: None,
        },
        &[&r],
        t(49 * crate::rules::HOUR),
    );
    assert_eq!(e.reason, Some(NotEvaluatedReason::Stale));
    assert_eq!(eval(&[70.0, 70.0, 70.0, 85.0]), up);
}
#[test]
fn events_and_overall() {
    let events = vec![
        EventView {
            event_id: "a".into(),
            kind: EventKind::NeedHelp,
            occurred_at: t(0),
            priority: Priority::Urgent,
        },
        EventView {
            event_id: "b".into(),
            kind: EventKind::FeelingReport,
            occurred_at: t(0),
            priority: Priority::Normal,
        },
    ];
    let active = evaluate::evaluate_events(&events, t(1));
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].status, Status::Alarm);
    // B2: las alarmas por evento no caducan (24 h, 48 h, evento antiguo).
    for later in [
        crate::rules::DAY,
        2 * crate::rules::DAY,
        365 * crate::rules::DAY,
    ] {
        let still = evaluate::evaluate_events(&events, t(later));
        assert_eq!(still.len(), 1);
        assert_eq!(still[0].escalation, Escalation::Urgent);
    }
    assert_eq!(evaluate::overall(&[]).status, Status::NotEvaluated);
    assert_eq!(evaluate::overall(&active).escalation, Escalation::Urgent);
    assert_eq!(evaluate::body_zone_status(&[]).len(), BodyZone::ALL.len());
    assert_eq!(evaluate::evaluate_all(&[], &[], t(0)).len(), 7);
}
fn arbitrary_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|n| json!(n)),
        ".{0,30}".prop_map(Value::String)
    ];
    leaf.prop_recursive(3, 48, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..5).prop_map(Value::Array),
            prop::collection::hash_map("[a-z_]{0,12}", inner, 0..5)
                .prop_map(|m| Value::Object(m.into_iter().collect()))
        ]
    })
}
proptest! {
 #[test] fn arbitrary_value_never_panics(v in arbitrary_json(), index in 0usize..Indicator::ALL.len()) { let _ = Measurement::validate(Indicator::ALL[index],&v,None); }
 #[test] fn positive_values_accepted(x in any::<f64>().prop_filter("positive finite", |x| x.is_finite() && *x > 0.0)) { for (i,key) in [(Indicator::HeartRate,"bpm"),(Indicator::BodyTemperature,"celsius"),(Indicator::Glucose,"mg_dl")] { let mut o=serde_json::Map::new(); o.insert(key.into(),json!(x)); prop_assert!(Measurement::validate(i,&Value::Object(o),None).is_ok()); } let bp=json!({"systolic":x,"diastolic":x}); let ok=Measurement::validate(Indicator::BloodPressure,&bp,None).is_ok(); prop_assert!(ok); }
 #[test] fn round_trip(x in 0.01f64..1000.0, score in 0u8..=10) { for (i,v) in [(Indicator::HeartRate,json!({"bpm":x})),(Indicator::BloodPressure,json!({"systolic":x,"diastolic":x})),(Indicator::BodyTemperature,json!({"celsius":x})),(Indicator::Glucose,json!({"mg_dl":x})),(Indicator::Sleep,json!({"hours":x.min(24.0),"quality":"good","window_start":"2026-11-13T22:00:00.000Z","window_end":"2026-11-14T22:00:00.000Z"})),(Indicator::Mood,json!({"score":score})),(Indicator::OxygenInBlood,json!({"percentage":x.min(100.0)})),(Indicator::SpaceRadiation,json!({"usv_h":x})),(Indicator::DistanceFromEarth,json!({"km":x})),(Indicator::Nutrition,json!({"kcal":x})),(Indicator::Symptom,json!({"body_zone":"head","score":score}))] { let m=Measurement::validate(i,&v,None).unwrap(); prop_assert_eq!(Measurement::validate(i,&m.to_value(),None).unwrap(),m); } }
 #[test] fn scores(n in any::<u8>()) { prop_assert_eq!(Score::try_from(n).is_ok(),n<=10); let mood=json!({"score":n}); let ok=Measurement::validate(Indicator::Mood,&mood,None).is_ok(); prop_assert_eq!(ok,n<=10); }
 #[test] fn stable_requires_history(values in prop::collection::vec(1.0f64..200.0,0..4)) { let result=eval(&values); prop_assert!(result.status != Status::Stable || result.history_count >= 3); }
}

#[test]
fn thresholds_and_floors() {
    for (indicator, value, status, escalation) in [
        (
            Indicator::HeartRate,
            json!({"bpm":39.9}),
            Status::Alarm,
            Escalation::Urgent,
        ),
        (
            Indicator::HeartRate,
            json!({"bpm":100}),
            Status::NotEvaluated,
            Escalation::Continue,
        ),
        (
            Indicator::BloodPressure,
            json!({"systolic":130,"diastolic":121}),
            Status::Alarm,
            Escalation::Urgent,
        ),
        (
            Indicator::BodyTemperature,
            json!({"celsius":38.1}),
            Status::Change,
            Escalation::SelfCare,
        ),
        (
            Indicator::Glucose,
            json!({"mg_dl":54}),
            Status::Change,
            Escalation::SelfCare,
        ),
        (
            Indicator::Sleep,
            json!({"hours":3,"quality":null,"window_start":"2026-01-01T00:00:00Z","window_end":"2026-01-01T08:00:00Z"}),
            Status::Change,
            Escalation::FlagForGround,
        ),
        (
            Indicator::Mood,
            json!({"score":1}),
            Status::Change,
            Escalation::FlagForGround,
        ),
        (
            Indicator::OxygenInBlood,
            json!({"percentage":91}),
            Status::Alarm,
            Escalation::Urgent,
        ),
        (
            Indicator::OxygenInBlood,
            json!({"percentage":93}),
            Status::Change,
            Escalation::SelfCare,
        ),
        (
            Indicator::Symptom,
            json!({"score":7,"body_zone":"head"}),
            Status::Change,
            Escalation::FlagForGround,
        ),
    ] {
        let m = measurement(indicator, value);
        let r = RecordView {
            record_id: "one".into(),
            indicator,
            body_zone: m.body_zone(),
            measurement: m,
            measured_at: t(0),
            logged_at: t(0),
            received_at: None,
            seq: None,
            local_order: 0,
            replaces: None,
        };
        let e = evaluate::evaluate_series(
            SeriesKey {
                indicator,
                body_zone: r.body_zone,
            },
            &[&r],
            t(0),
        );
        assert_eq!((e.status, e.escalation), (status, escalation));
    }
}
#[test]
fn secondary_pressure_history() {
    let mut records = Vec::new();
    for (n, d) in [70, 70, 70, 80].into_iter().enumerate() {
        let mut r = record(&n.to_string(), 110.0, i64::try_from(n).unwrap(), None);
        r.indicator = Indicator::BloodPressure;
        r.measurement = measurement(
            Indicator::BloodPressure,
            json!({"systolic":110,"diastolic":d}),
        );
        records.push(r);
    }
    let refs: Vec<_> = records.iter().collect();
    let result = evaluate::evaluate_series(
        SeriesKey {
            indicator: Indicator::BloodPressure,
            body_zone: None,
        },
        &refs,
        t(4),
    );
    assert_eq!(result.status, Status::Change);
    assert_eq!(result.direction, Some(Direction::Up));
}
#[test]
fn sleep_valid_window_and_future_reference() {
    let value = json!({"hours":1,"quality":"fair","window_start":"2026-01-01T00:00:00Z","window_end":"2026-01-01T01:00:00Z"});
    let m = measurement(Indicator::Sleep, value);
    assert_eq!(
        Measurement::validate(Indicator::Sleep, &m.to_value(), None).unwrap(),
        m
    );
    let mut r = record("r", 70.0, crate::rules::FUTURE + 1, None);
    r.logged_at = t(0);
    r.received_at = Some(t(1));
    let records = vec![r];
    assert!(!EffectiveSet::build(&records).is_future("r"));
}

// ---- Añadidos por Claude en la integración ----

#[test]
fn rfc3339_extremes_never_panic_and_parse_back() {
    for ms in [i64::MIN, i64::MAX, 0, -1, 1_763_154_333_120] {
        let s = Timestamp::from_millis(ms).to_rfc3339();
        assert!(Timestamp::parse_rfc3339(&s).is_ok(), "{s}");
    }
    assert_eq!(
        Timestamp::from_millis(i64::MAX).to_rfc3339(),
        "9999-12-31T23:59:59.999Z"
    );
    assert_eq!(
        Timestamp::from_millis(i64::MIN).to_rfc3339(),
        "0000-01-01T00:00:00.000Z"
    );
}

#[test]
fn need_help_with_clock_ahead_still_alarms() {
    let now = t(10 * 86_400_000);
    let ev = EventView {
        event_id: "e".into(),
        kind: EventKind::NeedHelp,
        occurred_at: now.saturating_add_ms(30 * 60_000),
        priority: Priority::Urgent,
    };
    let out = evaluate::evaluate_events(&[ev], now);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].status, Status::Alarm);
}

#[test]
fn single_reading_inside_range_is_not_stable() {
    let r = eval(&[72.0]);
    assert_eq!(r.status, Status::NotEvaluated);
    assert_eq!(r.reason, Some(NotEvaluatedReason::TooFewData));
}

#[test]
fn single_extreme_reading_alarms_without_history() {
    let r = eval(&[210.0]);
    assert_eq!(r.status, Status::Alarm);
    assert_eq!(r.escalation, Escalation::Urgent);
}

#[test]
fn extreme_real_values_are_valid_but_warned() {
    let now = t(10 * 86_400_000);
    for (i, v) in [
        (Indicator::HeartRate, json!({"bpm": 300})),
        (Indicator::Glucose, json!({"mg_dl": 2375})),
        (Indicator::BodyTemperature, json!({"celsius": 14.0})),
    ] {
        let m = Measurement::validate(i, &v, Some(i.unit())).unwrap();
        assert!(!plausibility::check(&m, now, now).is_empty());
    }
}

#[test]
fn stale_alarm_reading_keeps_alarm() {
    let records = [record("a", 35.0, 0, None)];
    let refs: Vec<_> = records.iter().collect();
    let key = SeriesKey {
        indicator: Indicator::HeartRate,
        body_zone: None,
    };
    let out = evaluate::evaluate_series(key, &refs, t(5 * 86_400_000));
    assert_eq!(out.status, Status::Alarm);
    let normal = [record("b", 70.0, 0, None)];
    let refs: Vec<_> = normal.iter().collect();
    let out = evaluate::evaluate_series(key, &refs, t(5 * 86_400_000));
    assert_eq!(out.reason, Some(NotEvaluatedReason::Stale));
}

#[test]
fn overall_is_never_stable_with_unevaluated_series() {
    let mut stable = eval(&[70.0, 70.0, 70.0, 70.0]);
    assert_eq!(stable.status, Status::Stable);
    let no_data = evaluate::evaluate_series(
        SeriesKey {
            indicator: Indicator::Glucose,
            body_zone: None,
        },
        &[],
        t(0),
    );
    assert_eq!(evaluate::overall(&[stable.clone()]).status, Status::Stable);
    let o = evaluate::overall(&[stable.clone(), no_data.clone()]);
    assert_eq!(o.status, Status::NotEvaluated);
    assert_eq!(o.worst, vec!["glucose".to_owned()]);
    assert_eq!(
        evaluate::overall(&[stable.clone(), eval(&[72.0])]).status,
        Status::NotEvaluated
    );
    let alarm = eval(&[210.0]);
    assert_eq!(evaluate::overall(&[alarm, no_data]).status, Status::Alarm);
    stable.status = Status::Change;
    assert_eq!(evaluate::overall(&[stable]).status, Status::Change);
}

#[test]
fn sleep_requires_window() {
    let no_window = json!({"hours": 7, "quality": "good"});
    let e = Measurement::validate(Indicator::Sleep, &no_window, None).unwrap_err();
    assert_eq!(e.field, "window_start");
    let ok = json!({"hours": 7, "quality": null,
        "window_start": "2026-11-13T22:00:00Z", "window_end": "2026-11-14T06:00:00Z"});
    assert!(Measurement::validate(Indicator::Sleep, &ok, None).is_ok());
    let too_long = json!({"hours": 9, "quality": null,
        "window_start": "2026-11-13T22:00:00Z", "window_end": "2026-11-14T06:00:00Z"});
    assert!(Measurement::validate(Indicator::Sleep, &too_long, None).is_err());
}

#[test]
fn new_indicator_bounds_and_plausibility_edges() {
    use crate::plausibility::check;
    let now = Timestamp::from_millis(1_800_000_000_000);
    for (i, v) in [
        (Indicator::OxygenInBlood, json!({"percentage": 0})),
        (Indicator::OxygenInBlood, json!({"percentage": -1})),
        (Indicator::OxygenInBlood, json!({"percentage": 100.1})),
        (Indicator::SpaceRadiation, json!({"usv_h": -0.1})),
        (Indicator::DistanceFromEarth, json!({"km": -1})),
        (Indicator::Nutrition, json!({"kcal": -5})),
    ] {
        assert!(Measurement::validate(i, &v, None).is_err(), "{i:?} {v}");
    }
    let warned = |i, v: serde_json::Value| {
        let m = Measurement::validate(i, &v, None).unwrap();
        check(&m, now, now)
            .iter()
            .any(|w| w.code == "unusual_value")
    };
    assert!(!warned(
        Indicator::OxygenInBlood,
        json!({"percentage": 100})
    ));
    assert!(!warned(Indicator::OxygenInBlood, json!({"percentage": 80})));
    assert!(warned(
        Indicator::OxygenInBlood,
        json!({"percentage": 79.9})
    ));
    assert!(!warned(Indicator::Nutrition, json!({"kcal": 6000})));
    assert!(warned(Indicator::Nutrition, json!({"kcal": 6000.1})));
    assert!(!warned(Indicator::SpaceRadiation, json!({"usv_h": 10_000})));
    assert!(warned(
        Indicator::SpaceRadiation,
        json!({"usv_h": 10_000.1})
    ));
    assert!(!warned(
        Indicator::DistanceFromEarth,
        json!({"km": 401_000_000})
    ));
}

#[test]
fn overall_with_only_context_is_not_evaluated_without_worst() {
    use crate::evaluate::{evaluate_series, overall};
    let now = Timestamp::from_millis(1_800_000_000_000);
    let ctx = evaluate_series(
        crate::effective::SeriesKey {
            indicator: Indicator::Nutrition,
            body_zone: None,
        },
        &[],
        now,
    );
    assert_eq!(
        ctx.reason,
        Some(crate::evaluate::NotEvaluatedReason::NotApplicable)
    );
    assert_eq!(ctx.history_needed, 0);
    let o = overall(&[ctx]);
    assert_eq!(o.status, Status::NotEvaluated);
    assert!(o.worst.is_empty());
}
