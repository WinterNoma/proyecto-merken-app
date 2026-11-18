//! Evaluación pura de series, eventos y estado general.
use crate::{
    RULES_VERSION, VERSION,
    catalog::{BodyZone, Indicator},
    effective::{EffectiveSet, RecordView, SeriesKey},
    measurement::Measurement,
    record::{EventKind, Priority},
    rules::{self, Rule},
    time::Timestamp,
};
use serde::Serialize;
macro_rules! eval_enum { ($name:ident { $($v:ident),+ $(,)? }) => { #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)] #[serde(rename_all = "snake_case")] pub enum $name { $($v),+ } }; }
eval_enum!(Status {
    NotEvaluated,
    Stable,
    Change,
    Alarm
});
// `NotApplicable`: indicador de contexto (radiación, distancia, nutrición), sin estado.
eval_enum!(NotEvaluatedReason {
    NoData,
    TooFewData,
    Stale,
    NotApplicable
});
eval_enum!(Direction { Up, Down });
eval_enum!(Escalation {
    Continue,
    SelfCare,
    FlagForGround,
    Urgent
});
eval_enum!(SourceKind {
    Published,
    ApprovedProtocol,
    DemoHeuristic
});
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Evaluation {
    pub indicator: String,
    pub body_zone: Option<BodyZone>,
    pub status: Status,
    pub reason: Option<NotEvaluatedReason>,
    pub direction: Option<Direction>,
    pub persistent: bool,
    pub escalation: Escalation,
    pub rule_id: String,
    pub rule_version: String,
    pub source_kind: SourceKind,
    pub source_ref: Option<String>,
    pub input_record_ids: Vec<String>,
    pub input_event_ids: Vec<String>,
    pub core_version: String,
    pub evaluated_at: Timestamp,
    pub latest_value: Option<serde_json::Value>,
    pub latest_measured_at: Option<Timestamp>,
    pub baseline: Option<f64>,
    pub history_count: usize,
    pub history_needed: usize,
    pub summary: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventView {
    pub event_id: String,
    pub kind: EventKind,
    pub occurred_at: Timestamp,
    pub priority: Priority,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Overall {
    pub status: Status,
    pub escalation: Escalation,
    pub worst: Vec<String>,
}
fn median(mut values: Vec<f64>) -> Option<f64> {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    if n == 0 {
        None
    } else if n % 2 == 1 {
        values.get(n / 2).copied()
    } else {
        Some(values.get(n / 2 - 1)? / 2.0 + values.get(n / 2)? / 2.0)
    }
}
fn secondary(m: &Measurement) -> Option<f64> {
    if let Measurement::BloodPressure { diastolic, .. } = m {
        Some(diastolic.value())
    } else {
        None
    }
}
fn matches_rule(rule: &Rule, m: &Measurement) -> bool {
    rule.primary.0.matches(m.primary(), rule.primary.1)
        || rule
            .secondary
            .is_some_and(|(op, n)| secondary(m).is_some_and(|x| op.matches(x, n)))
}
fn deviation(
    value: f64,
    baseline: f64,
    delta: f64,
    allowed: Option<Direction>,
) -> Option<Direction> {
    let dir = if value - baseline >= delta {
        Some(Direction::Up)
    } else if baseline - value >= delta {
        Some(Direction::Down)
    } else {
        None
    };
    dir.filter(|d| allowed.is_none_or(|a| a == *d))
}
fn base_eval(indicator: String, zone: Option<BodyZone>, now: Timestamp) -> Evaluation {
    Evaluation {
        indicator,
        body_zone: zone,
        status: Status::NotEvaluated,
        reason: Some(NotEvaluatedReason::NoData),
        direction: None,
        persistent: false,
        escalation: Escalation::Continue,
        rule_id: String::new(),
        rule_version: RULES_VERSION.into(),
        source_kind: SourceKind::DemoHeuristic,
        source_ref: None,
        input_record_ids: Vec::new(),
        input_event_ids: Vec::new(),
        core_version: VERSION.into(),
        evaluated_at: now,
        latest_value: None,
        latest_measured_at: None,
        baseline: None,
        history_count: 0,
        history_needed: 3,
        summary: "No data available".into(),
    }
}
/// Evalúa una serie ordenada de registros efectivos.
pub fn evaluate_series(key: SeriesKey, series: &[&RecordView], now: Timestamp) -> Evaluation {
    let mut out = base_eval(key.indicator.as_str().into(), key.body_zone, now);
    out.rule_id = format!("{}.personal_change", key.indicator.as_str());
    if !key.indicator.is_evaluated() {
        out.reason = Some(NotEvaluatedReason::NotApplicable);
        out.rule_id = "context.not_evaluated".into();
        out.source_kind = SourceKind::ApprovedProtocol;
        out.source_ref = Some(
            "Pulsys catalog 2026-11-14: context indicator, no health status (RF-12 for radiation)"
                .into(),
        );
        out.history_needed = 0;
        out.summary = "Context only; health evaluation does not apply".into();
        if let Some(latest) = series.last() {
            out.latest_value = Some(latest.measurement.to_value());
            out.latest_measured_at = Some(latest.measured_at);
            out.input_record_ids.push(latest.record_id.clone());
        }
        return out;
    }
    let Some((latest, earlier)) = series.split_last() else {
        return out;
    };
    out.latest_value = Some(latest.measurement.to_value());
    out.latest_measured_at = Some(latest.measured_at);
    out.input_record_ids.push(latest.record_id.clone());
    let (delta, allowed, stale) = rules::history(key.indicator);
    if latest.measured_at < now.saturating_add_ms(-stale) {
        // Un dato antiguo no prueba que la alarma se haya resuelto: si la última
        // lectura estaba en rango de alarma, la alarma sigue hasta que haya otra.
        let stale_alarm = rules::ABSOLUTE
            .iter()
            .filter(|r| {
                r.indicator == key.indicator
                    && r.status == Status::Alarm
                    && matches_rule(r, &latest.measurement)
            })
            .min_by(|a, b| a.id.cmp(b.id));
        if let Some(rule) = stale_alarm {
            out.status = Status::Alarm;
            out.reason = None;
            out.escalation = Escalation::Urgent;
            out.rule_id = rule.id.into();
            out.source_kind = rule.source;
            out.source_ref = rule.source_ref.map(str::to_owned);
            out.summary = "Last reading was in the alarm range. Take a new reading".into();
            return out;
        }
        out.reason = Some(NotEvaluatedReason::Stale);
        out.rule_id = format!("{}.stale", key.indicator.as_str());
        out.summary = "Latest data is too old".into();
        return out;
    }
    let prior: Vec<&RecordView> = earlier
        .iter()
        .rev()
        .copied()
        .filter(|r| {
            r.measured_at >= latest.measured_at.saturating_add_ms(-rules::HISTORY)
                && r.measured_at <= latest.measured_at
        })
        .collect();
    out.history_count = prior.len();
    out.input_record_ids
        .extend(prior.iter().map(|r| r.record_id.clone()));
    let history = if prior.len() >= 5 {
        prior.get(2..).unwrap_or_default()
    } else {
        prior.as_slice()
    };
    if prior.len() >= 3 {
        out.baseline = median(history.iter().map(|r| r.measurement.primary()).collect());
    }
    let bp_baseline = if prior.len() >= 3 {
        median(
            history
                .iter()
                .filter_map(|r| secondary(&r.measurement))
                .collect(),
        )
    } else {
        None
    };
    let personal_dir = out
        .baseline
        .and_then(|b| deviation(latest.measurement.primary(), b, delta, allowed))
        .or_else(|| {
            secondary(&latest.measurement)
                .zip(bp_baseline)
                .and_then(|(v, b)| deviation(v, b, rules::BP_DIASTOLIC_DELTA, allowed))
        });
    let personal_persistent = personal_dir.is_some_and(|direction| {
        prior.iter().take(2).count() == 2
            && prior.iter().take(2).all(|r| {
                let primary = out
                    .baseline
                    .and_then(|b| deviation(r.measurement.primary(), b, delta, allowed));
                let bp = secondary(&r.measurement)
                    .zip(bp_baseline)
                    .and_then(|(v, b)| deviation(v, b, rules::BP_DIASTOLIC_DELTA, allowed));
                primary == Some(direction) || bp == Some(direction)
            })
    }) && prior.len() >= 5;
    let mut selected: Option<&Rule> = None;
    let mut floor = Escalation::Continue;
    for rule in rules::ABSOLUTE
        .iter()
        .filter(|r| r.indicator == key.indicator && matches_rule(r, &latest.measurement))
    {
        floor = floor.max(rule.floor);
        if selected
            .is_none_or(|s| rule.status > s.status || (rule.status == s.status && rule.id < s.id))
        {
            selected = Some(rule);
        }
    }
    let personal_id = format!("{}.personal_change", key.indicator.as_str());
    let choose_personal = personal_dir.is_some()
        && selected.is_none_or(|r| {
            r.status < Status::Change || (r.status == Status::Change && personal_id.as_str() < r.id)
        });
    if choose_personal {
        out.status = Status::Change;
        out.reason = None;
        out.direction = personal_dir;
        out.persistent = personal_persistent;
    } else if let Some(rule) = selected {
        out.status = rule.status;
        out.reason = None;
        out.direction = if rule.status == Status::Change {
            Some(rule.direction)
        } else {
            None
        };
        out.rule_id = rule.id.into();
        out.source_kind = rule.source;
        out.source_ref = rule.source_ref.map(str::to_owned);
        out.persistent = prior.len() >= 2
            && prior
                .iter()
                .take(2)
                .all(|r| matches_rule(rule, &r.measurement));
    } else if prior.len() >= 3 {
        out.status = Status::Stable;
        out.reason = None;
    } else {
        out.reason = Some(NotEvaluatedReason::TooFewData);
        out.summary = "Not enough recent history".into();
        return out;
    }
    out.escalation = match out.status {
        Status::Alarm => Escalation::Urgent,
        Status::Change if out.persistent => Escalation::FlagForGround,
        Status::Change => Escalation::SelfCare,
        _ => Escalation::Continue,
    }
    .max(floor);
    out.summary = match (out.status, out.direction) {
        (Status::Alarm, _) => "Urgent attention recommended",
        (Status::Change, Some(Direction::Up)) => "Above your recent range",
        (Status::Change, Some(Direction::Down)) => "Below your recent range",
        (Status::Stable, _) => "Within your recent range",
        _ => "Not evaluated",
    }
    .into();
    out
}
pub fn evaluate_events(events: &[EventView], now: Timestamp) -> Vec<Evaluation> {
    let mut active: Vec<&EventView> = events
        .iter()
        // Una alarma por evento no caduca con el tiempo (revisión de Sol, B2):
        // sigue activa hasta que el contrato tenga una resolución explícita.
        // Tampoco importa la hora: un reloj desajustado no la silencia.
        .filter(|e| e.kind.is_alarm())
        .collect();
    active.sort_by(|a, b| {
        a.occurred_at
            .cmp(&b.occurred_at)
            .then_with(|| a.event_id.cmp(&b.event_id))
    });
    active
        .into_iter()
        .map(|event| {
            let mut out = base_eval(event.kind.as_str().into(), None, now);
            out.status = Status::Alarm;
            out.reason = None;
            out.escalation = Escalation::Urgent;
            out.rule_id = rules::EVENT_ALARM_ID.into();
            out.source_kind = SourceKind::ApprovedProtocol;
            out.source_ref = Some(rules::EVENT_ALARM_SOURCE.into());
            out.input_event_ids.push(event.event_id.clone());
            out.summary = "Urgent attention recommended".into();
            out
        })
        .collect()
}
/// Estado global (revisión de Sol, B3): `alarm` > `change`; si no hay ninguno
/// de los dos y alguna serie está sin evaluar, `not_evaluated`. Solo es
/// `stable` cuando **todas** las series presentes están evaluadas y estables.
/// `worst` son las series que determinan el resultado.
pub fn overall(evals: &[Evaluation]) -> Overall {
    let evals: Vec<&Evaluation> = evals
        .iter()
        .filter(|e| e.reason != Some(NotEvaluatedReason::NotApplicable))
        .collect();
    let max = evals.iter().map(|e| e.status).max();
    let status = match max {
        Some(s @ (Status::Alarm | Status::Change)) => s,
        Some(_) if evals.iter().any(|e| e.status == Status::NotEvaluated) => Status::NotEvaluated,
        Some(_) => Status::Stable,
        None => Status::NotEvaluated,
    };
    let escalation = evals
        .iter()
        .map(|e| e.escalation)
        .max()
        .unwrap_or(Escalation::Continue);
    let mut worst: Vec<String> = evals
        .iter()
        .filter(|e| e.status == status)
        .map(|e| e.indicator.clone())
        .collect();
    worst.sort();
    worst.dedup();
    Overall {
        status,
        escalation,
        worst,
    }
}
pub fn evaluate_all(
    records: &[RecordView],
    events: &[EventView],
    now: Timestamp,
) -> Vec<Evaluation> {
    let effective = EffectiveSet::build(records);
    let series = effective.series();
    let mut out = Vec::new();
    for indicator in Indicator::ALL {
        if indicator == Indicator::Symptom {
            continue;
        }
        let key = SeriesKey {
            indicator,
            body_zone: None,
        };
        let data = series
            .iter()
            .find(|(k, _)| *k == key)
            .map_or(&[] as &[&RecordView], |(_, v)| v.as_slice());
        // Los indicadores de contexto solo aparecen si hay datos (para mostrar
        // el último valor); nunca cuentan para `overall`.
        if !indicator.is_evaluated() && data.is_empty() {
            continue;
        }
        out.push(evaluate_series(key, data, now));
    }
    for (key, data) in &series {
        if key.indicator == Indicator::Symptom {
            out.push(evaluate_series(*key, data, now));
        }
    }
    out.extend(evaluate_events(events, now));
    out
}
pub fn body_zone_status(evals: &[Evaluation]) -> Vec<(BodyZone, Status)> {
    BodyZone::ALL
        .into_iter()
        .map(|zone| {
            let status = evals
                .iter()
                .filter(|e| e.indicator == Indicator::Symptom.as_str() && e.body_zone == Some(zone))
                .map(|e| e.status)
                .max()
                .unwrap_or(Status::NotEvaluated);
            (zone, status)
        })
        .collect()
}
