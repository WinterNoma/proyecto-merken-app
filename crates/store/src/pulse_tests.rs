use crate::{
    SCHEMA_VERSION, Scenario, Store, StoreError,
    api::ListRecordsInput,
    pulse::{PulseDraft, PulseOutboxState, PulseReceipt, SaveOutcome},
};
use pulsys_core::{catalog::Indicator, rules::DAY, time::Timestamp};
use rusqlite::Connection;
use std::error::Error;

mod confirmation;
mod history;

const NOW_MS: i64 = 1_800_000_000_000;
const VERSION: &str = "0.1.0";
type TestResult = Result<(), Box<dyn Error>>;

fn now() -> Timestamp {
    Timestamp::from_millis(NOW_MS)
}

fn draft(number: u32) -> PulseDraft {
    PulseDraft {
        key: format!("00000000-0000-4000-8000-{number:012x}"),
        bpm: 73.5,
        measured_at_ms: NOW_MS,
        note: None,
        replaces: None,
    }
}

fn saved(outcome: SaveOutcome) -> Result<PulseReceipt, Box<dyn Error>> {
    match outcome {
        SaveOutcome::Saved(receipt) => Ok(receipt),
        SaveOutcome::NeedsConfirmation(_) => Err("Expected a saved pulse".into()),
    }
}

fn failure(result: Result<SaveOutcome, StoreError>) -> Result<StoreError, Box<dyn Error>> {
    result.err().ok_or_else(|| "Expected a store error".into())
}

