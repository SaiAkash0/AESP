use anyhow::Result;
use crate::storage::Storage;
use crate::storage::queries;
use super::{QueryResult, GraphStatus, Entity};
use super::entity::row_to_entity;

pub fn query_entities(
    storage: &Storage,
    query: &str,
    type_filter: Option<&str>,
    _depth: u32,
    trust_filter: &str,
    max_results: u32,
) -> Result<Vec<QueryResult>> {
    let scored_entities = if let Some(entity_type) = type_filter {
        search_by_type(storage, entity_type, max_results)?
    } else {
        search_fts_scored(storage, query, max_results)?
    };

    let mut results = Vec::new();
    for (entity, score) in scored_entities {
        if !passes_trust_filter(&entity, trust_filter) {
            continue;
        }

        let relationships =
            super::get_relationships_for_entity(storage, &entity.id)?;
        let annotations =
            super::get_annotations_for_entity(storage, &entity.id)?;

        let signature = extract_signature(&entity);

        results.push(QueryResult {
            id: entity.id,
            entity_type: entity.entity_type,
            name: entity.name.clone(),
            qualified_name: entity.qualified_name,
            file_path: entity.file_path,
            start_line: entity.start_line,
            end_line: entity.end_line,
            verification_status: entity.verification_status,
            confidence: entity.confidence,
            signature,
            relevance_score: score,
            relationships,
            annotations,
        });
    }

    results.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap_or(std::cmp::Ordering::Equal));
    Ok(results)
}

/// FTS5 search with BM25 relevance scoring and LIKE fallback.
fn search_fts_scored(storage: &Storage, query: &str, limit: u32) -> Result<Vec<(Entity, f64)>> {
    if query.is_empty() {
        let entities = storage.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, entity_type, name, qualified_name, file_path,
                        start_line, end_line, properties, source_type,
                        verification_status, confidence, memory_scope, content_hash,
                        created_at, updated_at
                 FROM entities ORDER BY updated_at DESC LIMIT ?1"
            )?;
            let mut rows = stmt.query(rusqlite::params![limit])?;
            let mut out = Vec::new();
            while let Some(row) = rows.next()? {
                out.push((row_to_entity(row)?, 0.5));
            }
            Ok(out)
        })?;
        return Ok(entities);
    }

    let fts_query = query
        .split_whitespace()
        .map(|w| {
            let clean = w.replace('"', "").replace('\'', "");
            format!("\"{}\" OR {}*", clean, clean)
        })
        .collect::<Vec<_>>()
        .join(" OR ");

    let fts_result: Result<Vec<(Entity, f64)>> = storage.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT e.id, e.entity_type, e.name, e.qualified_name, e.file_path,
                    e.start_line, e.end_line, e.properties, e.source_type,
                    e.verification_status, e.confidence, e.memory_scope, e.content_hash,
                    e.created_at, e.updated_at,
                    -rank as relevance
             FROM entities e
             JOIN entities_fts f ON e.rowid = f.rowid
             WHERE entities_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2"
        )?;
        let mut rows = stmt.query(rusqlite::params![fts_query, limit])?;
        let mut entities = Vec::new();
        while let Some(row) = rows.next()? {
            let entity = row_to_entity(row)?;
            let raw_score: f64 = row.get(15)?;
            entities.push((entity, raw_score));
        }
        Ok(entities)
    });

    match fts_result {
        Ok(entities) if !entities.is_empty() => {
            Ok(normalize_scores(entities))
        }
        _ => {
            let like_pattern = format!("%{}%", query);
            storage.with_conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, entity_type, name, qualified_name, file_path,
                            start_line, end_line, properties, source_type,
                            verification_status, confidence, memory_scope, content_hash,
                            created_at, updated_at
                     FROM entities
                     WHERE name LIKE ?1 OR qualified_name LIKE ?1 OR properties LIKE ?1
                     LIMIT ?2"
                )?;
                let mut rows = stmt.query(rusqlite::params![like_pattern, limit])?;
                let mut entities = Vec::new();
                while let Some(row) = rows.next()? {
                    entities.push((row_to_entity(row)?, 0.3));
                }
                Ok(entities)
            })
        }
    }
}

fn normalize_scores(entities: Vec<(Entity, f64)>) -> Vec<(Entity, f64)> {
    if entities.is_empty() {
        return entities;
    }
    let max_score = entities.iter().map(|(_, s)| *s).fold(f64::NEG_INFINITY, f64::max);
    let min_score = entities.iter().map(|(_, s)| *s).fold(f64::INFINITY, f64::min);
    let range = max_score - min_score;

    entities
        .into_iter()
        .map(|(e, score)| {
            let norm = if range > 0.001 {
                0.3 + 0.7 * ((score - min_score) / range)
            } else {
                1.0
            };
            (e, norm)
        })
        .collect()
}

fn search_by_type(storage: &Storage, entity_type: &str, limit: u32) -> Result<Vec<(Entity, f64)>> {
    storage.with_conn(|conn| {
        let mut stmt = conn.prepare(queries::GET_ENTITIES_BY_TYPE)?;
        let mut rows = stmt.query(rusqlite::params![entity_type, limit])?;
        let mut entities = Vec::new();
        while let Some(row) = rows.next()? {
            entities.push((row_to_entity(row)?, 0.5));
        }
        Ok(entities)
    })
}

fn passes_trust_filter(entity: &Entity, filter: &str) -> bool {
    match filter {
        "verified_only" => entity.verification_status == "verified",
        "exclude_stale" => {
            entity.verification_status != "stale" && entity.verification_status != "retracted"
        }
        "exclude_retracted" => entity.verification_status != "retracted",
        _ => true,
    }
}

fn extract_signature(entity: &Entity) -> Option<String> {
    entity
        .properties
        .get("signature")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

pub fn get_status(storage: &Storage) -> Result<GraphStatus> {
    storage.with_conn(|conn| {
        let total_entities: u64 =
            conn.query_row(queries::COUNT_ENTITIES, [], |r| r.get(0))?;
        let total_relationships: u64 =
            conn.query_row(queries::COUNT_RELATIONSHIPS, [], |r| r.get(0))?;
        let total_annotations: u64 =
            conn.query_row(queries::COUNT_ANNOTATIONS, [], |r| r.get(0))?;
        let total_decisions: u64 =
            conn.query_row(queries::COUNT_DECISIONS, [], |r| r.get(0))?;
        let total_events: u64 =
            conn.query_row(queries::COUNT_EVENTS, [], |r| r.get(0))?;
        let active_constraints: u64 =
            conn.query_row(queries::COUNT_ACTIVE_CONSTRAINTS, [], |r| r.get(0))?;

        let mut by_type_stmt = conn.prepare(queries::ENTITIES_BY_TYPE_COUNT)?;
        let entities_by_type: Vec<(String, u64)> = by_type_stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();

        let mut by_ver_stmt = conn.prepare(queries::ENTITIES_BY_VERIFICATION_COUNT)?;
        let entities_by_verification: Vec<(String, u64)> = by_ver_stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(GraphStatus {
            total_entities,
            total_relationships,
            total_annotations,
            total_decisions,
            total_events,
            active_constraints,
            entities_by_type,
            entities_by_verification,
        })
    })
}
