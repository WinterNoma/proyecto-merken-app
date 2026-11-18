use pulsys_api::{
    ApiError, App, BodyZone, HistoryQuery, NotEvaluatedReason, PreparedPulse, PulseInput,
    PulseOutboxState, PulseReceipt, RecordSource, SaveOutcome, Status, Timestamp, close_store,
    confirm_pulse, effective_pulse_history, open_store, prepare_pulse, zone_status,
};
use pulsys_core::rules::{DAY, FUTURE, MINUTE, OLD};
use rusqlite::Connection;
use std::{
    error::Error,
    path::Path,
    sync::{
        Arc, Barrier,
        atomic::{AtomicI64, AtomicUsize, Ordering},
    },
};

const NOW_MS: i64 = 1_800_000_000_000;
const APP_VERSION: &str = "0.1.0";
type TestResult = Result<(), Box<dyn Error>>;

fn input(number: u32, bpm: f64) -> PulseInput {
    PulseInput {
        key: format!("00000000-0000-4000-8000-{number:012x}"),
        bpm,
        measured_at_ms: None,
        note: None,
        replaces: None,
    }
}

fn fixed_app(private_dir: &Path, now_ms: i64) -> Result<App, ApiError> {
    open_store(
        private_dir,
        APP_VERSION.into(),
        Box::new(move || Ok(Timestamp::from_millis(now_ms))),
    )
}

fn saved(outcome: SaveOutcome) -> Result<PulseReceipt, Box<dyn Error>> {
    match outcome {
        SaveOutcome::Saved(receipt) => Ok(receipt),
        SaveOutcome::NeedsConfirmation(_) => Err("Expected a saved pulse".into()),
    }
}

fn pending(outcome: SaveOutcome) -> Result<PreparedPulse, Box<dyn Error>> {
    match outcome {
        SaveOutcome::NeedsConfirmation(action) => Ok(action),
        SaveOutcome::Saved(_) => Err("Expected a confirmation prompt".into()),
    }
}

fn database(private_dir: &Path) -> rusqlite::Result<Connection> {
    Connection::open(private_dir.join("pulsys.sqlite3"))
}

fn write_counts(conn: &Connection) -> rusqlite::Result<(i64, i64, i64, i64)> {
    conn.query_row(
        "SELECT (SELECT count(*) FROM records), (SELECT count(*) FROM outbox),
                (SELECT count(*) FROM pulse_operations), (SELECT count(*) FROM evaluations)",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
}

fn record_outbox_count(conn: &Connection, record_id: &str) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT count(*) FROM outbox WHERE entity = 'record' AND entity_id = ?1",
        [record_id],
        |row| row.get(0),
    )
}

#[test]
fn dismissing_a_warning_leaves_no_rows() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut app = fixed_app(dir.path(), NOW_MS)?;
    let conn = database(dir.path())?;
    let action = prepare_pulse(&app, input(1, 201.0))?;
    assert!(action.requires_confirmation());
    assert_eq!(write_counts(&conn)?, (0, 0, 0, 0));

    let warning = pending(confirm_pulse(&mut app, action)?)?;
    assert!(warning.warnings().iter().any(|warning| {
        warning.code == "unusual_value" && warning.rule_id == "plaus.heart_rate"
    }));
    close_store(app)?;
    assert_eq!(write_counts(&conn)?, (0, 0, 0, 0));
    Ok(())
}

#[test]
fn correction_keeps_the_original_and_moves_the_effective_point() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut app = fixed_app(dir.path(), NOW_MS)?;
    let conn = database(dir.path())?;
    let original_at_ms = NOW_MS - 2 * MINUTE;
    let original = PulseInput {
        measured_at_ms: Some(original_at_ms),
        ..input(1, 72.5)
    };
    let prepared = prepare_pulse(&app, original)?;
    let original = saved(confirm_pulse(&mut app, prepared)?)?;
    let correction = PulseInput {
        replaces: Some(original.record_id.clone()),
        ..input(2, 74.25)
    };
    let prepared = prepare_pulse(&app, correction)?;
    let correction = saved(confirm_pulse(&mut app, prepared)?)?;

    assert_eq!(correction.root_id, original.record_id);
    assert_eq!(
        correction.replaces.as_deref(),
        Some(original.record_id.as_str())
    );
    let original_value: String = conn.query_row(
        "SELECT value_json FROM records WHERE record_id = ?1",
        [&original.record_id],
        |row| row.get(0),
    )?;
    assert_eq!(original_value, r#"{"bpm":72.5}"#);
    assert_eq!(write_counts(&conn)?, (2, 2, 2, 0));

    let points = effective_pulse_history(&app, HistoryQuery::default())?;
    assert_eq!(points.len(), 1);
    assert_eq!(
        points.first().map(|point| &point.record_id),
        Some(&correction.record_id)
    );
    let old_interval = HistoryQuery {
        from_ms: Some(original_at_ms),
        to_ms: Some(original_at_ms),
        limit: None,
    };
    assert!(effective_pulse_history(&app, old_interval)?.is_empty());
    Ok(())
}

