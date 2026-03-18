use anyhow::Result;
use crate::storage::Storage;
use crate::storage::queries;
use crate::events;
use super::Entity;

pub fn insert_entity(storage: &Storage, entity: &Entity) -> Result<()> {
    storage.with_conn_mut(|conn| {
        conn.execute(
            queries::INSERT_ENTITY,
            rusqlite::params![
                entity.id,
                entity.entity_type,
                entity.name,
                entity.qualified_name,
                entity.file_path,
                entity.start_line,
                entity.end_line,
                entity.properties.to_string(),
                entity.source_type,
                entity.verification_status,
                entity.confidence,
                Option::<u64>::None,
                entity.memory_scope,
                entity.content_hash,
            ],
        )?;
        Ok(())
    })?;

    events::emit_entity_created(storage, entity)?;
    Ok(())
}

pub fn insert_entity_batch(storage: &Storage, entities: &[Entity]) -> Result<()> {
    storage.with_conn_mut(|conn| {
        conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(queries::INSERT_ENTITY)?;
            for entity in entities {
                stmt.execute(rusqlite::params![
                    entity.id,
                    entity.entity_type,
                    entity.name,
                    entity.qualified_name,
                    entity.file_path,
                    entity.start_line,
                    entity.end_line,
                    entity.properties.to_string(),
                    entity.source_type,
                    entity.verification_status,
                    entity.confidence,
                    Option::<u64>::None,
                    entity.memory_scope,
                    entity.content_hash,
                ])?;
            }
        }
        tx.commit()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(())
    })
}

pub fn get_entity_by_qualified_name(storage: &Storage, qualified_name: &str) -> Result<Option<Entity>> {
    storage.with_conn(|conn| {
        let mut stmt = conn.prepare(queries::GET_ENTITY_BY_QUALIFIED_NAME)?;
        let mut rows = stmt.query(rusqlite::params![qualified_name])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_entity(row)?))
        } else {
            Ok(None)
        }
    })
}

pub fn get_entities_by_file(storage: &Storage, file_path: &str) -> Result<Vec<Entity>> {
    storage.with_conn(|conn| {
        let mut stmt = conn.prepare(queries::GET_ENTITIES_BY_FILE)?;
        let mut rows = stmt.query(rusqlite::params![file_path])?;
        let mut entities = Vec::new();
        while let Some(row) = rows.next()? {
            entities.push(row_to_entity(row)?);
        }
        Ok(entities)
    })
}

pub fn delete_entities_by_file(storage: &Storage, file_path: &str) -> Result<u64> {
    storage.with_conn_mut(|conn| {
        let count = conn.execute(queries::DELETE_ENTITIES_BY_FILE, rusqlite::params![file_path])?;
        Ok(count as u64)
    })
}

/// Query the DB for all entity IDs that actually exist after batch insert.
/// Returns a set of (id) for fast lookup.
pub fn get_all_entity_ids(storage: &Storage) -> Result<std::collections::HashSet<String>> {
    storage.with_conn(|conn| {
        let mut stmt = conn.prepare("SELECT id FROM entities")?;
        let mut rows = stmt.query([])?;
        let mut ids = std::collections::HashSet::new();
        while let Some(row) = rows.next()? {
            ids.insert(row.get::<_, String>(0)?);
        }
        Ok(ids)
    })
}

pub fn row_to_entity(row: &rusqlite::Row) -> Result<Entity> {
    let props_str: String = row.get(7)?;
    let properties: serde_json::Value =
        serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));

    Ok(Entity {
        id: row.get(0)?,
        entity_type: row.get(1)?,
        name: row.get(2)?,
        qualified_name: row.get(3)?,
        file_path: row.get(4)?,
        start_line: row.get::<_, Option<i32>>(5)?.map(|v| v as u32),
        end_line: row.get::<_, Option<i32>>(6)?.map(|v| v as u32),
        properties,
        source_type: row.get(8)?,
        verification_status: row.get(9)?,
        confidence: row.get(10)?,
        memory_scope: row.get(11)?,
        content_hash: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}
