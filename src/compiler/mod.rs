use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::config::AespConfig;
use crate::constraints;
use crate::graph;
use crate::storage::Storage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPackage {
    pub task: String,
    pub generated_at: String,
    pub token_count: u32,
    pub token_budget: u32,
    pub active_constraints: Vec<ConstraintEntry>,
    pub trust_summary: TrustSummary,
    pub project_map: ProjectMap,
    pub entities: EntityTiers,
    pub decision_history: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintEntry {
    pub id: String,
    pub rule: String,
    pub scope: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustSummary {
    pub total_entities_in_pack: u32,
    pub verified: u32,
    pub unverified: u32,
    pub stale: u32,
    pub contradicted: u32,
    pub overall_confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMap {
    pub overview: String,
    pub structure: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityTiers {
    pub tier_1_full: Vec<serde_json::Value>,
    pub tier_2_signatures: Vec<serde_json::Value>,
    pub tier_3_summaries: Vec<serde_json::Value>,
}

pub fn compile_context(
    storage: &Storage,
    config: &AespConfig,
    task: &str,
    token_budget: u32,
    focus_entities: &[String],
    include_decisions: bool,
    include_constraints: bool,
    trust_filter: &str,
) -> Result<ContextPackage> {
    let t_start = std::time::Instant::now();

    let budget = if token_budget > 0 { token_budget } else { config.context_compiler.default_token_budget };

    let active_constraints = if include_constraints {
        constraints::list_active_constraints(storage, None)?
            .into_iter()
            .map(|c| ConstraintEntry {
                id: c.id,
                rule: c.rule,
                scope: c.scope,
                severity: c.severity,
            })
            .collect()
    } else {
        Vec::new()
    };

    let t0 = std::time::Instant::now();
    let seed_results = select_seeds(storage, task, focus_entities, trust_filter)?;
    eprintln!("AESP context_pack: seed selection took {:?}, found {} seeds", t0.elapsed(), seed_results.len());

    let mut verified = 0u32;
    let mut unverified = 0u32;
    let mut stale = 0u32;
    let mut contradicted = 0u32;
    let mut total_confidence = 0.0f64;

    for r in &seed_results {
        match r.verification_status.as_str() {
            "verified" => verified += 1,
            "stale" => stale += 1,
            "contradicted" => contradicted += 1,
            _ => unverified += 1,
        }
        total_confidence += r.confidence;
    }

    let total = seed_results.len() as u32;
    let overall_confidence = if total > 0 {
        total_confidence / total as f64
    } else {
        0.0
    };

    let constraint_budget = (budget as f64 * config.context_compiler.constraint_budget_percent as f64 / 100.0) as u32;
    let remaining_budget = budget - constraint_budget;

    let tier1_budget = (remaining_budget as f64 * 0.5) as u32;
    let tier2_budget = (remaining_budget as f64 * 0.33) as u32;
    let tier3_budget = (remaining_budget as f64 * 0.11) as u32;

    let t1 = std::time::Instant::now();
    let mut tier1 = Vec::new();
    let mut tier2 = Vec::new();
    let mut tier3 = Vec::new();
    let mut tokens_used = 0u32;

    for (i, result) in seed_results.iter().enumerate() {
        let entity_json = serde_json::json!({
            "qualified_name": result.qualified_name,
            "type": result.entity_type,
            "relevance_score": result.relevance_score,
            "trust": {
                "verification_status": result.verification_status,
                "confidence": result.confidence,
            },
            "signature": result.signature,
            "relationships": result.relationships,
            "annotations": result.annotations,
        });

        let token_est = estimate_tokens(&serde_json::to_string(&entity_json)?);

        if i < 5 && tokens_used + token_est < tier1_budget {
            tier1.push(entity_json);
            tokens_used += token_est;
        } else if i < 15 && tokens_used + token_est < tier1_budget + tier2_budget {
            tier2.push(serde_json::json!({
                "qualified_name": result.qualified_name,
                "type": result.entity_type,
                "relevance_score": result.relevance_score,
                "trust": { "verification_status": result.verification_status, "confidence": result.confidence },
                "signature": result.signature,
            }));
            tokens_used += token_est / 2;
        } else if tokens_used + 20 < tier1_budget + tier2_budget + tier3_budget {
            tier3.push(serde_json::json!({
                "qualified_name": result.qualified_name,
                "type": result.entity_type,
                "summary": format!("{} ({})", result.name, result.entity_type),
            }));
            tokens_used += 20;
        }
    }
    eprintln!("AESP context_pack: ranking+packing took {:?}, tiers {}/{}/{}", t1.elapsed(), tier1.len(), tier2.len(), tier3.len());

    let decision_history = if include_decisions {
        let decisions = crate::decisions::query_decisions(storage, Some(task), None, None, None, 5)?;
        decisions
            .into_iter()
            .map(|d| {
                serde_json::json!({
                    "task": d.task,
                    "attempt": d.attempt_number,
                    "approach": d.approach,
                    "result": d.outcome,
                    "reason": d.what_failed,
                    "learned": d.recommendations,
                })
            })
            .collect()
    } else {
        Vec::new()
    };

    let t2 = std::time::Instant::now();
    let project_map = generate_project_map(storage)?;
    eprintln!("AESP context_pack: project_map took {:?}", t2.elapsed());

    eprintln!("AESP context_pack: total {:?}", t_start.elapsed());

    Ok(ContextPackage {
        task: task.to_string(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        token_count: tokens_used,
        token_budget: budget,
        active_constraints,
        trust_summary: TrustSummary {
            total_entities_in_pack: total,
            verified,
            unverified,
            stale,
            contradicted,
            overall_confidence,
        },
        project_map,
        entities: EntityTiers {
            tier_1_full: tier1,
            tier_2_signatures: tier2,
            tier_3_summaries: tier3,
        },
        decision_history,
    })
}

/// Lightweight entity stub for seed selection — no rels/annotations yet.
struct SeedCandidate {
    id: String,
    entity_type: String,
    name: String,
    qualified_name: String,
    file_path: Option<String>,
    start_line: Option<u32>,
    end_line: Option<u32>,
    verification_status: String,
    confidence: f64,
    signature: Option<String>,
    relevance_score: f64,
}

/// Select seed entities using task keywords via FTS5 + focus_entities.
/// Phase 1: lightweight entity-only queries (no rels/annotations).
/// Phase 2: dedup, truncate to 10, THEN fetch rels/annotations for finals only.
fn select_seeds(
    storage: &Storage,
    task: &str,
    focus_entities: &[String],
    trust_filter: &str,
) -> Result<Vec<graph::QueryResult>> {
    let mut candidates: Vec<SeedCandidate> = Vec::new();

    // Phase 1a: Fetch explicit focus entities at max relevance (lightweight)
    for qname in focus_entities {
        if let Some(entity) = graph::get_entity_by_qualified_name(storage, qname)? {
            let sig = entity.properties.get("signature").and_then(|v| v.as_str()).map(String::from);
            candidates.push(SeedCandidate {
                id: entity.id,
                entity_type: entity.entity_type,
                name: entity.name,
                qualified_name: entity.qualified_name,
                file_path: entity.file_path,
                start_line: entity.start_line,
                end_line: entity.end_line,
                verification_status: entity.verification_status,
                confidence: entity.confidence,
                signature: sig,
                relevance_score: 1.0,
            });
        }
    }

    // Phase 1b: Extract keywords from task and do lightweight searches
    if !task.is_empty() {
        let keywords = extract_keywords(task);

        if !keywords.is_empty() {
            // FTS5 search with BM25 scoring (returns scored entities, no rels)
            let fts_candidates = search_fts_lightweight(storage, &keywords.join(" "), 15)?;
            candidates.extend(fts_candidates);

            // LIKE fallback on names/paths for each keyword
            for keyword in &keywords {
                let like_candidates = search_like_lightweight(storage, keyword, 5)?;
                candidates.extend(like_candidates);
            }
        }
    }

    // Phase 1c: If nothing found, fall back to recent entities
    if candidates.is_empty() {
        let recent = search_fts_lightweight(storage, "", 15)?;
        candidates.extend(recent);
    }

    // Phase 2a: Deduplicate by qualified_name, keeping highest score
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut deduped: Vec<SeedCandidate> = Vec::new();
    for c in candidates {
        if let Some(&idx) = seen.get(&c.qualified_name) {
            if c.relevance_score > deduped[idx].relevance_score {
                deduped[idx] = c;
            }
        } else {
            seen.insert(c.qualified_name.clone(), deduped.len());
            deduped.push(c);
        }
    }

    // Phase 2b: Sort by relevance descending, cap at 10
    deduped.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap_or(std::cmp::Ordering::Equal));
    deduped.truncate(10);

    // Phase 3: Only NOW fetch rels/annotations for the final 10
    let mut results: Vec<graph::QueryResult> = Vec::new();
    for c in deduped {
        let passes = match trust_filter {
            "verified_only" => c.verification_status == "verified",
            "exclude_stale" => c.verification_status != "stale" && c.verification_status != "retracted",
            "exclude_retracted" => c.verification_status != "retracted",
            _ => true,
        };
        if !passes {
            continue;
        }

        let rels = graph::get_relationships_for_entity(storage, &c.id)?;
        let anns = graph::get_annotations_for_entity(storage, &c.id)?;

        results.push(graph::QueryResult {
            id: c.id,
            entity_type: c.entity_type,
            name: c.name,
            qualified_name: c.qualified_name,
            file_path: c.file_path,
            start_line: c.start_line,
            end_line: c.end_line,
            verification_status: c.verification_status,
            confidence: c.confidence,
            signature: c.signature,
            relevance_score: c.relevance_score,
            relationships: rels,
            annotations: anns,
        });
    }

    Ok(results)
}

/// FTS5 search returning lightweight candidates (no rels/annotations, no deadlock).
fn search_fts_lightweight(storage: &Storage, query: &str, limit: usize) -> Result<Vec<SeedCandidate>> {
    storage.with_conn(|conn| {
        if query.is_empty() {
            let mut stmt = conn.prepare(
                "SELECT id, entity_type, name, qualified_name, file_path,
                        start_line, end_line, properties,
                        verification_status, confidence
                 FROM entities ORDER BY updated_at DESC LIMIT ?1"
            )?;
            let mut rows = stmt.query(rusqlite::params![limit as u32])?;
            let mut out = Vec::new();
            while let Some(row) = rows.next()? {
                out.push(row_to_seed_candidate(row, 0.5)?);
            }
            return Ok(out);
        }

        let fts_query = query
            .split_whitespace()
            .map(|w| {
                let clean = w.replace('"', "").replace('\'', "");
                format!("\"{}\" OR {}*", clean, clean)
            })
            .collect::<Vec<_>>()
            .join(" OR ");

        let mut stmt = conn.prepare(
            "SELECT e.id, e.entity_type, e.name, e.qualified_name, e.file_path,
                    e.start_line, e.end_line, e.properties,
                    e.verification_status, e.confidence,
                    -rank as relevance
             FROM entities e
             JOIN entities_fts f ON e.rowid = f.rowid
             WHERE entities_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2"
        )?;
        let mut rows = stmt.query(rusqlite::params![fts_query, limit as u32])?;
        let mut entities = Vec::new();
        while let Some(row) = rows.next()? {
            let raw_score: f64 = row.get(10)?;
            entities.push(row_to_seed_candidate(row, raw_score)?);
        }

        if entities.is_empty() {
            return Ok(entities);
        }

        // Normalize scores to 0.3-1.0
        let max_s = entities.iter().map(|e| e.relevance_score).fold(f64::NEG_INFINITY, f64::max);
        let min_s = entities.iter().map(|e| e.relevance_score).fold(f64::INFINITY, f64::min);
        let range = max_s - min_s;
        for e in &mut entities {
            e.relevance_score = if range > 0.001 {
                0.3 + 0.7 * ((e.relevance_score - min_s) / range)
            } else {
                1.0
            };
        }

        Ok(entities)
    })
}

/// LIKE search returning lightweight candidates (no rels/annotations, no deadlock).
fn search_like_lightweight(storage: &Storage, keyword: &str, limit: usize) -> Result<Vec<SeedCandidate>> {
    storage.with_conn(|conn| {
        let pattern = format!("%{}%", keyword);
        let mut stmt = conn.prepare(
            "SELECT id, entity_type, name, qualified_name, file_path,
                    start_line, end_line, properties,
                    verification_status, confidence
             FROM entities
             WHERE (name LIKE ?1 OR file_path LIKE ?1)
             AND entity_type IN ('function', 'file', 'class', 'type_definition')
             LIMIT ?2"
        )?;
        let mut rows = stmt.query(rusqlite::params![pattern, limit as u32])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(row_to_seed_candidate(row, 0.4)?);
        }
        Ok(out)
    })
}

/// Convert a row from a lightweight SELECT (10 columns) into a SeedCandidate.
fn row_to_seed_candidate(row: &rusqlite::Row, default_score: f64) -> Result<SeedCandidate> {
    let props_str: String = row.get(7)?;
    let properties: serde_json::Value = serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));
    let sig = properties.get("signature").and_then(|v| v.as_str()).map(String::from);

    Ok(SeedCandidate {
        id: row.get(0)?,
        entity_type: row.get(1)?,
        name: row.get(2)?,
        qualified_name: row.get(3)?,
        file_path: row.get(4)?,
        start_line: row.get::<_, Option<i32>>(5)?.map(|v| v as u32),
        end_line: row.get::<_, Option<i32>>(6)?.map(|v| v as u32),
        verification_status: row.get(8)?,
        confidence: row.get(9)?,
        signature: sig,
        relevance_score: default_score,
    })
}

fn extract_keywords(task: &str) -> Vec<String> {
    const STOP_WORDS: &[&str] = &[
        "a", "an", "the", "in", "on", "at", "to", "for", "of", "with",
        "is", "are", "was", "were", "be", "been", "being",
        "i", "me", "my", "we", "our", "you", "your",
        "it", "its", "this", "that", "these", "those",
        "and", "or", "but", "not", "no", "do", "does",
        "has", "have", "had", "will", "would", "could", "should",
        "get", "got", "need", "want", "use", "find", "show",
        "how", "what", "where", "when", "which", "who",
        "fix", "bug", "issue", "problem", "help", "please",
        "can", "all", "about", "up", "down", "out", "into",
    ];

    task.to_lowercase()
        .split_whitespace()
        .filter(|w| w.len() > 2)
        .filter(|w| !STOP_WORDS.contains(w))
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|w| !w.is_empty())
        .collect()
}

