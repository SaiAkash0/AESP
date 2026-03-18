use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "aesp", version, about = "Agent Execution & State Protocol")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize AESP for a project
    Init {
        /// Path to the project directory
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Schema to use (default: auto-detect)
        #[arg(long, default_value = "code")]
        schema: String,

        /// Start file watcher after init
        #[arg(long)]
        watch: bool,
    },

    /// Start the MCP server
    Serve {
        /// Path to the project directory
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Log level
        #[arg(long, default_value = "info")]
        log_level: String,
    },

    /// Query the world graph
    Query {
        /// Search query
        query: String,

        /// Filter by entity type
        #[arg(long, short = 't')]
        r#type: Option<String>,

        /// Graph traversal depth
        #[arg(long, short = 'd', default_value = "2")]
        depth: u32,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Project path
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },

    /// Show project graph status
    Status {
        /// Project path
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Verbose output
        #[arg(long, short = 'v')]
        verbose: bool,
    },

    /// Reindex the project
    Reindex {
        /// Project path
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Full reindex (default: incremental)
        #[arg(long)]
        full: bool,

        /// Specific path to reindex
        #[arg(long)]
        target: Option<PathBuf>,
    },

    /// Inspect the event ledger
    Inspect {
        /// Project path
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Show recent timeline
        #[arg(long)]
        timeline: bool,

        /// Show history for an entity
        #[arg(long)]
        entity: Option<String>,

        /// Show events for a session
        #[arg(long)]
        session: Option<String>,

        /// Show contradictions
        #[arg(long)]
        contradictions: bool,

        /// Limit results
        #[arg(long, default_value = "50")]
        limit: u32,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("aesp=info".parse()?))
        .with_target(false)
        .init();

    match cli.command {
        Commands::Init {
            path,
            schema,
            watch,
        } => cmd_init(path, schema, watch),
        Commands::Serve { path, log_level } => cmd_serve(path, log_level),
        Commands::Query {
            query,
            r#type,
            depth,
            json,
            path,
        } => cmd_query(path, query, r#type, depth, json),
        Commands::Status { path, verbose } => cmd_status(path, verbose),
        Commands::Reindex { path, full, target } => cmd_reindex(path, full, target),
        Commands::Inspect {
            path,
            timeline,
            entity,
            session,
            contradictions,
            limit,
        } => cmd_inspect(path, timeline, entity, session, contradictions, limit),
    }
}

fn cmd_init(path: PathBuf, schema: String, _watch: bool) -> Result<()> {
    let path = std::fs::canonicalize(&path)?;
    tracing::info!("Initializing AESP for project at: {}", path.display());

    let config = aesp::config::AespConfig::default_for_project(&path, &schema);
    let aesp_dir = path.join(".aesp");
    std::fs::create_dir_all(&aesp_dir)?;

    let config_path = aesp_dir.join("config.toml");
    let config_str = toml::to_string_pretty(&config)?;
    std::fs::write(&config_path, config_str)?;
    tracing::info!("Created config at: {}", config_path.display());

    let db_path = aesp_dir.join("graph.db");
    let storage = aesp::storage::Storage::open(&db_path)?;
    storage.run_migrations()?;
    tracing::info!("Created database at: {}", db_path.display());

    let schema_registry = aesp::schema::SchemaRegistry::new();
    let code_schema = schema_registry.get_schema("code")?;
    tracing::info!("Loaded schema: {} v{}", code_schema.name, code_schema.version);

    tracing::info!("Indexing project...");
    let stats = aesp::indexer::index_project(&path, &storage, &code_schema, &config)?;
    tracing::info!(
        "Indexed {} files, {} entities, {} relationships",
        stats.files_indexed,
        stats.entities_created,
        stats.relationships_created
    );

    println!("AESP initialized successfully!");
    println!("  Project: {}", path.display());
    println!("  Schema: {}", schema);
    println!("  Files indexed: {}", stats.files_indexed);
    println!("  Entities: {}", stats.entities_created);
    println!("  Relationships: {}", stats.relationships_created);
    println!("\nAdd to your MCP config:");
    println!("  {{");
    println!("    \"mcpServers\": {{");
    println!("      \"aesp\": {{");
    println!("        \"command\": \"aesp\",");
    println!("        \"args\": [\"serve\", \"--project\", \"{}\"]", path.display());
    println!("      }}");
    println!("    }}");
    println!("  }}");

    Ok(())
}

fn cmd_serve(path: PathBuf, _log_level: String) -> Result<()> {
    let path = std::fs::canonicalize(&path)?;
    let db_path = path.join(".aesp/graph.db");
    if !db_path.exists() {
        anyhow::bail!("AESP not initialized. Run `aesp init` first.");
    }

    let storage = aesp::storage::Storage::open(&db_path)?;
    let config = aesp::config::AespConfig::load_from_project(&path)?;

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(aesp::mcp::serve(storage, config, path))?;
    Ok(())
}

