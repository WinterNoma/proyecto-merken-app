use pulsys_bridge::api::{
    HistoryQuery, PreparedPulse, PulseConfirmation, PulseInput, PulseReceipt, SaveOutcome, Session,
    close_store, confirm_pulse, decode_prepared_pulse, effective_pulse_history,
    encode_prepared_pulse, open_store, prepare_pulse,
};
use std::{error::Error, fs};

#[test]
fn approved_correction_survives_restart_and_replays_the_same_receipt() -> Result<(), Box<dyn Error>>
{
    let directory = tempfile::tempdir()?;
    let private_dir = directory.path().to_str().ok_or("Expected a UTF-8 path")?;
    let pending_file = directory.path().join("pulse-pending");
    let session = open_store(private_dir.into(), "test".into())?;
    let correction = approved_correction(&session)?;
    fs::write(&pending_file, encode_prepared_pulse(correction.clone())?)?;
    close_store(&session)?;
    drop(session);

    let session = open_store(private_dir.into(), "test".into())?;
    let restored = decode_prepared_pulse(fs::read(&pending_file)?)?;
    assert_eq!(restored, correction);
    let receipt = saved(confirm_pulse(&session, restored)?)?;
    assert_eq!(
        Some(&receipt.root_id),
        correction.context.correction_root_id.as_ref()
    );
    close_store(&session)?;
    drop(session);

    let session = open_store(private_dir.into(), "test".into())?;
    let restored = decode_prepared_pulse(fs::read(&pending_file)?)?;
    assert_eq!(saved(confirm_pulse(&session, restored)?)?, receipt);
    let points = effective_pulse_history(&session, HistoryQuery::default())?;
    assert_eq!(points.len(), 1);
    assert_eq!(
        points.first().map(|point| &point.record_id),
        Some(&receipt.record_id)
    );
    close_store(&session)?;
    Ok(())
}

fn approved_correction(session: &Session) -> Result<PreparedPulse, Box<dyn Error>> {
    let original = prepare_pulse(
        session,
        PulseInput {
            key: "00000000-0000-4000-8000-000000000001".into(),
            bpm: 73.5,
            measured_at_ms: None,
            note: None,
            replaces: None,
        },
    )?;
    assert_eq!(original.confirmation, PulseConfirmation::NotRequired);
    let original = saved(confirm_pulse(session, original)?)?;
    let mut correction = prepare_pulse(
        session,
        PulseInput {
            key: "00000000-0000-4000-8000-000000000002".into(),
            bpm: f64::from_bits(201.0_f64.to_bits() + 1),
            measured_at_ms: Some(original.measured_at_ms),
            note: Some("Synthetic correction".into()),
            replaces: Some(original.record_id),
        },
    )?;
    assert_eq!(correction.confirmation, PulseConfirmation::Required);
    assert_eq!(
        correction.context.correction_root_id,
        Some(original.root_id)
    );
    assert!(matches!(
        confirm_pulse(session, correction.clone())?,
        SaveOutcome::NeedsConfirmation { .. }
    ));

    correction.confirmation = PulseConfirmation::Approved;
    Ok(correction)
}

fn saved(outcome: SaveOutcome) -> Result<PulseReceipt, Box<dyn Error>> {
    match outcome {
        SaveOutcome::Saved { receipt } => Ok(receipt),
        SaveOutcome::NeedsConfirmation { .. } => Err("Expected a saved pulse".into()),
    }
}

#[test]
fn unknown_durable_version_returns_a_typed_action_error() -> Result<(), Box<dyn Error>> {
    let error = decode_prepared_pulse(br#"{"version":"2","action":{}}"#.to_vec())
        .err()
        .ok_or("Expected an unsupported prepared pulse format")?;
    assert_eq!(error.code, "unsupported_action_version");
    assert_eq!(error.field.as_deref(), Some("action.version"));
    Ok(())
}

#[test]
fn renewed_warning_survives_restart_until_explicit_approval() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let pending_file = directory.path().join("pulse-pending");
    let first = fixed_app(directory.path(), 1_800_000_000_000)?;
    let original = pulsys_api::prepare_pulse(
        &first,
        pulsys_api::PulseInput {
            key: "00000000-0000-4000-8000-000000000003".into(),
            bpm: 73.5,
            measured_at_ms: None,
            note: None,
            replaces: None,
        },
    )?
    .approve();
    fs::write(&pending_file, encode_prepared_pulse(original.try_into()?)?)?;
    pulsys_api::close_store(first)?;

    let mut later = fixed_app(directory.path(), 1_800_691_200_000)?;
    let original = decode_prepared_pulse(fs::read(&pending_file)?)?;
    let outcome = pulsys_api::confirm_pulse(&mut later, original.try_into()?)?;
    let pulsys_api::SaveOutcome::NeedsConfirmation(renewed) = outcome else {
        return Err("Expected the changed warning to require approval".into());
    };
    let renewed: PreparedPulse = renewed.try_into()?;
    assert_eq!(renewed.confirmation, PulseConfirmation::Required);
    assert!(
        renewed
            .warnings
            .iter()
            .any(|warning| warning.code == "old_measurement")
    );
    assert!(pulsys_api::effective_pulse_history(&later, Default::default())?.is_empty());
    fs::write(&pending_file, encode_prepared_pulse(renewed.clone())?)?;
    pulsys_api::close_store(later)?;

    let mut reopened = fixed_app(directory.path(), 1_800_691_200_000)?;
    let restored = decode_prepared_pulse(fs::read(&pending_file)?)?;
    assert_eq!(restored, renewed);
    assert!(pulsys_api::effective_pulse_history(&reopened, Default::default())?.is_empty());
    let restored: pulsys_api::PreparedPulse = restored.try_into()?;
    assert!(matches!(
        pulsys_api::confirm_pulse(&mut reopened, restored.approve())?,
        pulsys_api::SaveOutcome::Saved(_)
    ));
    assert_eq!(
        pulsys_api::effective_pulse_history(&reopened, Default::default())?.len(),
        1
    );
    pulsys_api::close_store(reopened)?;
    Ok(())
}

fn fixed_app(
    directory: &std::path::Path,
    now_ms: i64,
) -> Result<pulsys_api::App, pulsys_api::ApiError> {
    pulsys_api::open_store(
        directory,
        "test".into(),
        Box::new(move || Ok(pulsys_api::Timestamp::from_millis(now_ms))),
    )
}

#[test]
fn malformed_supported_version_has_a_distinct_error() -> Result<(), Box<dyn Error>> {
    let error = decode_prepared_pulse(br#"{"version":"1","action":{}}"#.to_vec())
        .err()
        .ok_or("Expected an invalid prepared pulse")?;
    assert_eq!(error.code, "invalid_value");
    assert_eq!(error.field.as_deref(), Some("action"));
    Ok(())
}
