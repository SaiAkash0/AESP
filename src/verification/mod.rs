use anyhow::Result;
use crate::storage::Storage;
use crate::events;

pub fn verify_entity(
    storage: &Storage,
    qualified_name: &str,
    evidence: Option<&str>,
    new_confidence: Option<f64>,
    agent_id: Option<&str>,
) -> Result<()> {
    let confidence = new_confidence.unwrap_or(0.95);
    storage.with_conn_mut(|conn| {
        conn.execute(
            "UPDATE entities SET verification_status = 'verified', confidence = ?1,
             verified_at = datetime('now'), verified_by = ?2, updated_at = datetime('now')
             WHERE qualified_name = ?3",
            rusqlite::params![confidence, agent_id, qualified_name],
        )?;
        Ok(())
    })?;

    events::emit_event(
        storage,
        "verification_changed",
        "entity",
        qualified_name,
        "verify",
        "agent_write",
        None,
        evidence,
    )?;
    Ok(())
}

pub fn contradict_entity(
    storage: &Storage,
    qualified_name: &str,
    contradicting_fact: Option<&str>,
    evidence: Option<&str>,
) -> Result<()> {
    storage.with_conn_mut(|conn| {
        conn.execute(
            "UPDATE entities SET verification_status = 'contradicted', confidence = 0.1,
             updated_at = datetime('now') WHERE qualified_name = ?1",
            rusqlite::params![qualified_name],
        )?;
        Ok(())
    })?;

    events::emit_event(
        storage,
        "contradiction_detected",
        "entity",
        qualified_name,
        "contradict",
        "agent_write",
        None,
        contradicting_fact.or(evidence),
    )?;
    Ok(())
}

pub fn mark_stale(storage: &Storage, qualified_name: &str) -> Result<()> {
    storage.with_conn_mut(|conn| {
        conn.execute(
            "UPDATE entities SET verification_status = 'stale', updated_at = datetime('now')
             WHERE qualified_name = ?1",
            rusqlite::params![qualified_name],
        )?;
        Ok(())
    })?;

    events::emit_event(
        storage,
        "verification_changed",
        "entity",
        qualified_name,
        "stale",
        "auto_staleness",
        None,
        None,
    )?;
    Ok(())
}

pub fn retract_entity(storage: &Storage, qualified_name: &str, evidence: Option<&str>) -> Result<()> {
    storage.with_conn_mut(|conn| {
        conn.execute(
            "UPDATE entities SET verification_status = 'retracted', confidence = 0.0,
             updated_at = datetime('now') WHERE qualified_name = ?1",
            rusqlite::params![qualified_name],
        )?;
        Ok(())
    })?;

    events::emit_event(
        storage,
        "verification_changed",
        "entity",
        qualified_name,
        "retract",
        "agent_write",
        None,
        evidence,
    )?;
    Ok(())
}

pub fn check_staleness(storage: &Storage, default_ttl: u64) -> Result<u32> {
    let count = storage.with_conn_mut(|conn| {
        let affected = conn.execute(
            "UPDATE entities SET verification_status = 'stale'
             WHERE verification_status = 'verified'
             AND verified_at IS NOT NULL
             AND (julianday('now') - julianday(verified_at)) * 86400 >
                 COALESCE(staleness_ttl, ?1)",
            rusqlite::params![default_ttl as i64],
        )?;
        Ok(affected as u32)
    })?;
    Ok(count)
}
