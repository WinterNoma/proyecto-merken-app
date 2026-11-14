//! Registros, estado actual, evaluaciones guardadas, DEMO y página de descarga.
//!
//! Toda decisión de salud la toma `merken-core`; aquí solo se carga, se
//! guarda y se monta el JSON que espera la interfaz (`ui/src/core-api.ts`).

use crate::{
    Store, StoreError,
    api::{
        BodyZoneStatus, CorrectRecordInput, ImportResult, ListRecordsInput, LogRecordInput,
        LogResult, RecordItem, StatusResponse, SyncStatus,
    },
};
use merken_core::{
    catalog::{BodyZone, Indicator},
    effective::{EffectiveSet, RecordView},
    evaluate::{self, Evaluation, EventView, NotEvaluatedReason},
    measurement::Measurement,
    plausibility,
    record::{EventKind, Priority, RecordDto, RecordSource},
    time::{Timestamp, mission_elapsed},
    validate_text,
};
use merken_seed::Scenario;
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Límite de caracteres de `note`, `message` y `description` (contrato §9).
pub const MAX_TEXT: usize = 2000;
const DEFAULT_LIMIT: usize = 100;
const MAX_LIMIT: usize = 500;

/// Fila de `records` ya validada con `merken-core`.
struct Row {
    view: RecordView,
    value: Value,
    unit: String,
    source: RecordSource,
    note: Option<String>,
    root_id: String,
    is_demo: bool,
    sync: String,
}

/// Cambio recibido en `GET /v1/sync/pull` (contrato §7).
#[derive(Clone, Debug, Deserialize)]
pub struct PulledChange {
    pub seq: i64,
    pub entity: String,
    pub id: String,
    pub payload: Value,
}

fn err(code: &str, field: &str, message: &str) -> StoreError {
    StoreError {
        code: code.into(),
        field: Some(field.into()),
        message: message.into(),
    }
}

fn enum_from_str<T: for<'de> Deserialize<'de>>(s: &str) -> Result<T, StoreError> {
    Ok(serde_json::from_value(Value::String(s.to_owned()))?)
}

fn enum_to_str<T: Serialize>(v: &T) -> Result<String, StoreError> {
    match serde_json::to_value(v)? {
        Value::String(s) => Ok(s),
        _ => Err(StoreError::storage()),
    }
}

fn series_key_str(indicator: &str, zone: Option<BodyZone>) -> String {
    match zone {
        Some(z) => format!("{indicator}:{}", z.as_str()),
        None => indicator.to_owned(),
    }
}

/// Lo que se compara para decidir si una evaluación cambió.
fn signature(e: &Evaluation) -> Value {
    serde_json::json!([
        e.status,
        e.reason,
        e.direction,
        e.persistent,
        e.escalation,
        e.rule_id,
        e.rule_version
    ])
}

/// Payload de `health_evaluations` (contrato §3), sin los campos solo de la interfaz.
fn evaluation_payload(id: &str, e: &Evaluation, app_version: &str) -> Value {
    serde_json::json!({
        "evaluation_id": id,
        "indicator": e.indicator,
        "body_zone": e.body_zone,
        "status": e.status,
        "reason": e.reason,
        "direction": e.direction,
        "persistent": e.persistent,
        "escalation": e.escalation,
        "rule_id": e.rule_id,
        "rule_version": e.rule_version,
        "source_kind": e.source_kind,
        "source_ref": e.source_ref,
        "input_record_ids": e.input_record_ids,
        "input_event_ids": e.input_event_ids,
        "core_version": e.core_version,
        "evaluated_at": e.evaluated_at,
        "schema_version": 1,
        "app_version": app_version,
    })
}

