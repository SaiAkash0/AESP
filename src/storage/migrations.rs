use anyhow::Result;
use rusqlite::Connection;

pub fn run_all(conn: &Connection) -> Result<()> {
    conn.execute_batch(MIGRATION_V001)?;
    Ok(())
}

const MIGRATION_V001: &str = r#"
-- ============================================================
-- ENTITIES
-- ============================================================
CREATE TABLE IF NOT EXISTS entities (
    id TEXT PRIMARY KEY,
    entity_type TEXT NOT NULL,
    name TEXT NOT NULL,
    qualified_name TEXT NOT NULL UNIQUE,
    file_path TEXT,
    start_line INTEGER,
    end_line INTEGER,
    properties TEXT NOT NULL DEFAULT '{}',
    source_type TEXT NOT NULL DEFAULT 'indexed',
    verification_status TEXT NOT NULL DEFAULT 'unverified',
    confidence REAL NOT NULL DEFAULT 1.0,
    verified_at TEXT,
    verified_by TEXT,
    staleness_ttl INTEGER,
    memory_scope TEXT NOT NULL DEFAULT 'persistent',
    session_id TEXT,
    content_hash TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(entity_type);
CREATE INDEX IF NOT EXISTS idx_entities_name ON entities(name);
CREATE INDEX IF NOT EXISTS idx_entities_file_path ON entities(file_path);
CREATE INDEX IF NOT EXISTS idx_entities_qualified_name ON entities(qualified_name);
CREATE INDEX IF NOT EXISTS idx_entities_verification ON entities(verification_status);
CREATE INDEX IF NOT EXISTS idx_entities_source_type ON entities(source_type);
CREATE INDEX IF NOT EXISTS idx_entities_memory_scope ON entities(memory_scope);
CREATE INDEX IF NOT EXISTS idx_entities_session ON entities(session_id);

CREATE VIRTUAL TABLE IF NOT EXISTS entities_fts USING fts5(
    name,
    qualified_name,
    properties,
    content=entities,
    content_rowid=rowid
);

CREATE TRIGGER IF NOT EXISTS entities_ai AFTER INSERT ON entities BEGIN
    INSERT INTO entities_fts(rowid, name, qualified_name, properties)
    VALUES (new.rowid, new.name, new.qualified_name, new.properties);
END;

CREATE TRIGGER IF NOT EXISTS entities_ad AFTER DELETE ON entities BEGIN
    INSERT INTO entities_fts(entities_fts, rowid, name, qualified_name, properties)
    VALUES('delete', old.rowid, old.name, old.qualified_name, old.properties);
END;

CREATE TRIGGER IF NOT EXISTS entities_au AFTER UPDATE ON entities BEGIN
    INSERT INTO entities_fts(entities_fts, rowid, name, qualified_name, properties)
    VALUES('delete', old.rowid, old.name, old.qualified_name, old.properties);
    INSERT INTO entities_fts(rowid, name, qualified_name, properties)
    VALUES (new.rowid, new.name, new.qualified_name, new.properties);
END;

-- ============================================================
-- RELATIONSHIPS
-- ============================================================
CREATE TABLE IF NOT EXISTS relationships (
    id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    target_id TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    relationship_type TEXT NOT NULL,
    properties TEXT NOT NULL DEFAULT '{}',
    source_type TEXT NOT NULL DEFAULT 'indexed',
    verification_status TEXT NOT NULL DEFAULT 'unverified',
    confidence REAL NOT NULL DEFAULT 1.0,
    weight REAL NOT NULL DEFAULT 1.0,
    memory_scope TEXT NOT NULL DEFAULT 'persistent',
    session_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_rel_source ON relationships(source_id);
CREATE INDEX IF NOT EXISTS idx_rel_target ON relationships(target_id);
CREATE INDEX IF NOT EXISTS idx_rel_type ON relationships(relationship_type);

-- ============================================================
-- ANNOTATIONS
-- ============================================================
CREATE TABLE IF NOT EXISTS annotations (
    id TEXT PRIMARY KEY,
    entity_id TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    annotation_type TEXT NOT NULL,
    content TEXT NOT NULL,
    author TEXT NOT NULL DEFAULT 'agent',
    tags TEXT NOT NULL DEFAULT '[]',
    resolved INTEGER NOT NULL DEFAULT 0,
    source_type TEXT NOT NULL DEFAULT 'agent_inferred',
    verification_status TEXT NOT NULL DEFAULT 'unverified',
    confidence REAL NOT NULL DEFAULT 0.5,
    verified_at TEXT,
    evidence_refs TEXT NOT NULL DEFAULT '[]',
    memory_scope TEXT NOT NULL DEFAULT 'persistent',
    session_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_annotations_entity ON annotations(entity_id);
CREATE INDEX IF NOT EXISTS idx_annotations_type ON annotations(annotation_type);
CREATE INDEX IF NOT EXISTS idx_annotations_resolved ON annotations(resolved);
CREATE INDEX IF NOT EXISTS idx_annotations_verification ON annotations(verification_status);
CREATE INDEX IF NOT EXISTS idx_annotations_memory_scope ON annotations(memory_scope);

CREATE VIRTUAL TABLE IF NOT EXISTS annotations_fts USING fts5(
    content,
    tags,
    content=annotations,
    content_rowid=rowid
);

CREATE TRIGGER IF NOT EXISTS annotations_ai AFTER INSERT ON annotations BEGIN
    INSERT INTO annotations_fts(rowid, content, tags)
    VALUES (new.rowid, new.content, new.tags);
END;

CREATE TRIGGER IF NOT EXISTS annotations_ad AFTER DELETE ON annotations BEGIN
    INSERT INTO annotations_fts(annotations_fts, rowid, content, tags)
    VALUES('delete', old.rowid, old.content, old.tags);
END;

-- ============================================================
-- DECISIONS
-- ============================================================
CREATE TABLE IF NOT EXISTS decisions (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    task TEXT NOT NULL,
    task_hash TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    attempt_number INTEGER NOT NULL DEFAULT 1,
    approach TEXT NOT NULL,
    approach_type TEXT,
    entities_involved TEXT NOT NULL DEFAULT '[]',
    changes_made TEXT NOT NULL DEFAULT '[]',
    outcome TEXT NOT NULL,
    evidence TEXT,
    side_effects TEXT NOT NULL DEFAULT '[]',
    what_worked TEXT,
    what_failed TEXT,
    root_cause TEXT,
    constraints_discovered TEXT NOT NULL DEFAULT '[]',
    recommendations TEXT,
    tags TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_decisions_session ON decisions(session_id);
CREATE INDEX IF NOT EXISTS idx_decisions_task_hash ON decisions(task_hash);
CREATE INDEX IF NOT EXISTS idx_decisions_outcome ON decisions(outcome);

CREATE VIRTUAL TABLE IF NOT EXISTS decisions_fts USING fts5(
    task,
    approach,
    evidence,
    what_failed,
    root_cause,
    recommendations,
    content=decisions,
    content_rowid=rowid
);

CREATE TRIGGER IF NOT EXISTS decisions_ai AFTER INSERT ON decisions BEGIN
    INSERT INTO decisions_fts(rowid, task, approach, evidence, what_failed, root_cause, recommendations)
    VALUES (new.rowid, new.task, new.approach, new.evidence, new.what_failed, new.root_cause, new.recommendations);
END;

-- ============================================================
-- SESSIONS
-- ============================================================
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    task_description TEXT,
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    ended_at TEXT,
    decision_count INTEGER NOT NULL DEFAULT 0,
    memory_archived INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_sessions_agent ON sessions(agent_id);

-- ============================================================
-- STATE EVENT LEDGER
-- ============================================================
CREATE TABLE IF NOT EXISTS state_events (
    id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    session_id TEXT,
    agent_id TEXT,
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    operation TEXT NOT NULL,
    before_state TEXT,
    after_state TEXT,
    trigger_source TEXT NOT NULL DEFAULT 'agent_write',
    related_events TEXT NOT NULL DEFAULT '[]',
    metadata TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_events_type ON state_events(event_type);
CREATE INDEX IF NOT EXISTS idx_events_timestamp ON state_events(timestamp);
CREATE INDEX IF NOT EXISTS idx_events_session ON state_events(session_id);
CREATE INDEX IF NOT EXISTS idx_events_target ON state_events(target_type, target_id);
CREATE INDEX IF NOT EXISTS idx_events_agent ON state_events(agent_id);

-- ============================================================
-- CONSTRAINTS
-- ============================================================
CREATE TABLE IF NOT EXISTS constraints (
    id TEXT PRIMARY KEY,
    rule TEXT NOT NULL,
    scope TEXT NOT NULL DEFAULT 'session',
    session_id TEXT,
    severity TEXT NOT NULL DEFAULT 'hard',
    category TEXT NOT NULL DEFAULT 'custom',
    created_by TEXT NOT NULL DEFAULT 'agent',
    active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    deactivated_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_constraints_active ON constraints(active);
CREATE INDEX IF NOT EXISTS idx_constraints_scope ON constraints(scope);
CREATE INDEX IF NOT EXISTS idx_constraints_session ON constraints(session_id);
CREATE INDEX IF NOT EXISTS idx_constraints_severity ON constraints(severity);

-- ============================================================
-- TOOL INGESTIONS
-- ============================================================
CREATE TABLE IF NOT EXISTS tool_ingestions (
    id TEXT PRIMARY KEY,
    session_id TEXT,
    tool_name TEXT NOT NULL,
    raw_output_hash TEXT NOT NULL,
    raw_output_size INTEGER NOT NULL,
    context TEXT NOT NULL,
    related_entities TEXT NOT NULL DEFAULT '[]',
    output_format TEXT NOT NULL DEFAULT 'auto',
    facts_extracted INTEGER NOT NULL DEFAULT 0,
    contradictions_found INTEGER NOT NULL DEFAULT 0,
    entities_updated INTEGER NOT NULL DEFAULT 0,
    entities_created INTEGER NOT NULL DEFAULT 0,
    summary TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_ingestions_session ON tool_ingestions(session_id);
CREATE INDEX IF NOT EXISTS idx_ingestions_tool ON tool_ingestions(tool_name);

-- ============================================================
-- FILE HASHES
-- ============================================================
CREATE TABLE IF NOT EXISTS file_hashes (
    file_path TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL,
    last_indexed TEXT NOT NULL DEFAULT (datetime('now')),
    entity_count INTEGER NOT NULL DEFAULT 0
);

-- ============================================================
-- METADATA
-- ============================================================
CREATE TABLE IF NOT EXISTS aesp_metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT OR IGNORE INTO aesp_metadata (key, value) VALUES ('schema_version', '1');
INSERT OR IGNORE INTO aesp_metadata (key, value) VALUES ('aesp_version', '0.1.0');
"#;
