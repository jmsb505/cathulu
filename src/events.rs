use rusqlite::{params, Transaction};
use serde_json::Value;

pub(crate) fn append_event(
    transaction: &Transaction<'_>,
    aggregate_type: &str,
    aggregate_id: impl ToString,
    event_type: &str,
    payload: &Value,
    created_at: i64,
) -> Result<i64, String> {
    let payload_json = serde_json::to_string(payload)
        .map_err(|error| format!("cannot serialize event payload: {error}"))?;
    if payload_json.len() > 32 * 1024 {
        return Err("event payload exceeds the 32 KiB limit".to_string());
    }
    transaction
        .execute(
            "INSERT INTO events (aggregate_type, aggregate_id, event_type, payload_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![aggregate_type, aggregate_id.to_string(), event_type, payload_json, created_at],
        )
        .map_err(|error| format!("cannot append Cathulu event: {error}"))?;
    Ok(transaction.last_insert_rowid())
}
