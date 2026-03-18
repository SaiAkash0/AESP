use anyhow::Result;
use std::path::PathBuf;
use uuid::Uuid;
use crate::config::AespConfig;
use crate::storage::Storage;

pub fn handle_tool_call(
    tool_name: &str,
    arguments: &serde_json::Value,
    storage: &Storage,
    config: &AespConfig,
    project_root: &PathBuf,
) -> Result<serde_json::Value> {
    match tool_name {
        "aesp_start_task" => handle_start_task(arguments, storage, config),
        "aesp_query" => handle_query(arguments, storage),
        "aesp_write" => handle_write(arguments, storage),
        "aesp_context_pack" => handle_context_pack(arguments, storage, config),
        "aesp_decision_log" => handle_decision_log(arguments, storage),
        "aesp_graph_view" => handle_graph_view(arguments, storage),
        "aesp_status" => handle_status(storage),
        "aesp_reindex" => handle_reindex(arguments, storage, config, project_root),
        "aesp_session" => handle_session(arguments, storage),
        "aesp_verify" => handle_verify(arguments, storage),
        "aesp_constrain" => handle_constrain(arguments, storage),
        "aesp_ingest_tool_result" => handle_ingest(arguments, storage),
        "aesp_inspect" => handle_inspect(arguments, storage),
        _ => Err(anyhow::anyhow!("Unknown tool: {}", tool_name)),
    }
}

fn handle_query(args: &serde_json::Value, storage: &Storage) -> Result<serde_json::Value> {
    let query = args.get("query").and_then(|q| q.as_str()).unwrap_or("");
    let max_results = args.get("max_results").and_then(|m| m.as_u64()).unwrap_or(10) as u32;
    let depth = args.get("depth").and_then(|d| d.as_u64()).unwrap_or(2) as u32;
    let trust_filter = args.get("trust_filter").and_then(|t| t.as_str()).unwrap_or("all");

    let results = crate::graph::query_entities(storage, query, None, depth, trust_filter, max_results)?;
    let result_json = serde_json::to_value(&results)?;

    Ok(mcp_text_result(&serde_json::json!({
        "results": result_json,
        "total_results": results.len(),
    })))
}

