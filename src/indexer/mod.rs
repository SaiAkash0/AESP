use anyhow::Result;
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;

use crate::config::AespConfig;
use crate::graph::{Entity, Relationship};
use crate::parser;
use crate::schema::Schema;
use crate::storage::Storage;

#[derive(Debug, Default)]
pub struct IndexStats {
    pub files_indexed: u64,
    pub entities_created: u64,
    pub relationships_created: u64,
    pub relationships_skipped: u64,
    pub files_skipped: u64,
    pub entities_deduped: u64,
}

pub fn index_project(
    project_root: &Path,
    storage: &Storage,
    _schema: &Schema,
    config: &AespConfig,
) -> Result<IndexStats> {
    let mut stats = IndexStats::default();

    // Accumulate raw parsed data: entities keyed by qualified_name for dedup,
    // and a flat vec of relationships with the entity UUIDs from parsing.
    let mut entity_map: HashMap<String, Entity> = HashMap::new();
    let mut raw_relationships: Vec<Relationship> = Vec::new();

    // Maps entity UUID -> qualified_name so we can remap relationship endpoints
    // after deduplication collapses multiple UUIDs to the surviving one.
    let mut id_to_qn: HashMap<String, String> = HashMap::new();

    let project_entity = Entity {
        id: uuid::Uuid::new_v4().to_string(),
        entity_type: "project".to_string(),
        name: config.project.name.clone(),
        qualified_name: ".".to_string(),
        file_path: None,
        start_line: None,
        end_line: None,
        properties: serde_json::json!({
            "root_path": project_root.to_string_lossy(),
            "language_primary": config.project.languages.first().unwrap_or(&"unknown".to_string()),
            "languages": config.project.languages,
        }),
        source_type: "indexed".to_string(),
        verification_status: "unverified".to_string(),
        confidence: 1.0,
        memory_scope: "persistent".to_string(),
        content_hash: None,
        created_at: String::new(),
        updated_at: String::new(),
    };
    id_to_qn.insert(project_entity.id.clone(), project_entity.qualified_name.clone());
    entity_map.insert(project_entity.qualified_name.clone(), project_entity);

    // ── Pass 0: Walk all files, parse, collect everything ──
    for entry in WalkDir::new(project_root)
        .follow_links(config.indexing.follow_symlinks)
        .into_iter()
        .filter_entry(|e| !should_ignore(e.path(), project_root))
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();

        if !parser::languages::is_parseable(path) {
            continue;
        }

        let metadata = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => { stats.files_skipped += 1; continue; }
        };
        if metadata.len() > config.indexing.max_file_size_kb * 1024 {
            stats.files_skipped += 1;
            continue;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => {
                stats.files_skipped += 1;
                continue;
            }
        };

        let hash = file_content_hash(&content);
        let relative_path = path
            .strip_prefix(project_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        let existing_hash = get_file_hash(storage, &relative_path);
        if let Some(existing) = existing_hash {
            if existing == hash {
                stats.files_skipped += 1;
                continue;
            }
        }

        match parser::parse_file(path, project_root) {
            Ok(result) => {
                update_file_hash(storage, &relative_path, &hash, result.entities.len() as u32)?;

                for entity in result.entities {
                    id_to_qn.insert(entity.id.clone(), entity.qualified_name.clone());

                    if entity_map.contains_key(&entity.qualified_name) {
                        stats.entities_deduped += 1;
                    }
                    // Last-writer-wins: later files overwrite earlier ones for the same qn
                    entity_map.insert(entity.qualified_name.clone(), entity);
                }

                raw_relationships.extend(result.relationships);
                stats.files_indexed += 1;
            }
            Err(e) => {
                tracing::warn!("Failed to parse {}: {}", path.display(), e);
                stats.files_skipped += 1;
            }
        }
    }

    // ── Build the deduped entity list and a qn->surviving_id map ──
    let deduped_entities: Vec<Entity> = entity_map.values().cloned().collect();
    let qn_to_id: HashMap<String, String> = deduped_entities
        .iter()
        .map(|e| (e.qualified_name.clone(), e.id.clone()))
        .collect();

    stats.entities_created = deduped_entities.len() as u64;

    // ── Pass 1: Insert ALL entities ──
    crate::graph::insert_entity_batch(storage, &deduped_entities)?;

    // Query the DB for the actual set of entity IDs that survived insertion.
    // This is the ground truth — no stale UUIDs from replaced rows.
    let db_entity_ids = crate::graph::get_all_entity_ids(storage)?;

    // ── Pass 2: Remap & insert relationships ──
    let mut valid_relationships: Vec<Relationship> = Vec::new();
    for mut rel in raw_relationships {
        // Remap source: UUID -> qn -> surviving UUID
        rel.source_id = remap_id(&rel.source_id, &id_to_qn, &qn_to_id);
        // Remap target: UUID -> qn -> surviving UUID
        // For "calls" relationships, target_id is already a qualified_name, so
        // try qn_to_id directly as well.
        rel.target_id = remap_id(&rel.target_id, &id_to_qn, &qn_to_id);

        let source_exists = db_entity_ids.contains(&rel.source_id);
        let target_exists = db_entity_ids.contains(&rel.target_id);

        if source_exists && target_exists {
            valid_relationships.push(rel);
        } else {
            tracing::debug!(
                "Skipping relationship {} -> {} (type: {}) — external dependency or missing entity",
                rel.source_id,
                rel.target_id,
                rel.relationship_type
            );
            stats.relationships_skipped += 1;
        }
    }

    stats.relationships_created = valid_relationships.len() as u64;
    crate::graph::insert_relationship_batch(storage, &valid_relationships)?;

    crate::events::emit_event(
        storage,
        "reindex_completed",
        "project",
        ".",
        "create",
        "reindex",
        None,
        Some(&serde_json::json!({
            "files_indexed": stats.files_indexed,
            "entities_created": stats.entities_created,
            "relationships_created": stats.relationships_created,
            "relationships_skipped": stats.relationships_skipped,
            "entities_deduped": stats.entities_deduped,
        }).to_string()),
    )?;

    Ok(stats)
}

