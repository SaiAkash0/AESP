use anyhow::Result;
use std::collections::{HashSet, VecDeque};
use crate::storage::Storage;
use super::{Entity, RelationshipInfo};

pub struct TraversalResult {
    pub entities: Vec<(Entity, u32)>,
    pub relationships: Vec<RelationshipInfo>,
}

pub fn bfs_from_entity(
    storage: &Storage,
    start_entity_id: &str,
    max_depth: u32,
) -> Result<TraversalResult> {
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, u32)> = VecDeque::new();
    let mut result_entities: Vec<(Entity, u32)> = Vec::new();
    let mut result_relationships: Vec<RelationshipInfo> = Vec::new();

    queue.push_back((start_entity_id.to_string(), 0));
    visited.insert(start_entity_id.to_string());

    while let Some((entity_id, depth)) = queue.pop_front() {
        if depth > max_depth {
            continue;
        }

        if let Some(entity) = get_entity_by_id(storage, &entity_id)? {
            result_entities.push((entity, depth));
        }

        let rels = super::get_relationships_for_entity(storage, &entity_id)?;
        for rel in &rels {
            result_relationships.push(rel.clone());

            let neighbor_qn = &rel.target_qualified_name;
            if !visited.contains(neighbor_qn) && depth + 1 <= max_depth {
                if let Some(neighbor) = get_entity_by_qn(storage, neighbor_qn)? {
                    visited.insert(neighbor.id.clone());
                    queue.push_back((neighbor.id.clone(), depth + 1));
                }
            }
        }
    }

    Ok(TraversalResult {
        entities: result_entities,
        relationships: result_relationships,
    })
}

fn get_entity_by_id(storage: &Storage, id: &str) -> Result<Option<Entity>> {
    storage.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, entity_type, name, qualified_name, file_path,
                    start_line, end_line, properties, source_type,
                    verification_status, confidence, memory_scope, content_hash,
                    created_at, updated_at
             FROM entities WHERE id = ?1",
        )?;
        let mut rows = stmt.query(rusqlite::params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(super::entity::row_to_entity(row)?))
        } else {
            Ok(None)
        }
    })
}

fn get_entity_by_qn(storage: &Storage, qualified_name: &str) -> Result<Option<Entity>> {
    super::get_entity_by_qualified_name(storage, qualified_name)
}
