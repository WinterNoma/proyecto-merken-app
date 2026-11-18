use crate::{Store, StoreError};
use pulsys_core::time::Timestamp;
use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, Serialize)]
pub struct OutboxItem {
    pub outbox_id: i64,
    pub entity: String,
    pub entity_id: String,
    pub payload: Value,
    pub state: String,
    pub attempts: i64,
    pub last_error: Option<String>,
    pub created_at: Timestamp,
}
impl Store {
    pub fn pending_outbox(&self, limit: usize) -> Result<Vec<OutboxItem>, StoreError> {
        let mut stmt=self.conn.prepare("SELECT outbox_id,entity,entity_id,payload_json,state,attempts,last_error,created_at FROM outbox WHERE state='pending' ORDER BY outbox_id LIMIT ?1")?;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = stmt.query_map([limit], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, i64>(7)?,
            ))
        })?;
        let mut result = Vec::new();
        for row in rows {
            let (outbox_id, entity, entity_id, payload, state, attempts, last_error, created_at) =
                row?;
            result.push(OutboxItem {
                outbox_id,
                entity,
                entity_id,
                payload: serde_json::from_str(&payload)?,
                state,
                attempts,
                last_error,
                created_at: Timestamp::from_millis(created_at),
            });
        }
        Ok(result)
    }
    pub fn mark_synced(
        &mut self,
        outbox_id: i64,
        seq: i64,
        received_at: Timestamp,
    ) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        let item: Option<(String, String)> = tx
            .query_row(
                "SELECT entity,entity_id FROM outbox WHERE outbox_id=?1",
                [outbox_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (entity, id) = item.ok_or_else(|| StoreError {
            code: "not_found".into(),
            field: Some("outbox_id".into()),
            message: "Outbox entry not found".into(),
        })?;
        match entity.as_str() {
            "record" => {
                tx.execute(
                    "UPDATE records SET seq=?1, received_at=?2 WHERE record_id=?3",
                    params![seq, received_at.millis(), id],
                )?;
            }
            "event" => {
                tx.execute(
                    "UPDATE events SET seq=?1, received_at=?2 WHERE event_id=?3",
                    params![seq, received_at.millis(), id],
                )?;
            }
            _ => {}
        }
        tx.execute("UPDATE outbox SET state='synced', attempts=attempts+1, last_error=NULL WHERE outbox_id=?1",[outbox_id])?;
        tx.execute("INSERT INTO sync_state(key,value) VALUES('last_push_at',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[received_at.to_rfc3339()])?;
        tx.commit()?;
        Ok(())
    }
    pub fn mark_failed(
        &mut self,
        outbox_id: i64,
        error: &str,
        retryable: bool,
    ) -> Result<(), StoreError> {
        let state = if retryable { "pending" } else { "failed" };
        let n = self.conn.execute(
            "UPDATE outbox SET state=?1, attempts=attempts+1,last_error=?2 WHERE outbox_id=?3",
            params![state, error, outbox_id],
        )?;
        if n == 0 {
            return Err(StoreError {
                code: "not_found".into(),
                field: Some("outbox_id".into()),
                message: "Outbox entry not found".into(),
            });
        }
        Ok(())
    }
}
