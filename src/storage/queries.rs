pub const INSERT_ENTITY: &str = r#"
    INSERT OR REPLACE INTO entities
        (id, entity_type, name, qualified_name, file_path, start_line, end_line,
         properties, source_type, verification_status, confidence, staleness_ttl,
         memory_scope, content_hash, created_at, updated_at)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
            COALESCE((SELECT created_at FROM entities WHERE qualified_name = ?4), datetime('now')),
            datetime('now'))
"#;

pub const INSERT_RELATIONSHIP: &str = r#"
    INSERT OR IGNORE INTO relationships
        (id, source_id, target_id, relationship_type, properties,
         source_type, verification_status, confidence, weight, memory_scope)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
"#;

pub const INSERT_ANNOTATION: &str = r#"
    INSERT INTO annotations
        (id, entity_id, annotation_type, content, author, tags,
         source_type, verification_status, confidence, memory_scope, session_id)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
"#;

pub const INSERT_DECISION: &str = r#"
    INSERT INTO decisions
        (id, session_id, task, task_hash, sequence, attempt_number,
         approach, approach_type, entities_involved, changes_made,
         outcome, evidence, side_effects, what_worked, what_failed,
         root_cause, constraints_discovered, recommendations, tags)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
"#;

pub const INSERT_EVENT: &str = r#"
    INSERT INTO state_events
        (id, event_type, session_id, agent_id, target_type, target_id,
         operation, before_state, after_state, trigger_source, related_events, metadata)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
"#;

pub const INSERT_CONSTRAINT: &str = r#"
    INSERT INTO constraints
        (id, rule, scope, session_id, severity, category, created_by)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
"#;

pub const INSERT_FILE_HASH: &str = r#"
    INSERT OR REPLACE INTO file_hashes (file_path, content_hash, last_indexed, entity_count)
    VALUES (?1, ?2, datetime('now'), ?3)
"#;

pub const SEARCH_ENTITIES_FTS: &str = r#"
    SELECT e.id, e.entity_type, e.name, e.qualified_name, e.file_path,
           e.start_line, e.end_line, e.properties, e.source_type,
           e.verification_status, e.confidence, e.memory_scope, e.content_hash,
           e.created_at, e.updated_at
    FROM entities e
    JOIN entities_fts f ON e.rowid = f.rowid
    WHERE entities_fts MATCH ?1
    ORDER BY rank
    LIMIT ?2
"#;

pub const GET_ENTITY_BY_QUALIFIED_NAME: &str = r#"
    SELECT id, entity_type, name, qualified_name, file_path,
           start_line, end_line, properties, source_type,
           verification_status, confidence, memory_scope, content_hash,
           created_at, updated_at
    FROM entities
    WHERE qualified_name = ?1
"#;

pub const GET_ENTITIES_BY_TYPE: &str = r#"
    SELECT id, entity_type, name, qualified_name, file_path,
           start_line, end_line, properties, source_type,
           verification_status, confidence, memory_scope, content_hash,
           created_at, updated_at
    FROM entities
    WHERE entity_type = ?1
    LIMIT ?2
"#;

pub const GET_ENTITIES_BY_FILE: &str = r#"
    SELECT id, entity_type, name, qualified_name, file_path,
           start_line, end_line, properties, source_type,
           verification_status, confidence, memory_scope, content_hash,
           created_at, updated_at
    FROM entities
    WHERE file_path = ?1
    ORDER BY start_line
"#;

pub const GET_RELATIONSHIPS_FOR_ENTITY: &str = r#"
    SELECT r.id, r.source_id, r.target_id, r.relationship_type, r.properties,
           r.source_type, r.verification_status, r.confidence, r.weight,
           e_src.qualified_name as source_name,
           e_tgt.qualified_name as target_name
    FROM relationships r
    JOIN entities e_src ON r.source_id = e_src.id
    JOIN entities e_tgt ON r.target_id = e_tgt.id
    WHERE r.source_id = ?1 OR r.target_id = ?1
"#;

pub const GET_ANNOTATIONS_FOR_ENTITY: &str = r#"
    SELECT id, entity_id, annotation_type, content, author, tags,
           source_type, verification_status, confidence, resolved,
           memory_scope, created_at
    FROM annotations
    WHERE entity_id = ?1
    ORDER BY created_at DESC
"#;

pub const COUNT_ENTITIES: &str = "SELECT COUNT(*) FROM entities";
pub const COUNT_RELATIONSHIPS: &str = "SELECT COUNT(*) FROM relationships";
pub const COUNT_ANNOTATIONS: &str = "SELECT COUNT(*) FROM annotations";
pub const COUNT_DECISIONS: &str = "SELECT COUNT(*) FROM decisions";
pub const COUNT_EVENTS: &str = "SELECT COUNT(*) FROM state_events";
pub const COUNT_ACTIVE_CONSTRAINTS: &str = "SELECT COUNT(*) FROM constraints WHERE active = 1";

pub const ENTITIES_BY_TYPE_COUNT: &str = r#"
    SELECT entity_type, COUNT(*) as cnt FROM entities GROUP BY entity_type ORDER BY cnt DESC
"#;

pub const ENTITIES_BY_VERIFICATION_COUNT: &str = r#"
    SELECT verification_status, COUNT(*) as cnt FROM entities GROUP BY verification_status ORDER BY cnt DESC
"#;

pub const DELETE_ENTITIES_BY_FILE: &str = "DELETE FROM entities WHERE file_path = ?1";

pub const GET_FILE_HASH: &str = "SELECT content_hash FROM file_hashes WHERE file_path = ?1";

pub const TIMELINE_EVENTS: &str = r#"
    SELECT id, event_type, timestamp, session_id, agent_id,
           target_type, target_id, operation, before_state, after_state,
           trigger_source, related_events, metadata
    FROM state_events
    ORDER BY timestamp DESC
    LIMIT ?1
"#;

pub const EVENTS_BY_TARGET: &str = r#"
    SELECT id, event_type, timestamp, session_id, agent_id,
           target_type, target_id, operation, before_state, after_state,
           trigger_source, related_events, metadata
    FROM state_events
    WHERE target_id = ?1
    ORDER BY timestamp DESC
    LIMIT ?2
"#;

pub const EVENTS_BY_SESSION: &str = r#"
    SELECT id, event_type, timestamp, session_id, agent_id,
           target_type, target_id, operation, before_state, after_state,
           trigger_source, related_events, metadata
    FROM state_events
    WHERE session_id = ?1
    ORDER BY timestamp DESC
    LIMIT ?2
"#;

pub const EVENTS_BY_TYPE: &str = r#"
    SELECT id, event_type, timestamp, session_id, agent_id,
           target_type, target_id, operation, before_state, after_state,
           trigger_source, related_events, metadata
    FROM state_events
    WHERE event_type = ?1
    ORDER BY timestamp DESC
    LIMIT ?2
"#;