impl Store {
    fn load_rows(&self) -> Result<Vec<Row>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT r.record_id, r.indicator, r.body_zone, r.value_json, r.unit, r.measured_at,
                    r.logged_at, r.source, r.replaces, r.root_id, r.note, r.seq, r.received_at,
                    r.local_order, r.is_demo, o.state
             FROM records r
             LEFT JOIN outbox o ON o.entity = 'record' AND o.entity_id = r.record_id
             ORDER BY r.local_order",
        )?;
        let raw = stmt.query_map([], |r| {
            Ok((
                (
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, i64>(6)?,
                    r.get::<_, String>(7)?,
                ),
                (
                    r.get::<_, Option<String>>(8)?,
                    r.get::<_, String>(9)?,
                    r.get::<_, Option<String>>(10)?,
                    r.get::<_, Option<i64>>(11)?,
                    r.get::<_, Option<i64>>(12)?,
                    r.get::<_, i64>(13)?,
                    r.get::<_, bool>(14)?,
                    r.get::<_, Option<String>>(15)?,
                ),
            ))
        })?;
        let mut rows = Vec::new();
        for item in raw {
            let (
                (record_id, indicator, zone, value_json, unit, measured_at, logged_at, source),
                (replaces, root_id, note, seq, received_at, local_order, is_demo, outbox_state),
            ) = item?;
            // RNF-02: lo que sale de SQLite se vuelve a validar. Una fila corrupta
            // se ignora en la evaluación en lugar de tumbar la app.
            let Ok(indicator) = Indicator::parse(&indicator) else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<Value>(&value_json) else {
                continue;
            };
            let Ok(measurement) = Measurement::validate(indicator, &value, Some(&unit)) else {
                continue;
            };
            let Ok(source) = enum_from_str::<RecordSource>(&source) else {
                continue;
            };
            let body_zone = zone.and_then(|z| BodyZone::parse(&z).ok());
            let sync = if is_demo {
                "local_only".to_owned()
            } else {
                outbox_state.unwrap_or_else(|| "synced".to_owned())
            };
            rows.push(Row {
                view: RecordView {
                    record_id,
                    indicator,
                    body_zone,
                    measurement,
                    measured_at: Timestamp::from_millis(measured_at),
                    logged_at: Timestamp::from_millis(logged_at),
                    received_at: received_at.map(Timestamp::from_millis),
                    seq,
                    local_order,
                    replaces,
                },
                value,
                unit,
                source,
                note,
                root_id,
                is_demo,
                sync,
            });
        }
        Ok(rows)
    }

    fn load_event_views(&self) -> Result<Vec<(EventView, bool)>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT event_id, kind, occurred_at, priority, is_demo FROM events")?;
        let raw = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, bool>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for item in raw {
            let (event_id, kind, occurred_at, priority, is_demo) = item?;
            let (Ok(kind), Ok(priority)) = (
                enum_from_str::<EventKind>(&kind),
                enum_from_str::<Priority>(&priority),
            ) else {
                continue;
            };
            out.push((
                EventView {
                    event_id,
                    kind,
                    occurred_at: Timestamp::from_millis(occurred_at),
                    priority,
                },
                is_demo,
            ));
        }
        Ok(out)
    }

    fn item(row: &Row, set: &EffectiveSet<'_>) -> RecordItem {
        let id = row.view.record_id.as_str();
        RecordItem {
            record_id: row.view.record_id.clone(),
            indicator: row.view.indicator,
            body_zone: row.view.body_zone,
            value: row.value.clone(),
            unit: row.unit.clone(),
            measured_at: row.view.measured_at,
            logged_at: row.view.logged_at,
            source: row.source,
            note: row.note.clone(),
            replaces: row.view.replaces.clone(),
            root_id: row.root_id.clone(),
            effective: set.is_effective(id),
            superseded_by: set.superseded_by(id).map(str::to_owned),
            future_timestamp: set.is_future(id),
            sync: row.sync.clone(),
            is_demo: row.is_demo,
        }
    }

    /// Valida, pide confirmación si hace falta y guarda un registro con su fila de outbox.
    #[allow(clippy::too_many_arguments)]
    fn save_record(
        &mut self,
        indicator: Indicator,
        value: &Value,
        measured_at: Option<Timestamp>,
        source: RecordSource,
        note: Option<String>,
        confirm_warnings: bool,
        replaces: Option<(String, String, bool)>,
        now: Timestamp,
        app_version: &str,
    ) -> Result<LogResult, StoreError> {
        if !matches!(
            source,
            RecordSource::Form | RecordSource::Text | RecordSource::Voice
        ) {
            return Err(err(
                "invalid_value",
                "source",
                "Only form, text or voice records can be created on the device",
            ));
        }
        let measurement = Measurement::validate(indicator, value, None).map_err(|mut e| {
            if !e.field.starts_with("value") {
                e.field = format!("value.{}", e.field);
            }
            StoreError::from(e)
        })?;
        if let Some(n) = &note {
            validate_text("note", n, MAX_TEXT)?;
        }
        let measured_at = measured_at.unwrap_or(now);
        let warnings = plausibility::check(&measurement, measured_at, now);
        if !warnings.is_empty() && !confirm_warnings {
            return Ok(LogResult::NeedsConfirmation { warnings });
        }
        let record_id = self.new_id(now)?;
        let (replaces_id, root_id, is_demo) = match replaces {
            Some((id, root, demo)) => (Some(id), root, demo),
            None => (None, record_id.clone(), false),
        };
        let dto = RecordDto {
            record_id: record_id.clone(),
            indicator,
            value: measurement.to_value(),
            unit: measurement.unit().to_owned(),
            measured_at,
            logged_at: now,
            source,
            interpreted_by: None,
            replaces: replaces_id.clone(),
            external_id: None,
            note: note.clone(),
            schema_version: 1,
            app_version: Some(app_version.to_owned()),
        };
        let tx = self.conn.transaction()?;
        if !is_demo {
            crate::clear_demo_in(&tx)?;
        }
        insert_record(&tx, &dto, measurement.body_zone(), &root_id, is_demo)?;
        if !is_demo {
            enqueue(&tx, "record", &record_id, &serde_json::to_value(&dto)?, now)?;
        }
        tx.commit()?;

        let status = self.status(now, app_version)?;
        let key = series_key_str(indicator.as_str(), measurement.body_zone());
        let evaluation = status
            .indicators
            .into_iter()
            .chain(status.symptoms)
            .find(|e| series_key_str(&e.indicator, e.body_zone) == key)
            // Un primer síntoma con hora futura no forma serie todavía: el registro
            // ya está guardado, así que se devuelve `saved` con `no_data`.
            .unwrap_or_else(|| {
                evaluate::evaluate_series(
                    merken_core::effective::SeriesKey {
                        indicator,
                        body_zone: measurement.body_zone(),
                    },
                    &[],
                    now,
                )
            });
        let rows = self.load_rows()?;
        let views: Vec<RecordView> = rows.iter().map(|r| r.view.clone()).collect();
        let set = EffectiveSet::build(&views);
        let record = rows
            .iter()
            .find(|r| r.view.record_id == record_id)
            .map(|r| Self::item(r, &set))
            .ok_or_else(StoreError::storage)?;
        Ok(LogResult::Saved {
            record: Box::new(record),
            warnings,
            evaluation: Box::new(evaluation),
        })
    }

    /// `log_record`: registra una medición o un síntoma nuevo.
    pub fn log_record(
        &mut self,
        input: LogRecordInput,
        now: Timestamp,
        app_version: &str,
    ) -> Result<LogResult, StoreError> {
        self.save_record(
            input.indicator,
            &input.value,
            input.measured_at,
            input.source.unwrap_or(RecordSource::Form),
            input.note,
            input.confirm_warnings,
            None,
            now,
            app_version,
        )
    }

    /// `correct_record`: guarda una versión nueva que apunta al registro corregido.
    pub fn correct_record(
        &mut self,
        input: CorrectRecordInput,
        now: Timestamp,
        app_version: &str,
    ) -> Result<LogResult, StoreError> {
        let old: Option<(String, Option<String>, String, bool)> = self
            .conn
            .query_row(
                "SELECT indicator, body_zone, root_id, is_demo FROM records WHERE record_id = ?1",
                [&input.replaces],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        let (indicator, zone, root_id, is_demo) = old.ok_or_else(|| {
            err(
                "not_found",
                "replaces",
                "The record to correct does not exist",
            )
        })?;
        let indicator = Indicator::parse(&indicator)?;
        if indicator == Indicator::Symptom {
            let new_zone = input.value.get("body_zone").and_then(Value::as_str);
            if new_zone != zone.as_deref() {
                return Err(err(
                    "invalid_value",
                    "value.body_zone",
                    "A correction must keep the same body zone",
                ));
            }
        }
        self.save_record(
            indicator,
            &input.value,
            input.measured_at,
            RecordSource::Form,
            input.note,
            input.confirm_warnings,
            Some((input.replaces, root_id, is_demo)),
            now,
            app_version,
        )
    }

    /// `list_records`: historial filtrado, del más reciente al más antiguo.
    pub fn list_records(&self, input: ListRecordsInput) -> Result<Vec<RecordItem>, StoreError> {
        let rows = self.load_rows()?;
        let views: Vec<RecordView> = rows.iter().map(|r| r.view.clone()).collect();
        let set = EffectiveSet::build(&views);
        let mut items: Vec<(&Row, RecordItem)> = rows
            .iter()
            .map(|r| (r, Self::item(r, &set)))
            .filter(|(r, it)| {
                input.indicator.is_none_or(|i| r.view.indicator == i)
                    && input.body_zone.is_none_or(|z| r.view.body_zone == Some(z))
                    && input.from.is_none_or(|f| r.view.measured_at >= f)
                    && input.to.is_none_or(|t| r.view.measured_at <= t)
                    && (input.include_superseded || it.effective)
            })
            .collect();
        items.sort_by(|(a, _), (b, _)| {
            b.view
                .measured_at
                .cmp(&a.view.measured_at)
                .then_with(|| b.view.local_order.cmp(&a.view.local_order))
        });
        let limit = input.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
        Ok(items.into_iter().take(limit).map(|(_, it)| it).collect())
    }

    /// `get_status`: evalúa todo con `merken-core` y guarda las evaluaciones que cambiaron.
    pub fn status(
        &mut self,
        now: Timestamp,
        app_version: &str,
    ) -> Result<StatusResponse, StoreError> {
        let mut rows = self.load_rows()?;
        let mut events = self.load_event_views()?;
        // Un dato real nunca se evalúa contra historial DEMO: en cuanto hay algo
        // real, lo DEMO deja de contar para el estado (sigue en el historial).
        if rows.iter().any(|r| !r.is_demo) || events.iter().any(|(_, d)| !*d) {
            rows.retain(|r| !r.is_demo);
            events.retain(|(_, d)| !*d);
        }
        let views: Vec<RecordView> = rows.iter().map(|r| r.view.clone()).collect();
        let event_views: Vec<EventView> = events.iter().map(|(e, _)| e.clone()).collect();
        let evals = evaluate::evaluate_all(&views, &event_views, now);

        let demo_records: std::collections::HashSet<&str> = rows
            .iter()
            .filter(|r| r.is_demo)
            .map(|r| r.view.record_id.as_str())
            .collect();
        let demo_events: std::collections::HashSet<&str> = events
            .iter()
            .filter(|(_, d)| *d)
            .map(|(e, _)| e.event_id.as_str())
            .collect();
        self.persist_changed(&evals, &demo_records, &demo_events, now, app_version)?;

        let overall = evaluate::overall(&evals);
        let body_zones = evaluate::body_zone_status(&evals)
            .into_iter()
            .map(|(body_zone, status)| BodyZoneStatus { body_zone, status })
            .collect();
        let mut indicators = Vec::new();
        let mut symptoms = Vec::new();
        let mut alerts = Vec::new();
        for e in evals {
            if !e.input_event_ids.is_empty() {
                alerts.push(e);
            } else if e.indicator == Indicator::Symptom.as_str() {
                symptoms.push(e);
            } else {
                indicators.push(e);
            }
        }
        let profile = self.get_profile()?;
        let met = profile
            .profile
            .as_ref()
            .and_then(|p| p.pointer("/astronaut/mission/epoch_utc"))
            .and_then(Value::as_str)
            .and_then(|s| Timestamp::parse_rfc3339(s).ok())
            .map(|epoch| mission_elapsed(epoch, now));
        let is_demo = profile.is_demo || !demo_records.is_empty() || !demo_events.is_empty();
        Ok(StatusResponse {
            evaluated_at: now,
            met,
            overall,
            indicators,
            symptoms,
            alerts,
            body_zones,
            sync: self.sync_status()?,
            is_demo,
        })
    }

    fn persist_changed(
        &mut self,
        evals: &[Evaluation],
        demo_records: &std::collections::HashSet<&str>,
        demo_events: &std::collections::HashSet<&str>,
        now: Timestamp,
        app_version: &str,
    ) -> Result<(), StoreError> {
        let mut pending = Vec::new();
        for e in evals {
            // "No hay datos" no es un hallazgo: no se guarda ni se sube. Los
            // indicadores de contexto tampoco: el contrato §3 no admite
            // `not_applicable` (revisión de Sol, B2). Sus registros sí se suben.
            if matches!(
                e.reason,
                Some(NotEvaluatedReason::NoData | NotEvaluatedReason::NotApplicable)
            ) {
                continue;
            }
            let key = match e.input_event_ids.first() {
                Some(event_id) => format!("event:{event_id}"),
                None => series_key_str(&e.indicator, e.body_zone),
            };
            let last: Option<String> = self
                .conn
                .query_row(
                    "SELECT payload_json FROM evaluations WHERE series_key = ?1
                     ORDER BY evaluated_at DESC, rowid DESC LIMIT 1",
                    [&key],
                    |r| r.get(0),
                )
                .optional()?;
            let unchanged = last
                .and_then(|p| serde_json::from_str::<Value>(&p).ok())
                .is_some_and(|p| {
                    let fields = [
                        "status",
                        "reason",
                        "direction",
                        "persistent",
                        "escalation",
                        "rule_id",
                        "rule_version",
                    ];
                    Value::Array(
                        fields
                            .iter()
                            .map(|f| p.get(*f).cloned().unwrap_or(Value::Null))
                            .collect(),
                    ) == signature(e)
                });
            if !unchanged {
                let is_demo = e
                    .input_record_ids
                    .iter()
                    .any(|id| demo_records.contains(id.as_str()))
                    || e.input_event_ids
                        .iter()
                        .any(|id| demo_events.contains(id.as_str()));
                pending.push((key, e, is_demo));
            }
        }
        if pending.is_empty() {
            return Ok(());
        }
        let ids: Vec<String> = pending
            .iter()
            .map(|_| self.new_id(now))
            .collect::<Result<_, _>>()?;
        let tx = self.conn.transaction()?;
        for ((key, e, is_demo), id) in pending.into_iter().zip(ids) {
            let payload = evaluation_payload(&id, e, app_version);
            tx.execute(
                "INSERT INTO evaluations(evaluation_id, series_key, payload_json, evaluated_at, is_demo)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, key, serde_json::to_string(&payload)?, now.millis(), is_demo],
            )?;
            if !is_demo {
                enqueue(&tx, "evaluation", &id, &payload, now)?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    fn sync_status(&self) -> Result<SyncStatus, StoreError> {
        let count = |state: &str| -> Result<i64, StoreError> {
            Ok(self.conn.query_row(
                "SELECT COUNT(*) FROM outbox WHERE state = ?1",
                [state],
                |r| r.get(0),
            )?)
        };
        let ts = |key: &str| -> Result<Option<Timestamp>, StoreError> {
            let v: Option<String> = self
                .conn
                .query_row("SELECT value FROM sync_state WHERE key = ?1", [key], |r| {
                    r.get(0)
                })
                .optional()?;
            Ok(v.and_then(|s| Timestamp::parse_rfc3339(&s).ok()))
        };
        Ok(SyncStatus {
            pending: count("pending")?,
            failed: count("failed")?,
            last_push_at: ts("last_push_at")?,
            last_pull_at: ts("last_pull_at")?,
        })
    }

    /// `true` si hay algún dato que no sea DEMO (perfil, registros o eventos).
    pub fn has_real_data(&self) -> Result<bool, StoreError> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM records WHERE is_demo = 0)
                 OR EXISTS(SELECT 1 FROM events WHERE is_demo = 0)
                 OR EXISTS(SELECT 1 FROM profile WHERE is_demo = 0)",
            [],
            |r| r.get(0),
        )?)
    }

    /// `load_demo`: carga un escenario DEMO. Nunca entra en la outbox.
    pub fn load_demo(
        &mut self,
        scenario: Scenario,
        now: Timestamp,
        app_version: &str,
    ) -> Result<StatusResponse, StoreError> {
        if self.has_real_data()? {
            return Err(err(
                "real_data_present",
                "scenario",
                "Demo data cannot be loaded while real data is stored on this device",
            ));
        }
        self.clear_demo()?;
        self.upsert_profile(&merken_seed::demo_profile(now), true, now)?;
        let data = merken_seed::scenario(scenario, now);
        let mut prepared = Vec::new();
        for r in &data.records {
            let m = Measurement::validate(r.indicator, &r.value, None)?;
            let id = self.new_id(now)?;
            prepared.push((
                RecordDto {
                    record_id: id.clone(),
                    indicator: r.indicator,
                    value: m.to_value(),
                    unit: m.unit().to_owned(),
                    measured_at: r.measured_at,
                    logged_at: r.measured_at,
                    source: RecordSource::Form,
                    interpreted_by: None,
                    replaces: None,
                    external_id: None,
                    note: r.note.clone(),
                    schema_version: 1,
                    app_version: Some(app_version.to_owned()),
                },
                m.body_zone(),
            ));
        }
        let mut events = Vec::new();
        for e in &data.events {
            let kind = enum_from_str::<EventKind>(e.kind)?;
            let zone = e.body_zone.map(BodyZone::parse).transpose()?;
            events.push((self.new_id(now)?, kind, zone, e));
        }
        let tx = self.conn.transaction()?;
        for (dto, zone) in &prepared {
            insert_record(&tx, dto, *zone, &dto.record_id, true)?;
        }
        for (id, kind, zone, e) in &events {
            tx.execute(
                "INSERT INTO events(event_id, kind, certainty, message, body_zone, priority,
                                    occurred_at, logged_at, is_demo)
                 VALUES (?1, ?2, 'symptom', ?3, ?4, ?5, ?6, ?6, 1)",
                params![
                    id,
                    kind.as_str(),
                    e.message,
                    zone.map(|z| z.as_str()),
                    enum_to_str(&kind.default_priority())?,
                    e.occurred_at.millis()
                ],
            )?;
        }
        tx.commit()?;
        self.status(now, app_version)
    }

    /// `import_synthetic`: carga registros **sintéticos** de la API de simulación
    /// (formato `RecordDto` del contrato). Se guardan como DEMO: nunca entran en
    /// la outbox y la primera escritura real los borra. Sustituye cualquier DEMO
    /// anterior (no se mezclan astronautas). Un registro inválido se cuenta en
    /// `rejected` y no detiene la carga.
    pub fn import_synthetic(
        &mut self,
        astronaut_id: &str,
        display_name: &str,
        records: &[Value],
        now: Timestamp,
    ) -> Result<ImportResult, StoreError> {
        if self.has_real_data()? {
            return Err(err(
                "real_data_present",
                "astronaut_id",
                "Simulated data cannot be loaded while real data is stored on this device",
            ));
        }
        merken_core::validate_text("display_name", display_name, 200)?;
        if display_name.trim().is_empty() {
            return Err(err(
                "invalid_value",
                "display_name",
                "Display name is empty",
            ));
        }
        // Un registro con fecha posterior a `now + FUTURE` no se carga: el núcleo
        // lo evaluaría como reciente aunque el reloj de la simulación vaya
        // adelantado (revisión de Sol, I2).
        let latest = now.saturating_add_ms(merken_core::rules::FUTURE);
        let mut seen = std::collections::HashSet::new();
        let mut prepared = Vec::new();
        let mut rejected = 0_usize;
        for raw in records {
            let parsed = serde_json::from_value::<RecordDto>(raw.clone())
                .ok()
                .filter(|dto| {
                    raw.get("astronaut_id").and_then(Value::as_str) == Some(astronaut_id)
                        && synthetic_dto_ok(dto, latest)
                })
                .and_then(|dto| {
                    Measurement::validate(dto.indicator, &dto.value, Some(&dto.unit))
                        .ok()
                        .map(|m| (dto, m))
                });
            match parsed {
                Some((mut dto, m)) if seen.insert(dto.record_id.clone()) => {
                    // Una corrección de la simulación no se encadena: cada lectura va sola.
                    dto.replaces = None;
                    dto.value = m.to_value();
                    prepared.push((dto, m.body_zone()));
                }
                _ => rejected += 1,
            }
        }
        let mut profile = merken_seed::demo_profile(now);
        if let Some(a) = profile.get_mut("astronaut").and_then(Value::as_object_mut) {
            a.insert("astronaut_id".into(), Value::from(astronaut_id));
            a.insert("username".into(), Value::from("simulation"));
            a.insert(
                "display_name".into(),
                Value::from(format!("SIMULATION {display_name}")),
            );
        }
        let first = prepared.iter().map(|(d, _)| d.measured_at).min();
        let last = prepared.iter().map(|(d, _)| d.measured_at).max();
        // Borrar la DEMO anterior, guardar el perfil y los registros va en una
        // sola transacción: si algo falla, la DEMO anterior queda intacta (Sol, B1).
        let tx = self.conn.transaction()?;
        crate::clear_demo_in(&tx)?;
        crate::upsert_profile_in(&tx, &profile, true, now)?;
        for (dto, zone) in &prepared {
            insert_record(&tx, dto, *zone, &dto.record_id, true)?;
        }
        tx.commit()?;
        Ok(ImportResult {
            astronaut_id: astronaut_id.to_owned(),
            display_name: display_name.to_owned(),
            imported: prepared.len(),
            rejected,
            first_measured_at: first,
            last_measured_at: last,
        })
    }

    /// Aplica una página de `pull` y guarda el cursor en una sola transacción
    /// (revisión de Sol, B4 y B5):
    /// - Todo cambio se conserva íntegro en `pulled_changes`, sea del tipo que sea.
    /// - `record` y `event` además se materializan. Si el ID ya existía (p. ej. se
    ///   subió pero se perdió la respuesta), se comprueba que el contenido clínico
    ///   coincide y solo se añaden `seq` y `received_at`; si no coincide → `conflict`.
    /// - Ante cualquier error la transacción se deshace y el cursor no avanza.
    pub fn apply_pull_page(
        &mut self,
        changes: &[PulledChange],
        next_cursor: &str,
        now: Timestamp,
    ) -> Result<usize, StoreError> {
        let tx = self.conn.transaction()?;
        let mut applied = 0;
        for c in changes {
            // El payload es la copia completa de la entidad con sus campos de
            // servidor (contrato §3): `seq` debe estar y coincidir.
            if c.payload.get("seq").and_then(Value::as_i64) != Some(c.seq) {
                return Err(err(
                    "invalid_schema",
                    "seq",
                    "Change seq is missing or does not match its payload",
                ));
            }
            // Un `seq` ya guardado con otro contenido es un diario incoherente.
            let previous: Option<(String, String, String)> = tx
                .query_row(
                    "SELECT entity, entity_id, payload_json FROM pulled_changes WHERE seq = ?1",
                    [c.seq],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            if let Some((entity, id, payload)) = previous {
                let same = entity == c.entity
                    && id == c.id
                    && serde_json::from_str::<Value>(&payload).ok().as_ref() == Some(&c.payload);
                if !same {
                    return Err(err(
                        "conflict",
                        "seq",
                        "The same seq arrived twice with different content",
                    ));
                }
                continue;
            }
            match c.entity.as_str() {
                "record" | "event" => {
                    let received_at = c
                        .payload
                        .get("received_at")
                        .and_then(Value::as_str)
                        .and_then(|s| Timestamp::parse_rfc3339(s).ok())
                        .ok_or_else(|| bad_payload(c))?;
                    if c.payload.get("schema_version").and_then(Value::as_i64) != Some(1) {
                        return Err(bad_payload(c));
                    }
                    applied += if c.entity == "record" {
                        pull_record(&tx, c, received_at)?
                    } else {
                        pull_event(&tx, c, received_at)?
                    };
                }
                _ => {}
            }
            tx.execute(
                "INSERT INTO pulled_changes(seq, entity, entity_id, payload_json, applied_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    c.seq,
                    c.entity,
                    c.id,
                    serde_json::to_string(&c.payload)?,
                    now.millis()
                ],
            )?;
        }
        if !changes.is_empty() {
            crate::clear_demo_in(&tx)?;
        }
        set_state(&tx, "pull_cursor", next_cursor)?;
        set_state(&tx, "last_pull_at", &now.to_rfc3339())?;
        tx.commit()?;
        Ok(applied)
    }
}