fn counts(conn: &Connection) -> rusqlite::Result<(i64, i64, i64)> {
    conn.query_row(
        "SELECT (SELECT count(*) FROM records), (SELECT count(*) FROM outbox),
                (SELECT count(*) FROM pulse_operations)",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )
}

#[test]
fn receipt_failure_rolls_back_record_outbox_and_demo_cleanup() -> TestResult {
    let mut store = Store::open_in_memory()?;
    store.load_demo(Scenario::Routine, now(), VERSION)?;
    let before = counts(&store.conn)?;
    let action = store.prepare_pulse(draft(1), now())?.approve();
    store.conn.execute_batch(
        "CREATE TRIGGER fail_receipt BEFORE INSERT ON pulse_operations
         BEGIN SELECT RAISE(ABORT, 'fixture'); END;",
    )?;

    let error = failure(store.confirm_pulse(action.clone(), || Ok(now()), VERSION))?;
    assert_eq!(error.code, "storage_error");
    assert_eq!(counts(&store.conn)?, before);
    assert!(store.get_profile()?.is_demo);

    store.conn.execute_batch("DROP TRIGGER fail_receipt")?;
    let receipt = saved(store.confirm_pulse(action, || Ok(now()), VERSION)?)?;
    assert!(!receipt.is_demo);
    assert!(!store.get_profile()?.is_demo);
    assert_eq!(counts(&store.conn)?, (1, 1, 1));
    Ok(())
}

#[test]
fn replay_conflicts_on_normalized_payload_and_survives_a_later_correction() -> TestResult {
    let mut store = Store::open_in_memory()?;
    let original_action = store.prepare_pulse(draft(1), now())?;
    let original = saved(store.confirm_pulse(original_action.clone(), || Ok(now()), VERSION)?)?;

    let changed = PulseDraft {
        note: Some("Synthetic note".into()),
        ..draft(1)
    };
    let changed = store.prepare_pulse(changed, now())?;
    let conflict = failure(store.confirm_pulse(changed, || Ok(now()), VERSION))?;
    assert_eq!(conflict.code, "idempotency_conflict");
    assert_eq!(conflict.field.as_deref(), Some("key"));
    assert_eq!(counts(&store.conn)?, (1, 1, 1));

    let correction = PulseDraft {
        replaces: Some(original.record_id.clone()),
        ..draft(2)
    };
    let correction = store.prepare_pulse(correction, now())?;
    let new = saved(store.confirm_pulse(correction, || Ok(now()), VERSION)?)?;
    assert_eq!(original.root_id, new.root_id);
    let later = now().saturating_add_ms(10 * DAY);
    assert_eq!(
        saved(store.confirm_pulse(original_action, || Ok(later), VERSION)?)?,
        original
    );
    assert_eq!(counts(&store.conn)?, (2, 2, 2));
    Ok(())
}

#[test]
fn changing_demo_generation_expires_old_corrections_and_renews_confirmation() -> TestResult {
    let mut store = Store::open_in_memory()?;
    store.load_demo(Scenario::Routine, now(), VERSION)?;
    let old_point = store
        .effective_pulse_history(None, None, Some(1))?
        .into_iter()
        .next()
        .ok_or("Fixture must contain a pulse")?;
    let correction = |number| PulseDraft {
        replaces: Some(old_point.record_id.clone()),
        ..draft(number)
    };
    let pending_demo = store.prepare_pulse(correction(1), now())?;
    let saved_demo = store.prepare_pulse(correction(2), now())?;
    let demo = saved(store.confirm_pulse(saved_demo.clone(), || Ok(now()), VERSION)?)?;
    assert!(demo.is_demo);
    assert_eq!(demo.outbox_state, PulseOutboxState::LocalOnly);
    assert_eq!(counts(&store.conn)?.1, 0);
    let real = store.prepare_pulse(draft(3), now())?.approve();
    let old_target = real.demo_to_replace().cloned().ok_or("Expected DEMO")?;

    store.load_demo(Scenario::Routine, now(), VERSION)?;
    for old in [pending_demo, saved_demo] {
        assert_eq!(
            failure(store.confirm_pulse(old, || Ok(now()), VERSION))?.code,
            "action_expired"
        );
    }
    let (receipt_json, fingerprint): (Option<String>, Option<Vec<u8>>) = store.conn.query_row(
        "SELECT receipt_json, fingerprint FROM pulse_operations WHERE key = ?1",
        [&demo.key],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!((receipt_json, fingerprint), (None, None));

    let SaveOutcome::NeedsConfirmation(updated) =
        store.confirm_pulse(real, || Ok(now()), VERSION)?
    else {
        return Err("The new DEMO must be confirmed".into());
    };
    let new_target = updated.demo_to_replace().ok_or("Expected DEMO")?;
    assert_eq!(old_target.astronaut_id, new_target.astronaut_id);
    assert_ne!(old_target.generation, new_target.generation);
    assert!(updated.requires_confirmation());
    assert_eq!(counts(&store.conn)?.1, 0);
    assert!(!saved(store.confirm_pulse(updated.approve(), || Ok(now()), VERSION)?)?.is_demo);
    assert_eq!(counts(&store.conn)?, (1, 1, 2));
    Ok(())
}

#[test]
fn prepared_action_cannot_move_to_another_local_profile() -> TestResult {
    let mut store = Store::open_in_memory()?;
    let action = store.prepare_pulse(draft(1), now())?;
    store.upsert_profile(&pulsys_seed::demo_profile(now()), false, now())?;
    let error = failure(store.confirm_pulse(action, || Ok(now()), VERSION))?;
    assert_eq!(error.code, "action_expired");
    assert_eq!(counts(&store.conn)?, (0, 0, 0));
    Ok(())
}

#[test]
fn correction_must_reference_an_existing_pulse() -> TestResult {
    let mut store = Store::open_in_memory()?;
    store.load_demo(Scenario::Routine, now(), VERSION)?;
    let existing = store
        .list_records(ListRecordsInput {
            indicator: Some(Indicator::Sleep),
            ..ListRecordsInput::default()
        })?
        .into_iter()
        .next()
        .ok_or("Fixture must contain sleep data")?;
    for (id, expected) in [
        ("missing".to_owned(), "not_found"),
        (existing.record_id, "invalid_value"),
    ] {
        let input = PulseDraft {
            replaces: Some(id),
            ..draft(1)
        };
        let error = store
            .prepare_pulse(input, now())
            .err()
            .ok_or("Expected an invalid correction")?;
        assert_eq!(error.code, expected);
        assert_eq!(error.field.as_deref(), Some("replaces"));
    }
    Ok(())
}

fn legacy_contents(conn: &Connection) -> rusqlite::Result<String> {
    conn.query_row(
        "SELECT json_array(
            (SELECT astronaut_json FROM profile WHERE id = 1),
            (SELECT json_group_array(json_array(record_id, replaces, value_json, root_id))
             FROM records ORDER BY local_order),
            (SELECT json_group_array(json_array(entity_id, state, payload_json)) FROM outbox),
            (SELECT value FROM sync_state WHERE key = 'pull_cursor'),
            (SELECT row_hash FROM telemetry_readings WHERE reading_id = 1),
            (SELECT last_hash FROM telemetry_heads WHERE astronaut_key = 'local')
        )",
        [],
        |row| row.get(0),
    )
}

#[test]
fn schema_v5_upgrades_without_rewriting_legacy_rows() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("legacy.sqlite3");
    let mut conn = Connection::open(&path)?;
    let tx = conn.transaction()?;
    Store::migrate_to_v5(&tx, 0)?;
    tx.execute_batch(
        r#"INSERT INTO profile VALUES (1,'{"astronaut_id":"local"}','{}','[]',1,0,0);
        INSERT INTO records(record_id,indicator,value_json,unit,measured_at,logged_at,source,
                            replaces,root_id,is_demo)
            VALUES ('original','heart_rate','{"bpm":71}','bpm',100,100,'form',NULL,'original',0),
                   ('corrected','heart_rate','{"bpm":73}','bpm',101,101,'form','original','original',0);
        INSERT INTO outbox(entity,entity_id,payload_json,state,created_at)
            VALUES ('record','original','{"legacy":true}','synced',100),
                   ('record','corrected','{"legacy":true}','pending',101);
        INSERT INTO sync_state VALUES ('pull_cursor','legacy-cursor');
        INSERT INTO telemetry_readings(astronaut_key,astronaut_name,indicator,value_json,unit,
            observed_at,batch_id,source,app_version,prev_hash,row_hash)
            VALUES ('local','Synthetic','heart_rate','{"bpm":71}','bpm',100,'batch','garmin','0.1.0','','hash');
        INSERT INTO telemetry_heads VALUES ('local','heart_rate',1,'hash',1);"#,
    )?;
    tx.commit()?;
    assert_eq!(
        conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?,
        5
    );
    let expected = legacy_contents(&conn)?;
    conn.close().map_err(|(_, error)| error)?;

    let mut store = Store::open(&path)?;
    let version: i64 = store
        .conn
        .pragma_query_value(None, "user_version", |row| row.get(0))?;
    assert_eq!(version, SCHEMA_VERSION);
    assert_eq!(legacy_contents(&store.conn)?, expected);
    assert_eq!(counts(&store.conn)?, (2, 2, 0));
    let action = store.prepare_pulse(draft(1), now())?;
    let receipt = saved(store.confirm_pulse(action.clone(), || Ok(now()), VERSION)?)?;
    store.close()?;

    let mut reopened = Store::open(&path)?;
    let actual = saved(reopened.confirm_pulse(action, || Ok(now()), VERSION)?)?;
    assert_eq!(actual, receipt);
    assert_eq!(counts(&reopened.conn)?, (3, 3, 1));
    Ok(())
}

