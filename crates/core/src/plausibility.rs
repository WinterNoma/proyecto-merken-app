//! Avisos que solicitan confirmación sin rechazar la medición.
use crate::{measurement::Measurement, rules, time::Timestamp};
use serde::Serialize;
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PlausibilityWarning {
    pub code: String,
    pub rule_id: String,
    pub message: String,
}
fn warning(code: &str, rule: &str, message: &str) -> PlausibilityWarning {
    PlausibilityWarning {
        code: code.into(),
        rule_id: rule.into(),
        message: message.into(),
    }
}
pub fn check(m: &Measurement, measured_at: Timestamp, now: Timestamp) -> Vec<PlausibilityWarning> {
    let mut result = Vec::new();
    if let Some((_, low, high, secondary)) = rules::PLAUSIBILITY
        .iter()
        .find(|(i, _, _, _)| *i == m.indicator())
    {
        let value = m.primary();
        let second_outside = match (m, secondary) {
            (Measurement::BloodPressure { diastolic, .. }, Some((lo, hi))) => {
                diastolic.value() < *lo || diastolic.value() > *hi
            }
            _ => false,
        };
        if value < *low || value > *high || second_outside {
            let id = format!("plaus.{}", m.indicator().as_str());
            result.push(warning(
                "unusual_value",
                &id,
                "This value is unusual. Please confirm it.",
            ));
        }
    }
    if let Measurement::BloodPressure {
        systolic,
        diastolic,
    } = m
        && diastolic.value() >= systolic.value()
    {
        result.push(warning(
            "diastolic_not_below_systolic",
            rules::BP_ORDER_ID,
            "Diastolic pressure is not below systolic pressure.",
        ));
    }
    if measured_at > now.saturating_add_ms(rules::FUTURE) {
        result.push(warning(
            "future_timestamp",
            rules::FUTURE_ID,
            "The measurement time is in the future.",
        ));
    }
    if measured_at < now.saturating_add_ms(-rules::OLD) {
        result.push(warning(
            "old_measurement",
            rules::OLD_ID,
            "The measurement is more than seven days old.",
        ));
    }
    result
}