fn bad_payload(c: &PulledChange) -> StoreError {
    err(
        "invalid_schema",
        "payload",
        &format!("Pulled {} {} has an invalid payload", c.entity, c.id),
    )
}

/// Marca como subida la fila de outbox de una entidad que Tierra ya tiene.
fn reconcile_outbox(tx: &Transaction<'_>, entity: &str, id: &str) -> Result<(), StoreError> {
    tx.execute(
        "UPDATE outbox SET state = 'synced', last_error = NULL WHERE entity = ?1 AND entity_id = ?2",
        params![entity, id],
    )?;
    Ok(())
}

/// `headache` salió del catálogo el 14-nov (revisión de Sol, B1). Un registro
/// histórico bien formado no bloquea el `pull`: queda íntegro en
/// `pulled_changes`, pero no se convierte en un indicador vigente.
fn pull_retired_headache(c: &PulledChange) -> Result<usize, StoreError> {
    let p = &c.payload;
    let score_ok = p
        .get("value")
        .and_then(Value::as_object)
        .filter(|v| v.len() == 1)
        .and_then(|v| v.get("score"))
        .and_then(Value::as_u64)
        .is_some_and(|s| s <= 10);
    let ok = p.get("record_id").and_then(Value::as_str) == Some(c.id.as_str())
        && p.get("unit").and_then(Value::as_str) == Some("score_0_10")
        && p.get("measured_at").and_then(Value::as_str).is_some()
        && score_ok;
    if ok { Ok(0) } else { Err(bad_payload(c)) }
}