fn cmd_query(
    path: PathBuf,
    query: String,
    type_filter: Option<String>,
    depth: u32,
    json_output: bool,
) -> Result<()> {
    let path = std::fs::canonicalize(&path)?;
    let db_path = path.join(".aesp/graph.db");
    if !db_path.exists() {
        anyhow::bail!("AESP not initialized. Run `aesp init` first.");
    }

    let storage = aesp::storage::Storage::open(&db_path)?;
    let results = aesp::graph::query_entities(
        &storage,
        &query,
        type_filter.as_deref(),
        depth,
        "all",
        10,
    )?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        if results.is_empty() {
            println!("No results found for: {}", query);
        } else {
            println!("Found {} results for \"{}\":\n", results.len(), query);
            for result in &results {
                let trust_badge = match result.verification_status.as_str() {
                    "verified" => "✓",
                    "stale" => "⏳",
                    "contradicted" => "⚠",
                    "retracted" => "✗",
                    _ => "?",
                };
                println!(
                    "  [{}] {} ({}) — {}",
                    trust_badge, result.qualified_name, result.entity_type, result.name
                );
                if let Some(ref sig) = result.signature {
                    println!("      {}", sig);
                }
            }
        }
    }

    Ok(())
}

fn cmd_status(path: PathBuf, verbose: bool) -> Result<()> {
    let path = std::fs::canonicalize(&path)?;
    let db_path = path.join(".aesp/graph.db");
    if !db_path.exists() {
        anyhow::bail!("AESP not initialized. Run `aesp init` first.");
    }

    let storage = aesp::storage::Storage::open(&db_path)?;
    let status = aesp::graph::get_status(&storage)?;

    println!("AESP Status for: {}", path.display());
    println!("  Entities: {}", status.total_entities);
    println!("  Relationships: {}", status.total_relationships);
    println!("  Annotations: {}", status.total_annotations);
    println!("  Decisions: {}", status.total_decisions);
    println!("  Events: {}", status.total_events);
    println!("  Constraints: {}", status.active_constraints);

    if verbose {
        println!("\nEntities by type:");
        for (entity_type, count) in &status.entities_by_type {
            println!("  {}: {}", entity_type, count);
        }
        println!("\nVerification status:");
        for (vstatus, count) in &status.entities_by_verification {
            println!("  {}: {}", vstatus, count);
        }
    }

    Ok(())
}

fn cmd_reindex(path: PathBuf, full: bool, target: Option<PathBuf>) -> Result<()> {
    let path = std::fs::canonicalize(&path)?;
    let db_path = path.join(".aesp/graph.db");
    if !db_path.exists() {
        anyhow::bail!("AESP not initialized. Run `aesp init` first.");
    }

    let storage = aesp::storage::Storage::open(&db_path)?;
    let config = aesp::config::AespConfig::load_from_project(&path)?;
    let schema_registry = aesp::schema::SchemaRegistry::new();
    let code_schema = schema_registry.get_schema("code")?;

    if full || target.is_none() {
        tracing::info!("Full reindex...");
        let stats = aesp::indexer::index_project(&path, &storage, &code_schema, &config)?;
        println!(
            "Reindexed: {} files, {} entities, {} relationships",
            stats.files_indexed, stats.entities_created, stats.relationships_created
        );
    } else if let Some(target_path) = target {
        tracing::info!("Incremental reindex of: {}", target_path.display());
        let stats =
            aesp::indexer::index_path(&path, &target_path, &storage, &code_schema, &config)?;
        println!(
            "Reindexed: {} files, {} entities, {} relationships",
            stats.files_indexed, stats.entities_created, stats.relationships_created
        );
    }

    Ok(())
}

fn cmd_inspect(
    path: PathBuf,
    timeline: bool,
    entity: Option<String>,
    session: Option<String>,
    contradictions: bool,
    limit: u32,
) -> Result<()> {
    let path = std::fs::canonicalize(&path)?;
    let db_path = path.join(".aesp/graph.db");
    if !db_path.exists() {
        anyhow::bail!("AESP not initialized. Run `aesp init` first.");
    }

    let storage = aesp::storage::Storage::open(&db_path)?;

    if let Some(entity_name) = entity {
        let events = aesp::events::query_by_target(&storage, &entity_name, limit)?;
        println!("Events for entity \"{}\":", entity_name);
        for event in &events {
            println!(
                "  [{}] {} — {} {} ({})",
                event.timestamp, event.event_type, event.operation, event.target_type, event.target_id
            );
        }
    } else if let Some(session_id) = session {
        let events = aesp::events::query_by_session(&storage, &session_id, limit)?;
        println!("Events for session \"{}\":", session_id);
        for event in &events {
            println!(
                "  [{}] {} — {} {}",
                event.timestamp, event.event_type, event.operation, event.target_type
            );
        }
    } else if contradictions {
        let events = aesp::events::query_by_type(&storage, "contradiction_detected", limit)?;
        println!("Contradictions:");
        for event in &events {
            println!("  [{}] {} — {}", event.timestamp, event.target_id, event.operation);
        }
    } else if timeline {
        let events = aesp::events::query_timeline(&storage, limit)?;
        println!("Recent events:");
        for event in &events {
            println!(
                "  [{}] {} — {} {} ({})",
                event.timestamp, event.event_type, event.operation, event.target_type, event.target_id
            );
        }
    } else {
        let events = aesp::events::query_timeline(&storage, limit)?;
        println!("Recent events (use --timeline, --entity, --session, or --contradictions):");
        for event in &events {
            println!(
                "  [{}] {} — {} {}",
                event.timestamp, event.event_type, event.operation, event.target_type
            );
        }
    }

    Ok(())
}
