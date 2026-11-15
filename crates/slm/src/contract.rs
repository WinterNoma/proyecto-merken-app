//! Exact offline V7 policy; checked against frozen Python vectors.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Reading {
    pub metric: String,
    pub value: Option<f64>,
    pub unit: String,
    pub measured_at: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Assessment {
    pub quality: String,
    pub freshness: String,
    pub core_action: String,
    pub exercise_clearance: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Input {
    pub version: String,
    pub source: String,
    pub readings: Vec<Reading>,
    pub assessment: Assessment,
    #[serde(default)]
    pub assigned_plan: Vec<String>,
    #[serde(default)]
    pub equipment: Vec<String>,
    pub preferred_activity: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub source: String,
    pub activity_id: String,
    pub recommendation: String,
    pub caveat: String,
    pub source_ids: Vec<String>,
}
#[derive(Deserialize)]
struct Card {
    kind: String,
    equipment: Option<String>,
    recommendation: String,
    source_id: String,
}
#[derive(Deserialize)]
struct Contract {
    system: String,
    cards: BTreeMap<String, Card>,
    caveats: BTreeMap<String, String>,
    units: BTreeMap<String, String>,
}
fn config() -> Result<Contract, String> {
    serde_json::from_str(include_str!("../assets/contract-v7.json"))
        .map_err(|_| "Offline contract is invalid".into())
}
pub fn system() -> Result<String, String> {
    Ok(config()?.system)
}
pub fn normalize(input: &Input) -> Result<Value, String> {
    let cfg = config()?;
    if input.version != "wellbeing-en-v7"
        || !["SIMULATED", "GARMIN_BACKEND_SAMPLE", "MANUAL"].contains(&input.source.as_str())
        || input.readings.is_empty()
        || input.readings.len() > 16
    {
        return Err("Unsupported source, version or reading count".into());
    }
    let mut seen = BTreeSet::new();
    for r in &input.readings {
        if cfg.units.get(&r.metric) != Some(&r.unit)
            || !seen.insert(&r.metric)
            || r.value.is_some_and(|v| !v.is_finite())
        {
            return Err("Invalid metric, unit or value".into());
        }
        if let Some(t) = &r.measured_at
            && (!t.ends_with('Z') || merken_core::time::Timestamp::parse_rfc3339(t).is_err())
        {
            return Err("Invalid measurement timestamp".into());
        }
    }
    let a = &input.assessment;
    if !["VALID", "INVALID", "UNKNOWN"].contains(&a.quality.as_str())
        || !["CURRENT", "STALE", "UNKNOWN"].contains(&a.freshness.as_str())
        || ![
            "NONE",
            "NOT_ASSESSED",
            "REVIEW_MEDICAL",
            "IMMEDIATE_PROTOCOL",
        ]
        .contains(&a.core_action.as_str())
        || !["CLEARED", "BLOCKED", "UNKNOWN"].contains(&a.exercise_clearance.as_str())
    {
        return Err("Invalid core assessment".into());
    }
    if input
        .preferred_activity
        .as_ref()
        .is_some_and(|p| !cfg.cards.contains_key(p))
    {
        return Err("Invalid preference".into());
    }
    for id in &input.assigned_plan {
        if cfg.cards.get(id).is_none_or(|c| c.kind != "exercise") {
            return Err("Invalid assigned plan".into());
        }
    }
    if input
        .equipment
        .iter()
        .any(|e| !["cycle", "treadmill", "resistance_device", "flywheel"].contains(&e.as_str()))
        || input.equipment.iter().collect::<BTreeSet<_>>().len() != input.equipment.len()
        || input.assigned_plan.iter().collect::<BTreeSet<_>>().len() != input.assigned_plan.len()
    {
        return Err("Invalid equipment or duplicate plan".into());
    }
    let complete = input
        .readings
        .iter()
        .all(|r| r.value.is_some() && r.measured_at.is_some());
    let exercise = complete
        && a.quality == "VALID"
        && a.freshness == "CURRENT"
        && a.core_action == "NONE"
        && a.exercise_clearance == "CLEARED";
    let mut ids = Vec::new();
    let caveat = if a.core_action == "IMMEDIATE_PROTOCOL" {
        ids.push("FOLLOW_PROTOCOL".to_string());
        "URGENT"
    } else if a.core_action == "REVIEW_MEDICAL" {
        ids.push("CONTACT_MEDICAL".to_string());
        "URGENT"
    } else {
        if exercise {
            for id in &input.assigned_plan {
                if cfg
                    .cards
                    .get(id)
                    .and_then(|c| c.equipment.as_ref())
                    .is_some_and(|e| input.equipment.contains(e))
                {
                    ids.push(id.clone());
                }
            }
        }
        let has_exercise = !ids.is_empty();
        ids.extend(
            ["READING", "MUSIC", "PUZZLE", "JOURNAL"]
                .iter()
                .map(|s| s.to_string()),
        );
        if !complete
            || a.quality != "VALID"
            || a.freshness != "CURRENT"
            || a.core_action == "NOT_ASSESSED"
        {
            "UNCERTAIN"
        } else if exercise && has_exercise {
            "CLEARED"
        } else {
            "NO_CLEARANCE"
        }
    };
    let allowed:Vec<Value>=ids.iter().filter_map(|id|cfg.cards.get(id).map(|c|json!({"activity_id":id,"recommendation":c.recommendation,"source_id":c.source_id}))).collect();
    Ok(
        json!({"version":input.version,"source":input.source,"readings":input.readings,"assessment":input.assessment,"preferred_activity":input.preferred_activity,"allowed_activities":allowed,"caveat":cfg.caveats.get(caveat).ok_or("Missing caveat")?}),
    )
}
pub fn expected(input: &Input) -> Result<Output, String> {
    let normalized = normalize(input)?;
    let allowed = normalized
        .get("allowed_activities")
        .and_then(Value::as_array)
        .ok_or("Missing permitted options")?;
    let chosen = allowed
        .iter()
        .find(|a| {
            a.get("activity_id").and_then(Value::as_str) == input.preferred_activity.as_deref()
        })
        .or_else(|| allowed.first())
        .ok_or("No permitted activity")?;
    let text = |key: &str| {
        chosen
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| "Invalid activity card".to_string())
    };
    Ok(Output {
        source: input.source.clone(),
        activity_id: text("activity_id")?,
        recommendation: text("recommendation")?,
        caveat: normalized
            .get("caveat")
            .and_then(Value::as_str)
            .ok_or("Missing caveat")?
            .to_owned(),
        source_ids: vec![text("source_id")?],
    })
}
pub fn accept(input: &Input, raw: &str) -> Result<Output, String> {
    let actual: Output =
        serde_json::from_str(raw).map_err(|_| "Model response is not valid contract JSON")?;
    if actual != expected(input)? {
        return Err("Model response rejected; follow the independent core action".into());
    }
    Ok(actual)
}
pub fn prompt(input: &Input) -> Result<String, String> {
    let text =
        serde_json::to_string(&normalize(input)?).map_err(|_| "Input could not be serialized")?;
    Ok(format!(
        "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n",
        system()?,
        text
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_python_policy_vectors_match() -> Result<(), String> {
        let vectors: Vec<Value> =
            serde_json::from_str(include_str!("../assets/policy-vectors.json"))
                .map_err(|e| e.to_string())?;
        assert_eq!(vectors.len(), 30);
        for v in vectors {
            let input: Input =
                serde_json::from_value(v.get("input").ok_or("Missing input")?.clone())
                    .map_err(|e| e.to_string())?;
            assert_eq!(
                &normalize(&input)?,
                v.get("normalized").ok_or("Missing normalized")?
            );
            let raw = v.get("output").ok_or("Missing output")?.to_string();
            accept(&input, &raw)?;
            let mut altered: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
            if let Some(o) = altered.as_object_mut() {
                o.insert("dose".into(), json!("invented"));
            }
            assert!(accept(&input, &altered.to_string()).is_err());
        }
        Ok(())
    }
}
