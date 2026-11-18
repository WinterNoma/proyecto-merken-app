use crate::api::{
    ApiError, HistoryQuery, PulseEvaluation, PulseInput, PulsePoint, PulseReceipt, SaveOutcome,
    SymptomEvaluation, ZoneSnapshot, ZoneState,
};
use pulsys_api as facade;

impl From<facade::ApiError> for ApiError {
    fn from(error: facade::ApiError) -> Self {
        Self {
            code: error.code,
            field: error.field,
            message: error.message,
        }
    }
}

impl From<PulseInput> for facade::PulseInput {
    fn from(input: PulseInput) -> Self {
        Self {
            key: input.key,
            bpm: input.bpm,
            measured_at_ms: input.measured_at_ms,
            note: input.note,
            replaces: input.replaces,
        }
    }
}

impl From<HistoryQuery> for facade::HistoryQuery {
    fn from(query: HistoryQuery) -> Self {
        Self {
            from_ms: query.from_ms,
            to_ms: query.to_ms,
            limit: query
                .limit
                .map(|limit| usize::try_from(limit.max(0)).unwrap_or(usize::MAX)),
        }
    }
}

impl TryFrom<facade::SaveOutcome> for SaveOutcome {
    type Error = ApiError;

    fn try_from(outcome: facade::SaveOutcome) -> Result<Self, Self::Error> {
        Ok(match outcome {
            facade::SaveOutcome::NeedsConfirmation(prepared) => Self::NeedsConfirmation {
                prepared: prepared.try_into()?,
            },
            facade::SaveOutcome::Saved(receipt) => Self::Saved {
                receipt: receipt.into(),
            },
        })
    }
}

impl From<facade::PulseReceipt> for PulseReceipt {
    fn from(receipt: facade::PulseReceipt) -> Self {
        Self {
            key: receipt.key,
            record_id: receipt.record_id,
            root_id: receipt.root_id,
            replaces: receipt.replaces,
            bpm: receipt.bpm,
            unit: receipt.unit,
            source: receipt.source.into(),
            measured_at_ms: receipt.measured_at_ms,
            logged_at_ms: receipt.logged_at_ms,
            is_demo: receipt.is_demo,
            outbox_state: receipt.outbox_state.into(),
        }
    }
}

impl From<facade::PulsePoint> for PulsePoint {
    fn from(point: facade::PulsePoint) -> Self {
        Self {
            record_id: point.record_id,
            root_id: point.root_id,
            replaces: point.replaces,
            bpm: point.bpm,
            unit: point.unit,
            source: point.source.into(),
            measured_at_ms: point.measured_at_ms,
            logged_at_ms: point.logged_at_ms,
            is_demo: point.is_demo,
        }
    }
}

impl From<facade::ZoneSnapshot> for ZoneSnapshot {
    fn from(snapshot: facade::ZoneSnapshot) -> Self {
        Self {
            evaluated_at_ms: snapshot.evaluated_at_ms,
            is_demo: snapshot.is_demo,
            zones: snapshot.zones.into_iter().map(ZoneState::from).collect(),
            pulse: snapshot.pulse.into(),
        }
    }
}

impl From<facade::ZoneState> for ZoneState {
    fn from(zone: facade::ZoneState) -> Self {
        Self {
            zone_id: zone.zone_id.into(),
            status: zone.status.into(),
            symptoms: zone
                .symptoms
                .into_iter()
                .map(SymptomEvaluation::from)
                .collect(),
        }
    }
}

impl From<facade::SymptomEvaluation> for SymptomEvaluation {
    fn from(evaluation: facade::SymptomEvaluation) -> Self {
        Self {
            status: evaluation.status.into(),
            reason: evaluation.reason.map(Into::into),
            evaluated_at_ms: evaluation.evaluated_at_ms,
            rule_id: evaluation.rule_id,
            rule_version: evaluation.rule_version,
            source_kind: evaluation.source_kind.into(),
            source_ref: evaluation.source_ref,
            input_record_ids: evaluation.input_record_ids,
            input_event_ids: evaluation.input_event_ids,
        }
    }
}

impl From<facade::PulseEvaluation> for PulseEvaluation {
    fn from(evaluation: facade::PulseEvaluation) -> Self {
        Self {
            status: evaluation.status.into(),
            reason: evaluation.reason.map(Into::into),
            direction: evaluation.direction.map(Into::into),
            persistent: evaluation.persistent,
            escalation: evaluation.escalation.into(),
            evaluated_at_ms: evaluation.evaluated_at_ms,
            latest_measured_at_ms: evaluation.latest_measured_at_ms,
            rule_id: evaluation.rule_id,
            rule_version: evaluation.rule_version,
            source_kind: evaluation.source_kind.into(),
            source_ref: evaluation.source_ref,
            input_record_ids: evaluation.input_record_ids,
            history_count: evaluation.history_count,
            history_needed: evaluation.history_needed,
            baseline_bpm: evaluation.baseline_bpm,
        }
    }
}
