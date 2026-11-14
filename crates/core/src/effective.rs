//! Selección determinista de correcciones y series efectivas.
use crate::{
    catalog::{BodyZone, Indicator},
    measurement::Measurement,
    rules,
    time::Timestamp,
};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
#[derive(Clone, Debug, PartialEq)]
pub struct RecordView {
    pub record_id: String,
    pub indicator: Indicator,
    pub body_zone: Option<BodyZone>,
    pub measurement: Measurement,
    pub measured_at: Timestamp,
    pub logged_at: Timestamp,
    pub received_at: Option<Timestamp>,
    pub seq: Option<i64>,
    pub local_order: i64,
    pub replaces: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub struct SeriesKey {
    pub indicator: Indicator,
    pub body_zone: Option<BodyZone>,
}
/// Índice de raíces, futuros y ganadores de cada grupo.
pub struct EffectiveSet<'a> {
    records: &'a [RecordView],
    roots: HashMap<&'a str, &'a str>,
    winners: HashMap<&'a str, &'a str>,
    future: HashSet<&'a str>,
}
fn order(r: &RecordView) -> (Option<i64>, i64, &str) {
    (r.seq, r.local_order, &r.record_id)
}
fn compare_order(a: &RecordView, b: &RecordView) -> std::cmp::Ordering {
    (a.seq.is_none(), order(a)).cmp(&(b.seq.is_none(), order(b)))
}
impl<'a> EffectiveSet<'a> {
    pub fn build(records: &'a [RecordView]) -> Self {
        let by_id: HashMap<&str, &RecordView> =
            records.iter().map(|r| (r.record_id.as_str(), r)).collect();
        let mut roots = HashMap::new();
        let mut future = HashSet::new();
        for r in records {
            if r.measured_at
                > r.received_at
                    .unwrap_or(r.logged_at)
                    .saturating_add_ms(rules::FUTURE)
            {
                future.insert(r.record_id.as_str());
            }
            let mut seen = HashSet::new();
            let mut cursor = r;
            let root = loop {
                if !seen.insert(cursor.record_id.as_str()) {
                    break r.record_id.as_str();
                }
                match &cursor.replaces {
                    None => break cursor.record_id.as_str(),
                    Some(id) => match by_id.get(id.as_str()) {
                        Some(next) => cursor = next,
                        None => break r.record_id.as_str(),
                    },
                }
            };
            roots.insert(r.record_id.as_str(), root);
        }
        let mut best: HashMap<&str, &RecordView> = HashMap::new();
        for r in records {
            if future.contains(r.record_id.as_str()) {
                continue;
            }
            if let Some(root) = roots.get(r.record_id.as_str()) {
                let replace = best
                    .get(root)
                    .is_none_or(|old| compare_order(r, old).is_gt());
                if replace {
                    best.insert(root, r);
                }
            }
        }
        let winners = best
            .into_iter()
            .map(|(root, r)| (root, r.record_id.as_str()))
            .collect();
        Self {
            records,
            roots,
            winners,
            future,
        }
    }
    pub fn is_effective(&self, record_id: &str) -> bool {
        !self.is_future(record_id)
            && self
                .roots
                .get(record_id)
                .and_then(|root| self.winners.get(root))
                .is_some_and(|id| *id == record_id)
    }
    pub fn superseded_by(&self, record_id: &str) -> Option<&str> {
        if self.is_future(record_id) {
            return None;
        }
        self.roots
            .get(record_id)
            .and_then(|root| self.winners.get(root))
            .copied()
            .filter(|id| *id != record_id)
    }
    pub fn is_future(&self, record_id: &str) -> bool {
        self.future.contains(record_id)
    }
    pub fn root_of(&self, record_id: &str) -> Option<&str> {
        self.roots.get(record_id).copied()
    }
    pub fn series(&self) -> Vec<(SeriesKey, Vec<&'a RecordView>)> {
        let mut grouped: BTreeMap<SeriesKey, Vec<&RecordView>> = BTreeMap::new();
        for r in self
            .records
            .iter()
            .filter(|r| self.is_effective(&r.record_id))
        {
            let key = SeriesKey {
                indicator: r.indicator,
                body_zone: if r.indicator == Indicator::Symptom {
                    r.body_zone
                } else {
                    None
                },
            };
            grouped.entry(key).or_default().push(r);
        }
        grouped
            .into_iter()
            .map(|(key, mut items)| {
                items.sort_by(|a, b| {
                    a.measured_at
                        .cmp(&b.measured_at)
                        .then_with(|| compare_order(a, b))
                });
                (key, items)
            })
            .collect()
    }
}
