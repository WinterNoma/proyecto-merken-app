use super::{TestResult, VERSION, counts, draft, failure, now, saved};
use crate::{
    PulledChange, Store,
    pulse::{PreparedPulse, PulseDraft, PulsePoint},
    record_graph::{VALID_RECORD_FUNCTION, valid_record_from_sql},
};
use pulsys_core::time::Timestamp;
use rusqlite::functions::FunctionFlags;
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn pull_pulse(store: &mut Store, id: &str, replaces: Option<&str>, seq: i64) -> TestResult {
    let at = now().saturating_add_ms((seq - 4) * 1_000);
    let change = PulledChange {
        seq,
        entity: "record".into(),
        id: id.into(),
        payload: json!({
            "record_id": id,
            "indicator": "heart_rate",
            "value": {"bpm": 73.0},
            "unit": "bpm",
            "source": "earth",
            "measured_at": at,
            "logged_at": at,
            "received_at": now(),
            "replaces": replaces,
            "schema_version": 1,
            "seq": seq,
        }),
    };
    assert_eq!(
        store.apply_pull_page(&[change], &format!("cursor-{id}"), now())?,
        1
    );
    Ok(())
}

fn winner_ids(points: &[PulsePoint]) -> Vec<(&str, &str)> {
    points
        .iter()
        .map(|point| (point.record_id.as_str(), point.root_id.as_str()))
        .collect()
}

fn correction_of_c(number: u32) -> PulseDraft {
    PulseDraft {
        replaces: Some("C".into()),
        ..draft(number)
    }
}

fn restore_orphan_correction(store: &Store) -> Result<PreparedPulse, Box<dyn std::error::Error>> {
    let action = store.prepare_pulse(draft(1), now())?;
    // La preparación anterior permitía guardar replaces con una raíz pendiente de resolver.
    let mut persisted = serde_json::to_value(action)?;
    *persisted
        .get_mut("replaces")
        .ok_or("Expected a replaces field")? = json!("C");
    Ok(serde_json::from_value(persisted)?)
}

#[test]
fn orphan_correction_is_rejected_before_its_ancestor_arrives() -> TestResult {
    let mut store = Store::open_in_memory()?;
    pull_pulse(&mut store, "C", Some("B"), 3)?;
    let before = store.effective_pulse_history(None, None, None)?;
    assert_eq!(winner_ids(&before), [("C", "C")]);
    let restored = restore_orphan_correction(&store)?;
    let changes_before = store.conn.total_changes();

    let attempt = store.confirm_pulse(restored, || Ok(now()), VERSION);
    assert_eq!(counts(&store.conn)?, (1, 0, 0));
    assert_eq!(store.conn.total_changes(), changes_before);
    let confirm_error = failure(attempt)?;
    let prepare_error = store
        .prepare_pulse(correction_of_c(1), now())
        .err()
        .ok_or("Expected an unresolved correction root")?;
    for error in [confirm_error, prepare_error] {
        assert_eq!(error.code, "unresolved_root");
        assert_eq!(error.field.as_deref(), Some("replaces"));
    }
    assert_eq!(store.effective_pulse_history(None, None, None)?, before);
    Ok(())
}

#[test]
fn out_of_order_corrections_share_the_resolved_root() -> TestResult {
    let mut store = Store::open_in_memory()?;
    pull_pulse(&mut store, "A", None, 1)?;
    pull_pulse(&mut store, "C", Some("B"), 3)?;
    let before = store.effective_pulse_history(None, None, None)?;
    assert_eq!(winner_ids(&before), [("C", "C"), ("A", "A")]);
    let stale = restore_orphan_correction(&store)?;

    pull_pulse(&mut store, "B", Some("A"), 2)?;
    let cached_root: String = store.conn.query_row(
        "SELECT root_id FROM records WHERE record_id = 'C'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(cached_root, "B");
    let linked = store.effective_pulse_history(None, None, None)?;
    assert_eq!(winner_ids(&linked), [("C", "A")]);
    let error = failure(store.confirm_pulse(stale, || Ok(now()), VERSION))?;
    assert_eq!(error.code, "action_expired");
    assert_eq!(counts(&store.conn)?, (3, 0, 0));

    let ready = store.prepare_pulse(correction_of_c(2), now())?;
    let receipt = saved(store.confirm_pulse(ready, || Ok(now()), VERSION)?)?;
    assert_eq!(receipt.replaces.as_deref(), Some("C"));
    assert_eq!(receipt.root_id, "A");
    let effective = store.effective_pulse_history(None, None, None)?;
    assert_eq!(
        winner_ids(&effective),
        [(receipt.record_id.as_str(), receipt.root_id.as_str())],
    );
    let old_interval = store.effective_pulse_history(
        None,
        Some(Timestamp::from_millis(now().millis() - 1)),
        None,
    )?;
    assert!(old_interval.is_empty());
    assert_eq!(counts(&store.conn)?, (4, 1, 1));
    Ok(())
}

#[test]
fn history_limit_stops_candidate_validation() -> TestResult {
    let store = Store::open_in_memory()?;
    store.conn.execute_batch(
        r#"WITH RECURSIVE numbers(n) AS (
            VALUES (1) UNION ALL SELECT n + 1 FROM numbers WHERE n < 2004
        )
        INSERT INTO records(record_id, indicator, value_json, unit, measured_at, logged_at,
                            source, root_id, is_demo)
        SELECT 'pulse-' || n, 'heart_rate', '{"bpm":73}', 'bpm', n, n,
               'form', 'pulse-' || n, 0 FROM numbers;"#,
    )?;
    let validations = Arc::new(AtomicUsize::new(0));
    let calls = Arc::clone(&validations);
    store.conn.create_scalar_function(
        VALID_RECORD_FUNCTION,
        4,
        FunctionFlags::SQLITE_UTF8,
        move |context| {
            calls.fetch_add(1, Ordering::Relaxed);
            valid_record_from_sql(context)
        },
    )?;

    let result = store.effective_pulse_history(None, None, Some(1))?;
    assert_eq!(winner_ids(&result), [("pulse-2004", "pulse-2004")]);
    assert_eq!(validations.load(Ordering::Relaxed), 1);
    Ok(())
}
