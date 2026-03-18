use serde_json::json;

pub fn get_tool_definitions() -> Vec<serde_json::Value> {
    vec![
        json!({
            "name": "aesp_start_task",
            "description": "START HERE — Call this before reading any files. Gets you oriented with relevant code, active constraints, past decisions, and project structure in one call. Returns everything you need to understand the codebase for your current task. Much faster and more accurate than reading files manually.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task": {
                        "type": "string",
                        "description": "What are you trying to do? Examples: 'fix user notification emails', 'add version control to documents', 'refactor auth system'"
                    }
                },
                "required": ["task"]
            }
        }),
        json!({
            "name": "aesp_query",
            "description": "Search the project's world graph for entities matching your query. Use natural language like 'what handles authentication' or keywords like 'user notifications'. Returns ranked entities with relationships and trust status.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query — natural language or keywords. Examples: 'user authentication', 'notification handler', 'database schema', 'API routes'"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of results to return (default 10)"
                    },
                    "include_relationships": {
                        "type": "boolean",
                        "description": "Whether to include relationship data for each result (default true)"
                    },
                    "depth": {
                        "type": "integer",
                        "description": "Graph traversal depth for expanding related entities (default 2)"
                    },
                    "trust_filter": {
                        "type": "string",
                        "description": "Filter results by trust status: 'all' returns everything, 'verified_only' returns only verified entities, 'exclude_stale' hides stale entities",
                        "enum": ["all", "verified_only", "exclude_stale", "exclude_retracted"]
                    }
                },
                "required": ["query"]
            }
        }),
        json!({
            "name": "aesp_write",
            "description": "Write information to the world graph. Add annotations to entities, create new entities, create relationships between entities, or update entity properties.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "description": "The write operation to perform: 'annotate' adds a note to an entity, 'create_entity' creates a new entity, 'create_relationship' links two entities, 'update_entity' modifies properties",
                        "enum": ["annotate", "create_entity", "create_relationship", "update_entity"]
                    },
                    "target": {
                        "type": "string",
                        "description": "Qualified name of the target entity, e.g. 'app/auth/login.tsx::handleLogin'"
                    },
                    "data": {
                        "type": "object",
                        "description": "Operation-specific data. For 'annotate': {type, content, tags}. For 'create_entity': {entity_type, properties}. For 'create_relationship': {source, target, relationship_type}. For 'update_entity': {properties}."
                    }
                },
                "required": ["operation", "target", "data"]
            }
        }),
        json!({
            "name": "aesp_context_pack",
            "description": "Get a curated, relevance-ranked context package for your current task within a token budget. Returns the most relevant entities, active constraints, trust summary, project map, and past decision history.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task": {
                        "type": "string",
                        "description": "Natural language description of the task you are working on. Examples: 'fix compliance checking workflow', 'add batch record validation', 'refactor authentication'"
                    },
                    "token_budget": {
                        "type": "integer",
                        "description": "Maximum token budget for the context package (default 8000)"
                    },
                    "focus_entities": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "List of entity qualified names to prioritize in the context package"
                    },
                    "include_decisions": {
                        "type": "boolean",
                        "description": "Whether to include past decision history (default true)"
                    },
                    "include_constraints": {
                        "type": "boolean",
                        "description": "Whether to include active constraints (default true)"
                    },
                    "trust_filter": {
                        "type": "string",
                        "description": "Filter entities by trust status",
                        "enum": ["all", "verified_only", "exclude_stale"]
                    }
                },
                "required": ["task"]
            }
        }),
        json!({
            "name": "aesp_decision_log",
            "description": "Record what you tried, what happened, and what you learned — or query past decisions to avoid repeating mistakes.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "description": "Whether to 'record' a new decision or 'query' past decisions",
                        "enum": ["record", "query"]
                    },
                    "task": {
                        "type": "string",
                        "description": "Description of the task this decision relates to"
                    },
                    "attempt": {
                        "type": "object",
                        "description": "For 'record': details of what was tried. Include 'approach' (string) and optionally 'approach_type' (string)."
                    },
                    "result": {
                        "type": "object",
                        "description": "For 'record': outcome of the attempt. Include 'outcome' ('success'|'failure'|'partial') and optionally 'evidence' (string)."
                    },
                    "learnings": {
                        "type": "object",
                        "description": "For 'record': what was learned. Include 'what_failed', 'root_cause', 'recommendations' (all strings)."
                    },
                    "query_filter": {
                        "type": "object",
                        "description": "For 'query': filter criteria. Include 'outcome' ('success'|'failure') and 'limit' (integer)."
                    }
                },
                "required": ["action", "task"]
            }
        }),
        json!({
            "name": "aesp_graph_view",
            "description": "Get a structural view of the world graph around a specific entity. Shows connected entities up to a given depth.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "root": {
                        "type": "string",
                        "description": "Qualified name of the root entity to start from, or '.' for the project root. Example: 'app/auth/login.tsx::handleLogin'"
                    },
                    "depth": {
                        "type": "integer",
                        "description": "How many relationship hops to traverse from the root (default 2)"
                    },
                    "view_type": {
                        "type": "string",
                        "description": "Type of graph view to generate",
                        "enum": ["tree", "dependencies", "call_graph", "reverse_dependencies"]
                    },
                    "filter_types": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Only include entities of these types, e.g. ['function', 'class']"
                    }
                },
                "required": ["root"]
            }
        }),
        json!({
            "name": "aesp_status",
            "description": "Get the current status of the AESP world graph — total entity counts by type, relationship counts, annotation counts, and verification status breakdown.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        json!({
            "name": "aesp_reindex",
            "description": "Trigger reindexing of the project. Use mode 'full' to rebuild the entire index, or 'path' to reindex specific files or directories.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "mode": {
                        "type": "string",
                        "description": "Reindex mode: 'full' rebuilds the entire project index, 'path' reindexes only the specified paths",
                        "enum": ["full", "path"]
                    },
                    "paths": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "For mode 'path': list of file or directory paths to reindex, relative to project root"
                    }
                },
                "required": ["mode"]
            }
        }),
        json!({
            "name": "aesp_session",
            "description": "Manage your working session. Start a new session, end the current one, or get info about recent sessions. Session-scoped constraints and memory are archived when a session ends.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "description": "Session action: 'start' begins a new session, 'end' closes the current session, 'info' lists recent sessions",
                        "enum": ["start", "end", "info"]
                    },
                    "task_description": {
                        "type": "string",
                        "description": "For 'start': a description of the task for this session"
                    }
                },
                "required": ["action"]
            }
        }),
        json!({
            "name": "aesp_verify",
            "description": "Update the trust status of an entity. Use after confirming facts are correct (verify), finding contradictions (contradict), noticing staleness (mark_stale), or retracting incorrect data (retract).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target": {
                        "type": "string",
                        "description": "Qualified name of the entity to update trust for, e.g. 'app/auth/login.tsx::handleLogin'"
                    },
                    "target_type": {
                        "type": "string",
                        "description": "Whether the target is an 'entity' or an 'annotation'",
                        "enum": ["entity", "annotation"]
                    },
                    "action": {
                        "type": "string",
                        "description": "Trust action: 'verify' confirms accuracy, 'contradict' marks as wrong, 'mark_stale' flags as potentially outdated, 'retract' removes from active use",
                        "enum": ["verify", "contradict", "mark_stale", "retract"]
                    },
                    "evidence": {
                        "type": "string",
                        "description": "Evidence or reason for this trust update"
                    },
                    "new_confidence": {
                        "type": "number",
                        "description": "New confidence score between 0.0 and 1.0"
                    },
                    "contradicting_fact": {
                        "type": "string",
                        "description": "For 'contradict': the correct fact that contradicts the current data"
                    }
                },
                "required": ["target", "action"]
            }
        }),
        json!({
            "name": "aesp_constrain",
            "description": "Manage constraints — persistent rules injected into every context pack. Use to add safety rules, quality requirements, or workflow policies that the agent must follow.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "description": "Constraint action: 'add' creates a new constraint, 'remove' deletes one by ID, 'list' shows all active constraints",
                        "enum": ["add", "remove", "list"]
                    },
                    "rule": {
                        "type": "string",
                        "description": "For 'add': the constraint rule text, e.g. 'Never modify production database tables directly'"
                    },
                    "scope": {
                        "type": "string",
                        "description": "Constraint scope: 'session' lasts until session ends, 'persistent' survives across sessions",
                        "enum": ["session", "persistent"]
                    },
                    "severity": {
                        "type": "string",
                        "description": "Constraint severity: 'hard' must never be violated, 'soft' is a preference",
                        "enum": ["hard", "soft"]
                    },
                    "category": {
                        "type": "string",
                        "description": "Constraint category for organization",
                        "enum": ["safety", "quality", "workflow", "access_control", "custom"]
                    },
                    "constraint_id": {
                        "type": "string",
                        "description": "For 'remove': the ID of the constraint to remove"
                    }
                },
                "required": ["action"]
            }
        }),
        json!({
            "name": "aesp_ingest_tool_result",
            "description": "Pass raw tool output through AESP's normalizer to extract structured facts and update the world graph. Reduces token waste by converting verbose output into structured data.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tool_name": {
                        "type": "string",
                        "description": "Name of the tool that produced the output, e.g. 'grep', 'git_log', 'test_runner'"
                    },
                    "raw_output": {
                        "type": "string",
                        "description": "The raw text output from the tool"
                    },
                    "context": {
                        "type": "string",
                        "description": "What you were trying to do when you ran the tool"
                    },
                    "related_entities": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Qualified names of entities related to this tool output"
                    },
                    "output_format": {
                        "type": "string",
                        "description": "Format hint for the raw output",
                        "enum": ["json", "log", "text", "csv", "auto"]
                    }
                },
                "required": ["tool_name", "raw_output", "context"]
            }
        }),
        json!({
            "name": "aesp_inspect",
            "description": "Inspect the event ledger to understand what happened and why. Query the timeline, trace entity history, review session events, or find contradictions.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query_type": {
                        "type": "string",
                        "description": "Type of inspection: 'timeline' shows recent events, 'entity_history' shows changes to a specific entity, 'session_events' shows events in a session, 'contradictions' shows detected contradictions",
                        "enum": ["timeline", "entity_history", "session_events", "contradictions", "event_type_filter"]
                    },
                    "target": {
                        "type": "string",
                        "description": "For 'entity_history': the entity ID. For 'session_events': the session ID. For 'event_type_filter': the event type to filter by."
                    },
                    "time_range": {
                        "type": "object",
                        "description": "Optional time range filter with 'from' and 'to' ISO date strings",
                        "properties": {
                            "from": { "type": "string", "description": "Start of time range (ISO date)" },
                            "to": { "type": "string", "description": "End of time range (ISO date)" }
                        }
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of events to return (default 50)"
                    }
                },
                "required": ["query_type"]
            }
        }),
    ]
}