#[test]
fn committed_request_survives_lost_response_and_reopen() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut app = fixed_app(dir.path(), NOW_MS)?;
    let bpm = f64::from_bits(0x4069_e3f3_5ba6_e8e3);
    let action = prepare_pulse(&app, input(1, bpm))?.approve();
    let pending_request = serde_json::to_string(&action)?;
    let stable_receipt = saved(confirm_pulse(
        &mut app,
        serde_json::from_str(&pending_request)?,
    )?)?;
    let conn = database(dir.path())?;
    assert_eq!(stable_receipt.outbox_state, PulseOutboxState::Pending);
    assert_eq!(stable_receipt.source, RecordSource::Form);
    assert_eq!(stable_receipt.unit, "bpm");
    assert_eq!(stable_receipt.bpm.to_bits(), bpm.to_bits());
    assert_eq!(stable_receipt.measured_at_ms, NOW_MS);
    assert_eq!(stable_receipt.logged_at_ms, NOW_MS);
    assert_eq!(saved(confirm_pulse(&mut app, action)?)?, stable_receipt);
    close_store(app)?;

    conn.execute(
        "UPDATE outbox SET state = 'synced' WHERE entity = 'record' AND entity_id = ?1",
        [&stable_receipt.record_id],
    )?;
    let mut reopened = fixed_app(dir.path(), NOW_MS + OLD + DAY)?;
    let recovered = serde_json::from_str(&pending_request)?;
    assert_eq!(
        saved(confirm_pulse(&mut reopened, recovered)?)?,
        stable_receipt
    );
    assert_eq!(record_outbox_count(&conn, &stable_receipt.record_id)?, 1);
    assert_eq!(write_counts(&conn)?, (1, 1, 1, 0));
    Ok(())
}

#[test]
fn saved_receipt_and_key_conflict_survive_a_failing_clock() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut app = fixed_app(dir.path(), NOW_MS)?;
    let action = prepare_pulse(&app, input(1, 74.0))?;
    let conflict = prepare_pulse(&app, input(1, 75.0))?;
    let fresh = prepare_pulse(&app, input(2, 74.0))?;
    let persisted = serde_json::to_string(&action)?;
    let receipt = saved(confirm_pulse(&mut app, action)?)?;
    close_store(app)?;

    let clock_error = ApiError {
        code: "clock_unavailable".into(),
        field: Some("now".into()),
        message: "Fixture clock is unavailable".into(),
    };
    let reads = Arc::new(AtomicUsize::new(0));
    let failure = clock_error.clone();
    let clock_reads = Arc::clone(&reads);
    let mut reopened = open_store(
        dir.path(),
        APP_VERSION.into(),
        Box::new(move || {
            clock_reads.fetch_add(1, Ordering::Relaxed);
            Err(failure.clone())
        }),
    )?;

    let replay = confirm_pulse(&mut reopened, serde_json::from_str(&persisted)?)?;
    assert_eq!(saved(replay)?, receipt);
    let error = confirm_pulse(&mut reopened, conflict)
        .err()
        .ok_or("Expected a key conflict")?;
    assert_eq!(error.code, "idempotency_conflict");
    assert_eq!(error.field.as_deref(), Some("key"));
    assert_eq!(reads.load(Ordering::Relaxed), 0);
    assert_eq!(confirm_pulse(&mut reopened, fresh), Err(clock_error));
    assert_eq!(reads.load(Ordering::Relaxed), 1);
    assert_eq!(write_counts(&database(dir.path())?)?, (1, 1, 1, 0));
    Ok(())
}

#[test]
fn two_writers_get_one_receipt() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut first = fixed_app(dir.path(), NOW_MS)?;
    let mut second = fixed_app(dir.path(), NOW_MS + MINUTE)?;
    let action = prepare_pulse(&first, input(1, 74.0))?;
    let both_ready = Barrier::new(2);
    let (left, right) = std::thread::scope(|scope| {
        let left = scope.spawn(|| {
            both_ready.wait();
            confirm_pulse(&mut first, action.clone())
        });
        let right = scope.spawn(|| {
            both_ready.wait();
            confirm_pulse(&mut second, action.clone())
        });
        (left.join(), right.join())
    });
    let first_receipt = saved(left.map_err(|_| "First writer failed")??)?;
    let second_receipt = saved(right.map_err(|_| "Second writer failed")??)?;
    assert_eq!(first_receipt, second_receipt);
    let conn = database(dir.path())?;
    assert_eq!(write_counts(&conn)?, (1, 1, 1, 0));
    assert_eq!(record_outbox_count(&conn, &first_receipt.record_id)?, 1);
    Ok(())
}

