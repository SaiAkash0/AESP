use anyhow::Result;
use crate::storage::Storage;
use crate::storage::queries;
use super::{Relationship, RelationshipInfo};

pub fn insert_relationship(storage: &Storage, rel: &Relationship) -> Result<()> {
    storage.with_conn_mut(|conn| {
        conn.execute(
            queries::INSERT_RELATIONSHIP,
            rusqlite::params![
                rel.id,
                rel.source_id,
                rel.target_id,
                rel.relationship_type,
                rel.properties.to_string(),
                rel.source_type,
                rel.verification_status,
                rel.confidence,
                rel.weight,
                rel.memory_scope,
            ],
        )?;
        Ok(())
    })
}

/// Insert relationships in a batch. FK checks are disabled for the duration
/// of the transaction so that any stale entity references from INSERT OR REPLACE
/// deduplication don't cause hard failures. The caller is expected to have
/// already filtered relationships against known entity IDs.
pub fn insert_relationship_batch(storage: &Storage, rels: &[Relationship]) -> Result<()> {
    if rels.is_empty() {
        return Ok(());
    }

    storage.with_conn_mut(|conn| {
        conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(queries::INSERT_RELATIONSHIP)?;
            for rel in rels {
                if let Err(e) = stmt.execute(rusqlite::params![
                    rel.id,
                    rel.source_id,
                    rel.target_id,
                    rel.relationship_type,
                    rel.properties.to_string(),
                    rel.source_type,
                    rel.verification_status,
                    rel.confidence,
                    rel.weight,
                    rel.memory_scope,
                ]) {
                    tracing::debug!(
                        "Skipping relationship insert {} -> {} ({}): {}",
                        rel.source_id, rel.target_id, rel.relationship_type, e
                    );
                }
            }
        }
        tx.commit()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(())
    })
}

pub fn get_relationships_for_entity(
    storage: &Storage,
    entity_id: &str,
) -> Result<Vec<RelationshipInfo>> {
    storage.with_conn(|conn| {
        let mut stmt = conn.prepare(queries::GET_RELATIONSHIPS_FOR_ENTITY)?;
        let mut rows = stmt.query(rusqlite::params![entity_id])?;
        let mut infos = Vec::new();
        while let Some(row) = rows.next()? {
            let source_id: String = row.get(1)?;
            let target_name: String = row.get(10)?;
            let source_name: String = row.get(9)?;
            let rel_type: String = row.get(3)?;

            let (direction, target_qn) = if source_id == entity_id {
                ("outgoing".to_string(), target_name)
            } else {
                ("incoming".to_string(), source_name)
            };

            infos.push(RelationshipInfo {
                relationship_type: rel_type,
                direction,
                target_qualified_name: target_qn,
            });
        }
        Ok(infos)
    })
}