pub fn index_path(
    project_root: &Path,
    target: &Path,
    storage: &Storage,
    schema: &Schema,
    config: &AespConfig,
) -> Result<IndexStats> {
    let full_path = if target.is_absolute() {
        target.to_path_buf()
    } else {
        project_root.join(target)
    };

    if full_path.is_file() {
        index_single_file(project_root, &full_path, storage, config)
    } else {
        index_project(&full_path, storage, schema, config)
    }
}

fn index_single_file(
    project_root: &Path,
    file_path: &Path,
    storage: &Storage,
    _config: &AespConfig,
) -> Result<IndexStats> {
    let mut stats = IndexStats::default();

    let relative_path = file_path
        .strip_prefix(project_root)
        .unwrap_or(file_path)
        .to_string_lossy()
        .replace('\\', "/");

    crate::graph::delete_entities_by_file(storage, &relative_path)?;

    match parser::parse_file(file_path, project_root) {
        Ok(result) => {
            let content = std::fs::read_to_string(file_path)?;
            let hash = file_content_hash(&content);
            update_file_hash(storage, &relative_path, &hash, result.entities.len() as u32)?;

            // Pass 1: entities
            crate::graph::insert_entity_batch(storage, &result.entities)?;
            stats.entities_created = result.entities.len() as u64;

            // Pass 2: relationships — check against the DB for ground truth
            let db_ids = crate::graph::get_all_entity_ids(storage)?;
            let mut valid_rels = Vec::new();
            for rel in &result.relationships {
                if db_ids.contains(&rel.source_id) && db_ids.contains(&rel.target_id) {
                    valid_rels.push(rel.clone());
                } else {
                    tracing::debug!(
                        "Skipping relationship {} -> {} (type: {}) — external dependency or missing entity",
                        rel.source_id, rel.target_id, rel.relationship_type
                    );
                    stats.relationships_skipped += 1;
                }
            }

            crate::graph::insert_relationship_batch(storage, &valid_rels)?;
            stats.relationships_created = valid_rels.len() as u64;
            stats.files_indexed = 1;
        }
        Err(e) => {
            tracing::warn!("Failed to parse {}: {}", file_path.display(), e);
            stats.files_skipped = 1;
        }
    }

    Ok(stats)
}

fn should_ignore(path: &Path, project_root: &Path) -> bool {
    let relative = path.strip_prefix(project_root).unwrap_or(path);

    for component in relative.components() {
        let name = component.as_os_str().to_string_lossy();
        for ignored in crate::config::BUILTIN_IGNORE_DIRS {
            if name == *ignored {
                return true;
            }
        }
    }

    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        for ignored_ext in crate::config::BUILTIN_IGNORE_EXTENSIONS {
            if ext == *ignored_ext {
                return true;
            }
        }
    }

    false
}

fn file_content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn get_file_hash(storage: &Storage, file_path: &str) -> Option<String> {
    storage
        .with_conn(|conn| {
            let mut stmt = conn.prepare(crate::storage::queries::GET_FILE_HASH)?;
            let result: Result<String, _> = stmt.query_row(rusqlite::params![file_path], |r| r.get(0));
            Ok(result.ok())
        })
        .ok()
        .flatten()
}

fn update_file_hash(
    storage: &Storage,
    file_path: &str,
    hash: &str,
    entity_count: u32,
) -> Result<()> {
    storage.with_conn_mut(|conn| {
        conn.execute(
            crate::storage::queries::INSERT_FILE_HASH,
            rusqlite::params![file_path, hash, entity_count],
        )?;
        Ok(())
    })
}

/// Resolve an ID through the dedup chain:
/// 1. If it's a UUID in id_to_qn, map UUID -> qn -> surviving_id
/// 2. If it's already a qualified_name in qn_to_id, map directly to surviving_id
/// 3. Otherwise, return it unchanged (may be an external ref that gets filtered later)
fn remap_id(
    id: &str,
    id_to_qn: &HashMap<String, String>,
    qn_to_id: &HashMap<String, String>,
) -> String {
    // Path 1: UUID -> qn -> surviving UUID
    if let Some(qn) = id_to_qn.get(id) {
        if let Some(surviving) = qn_to_id.get(qn) {
            return surviving.clone();
        }
    }
    // Path 2: id is itself a qualified_name (used by "calls" relationships)
    if let Some(surviving) = qn_to_id.get(id) {
        return surviving.clone();
    }
    // Path 3: not resolvable, return as-is
    id.to_string()
}