fn pull_record(
    tx: &Transaction<'_>,
    c: &PulledChange,
    received_at: Timestamp,
) -> Result<usize, StoreError> {
    if c.payload.get("indicator").and_then(Value::as_str) == Some("headache") {
        return pull_retired_headache(c);
    }
    let dto: RecordDto = serde_json::from_value(c.payload.clone()).map_err(|_| bad_payload(c))?;
    if dto.record_id != c.id {
        return Err(bad_payload(c));
    }
    let m = Measurement::validate(dto.indicator, &dto.value, Some(&dto.unit))
        .map_err(|_| bad_payload(c))?;
    let value_json = serde_json::to_string(&m.to_value())?;
    type Existing = (
        (String, String, String, i64, i64),
        (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<i64>,
        ),
    );
    let existing: Option<Existing> = tx
        .query_row(
            "SELECT indicator, value_json, unit, measured_at, logged_at,
                    source, interpreted_by_json, replaces, note, seq
             FROM records WHERE record_id = ?1",
            [&dto.record_id],
            |r| {
                Ok((
                    (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?),
                    (r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?),
                ))
            },
        )
        .optional()?;
    if let Some((
        (indicator, old_value, unit, measured_at, logged_at),
        (source, interp, replaces, note, seq),
    )) = existing
    {
        let old_value = serde_json::from_str::<Value>(&old_value)
            .ok()
            .and_then(|v| Measurement::validate(dto.indicator, &v, None).ok())
            .map(|m| m.to_value());
        let old_interp = interp.and_then(|j| serde_json::from_str::<Value>(&j).ok());
        let new_interp = dto
            .interpreted_by
            .as_ref()
            .map(serde_json::to_value)
            .transpose()?;
        // Mismo DTO de entrada (contrato §4): todos los campos que pone el origen.
        let same = indicator == dto.indicator.as_str()
            && old_value.as_ref() == Some(&m.to_value())
            && unit == dto.unit
            && measured_at == dto.measured_at.millis()
            && logged_at == dto.logged_at.millis()
            && source == enum_to_str(&dto.source)?
            && old_interp == new_interp
            && replaces == dto.replaces
            && note == dto.note;
        if !same {
            return Err(err(
                "conflict",
                "record_id",
                "A pulled record has the same id as a different local record",
            ));
        }
        // Los metadatos de servidor ya conocidos no se reescriben.
        if seq.is_some_and(|s| s != c.seq) {
            return Err(err(
                "conflict",
                "seq",
                "A synced record arrived with a different seq",
            ));
        }
        tx.execute(
            "UPDATE records SET seq = ?1, received_at = ?2 WHERE record_id = ?3",
            params![c.seq, received_at.millis(), dto.record_id],
        )?;
        reconcile_outbox(tx, "record", &dto.record_id)?;
        return Ok(0);
    }
    let root: String = match &dto.replaces {
        Some(rep) => tx
            .query_row(
                "SELECT root_id FROM records WHERE record_id = ?1",
                [rep],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or_else(|| rep.clone()),
        None => dto.record_id.clone(),
    };
    Ok(tx.execute(
        "INSERT INTO records(record_id, indicator, body_zone, value_json, unit, measured_at,
             logged_at, source, interpreted_by_json, replaces, root_id, note, seq, received_at, is_demo)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, 0)",
        params![
            dto.record_id,
            dto.indicator.as_str(),
            m.body_zone().map(|z| z.as_str()),
            value_json,
            m.unit(),
            dto.measured_at.millis(),
            dto.logged_at.millis(),
            enum_to_str(&dto.source)?,
            dto.interpreted_by.as_ref().map(serde_json::to_string).transpose()?,
            dto.replaces,
            root,
            dto.note,
            c.seq,
            received_at.millis(),
        ],
    )?)
}

