use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use uuid::Uuid;
use crate::storage::Storage;
use crate::events;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub id: String,
    pub session_id: String,
    pub task: String,
    pub task_hash: String,
    pub sequence: u32,
    pub attempt_number: u32,
    pub approach: String,
    pub approach_type: Option<String>,
    pub entities_involved: Vec<String>,
    pub changes_made: Vec<serde_json::Value>,
    pub outcome: String,
    pub evidence: Option<String>,
    pub side_effects: Vec<String>,
    pub what_worked: Option<String>,
    pub what_failed: Option<String>,
    pub root_cause: Option<String>,
    pub constraints_discovered: Vec<String>,
    pub recommendations: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
}

pub fn record_decision(
    storage: &Storage,
    session_id: &str,
    task: &str,
    approach: &str,
    approach_type: Option<&str>,
    entities_involved: &[String],
    outcome: &str,
    evidence: Option<&str>,
    what_worked: Option<&str>,
    what_failed: Option<&str>,
    root_cause: Option<&str>,
    recommendations: Option<&str>,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let task_hash = hash_task(task);

    let sequence = get_next_sequence(storage, session_id)?;

    storage.with_conn_mut(|conn| {
        conn.execute(
            crate::storage::queries::INSERT_DECISION,
            rusqlite::params![
                id,
                session_id,
                task,
                task_hash,
                sequence,
                1, // attempt_number
                approach,
                approach_type,
                serde_json::to_string(entities_involved)?,
                "[]", // changes_made
                outcome,
                evidence,
                "[]", // side_effects
                what_worked,
                what_failed,
                root_cause,
                "[]", // constraints_discovered
                recommendations,
                "[]", // tags
            ],
        )?;
        Ok(())
    })?;

    events::emit_event(
        storage,
        "decision_recorded",
        "decision",
        &id,
        "create",
        "agent_write",
        None,
        Some(&serde_json::json!({
            "task": task,
            "outcome": outcome,
        }).to_string()),
    )?;

    Ok(id)
}

pub fn query_decisions(
    storage: &Storage,
    task: Option<&str>,
    outcome: Option<&str>,
    session_id: Option<&str>,
    entity: Option<&str>,
    limit: u32,
) -> Result<Vec<Decision>> {
    storage.with_conn(|conn| {
        let mut conditions = vec!["1=1".to_string()];
        if let Some(sid) = session_id {
            conditions.push(format!("session_id = '{}'", sid));
        }
        if let Some(o) = outcome {
            conditions.push(format!("outcome = '{}'", o));
        }

        let where_clause = conditions.join(" AND ");
        let sql = format!(
            "SELECT id, session_id, task, task_hash, sequence, attempt_number,
                    approach, approach_type, entities_involved, changes_made,
                    outcome, evidence, side_effects, what_worked, what_failed,
                    root_cause, constraints_discovered, recommendations, tags, created_at
             FROM decisions WHERE {} ORDER BY created_at DESC LIMIT {}",
            where_clause, limit
        );

        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query([])?;
        let mut decisions = Vec::new();
        while let Some(row) = rows.next()? {
            decisions.push(row_to_decision(row)?);
        }

        if let Some(task_query) = task {
            decisions.retain(|d| d.task.to_lowercase().contains(&task_query.to_lowercase()));
        }
        if let Some(entity_qn) = entity {
            decisions.retain(|d| d.entities_involved.iter().any(|e| e.contains(entity_qn)));
        }

        Ok(decisions)
    })
}

fn row_to_decision(row: &rusqlite::Row) -> Result<Decision> {
    Ok(Decision {
        id: row.get(0)?,
        session_id: row.get(1)?,
        task: row.get(2)?,
        task_hash: row.get(3)?,
        sequence: row.get::<_, i32>(4)? as u32,
        attempt_number: row.get::<_, i32>(5)? as u32,
        approach: row.get(6)?,
        approach_type: row.get(7)?,
        entities_involved: serde_json::from_str(&row.get::<_, String>(8)?).unwrap_or_default(),
        changes_made: serde_json::from_str(&row.get::<_, String>(9)?).unwrap_or_default(),
        outcome: row.get(10)?,
        evidence: row.get(11)?,
        side_effects: serde_json::from_str(&row.get::<_, String>(12)?).unwrap_or_default(),
        what_worked: row.get(13)?,
        what_failed: row.get(14)?,
        root_cause: row.get(15)?,
        constraints_discovered: serde_json::from_str(&row.get::<_, String>(16)?).unwrap_or_default(),
        recommendations: row.get(17)?,
        tags: serde_json::from_str(&row.get::<_, String>(18)?).unwrap_or_default(),
        created_at: row.get(19)?,
    })
}

fn hash_task(task: &str) -> String {
    let normalized = task.to_lowercase().trim().to_string();
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn get_next_sequence(storage: &Storage, session_id: &str) -> Result<u32> {
    storage.with_conn(|conn| {
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM decisions WHERE session_id = ?1",
            rusqlite::params![session_id],
            |r| r.get(0),
        )?;
        Ok(count as u32 + 1)
    })
}