fn handle_write(args: &serde_json::Value, storage: &Storage) -> Result<serde_json::Value> {
    let operation = args.get("operation").and_then(|o| o.as_str()).unwrap_or("");
    let target = args.get("target").and_then(|t| t.as_str()).unwrap_or("");
    let data = args.get("data").cloned().unwrap_or(serde_json::json!({}));

    match operation {
        "annotate" => {
            let entity = crate::graph::get_entity_by_qualified_name(storage, target)?
                .ok_or_else(|| anyhow::anyhow!("Entity not found: {}", target))?;

            let ann_type = data.get("type").and_then(|t| t.as_str()).unwrap_or("note");
            let content = data.get("content").and_then(|c| c.as_str()).unwrap_or("");
            let tags: Vec<String> = data
                .get("tags")
                .and_then(|t| serde_json::from_value(t.clone()).ok())
                .unwrap_or_default();

            let annotation = crate::graph::Annotation {
                id: Uuid::new_v4().to_string(),
                entity_id: entity.id.clone(),
                annotation_type: ann_type.to_string(),
                content: content.to_string(),
                author: "agent".to_string(),
                tags,
                resolved: false,
                source_type: "agent_inferred".to_string(),
                verification_status: "unverified".to_string(),
                confidence: 0.7,
                memory_scope: "persistent".to_string(),
                created_at: String::new(),
            };

            crate::graph::insert_annotation(storage, &annotation)?;

            crate::events::emit_event(
                storage,
                "annotation_created",
                "annotation",
                &annotation.id,
                "create",
                "agent_write",
                None,
                Some(content),
            )?;

            Ok(mcp_text_result(&serde_json::json!({
                "status": "success",
                "annotation_id": annotation.id,
                "entity": target,
            })))
        }
        "create_entity" => {
            let entity_type = data.get("entity_type").and_then(|t| t.as_str()).unwrap_or("file");
            let properties = data.get("properties").cloned().unwrap_or(serde_json::json!({}));
            let name = target.rsplit("::").next().unwrap_or(target);

            let entity = crate::graph::Entity {
                id: Uuid::new_v4().to_string(),
                entity_type: entity_type.to_string(),
                name: name.to_string(),
                qualified_name: target.to_string(),
                file_path: None,
                start_line: None,
                end_line: None,
                properties,
                source_type: "agent_inferred".to_string(),
                verification_status: "unverified".to_string(),
                confidence: 0.7,
                memory_scope: "persistent".to_string(),
                content_hash: None,
                created_at: String::new(),
                updated_at: String::new(),
            };

            crate::graph::insert_entity(storage, &entity)?;

            Ok(mcp_text_result(&serde_json::json!({
                "status": "success",
                "entity_id": entity.id,
                "qualified_name": target,
            })))
        }
        "create_relationship" => {
            let source = data.get("source").and_then(|s| s.as_str()).unwrap_or("");
            let target_entity = data.get("target").and_then(|t| t.as_str()).unwrap_or("");
            let rel_type = data.get("relationship_type").and_then(|r| r.as_str()).unwrap_or("calls");

            let source_entity = crate::graph::get_entity_by_qualified_name(storage, source)?
                .ok_or_else(|| anyhow::anyhow!("Source entity not found: {}", source))?;
            let target_entity_obj = crate::graph::get_entity_by_qualified_name(storage, target_entity)?
                .ok_or_else(|| anyhow::anyhow!("Target entity not found: {}", target_entity))?;

            let rel = crate::graph::Relationship {
                id: Uuid::new_v4().to_string(),
                source_id: source_entity.id,
                target_id: target_entity_obj.id,
                relationship_type: rel_type.to_string(),
                properties: serde_json::json!({}),
                source_type: "agent_inferred".to_string(),
                verification_status: "unverified".to_string(),
                confidence: 0.7,
                weight: 1.0,
                memory_scope: "persistent".to_string(),
            };

            crate::graph::insert_relationship(storage, &rel)?;

            Ok(mcp_text_result(&serde_json::json!({
                "status": "success",
                "relationship_id": rel.id,
            })))
        }
        "update_entity" => {
            let properties = data.get("properties").cloned().unwrap_or(serde_json::json!({}));
            storage.with_conn_mut(|conn| {
                conn.execute(
                    "UPDATE entities SET properties = json_patch(properties, ?1), updated_at = datetime('now') WHERE qualified_name = ?2",
                    rusqlite::params![properties.to_string(), target],
                )?;
                Ok(())
            })?;

            Ok(mcp_text_result(&serde_json::json!({
                "status": "success",
                "entity": target,
            })))
        }
        _ => Err(anyhow::anyhow!("Unknown write operation: {}", operation)),
    }
}

fn handle_context_pack(
    args: &serde_json::Value,
    storage: &Storage,
    config: &AespConfig,
) -> Result<serde_json::Value> {
    let task = args.get("task").and_then(|t| t.as_str()).unwrap_or("");
    let token_budget = args.get("token_budget").and_then(|b| b.as_u64()).unwrap_or(8000) as u32;
    let focus_entities: Vec<String> = args
        .get("focus_entities")
        .and_then(|f| serde_json::from_value(f.clone()).ok())
        .unwrap_or_default();
    let include_decisions = args.get("include_decisions").and_then(|d| d.as_bool()).unwrap_or(true);
    let include_constraints = args.get("include_constraints").and_then(|c| c.as_bool()).unwrap_or(true);
    let trust_filter = args.get("trust_filter").and_then(|t| t.as_str()).unwrap_or("all");

    let package = crate::compiler::compile_context(
        storage,
        config,
        task,
        token_budget,
        &focus_entities,
        include_decisions,
        include_constraints,
        trust_filter,
    )?;

    let package_json = serde_json::to_value(&package)?;
    Ok(mcp_text_result(&package_json))
}