fn pull_event(
    tx: &Transaction<'_>,
    c: &PulledChange,
    received_at: Timestamp,
) -> Result<usize, StoreError> {
    let dto: merken_core::record::EventDto =
        serde_json::from_value(c.payload.clone()).map_err(|_| bad_payload(c))?;
    if dto.event_id != c.id {
        return Err(bad_payload(c));
    }
    type Existing = (
        String,
        String,
        Option<String>,
        Option<String>,
        String,
        i64,
        i64,
        Option<i64>,
    );
    let existing: Option<Existing> = tx
        .query_row(
            "SELECT kind, certainty, message, body_zone, priority, occurred_at, logged_at, seq
             FROM events WHERE event_id = ?1",
            [&dto.event_id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                ))
            },
        )
        .optional()?;
    if let Some((kind, certainty, message, zone, priority, occurred_at, logged_at, seq)) = existing
    {
        let same = kind == dto.kind.as_str()
            && certainty == enum_to_str(&dto.certainty)?
            && message == dto.message
            && zone.as_deref() == dto.body_zone.map(|z| z.as_str())
            && priority == enum_to_str(&dto.priority)?
            && occurred_at == dto.occurred_at.millis()
            && logged_at == dto.logged_at.millis();
        if !same {
            return Err(err(
                "conflict",
                "event_id",
                "A pulled event has the same id as a different local event",
            ));
        }
        if seq.is_some_and(|s| s != c.seq) {
            return Err(err(
                "conflict",
                "seq",
                "A synced event arrived with a different seq",
            ));
        }
        tx.execute(
            "UPDATE events SET seq = ?1, received_at = ?2 WHERE event_id = ?3",
            params![c.seq, received_at.millis(), dto.event_id],
        )?;
        reconcile_outbox(tx, "event", &dto.event_id)?;
        return Ok(0);
    }
    Ok(tx.execute(
        "INSERT INTO events(event_id, kind, certainty, message, body_zone, priority,
             occurred_at, logged_at, seq, received_at, is_demo)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0)",
        params![
            dto.event_id,
            dto.kind.as_str(),
            enum_to_str(&dto.certainty)?,
            dto.message,
            dto.body_zone.map(|z| z.as_str()),
            enum_to_str(&dto.priority)?,
            dto.occurred_at.millis(),
            dto.logged_at.millis(),
            c.seq,
            received_at.millis(),
        ],
    )?)
}

