mod defaults;

pub use defaults::*;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AespConfig {
    pub project: ProjectConfig,
    pub indexing: IndexingConfig,
    pub watcher: WatcherConfig,
    pub context_compiler: ContextCompilerConfig,
    pub verification: VerificationConfig,
    pub constraints: ConstraintsConfig,
    pub memory: MemoryConfig,
    pub storage: StorageConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub schema: String,
    pub languages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingConfig {
    pub ignore_patterns: Vec<String>,
    pub max_file_size_kb: u64,
    pub follow_symlinks: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherConfig {
    pub enabled: bool,
    pub debounce_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextCompilerConfig {
    pub default_token_budget: u32,
    pub keyword_match_weight: f64,
    pub proximity_weight: f64,
    pub recency_weight: f64,
    pub annotation_weight: f64,
    pub decision_weight: f64,
    pub importance_weight: f64,
    pub trust_weight: f64,
    pub constraint_budget_percent: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationConfig {
    pub default_staleness_ttl_indexed: u64,
    pub default_staleness_ttl_tool_returned: u64,
    pub default_staleness_ttl_agent_inferred: u64,
    pub auto_staleness_check: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintsConfig {
    pub inject_in_every_pack: bool,
    pub hard_constraint_format: String,
    pub soft_constraint_format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub archive_session_on_end: bool,
    pub session_memory_ttl_hours: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub db_path: String,
    pub wal_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file: String,
}

impl AespConfig {
    pub fn default_for_project(project_path: &Path, schema: &str) -> Self {
        let name = project_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project")
            .to_string();

        AespConfig {
            project: ProjectConfig {
                name,
                schema: schema.to_string(),
                languages: vec!["typescript".into(), "python".into()],
            },
            indexing: IndexingConfig {
                ignore_patterns: vec![
                    "*.test.ts".into(),
                    "*.spec.ts".into(),
                    "fixtures/".into(),
                    "*.generated.*".into(),
                ],
                max_file_size_kb: 500,
                follow_symlinks: false,
            },
            watcher: WatcherConfig {
                enabled: true,
                debounce_ms: 500,
            },
            context_compiler: ContextCompilerConfig {
                default_token_budget: 8000,
                keyword_match_weight: 0.25,
                proximity_weight: 0.20,
                recency_weight: 0.15,
                annotation_weight: 0.12,
                decision_weight: 0.10,
                importance_weight: 0.05,
                trust_weight: 0.13,
                constraint_budget_percent: 5,
            },
            verification: VerificationConfig {
                default_staleness_ttl_indexed: 86400,
                default_staleness_ttl_tool_returned: 3600,
                default_staleness_ttl_agent_inferred: 1800,
                auto_staleness_check: true,
            },
            constraints: ConstraintsConfig {
                inject_in_every_pack: true,
                hard_constraint_format: "⚠️ CONSTRAINT: {rule}".into(),
                soft_constraint_format: "💡 GUIDELINE: {rule}".into(),
            },
            memory: MemoryConfig {
                archive_session_on_end: true,
                session_memory_ttl_hours: 24,
            },
            storage: StorageConfig {
                db_path: ".aesp/graph.db".into(),
                wal_mode: true,
            },
            logging: LoggingConfig {
                level: "info".into(),
                file: ".aesp/aesp.log".into(),
            },
        }
    }

    pub fn load_from_project(project_path: &Path) -> Result<Self> {
        let config_path = project_path.join(".aesp/config.toml");
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: AespConfig = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Self::default_for_project(project_path, "code"))
        }
    }

    pub fn db_path(&self, project_path: &Path) -> PathBuf {
        project_path.join(&self.storage.db_path)
    }
}