fn handle_decision_log(args: &serde_json::Value, storage: &Storage) -> Result<serde_json::Value> {
    let action = args.get("action").and_then(|a| a.as_str()).unwrap_or("query");
    let task = args.get("task").and_then(|t| t.as_str()).unwrap_or("");

    match action {
        "record" => {
            let approach = args
                .get("attempt")
                .and_then(|a| a.get("approach"))
                .and_then(|a| a.as_str())
                .unwrap_or("");
            let approach_type = args
                .get("attempt")
                .and_then(|a| a.get("approach_type"))
                .and_then(|a| a.as_str());
            let outcome = args
                .get("result")
                .and_then(|r| r.get("outcome"))
                .and_then(|o| o.as_str())
                .unwrap_or("unknown");
            let evidence = args
                .get("result")
                .and_then(|r| r.get("evidence"))
                .and_then(|e| e.as_str());
            let what_failed = args
                .get("learnings")
                .and_then(|l| l.get("what_failed"))
                .and_then(|w| w.as_str());
            let root_cause = args
                .get("learnings")
                .and_then(|l| l.get("root_cause"))
                .and_then(|r| r.as_str());
            let recommendations = args
                .get("learnings")
                .and_then(|l| l.get("recommendations"))
                .and_then(|r| r.as_str());

            let session_id = "default-session";

            let id = crate::decisions::record_decision(
                storage,
                session_id,
                task,
                approach,
                approach_type,
                &[],
                outcome,
                evidence,
                None,
                what_failed,
                root_cause,
                recommendations,
            )?;

            Ok(mcp_text_result(&serde_json::json!({
                "status": "recorded",
                "decision_id": id,
            })))
        }
        "query" => {
            let outcome = args
                .get("query_filter")
                .and_then(|q| q.get("outcome"))
                .and_then(|o| o.as_str());
            let limit = args
                .get("query_filter")
                .and_then(|q| q.get("limit"))
                .and_then(|l| l.as_u64())
                .unwrap_or(10) as u32;

            let decisions = crate::decisions::query_decisions(
                storage,
                Some(task),
                outcome,
                None,
                None,
                limit,
            )?;

            Ok(mcp_text_result(&serde_json::to_value(&decisions)?))
        }
        _ => Err(anyhow::anyhow!("Unknown decision log action: {}", action)),
    }
}

fn handle_graph_view(args: &serde_json::Value, storage: &Storage) -> Result<serde_json::Value> {
    let root = args.get("root").and_then(|r| r.as_str()).unwrap_or(".");
    let depth = args.get("depth").and_then(|d| d.as_u64()).unwrap_or(2) as u32;

    if let Some(entity) = crate::graph::get_entity_by_qualified_name(storage, root)? {
        let traversal = crate::graph::bfs_from_entity(storage, &entity.id, depth)?;

        let nodes: Vec<serde_json::Value> = traversal
            .entities
            .iter()
            .map(|(e, d)| {
                serde_json::json!({
                    "qualified_name": e.qualified_name,
                    "type": e.entity_type,
                    "depth": d,
                    "verification_status": e.verification_status,
                })
            })
            .collect();

        Ok(mcp_text_result(&serde_json::json!({
            "root": root,
            "depth": depth,
            "nodes": nodes,
            "total_nodes": nodes.len(),
        })))
    } else {
        Ok(mcp_text_result(&serde_json::json!({
            "error": format!("Entity not found: {}", root),
        })))
    }
}

fn handle_status(storage: &Storage) -> Result<serde_json::Value> {
    let status = crate::graph::get_status(storage)?;
    Ok(mcp_text_result(&serde_json::to_value(&status)?))
}