fn set_state(tx: &Transaction<'_>, key: &str, value: &str) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO sync_state(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

fn insert_record(
    tx: &Transaction<'_>,
    dto: &RecordDto,
    zone: Option<BodyZone>,
    root_id: &str,
    is_demo: bool,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO records(record_id, indicator, body_zone, value_json, unit, measured_at,
             logged_at, source, interpreted_by_json, replaces, root_id, note, is_demo)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            dto.record_id,
            dto.indicator.as_str(),
            zone.map(|z| z.as_str()),
            serde_json::to_string(&dto.value)?,
            dto.unit,
            dto.measured_at.millis(),
            dto.logged_at.millis(),
            enum_to_str(&dto.source)?,
            dto.interpreted_by
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?,
            dto.replaces,
            root_id,
            dto.note,
            is_demo,
        ],
    )?;
    Ok(())
}

fn enqueue(
    tx: &Transaction<'_>,
    entity: &str,
    id: &str,
    payload: &Value,
    now: Timestamp,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO outbox(entity, entity_id, payload_json, state, created_at)
         VALUES (?1, ?2, ?3, 'pending', ?4)",
        params![entity, id, serde_json::to_string(payload)?, now.millis()],
    )?;
    Ok(())
}

/// Comprobaciones del contrato que `Measurement::validate` no cubre (revisión
/// de Sol, I3): versión de esquema conocida, identificador razonable, textos
/// dentro de sus límites y fecha no futura.
fn synthetic_dto_ok(dto: &RecordDto, latest: Timestamp) -> bool {
    let id_ok = |id: &str| !id.trim().is_empty() && id.chars().count() <= 64;
    let text_ok = |t: &Option<String>, max: usize| {
        t.as_deref()
            .is_none_or(|t| merken_core::validate_text("text", t, max).is_ok())
    };
    dto.schema_version == 1
        && id_ok(&dto.record_id)
        && dto.external_id.as_deref().is_none_or(id_ok)
        && text_ok(&dto.note, 2000)
        && text_ok(&dto.app_version, 64)
        && dto.measured_at <= latest
        && dto.logged_at <= latest
}
