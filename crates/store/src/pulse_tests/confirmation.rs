use super::{TestResult, VERSION, counts, draft, failure, now, saved};
use crate::{
    Scenario, Store,
    profile::ASTRONAUT_ID_FIELD,
    pulse::{PreparedPulse, PulseTarget, SaveOutcome},
};
use rusqlite::Connection;
use serde_json::{Value, json};

fn profile(astronaut: Value) -> Value {
    json!({
        "astronaut": astronaut,
        "medical_profile": {"profile_version": 1},
        "medical_conditions": [],
    })
}

#[test]
fn changing_person_expires_a_pending_pulse() -> TestResult {
    let mut store = Store::open_in_memory()?;
    let first = profile(json!({"astronaut_id": "crew-a"}));
    let second = profile(json!({"astronaut_id": "crew-b"}));
    store.upsert_profile(&first, false, now())?;
    let action = store.prepare_pulse(draft(1), now())?;
    assert!(matches!(
        action.target(),
        PulseTarget::Local { astronaut_id: Some(id), .. } if id == "crew-a"
    ));

    store.upsert_profile(&second, false, now())?;
    let error = failure(store.confirm_pulse(action, || Ok(now()), VERSION))?;
    assert_eq!(error.code, "action_expired");
    assert_eq!(error.field.as_deref(), Some("key"));
    assert_eq!(counts(&store.conn)?, (0, 0, 0));
    assert_eq!(store.get_profile()?.profile, Some(second));

    let same_person = store.prepare_pulse(draft(2), now())?;
    let updated = profile(json!({"astronaut_id": "crew-b", "display_name": "Synthetic"}));
    store.upsert_profile(&updated, false, now())?;
    saved(store.confirm_pulse(same_person, || Ok(now()), VERSION)?)?;
    assert_eq!(counts(&store.conn)?, (1, 1, 1));
    Ok(())
}

#[test]
fn a_v5_profile_without_id_needs_explicit_repair() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("legacy-profile.sqlite3");
    let mut conn = Connection::open(&path)?;
    let tx = conn.transaction()?;
    Store::migrate_to_v5(&tx, 0)?;
    tx.execute_batch(
        r#"INSERT INTO profile VALUES
            (1,'{"display_name":"Synthetic"}','{"profile_version":1}','[]',1,0,0);"#,
    )?;
    tx.commit()?;
    drop(conn);

    let mut store = Store::open(&path)?;
    let legacy = profile(json!({"display_name": "Synthetic"}));
    assert_eq!(store.get_profile()?.profile.as_ref(), Some(&legacy));
    let error = store
        .prepare_pulse(draft(1), now())
        .err()
        .ok_or("Expected a profile identity error")?;
    assert_eq!(error.code, "invalid_schema");
    assert_eq!(error.field.as_deref(), Some(ASTRONAUT_ID_FIELD));
    for invalid in [json!(null), json!("  "), json!(73)] {
        let invalid = profile(json!({"astronaut_id": invalid}));
        let error = store
            .upsert_profile(&invalid, false, now())
            .err()
            .ok_or("Expected a profile identity error")?;
        assert_eq!(error.code, "invalid_schema");
        assert_eq!(error.field.as_deref(), Some(ASTRONAUT_ID_FIELD));
    }
    assert_eq!(store.get_profile()?.profile, Some(legacy));
    assert_eq!(counts(&store.conn)?, (0, 0, 0));

    let repaired = profile(json!({"astronaut_id": "crew-a", "display_name": "Synthetic"}));
    store.upsert_profile(&repaired, false, now())?;
    let action = store.prepare_pulse(draft(1), now())?;
    assert!(matches!(
        action.target(),
        PulseTarget::Local { astronaut_id: Some(id), .. } if id == "crew-a"
    ));
    saved(store.confirm_pulse(action, || Ok(now()), VERSION)?)?;
    Ok(())
}

#[test]
fn incoherent_restored_action_cannot_replace_demo() -> TestResult {
    let mut store = Store::open_in_memory()?;
    store.load_demo(Scenario::Routine, now(), VERSION)?;
    let before = counts(&store.conn)?;
    let profile_before = store.get_profile()?.profile;

    let action = store.prepare_pulse(draft(1), now())?;
    assert!(action.warnings().is_empty());
    assert!(action.demo_to_replace().is_some());
    let mut serialized = serde_json::to_value(&action)?;
    *serialized
        .get_mut("confirmation")
        .ok_or("Expected confirmation field")? = json!("NotRequired");
    let restored: PreparedPulse = serde_json::from_value(serialized)?;

    let SaveOutcome::NeedsConfirmation(pending) =
        store.confirm_pulse(restored, || Ok(now()), VERSION)?
    else {
        return Err("Expected explicit confirmation before replacing DEMO".into());
    };
    assert!(pending.requires_confirmation());
    assert_eq!(pending.demo_to_replace(), action.demo_to_replace());
    assert_eq!(counts(&store.conn)?, before);
    assert_eq!(store.get_profile()?.profile, profile_before);
    assert!(store.get_profile()?.is_demo);
    Ok(())
}