fn handle_reindex(
    args: &serde_json::Value,
    storage: &Storage,
    config: &AespConfig,
    project_root: &PathBuf,
) -> Result<serde_json::Value> {
    let mode = args.get("mode").and_then(|m| m.as_str()).unwrap_or("full");

    let schema_registry = crate::schema::SchemaRegistry::new();
    let schema = schema_registry.get_schema("code")?;

    match mode {
        "full" => {
            let stats = crate::indexer::index_project(project_root, storage, schema, config)?;
            Ok(mcp_text_result(&serde_json::json!({
                "status": "reindexed",
                "files_indexed": stats.files_indexed,
                "entities_created": stats.entities_created,
                "relationships_created": stats.relationships_created,
            })))
        }
        "path" => {
            let paths: Vec<String> = args
                .get("paths")
                .and_then(|p| serde_json::from_value(p.clone()).ok())
                .unwrap_or_default();

            let mut total_stats = crate::indexer::IndexStats::default();
            for p in &paths {
                let target = std::path::PathBuf::from(p);
                let stats = crate::indexer::index_path(project_root, &target, storage, schema, config)?;
                total_stats.files_indexed += stats.files_indexed;
                total_stats.entities_created += stats.entities_created;
                total_stats.relationships_created += stats.relationships_created;
            }

            Ok(mcp_text_result(&serde_json::json!({
                "status": "reindexed",
                "paths": paths,
                "files_indexed": total_stats.files_indexed,
                "entities_created": total_stats.entities_created,
                "relationships_created": total_stats.relationships_created,
            })))
        }
        _ => Err(anyhow::anyhow!("Unknown reindex mode: {}", mode)),
    }
}

fn handle_session(args: &serde_json::Value, storage: &Storage) -> Result<serde_json::Value> {
    let action = args.get("action").and_then(|a| a.as_str()).unwrap_or("info");

    match action {
        "start" => {
            let task_desc = args.get("task_description").and_then(|t| t.as_str()).unwrap_or("");
            let session_id = Uuid::new_v4().to_string();

            storage.with_conn_mut(|conn| {
                conn.execute(
                    "INSERT INTO sessions (id, agent_id, task_description) VALUES (?1, ?2, ?3)",
                    rusqlite::params![session_id, "agent", task_desc],
                )?;
                Ok(())
            })?;

            crate::events::emit_event(
                storage,
                "session_started",
                "session",
                &session_id,
                "create",
                "agent_write",
                None,
                Some(task_desc),
            )?;

            Ok(mcp_text_result(&serde_json::json!({
                "status": "started",
                "session_id": session_id,
                "task_description": task_desc,
            })))
        }
        "end" => {
            let session_id = args
                .get("session_id")
                .and_then(|s| s.as_str())
                .unwrap_or("default-session");

            storage.with_conn_mut(|conn| {
                conn.execute(
                    "UPDATE sessions SET ended_at = datetime('now'), memory_archived = 1 WHERE id = ?1",
                    rusqlite::params![session_id],
                )?;
                Ok(())
            })?;

            crate::constraints::deactivate_session_constraints(storage, session_id)?;

            crate::events::emit_event(
                storage,
                "session_ended",
                "session",
                session_id,
                "update",
                "agent_write",
                None,
                None,
            )?;

            Ok(mcp_text_result(&serde_json::json!({
                "status": "ended",
                "session_id": session_id,
            })))
        }
        "info" => {
            let sessions = storage.with_conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, agent_id, task_description, started_at, ended_at, decision_count
                     FROM sessions ORDER BY started_at DESC LIMIT 5",
                )?;
                let mut rows = stmt.query([])?;
                let mut sessions = Vec::new();
                while let Some(row) = rows.next()? {
                    sessions.push(serde_json::json!({
                        "id": row.get::<_, String>(0)?,
                        "agent_id": row.get::<_, String>(1)?,
                        "task_description": row.get::<_, Option<String>>(2)?,
                        "started_at": row.get::<_, String>(3)?,
                        "ended_at": row.get::<_, Option<String>>(4)?,
                        "decision_count": row.get::<_, i32>(5)?,
                    }));
                }
                Ok(sessions)
            })?;

            Ok(mcp_text_result(&serde_json::json!({
                "sessions": sessions,
            })))
        }
        _ => Err(anyhow::anyhow!("Unknown session action: {}", action)),
    }
}

