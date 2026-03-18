use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use uuid::Uuid;
use crate::storage::Storage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionSummary {
    pub facts_extracted: u32,
    pub facts_added: Vec<ExtractedFact>,
    pub contradictions_found: u32,
    pub contradictions: Vec<serde_json::Value>,
    pub entities_updated: u32,
    pub entities_created: u32,
    pub next_suggested_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedFact {
    pub entity: String,
    pub fact: String,
    pub confidence: f64,
    pub source_type: String,
}

pub fn ingest_tool_result(
    storage: &Storage,
    tool_name: &str,
    raw_output: &str,
    context: &str,
    related_entities: &[String],
    output_format: &str,
    session_id: Option<&str>,
) -> Result<IngestionSummary> {
    let id = Uuid::new_v4().to_string();
    let output_hash = hash_output(raw_output);
    let output_size = raw_output.len() as i64;

    let facts = extract_facts(raw_output, output_format, related_entities);

    for fact in &facts {
        if let Some(entity_qn) = find_matching_entity(storage, &fact.entity)? {
            let annotation = crate::graph::Annotation {
                id: Uuid::new_v4().to_string(),
                entity_id: entity_qn.clone(),
                annotation_type: "fact".to_string(),
                content: fact.fact.clone(),
                author: format!("tool:{}", tool_name),
                tags: vec!["tool_extracted".into()],
                resolved: false,
                source_type: "tool_returned".to_string(),
                verification_status: "unverified".to_string(),
                confidence: fact.confidence,
                memory_scope: "persistent".to_string(),
                created_at: String::new(),
            };
            let _ = crate::graph::insert_annotation(storage, &annotation);
        }
    }

    let summary_text = format!(
        "Ingested {} output: {} facts extracted from {} bytes",
        tool_name,
        facts.len(),
        output_size
    );

    storage.with_conn_mut(|conn| {
        conn.execute(
            "INSERT INTO tool_ingestions
                (id, session_id, tool_name, raw_output_hash, raw_output_size, context,
                 related_entities, output_format, facts_extracted, contradictions_found,
                 entities_updated, entities_created, summary)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![
                id,
                session_id,
                tool_name,
                output_hash,
                output_size,
                context,
                serde_json::to_string(related_entities)?,
                output_format,
                facts.len() as i32,
                0,
                facts.len() as i32,
                0,
                summary_text,
            ],
        )?;
        Ok(())
    })?;

    crate::events::emit_event(
        storage,
        "tool_result_ingested",
        "tool_ingestion",
        &id,
        "create",
        "tool_ingest",
        None,
        Some(&summary_text),
    )?;

    Ok(IngestionSummary {
        facts_extracted: facts.len() as u32,
        facts_added: facts,
        contradictions_found: 0,
        contradictions: Vec::new(),
        entities_updated: 0,
        entities_created: 0,
        next_suggested_actions: Vec::new(),
    })
}

fn extract_facts(raw_output: &str, format: &str, related_entities: &[String]) -> Vec<ExtractedFact> {
    let mut facts = Vec::new();
    let detected_format = if format == "auto" { detect_format(raw_output) } else { format.to_string() };

    match detected_format.as_str() {
        "json" => {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(raw_output) {
                extract_json_facts(&parsed, related_entities, &mut facts, "");
            }
        }
        "log" => {
            for line in raw_output.lines() {
                let lower = line.to_lowercase();
                if lower.contains("error") || lower.contains("warning") || lower.contains("fail") {
                    let entity = related_entities.first().cloned().unwrap_or_else(|| "project".into());
                    facts.push(ExtractedFact {
                        entity,
                        fact: line.trim().to_string(),
                        confidence: 0.8,
                        source_type: "tool_returned".into(),
                    });
                }
            }
        }
        _ => {
            if !raw_output.trim().is_empty() {
                let entity = related_entities.first().cloned().unwrap_or_else(|| "project".into());
                let summary = if raw_output.len() > 200 {
                    format!("{}...", &raw_output[..200])
                } else {
                    raw_output.to_string()
                };
                facts.push(ExtractedFact {
                    entity,
                    fact: summary,
                    confidence: 0.6,
                    source_type: "tool_returned".into(),
                });
            }
        }
    }

    facts
}

fn extract_json_facts(
    value: &serde_json::Value,
    related_entities: &[String],
    facts: &mut Vec<ExtractedFact>,
    path: &str,
) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, val) in map {
                let new_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", path, key)
                };

                let lower_key = key.to_lowercase();
                if lower_key.contains("error")
                    || lower_key.contains("status")
                    || lower_key.contains("count")
                    || lower_key.contains("rate")
                    || lower_key.contains("time")
                    || lower_key.contains("version")
                {
                    let entity = related_entities.first().cloned().unwrap_or_else(|| "project".into());
                    facts.push(ExtractedFact {
                        entity,
                        fact: format!("{}: {}", new_path, val),
                        confidence: 0.85,
                        source_type: "tool_returned".into(),
                    });
                }

                if let serde_json::Value::Object(_) | serde_json::Value::Array(_) = val {
                    extract_json_facts(val, related_entities, facts, &new_path);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for (i, item) in arr.iter().enumerate().take(5) {
                extract_json_facts(item, related_entities, facts, &format!("{}[{}]", path, i));
            }
        }
        _ => {}
    }
}

fn detect_format(output: &str) -> String {
    let trimmed = output.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        "json".into()
    } else if trimmed.lines().any(|l| {
        let lower = l.to_lowercase();
        lower.contains("[error]") || lower.contains("[warn]") || lower.contains("[info]")
    }) {
        "log".into()
    } else {
        "text".into()
    }
}

fn hash_output(output: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(output.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn find_matching_entity(storage: &Storage, entity_ref: &str) -> Result<Option<String>> {
    let entity = crate::graph::get_entity_by_qualified_name(storage, entity_ref)?;
    Ok(entity.map(|e| e.id))
}