/// Generate a directory-level project map.
fn generate_project_map(storage: &Storage) -> Result<ProjectMap> {
    storage.with_conn(|conn| {
        let project_name: String = conn.query_row(
            "SELECT name FROM entities WHERE entity_type = 'project' LIMIT 1",
            [],
            |row| row.get(0),
        ).unwrap_or_else(|_| "Project".to_string());

        let total_files: i64 = conn.query_row(
            "SELECT COUNT(*) FROM entities WHERE entity_type = 'file'",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        let mut stmt = conn.prepare(
            "SELECT file_path FROM entities WHERE entity_type = 'file' AND file_path IS NOT NULL"
        )?;
        let paths: Vec<String> = stmt.query_map([], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();

        let mut dir_files: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        for p in &paths {
            let normalized = p.replace('\\', "/");
            let top = normalized.split('/').next().unwrap_or(&normalized);
            *dir_files.entry(top.to_string()).or_insert(0) += 1;
        }

        let mut dirs: Vec<(String, i64)> = dir_files.into_iter().collect();
        dirs.sort_by(|a, b| b.1.cmp(&a.1));

        let mut structure = Vec::new();
        for (dir, file_count) in dirs.iter().take(15) {
            let pattern = format!("{}%", dir);
            let func_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM entities WHERE entity_type = 'function' AND file_path LIKE ?1",
                rusqlite::params![pattern],
                |row| row.get(0),
            ).unwrap_or(0);

            structure.push(format!("{} — {} files, {} functions", dir, file_count, func_count));
        }

        let overview = format!(
            "{} — {} files across {} directories",
            project_name,
            total_files,
            dirs.len()
        );

        Ok(ProjectMap {
            overview,
            structure,
        })
    })
}

fn estimate_tokens(text: &str) -> u32 {
    (text.len() as f64 / 4.0).ceil() as u32
}
