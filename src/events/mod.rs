use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::storage::Storage;
use crate::storage::queries;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateEvent {
    pub id: String,
    pub event_type: String,
    pub timestamp: String,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
    pub target_type: String,
    pub target_id: String,
    pub operation: String,
    pub before_state: Option<String>,
    pub after_state: Option<String>,
    pub trigger_source: String,
    pub related_events: Vec<String>,
    pub metadata: serde_json::Value,
}

pub fn emit_event(
    storage: &Storage,
    event_type: &str,
    target_type: &str,
    target_id: &str,
    operation: &str,
    trigger_source: &str,
    before_state: Option<&str>,
    after_state: Option<&str>,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    storage.with_conn_mut(|conn| {
        conn.execute(
            queries::INSERT_EVENT,
            rusqlite::params![
                id,
                event_type,
                Option::<String>::None, // session_id
                Option::<String>::None, // agent_id
                target_type,
                target_id,
                operation,
                before_state,
                after_state,
                trigger_source,
                "[]",
                "{}",
            ],
        )?;
        Ok(id.clone())
    })
}

pub fn emit_entity_created(storage: &Storage, entity: &crate::graph::Entity) -> Result<String> {
    let after_json = serde_json::to_string(entity)?;
    emit_event(
        storage,
        "entity_created",
        "entity",
        &entity.qualified_name,
        "create",
        if entity.source_type == "indexed" { "reindex" } else { "agent_write" },
        None,
        Some(&after_json),
    )
}

pub fn query_timeline(storage: &Storage, limit: u32) -> Result<Vec<StateEvent>> {
    query_events_raw(storage, queries::TIMELINE_EVENTS, &[], limit)
}

pub fn query_by_target(storage: &Storage, target_id: &str, limit: u32) -> Result<Vec<StateEvent>> {
    storage.with_conn(|conn| {
        let mut stmt = conn.prepare(queries::EVENTS_BY_TARGET)?;
        let mut rows = stmt.query(rusqlite::params![target_id, limit])?;
        let mut events = Vec::new();
        while let Some(row) = rows.next()? {
            events.push(row_to_event(row)?);
        }
        Ok(events)
    })
}

pub fn query_by_session(storage: &Storage, session_id: &str, limit: u32) -> Result<Vec<StateEvent>> {
    storage.with_conn(|conn| {
        let mut stmt = conn.prepare(queries::EVENTS_BY_SESSION)?;
        let mut rows = stmt.query(rusqlite::params![session_id, limit])?;
        let mut events = Vec::new();
        while let Some(row) = rows.next()? {
            events.push(row_to_event(row)?);
        }
        Ok(events)
    })
}

pub fn query_by_type(storage: &Storage, event_type: &str, limit: u32) -> Result<Vec<StateEvent>> {
    storage.with_conn(|conn| {
        let mut stmt = conn.prepare(queries::EVENTS_BY_TYPE)?;
        let mut rows = stmt.query(rusqlite::params![event_type, limit])?;
        let mut events = Vec::new();
        while let Some(row) = rows.next()? {
            events.push(row_to_event(row)?);
        }
        Ok(events)
    })
}

fn query_events_raw(storage: &Storage, sql: &str, _params: &[&str], limit: u32) -> Result<Vec<StateEvent>> {
    storage.with_conn(|conn| {
        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query(rusqlite::params![limit])?;
        let mut events = Vec::new();
        while let Some(row) = rows.next()? {
            events.push(row_to_event(row)?);
        }
        Ok(events)
    })
}

fn row_to_event(row: &rusqlite::Row) -> Result<StateEvent> {
    let related_str: String = row.get(11)?;
    let metadata_str: String = row.get(12)?;

    Ok(StateEvent {
        id: row.get(0)?,
        event_type: row.get(1)?,
        timestamp: row.get(2)?,
        session_id: row.get(3)?,
        agent_id: row.get(4)?,
        target_type: row.get(5)?,
        target_id: row.get(6)?,
        operation: row.get(7)?,
        before_state: row.get(8)?,
        after_state: row.get(9)?,
        trigger_source: row.get(10)?,
        related_events: serde_json::from_str(&related_str).unwrap_or_default(),
        metadata: serde_json::from_str(&metadata_str).unwrap_or(serde_json::json!({})),
    })
}