#[test]
fn absent_invalid_and_old_have_distinct_results() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut app = fixed_app(dir.path(), NOW_MS)?;
    let empty = zone_status(&mut app)?;
    assert_eq!(empty.pulse.status, Status::NotEvaluated);
    assert_eq!(empty.pulse.reason, Some(NotEvaluatedReason::NoData));
    assert_eq!(empty.zones.len(), BodyZone::ALL.len());
    assert!(effective_pulse_history(&app, HistoryQuery::default())?.is_empty());

    for invalid_bpm in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
        let error = prepare_pulse(&app, input(1, invalid_bpm))
            .err()
            .ok_or("Expected an invalid-value error")?;
        assert_eq!(error.code, "invalid_value");
        assert_eq!(error.field.as_deref(), Some("value.bpm"));
    }

    let old_ms = NOW_MS - OLD - DAY;
    let old = PulseInput {
        measured_at_ms: Some(old_ms),
        ..input(2, 74.0)
    };
    let action = prepare_pulse(&app, old)?;
    assert!(
        action
            .warnings()
            .iter()
            .any(|w| w.code == "old_measurement")
    );
    assert_eq!(
        saved(confirm_pulse(&mut app, action.approve())?)?.measured_at_ms,
        old_ms
    );
    let historic = zone_status(&mut app)?;
    assert_eq!(historic.pulse.reason, Some(NotEvaluatedReason::Stale));
    assert_eq!(historic.pulse.latest_measured_at_ms, Some(old_ms));
    assert!(
        historic
            .zones
            .iter()
            .all(|zone| zone.status == Status::NotEvaluated)
    );
    Ok(())
}

#[test]
fn a_changed_warning_requires_fresh_confirmation() -> TestResult {
    let dir = tempfile::tempdir()?;
    let clock_ms = Arc::new(AtomicI64::new(NOW_MS));
    let now_for_clock = Arc::clone(&clock_ms);
    let mut app = open_store(
        dir.path(),
        APP_VERSION.into(),
        Box::new(move || {
            Ok(Timestamp::from_millis(
                now_for_clock.load(Ordering::Relaxed),
            ))
        }),
    )?;
    let future_ms = NOW_MS + FUTURE + MINUTE;
    let action = prepare_pulse(
        &app,
        PulseInput {
            measured_at_ms: Some(future_ms),
            ..input(1, 74.0)
        },
    )?;
    assert!(
        action
            .warnings()
            .iter()
            .any(|w| w.code == "future_timestamp")
    );
    clock_ms.store(future_ms, Ordering::Relaxed);
    let changed = pending(confirm_pulse(&mut app, action.approve())?)?;
    assert!(changed.requires_confirmation());
    assert!(changed.warnings().is_empty());
    assert_eq!(write_counts(&database(dir.path())?)?, (0, 0, 0, 0));
    assert_eq!(
        saved(confirm_pulse(&mut app, changed.approve())?)?.logged_at_ms,
        future_ms
    );
    Ok(())
}

#[test]
fn a_zone_refresh_error_keeps_a_saved_receipt() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut app = fixed_app(dir.path(), NOW_MS)?;
    let conn = database(dir.path())?;
    conn.execute_batch(
        "CREATE TRIGGER reject_evaluation BEFORE INSERT ON evaluations
         BEGIN SELECT RAISE(ABORT, 'fixture'); END;",
    )?;
    let action = prepare_pulse(&app, input(1, 73.0))?;
    let receipt = saved(confirm_pulse(&mut app, action.clone())?)?;
    let error = zone_status(&mut app)
        .err()
        .ok_or("Expected refresh to fail")?;
    assert_eq!(error.code, "storage_error");
    assert_eq!(saved(confirm_pulse(&mut app, action)?)?, receipt);
    assert_eq!(write_counts(&conn)?, (1, 1, 1, 0));
    Ok(())
}

#[test]
fn clock_is_read_once_per_timed_operation_and_accepts_signed_milliseconds() -> TestResult {
    let dir = tempfile::tempdir()?;
    let clock_reads = Arc::new(AtomicUsize::new(0));
    let reads = Arc::clone(&clock_reads);
    let mut app = open_store(
        dir.path(),
        APP_VERSION.into(),
        Box::new(move || {
            reads.fetch_add(1, Ordering::Relaxed);
            Ok(Timestamp::from_millis(-1))
        }),
    )?;
    assert_eq!(clock_reads.load(Ordering::Relaxed), 0);
    let action = prepare_pulse(&app, input(1, 74.0))?;
    assert_eq!(clock_reads.load(Ordering::Relaxed), 1);
    let receipt = saved(confirm_pulse(&mut app, action)?)?;
    assert_eq!(clock_reads.load(Ordering::Relaxed), 2);
    assert_eq!(receipt.measured_at_ms, -1);
    assert_eq!(receipt.logged_at_ms, -1);
    assert_eq!(zone_status(&mut app)?.evaluated_at_ms, -1);
    assert_eq!(clock_reads.load(Ordering::Relaxed), 3);
    assert_eq!(
        effective_pulse_history(&app, HistoryQuery::default())?.len(),
        1
    );
    close_store(app)?;
    assert_eq!(clock_reads.load(Ordering::Relaxed), 3);
    Ok(())
}