fn handle_verify(args: &serde_json::Value, storage: &Storage) -> Result<serde_json::Value> {
    let target = args.get("target").and_then(|t| t.as_str()).unwrap_or("");
    let action = args.get("action").and_then(|a| a.as_str()).unwrap_or("");
    let evidence = args.get("evidence").and_then(|e| e.as_str());
    let new_confidence = args.get("new_confidence").and_then(|c| c.as_f64());
    let contradicting_fact = args.get("contradicting_fact").and_then(|c| c.as_str());

    match action {
        "verify" => {
            crate::verification::verify_entity(storage, target, evidence, new_confidence, None)?;
            Ok(mcp_text_result(&serde_json::json!({
                "status": "verified",
                "target": target,
            })))
        }
        "contradict" => {
            crate::verification::contradict_entity(storage, target, contradicting_fact, evidence)?;
            Ok(mcp_text_result(&serde_json::json!({
                "status": "contradicted",
                "target": target,
            })))
        }
        "mark_stale" => {
            crate::verification::mark_stale(storage, target)?;
            Ok(mcp_text_result(&serde_json::json!({
                "status": "marked_stale",
                "target": target,
            })))
        }
        "retract" => {
            crate::verification::retract_entity(storage, target, evidence)?;
            Ok(mcp_text_result(&serde_json::json!({
                "status": "retracted",
                "target": target,
            })))
        }
        _ => Err(anyhow::anyhow!("Unknown verify action: {}", action)),
    }
}

fn handle_constrain(args: &serde_json::Value, storage: &Storage) -> Result<serde_json::Value> {
    let action = args.get("action").and_then(|a| a.as_str()).unwrap_or("list");

    match action {
        "add" => {
            let rule = args.get("rule").and_then(|r| r.as_str()).unwrap_or("");
            let scope = args.get("scope").and_then(|s| s.as_str()).unwrap_or("session");
            let severity = args.get("severity").and_then(|s| s.as_str()).unwrap_or("hard");
            let category = args.get("category").and_then(|c| c.as_str()).unwrap_or("custom");

            let id = crate::constraints::add_constraint(
                storage, rule, scope, severity, category, "agent", None,
            )?;

            Ok(mcp_text_result(&serde_json::json!({
                "status": "added",
                "constraint_id": id,
                "rule": rule,
                "scope": scope,
                "severity": severity,
            })))
        }
        "remove" => {
            let constraint_id = args.get("constraint_id").and_then(|c| c.as_str()).unwrap_or("");
            crate::constraints::remove_constraint(storage, constraint_id)?;
            Ok(mcp_text_result(&serde_json::json!({
                "status": "removed",
                "constraint_id": constraint_id,
            })))
        }
        "list" => {
            let constraints = crate::constraints::list_active_constraints(storage, None)?;
            Ok(mcp_text_result(&serde_json::to_value(&constraints)?))
        }
        _ => Err(anyhow::anyhow!("Unknown constrain action: {}", action)),
    }
}

fn handle_ingest(args: &serde_json::Value, storage: &Storage) -> Result<serde_json::Value> {
    let tool_name = args.get("tool_name").and_then(|t| t.as_str()).unwrap_or("");
    let raw_output = args.get("raw_output").and_then(|r| r.as_str()).unwrap_or("");
    let context = args.get("context").and_then(|c| c.as_str()).unwrap_or("");
    let related_entities: Vec<String> = args
        .get("related_entities")
        .and_then(|r| serde_json::from_value(r.clone()).ok())
        .unwrap_or_default();
    let output_format = args.get("output_format").and_then(|f| f.as_str()).unwrap_or("auto");

    let summary = crate::normalizer::ingest_tool_result(
        storage,
        tool_name,
        raw_output,
        context,
        &related_entities,
        output_format,
        None,
    )?;

    Ok(mcp_text_result(&serde_json::to_value(&summary)?))
}