#[test]
fn future_schema_is_rejected_before_changing_journal_mode() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("future.sqlite3");
    let conn = Connection::open(&path)?;
    conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)?;
    conn.close().map_err(|(_, error)| error)?;

    let error = Store::open(&path)
        .err()
        .ok_or("Expected an unsupported schema")?;
    assert_eq!(error.code, "invalid_schema");
    let conn = Connection::open(&path)?;
    let mode: String = conn.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
    assert_eq!(mode, "delete");
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    assert_eq!(version, SCHEMA_VERSION + 1);
    Ok(())
}

#[test]
fn indexed_history_matches_effective_set_for_edges_of_correction_graph() -> TestResult {
    let store = Store::open_in_memory()?;
    store.conn.execute_batch(
        r#"INSERT INTO records(record_id,indicator,value_json,unit,measured_at,logged_at,
                               source,replaces,root_id,seq,received_at,is_demo)
        VALUES
          ('high_root','heart_rate','{"bpm":70}','bpm',100,1000,'form',NULL,'high_root',1,NULL,0),
          ('in_branch','heart_rate','{"bpm":71}','bpm',300,1000,'form','high_root','high_root',99,NULL,0),
          ('above','heart_rate','{"bpm":72}','bpm',700,1000,'form','high_root','high_root',NULL,NULL,0),
          ('future','heart_rate','{"bpm":73}','bpm',999999,1000,'form','above','high_root',NULL,NULL,0),
          ('low_root','heart_rate','{"bpm":74}','bpm',200,1000,'form',NULL,'low_root',2,NULL,0),
          ('below','heart_rate','{"bpm":75}','bpm',99,1000,'form','low_root','low_root',2,NULL,0),
          ('tie_older','heart_rate','{"bpm":76}','bpm',310,1000,'form',NULL,'tie_older',NULL,NULL,0),
          ('tie_newer','heart_rate','{"bpm":77}','bpm',310,1000,'form',NULL,'tie_newer',NULL,NULL,0),
          ('orphan','heart_rate','{"bpm":78}','bpm',320,1000,'form','missing','missing',NULL,NULL,0),
          ('orphan_child','heart_rate','{"bpm":79}','bpm',330,1000,'form','orphan','missing',NULL,NULL,0),
          ('cycle_a','heart_rate','{"bpm":80}','bpm',340,1000,'form','cycle_b','cycle_b',NULL,NULL,0),
          ('cycle_b','heart_rate','{"bpm":81}','bpm',350,1000,'form','cycle_a','cycle_b',NULL,NULL,0),
          ('valid','heart_rate','{"bpm":82}','bpm',360,1000,'form',NULL,'valid',NULL,NULL,0),
          ('invalid','heart_rate','{"bpm":0}','bpm',370,1000,'form','valid','valid',NULL,NULL,0),
          ('cross','heart_rate','{"bpm":83}','bpm',380,1000,'form',NULL,'cross',NULL,NULL,0),
          ('other','mood','{"score":5}','score_0_10',390,1000,'form','cross','cross',NULL,NULL,0),
          ('received','heart_rate','{"bpm":84}','bpm',600001,0,'earth',NULL,'received',3,1000,0);"#,
    )?;
    let intervals = [
        (None, None, None),
        (Some(100), Some(380), None),
        (Some(310), Some(310), Some(1)),
        (None, Some(350), Some(2)),
    ];
    for (from, to, limit) in intervals {
        let (from, to) = (
            from.map(Timestamp::from_millis),
            to.map(Timestamp::from_millis),
        );
        let actual = store.effective_pulse_history(from, to, limit)?;
        let expected = store.list_records(ListRecordsInput {
            indicator: Some(Indicator::HeartRate),
            from,
            to,
            limit,
            ..ListRecordsInput::default()
        })?;
        assert_eq!(
            actual
                .iter()
                .map(|point| &point.record_id)
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|point| &point.record_id)
                .collect::<Vec<_>>(),
        );
    }
    let recent_tie = store.effective_pulse_history(
        Some(Timestamp::from_millis(310)),
        Some(Timestamp::from_millis(310)),
        Some(1),
    )?;
    assert_eq!(
        recent_tie.first().map(|point| point.record_id.as_str()),
        Some("tie_newer")
    );
    Ok(())
}
