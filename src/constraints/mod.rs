use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::storage::Storage;
use crate::events;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    pub id: String,
    pub rule: String,
    pub scope: String,
    pub session_id: Option<String>,
    pub severity: String,
    pub category: String,
    pub created_by: String,
    pub active: bool,
    pub created_at: String,
}

pub fn add_constraint(
    storage: &Storage,
    rule: &str,
    scope: &str,
    severity: &str,
    category: &str,
    created_by: &str,
    session_id: Option<&str>,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    storage.with_conn_mut(|conn| {
        conn.execute(
            crate::storage::queries::INSERT_CONSTRAINT,
            rusqlite::params![id, rule, scope, session_id, severity, category, created_by],
        )?;
        Ok(())
    })?;

    events::emit_event(
        storage,
        "constraint_added",
        "constraint",
        &id,
        "create",
        "agent_write",
        None,
        Some(rule),
    )?;

    Ok(id)
}

pub fn remove_constraint(storage: &Storage, constraint_id: &str) -> Result<()> {
    storage.with_conn_mut(|conn| {
        conn.execute(
            "UPDATE constraints SET active = 0, deactivated_at = datetime('now') WHERE id = ?1",
            rusqlite::params![constraint_id],
        )?;
        Ok(())
    })?;

    events::emit_event(
        storage,
        "constraint_removed",
        "constraint",
        constraint_id,
        "delete",
        "agent_write",
        None,
        None,
    )?;

    Ok(())
}

pub fn list_active_constraints(storage: &Storage, session_id: Option<&str>) -> Result<Vec<Constraint>> {
    storage.with_conn(|conn| {
        let sql = if let Some(sid) = session_id {
            format!(
                "SELECT id, rule, scope, session_id, severity, category, created_by, active, created_at
                 FROM constraints WHERE active = 1 AND (scope = 'persistent' OR session_id = '{}')
                 ORDER BY created_at",
                sid
            )
        } else {
            "SELECT id, rule, scope, session_id, severity, category, created_by, active, created_at
             FROM constraints WHERE active = 1
             ORDER BY created_at".to_string()
        };

        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query([])?;
        let mut constraints = Vec::new();
        while let Some(row) = rows.next()? {
            let active_int: i32 = row.get(7)?;
            constraints.push(Constraint {
                id: row.get(0)?,
                rule: row.get(1)?,
                scope: row.get(2)?,
                session_id: row.get(3)?,
                severity: row.get(4)?,
                category: row.get(5)?,
                created_by: row.get(6)?,
                active: active_int != 0,
                created_at: row.get(8)?,
            });
        }
        Ok(constraints)
    })
}

pub fn deactivate_session_constraints(storage: &Storage, session_id: &str) -> Result<u32> {
    let count = storage.with_conn_mut(|conn| {
        let affected = conn.execute(
            "UPDATE constraints SET active = 0, deactivated_at = datetime('now')
             WHERE scope = 'session' AND session_id = ?1 AND active = 1",
            rusqlite::params![session_id],
        )?;
        Ok(affected as u32)
    })?;
    Ok(count)
}