fn handle_inspect(args: &serde_json::Value, storage: &Storage) -> Result<serde_json::Value> {
    let query_type = args.get("query_type").and_then(|q| q.as_str()).unwrap_or("timeline");
    let target = args.get("target").and_then(|t| t.as_str());
    let limit = args.get("limit").and_then(|l| l.as_u64()).unwrap_or(50) as u32;

    let events = match query_type {
        "timeline" => crate::events::query_timeline(storage, limit)?,
        "entity_history" => {
            let t = target.unwrap_or("");
            crate::events::query_by_target(storage, t, limit)?
        }
        "session_events" => {
            let t = target.unwrap_or("");
            crate::events::query_by_session(storage, t, limit)?
        }
        "contradictions" => {
            crate::events::query_by_type(storage, "contradiction_detected", limit)?
        }
        "event_type_filter" => {
            let t = target.unwrap_or("");
            crate::events::query_by_type(storage, t, limit)?
        }
        _ => crate::events::query_timeline(storage, limit)?,
    };

    Ok(mcp_text_result(&serde_json::json!({
        "query_type": query_type,
        "events": serde_json::to_value(&events)?,
        "count": events.len(),
    })))
}

fn handle_start_task(
    args: &serde_json::Value,
    storage: &Storage,
    config: &AespConfig,
) -> Result<serde_json::Value> {
    let task = args.get("task").and_then(|t| t.as_str()).unwrap_or("");

    eprintln!("AESP start_task: '{}'", task);
    let t0 = std::time::Instant::now();

    // 1. Start a new session
    let session_id = Uuid::new_v4().to_string();
    storage.with_conn_mut(|conn| {
        conn.execute(
            "INSERT INTO sessions (id, agent_id, task_description) VALUES (?1, ?2, ?3)",
            rusqlite::params![session_id, "agent", task],
        )?;
        Ok(())
    })?;

    crate::events::emit_event(
        storage,
        "session_started",
        "session",
        &session_id,
        "create",
        "aesp_start_task",
        None,
        Some(task),
    )?;

    // 2. Compile context pack
    let package = crate::compiler::compile_context(
        storage, config, task, 8000, &[], true, true, "all",
    )?;

    // 3. Query past decisions related to this task
    let past_decisions = crate::decisions::query_decisions(
        storage, Some(task), None, None, None, 5,
    )?;
    let decisions_json: Vec<serde_json::Value> = past_decisions
        .into_iter()
        .map(|d| serde_json::json!({
            "task": d.task,
            "attempt": d.attempt_number,
            "approach": d.approach,
            "outcome": d.outcome,
            "what_failed": d.what_failed,
            "root_cause": d.root_cause,
            "recommendations": d.recommendations,
        }))
        .collect();

    // 4. Build the tip
    let total_entities = package.entities.tier_1_full.len()
        + package.entities.tier_2_signatures.len()
        + package.entities.tier_3_summaries.len();

    let top_files: Vec<String> = package.entities.tier_1_full.iter()
        .chain(package.entities.tier_2_signatures.iter())
        .filter_map(|e| e.get("qualified_name").and_then(|q| q.as_str()))
        .filter(|q| q.contains('/'))
        .take(3)
        .map(|s| s.to_string())
        .collect();

    let constraint_count = package.active_constraints.len();

    let tip = format!(
        "I found {} relevant entities for your task. The most relevant are: [{}]. There are {} active constraints to respect.",
        total_entities,
        top_files.join(", "),
        constraint_count,
    );

    eprintln!("AESP start_task: completed in {:?}", t0.elapsed());

    let package_json = serde_json::to_value(&package)?;

    Ok(mcp_text_result(&serde_json::json!({
        "session_id": session_id,
        "message": "AESP Context Loaded. Use aesp_query for deeper searches, aesp_write to annotate findings, aesp_decision_log to record attempts.",
        "context_pack": package_json,
        "past_decisions": decisions_json,
        "tip": tip,
    })))
}

fn mcp_text_result(value: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string(value).unwrap_or_default(),
        }]
    })
}
