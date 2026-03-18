# Changelog

All notable changes to AESP will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-03-18

### Added

- **World Graph Engine** — SQLite-backed entity and relationship storage with full CRUD operations
- **Tree-sitter parsing** for TypeScript, JavaScript, and Python — extracts functions, classes, types, interfaces, enums, variables, imports, and call relationships
- **FTS5 full-text search** with BM25 relevance ranking and LIKE fallback
- **Context Compiler** — keyword-aware seed selection, token budgeting, tiered entity packing, and directory-level project maps
- **Decision Log** — record what you tried, what happened, and what you learned; query past decisions to avoid repeating mistakes
- **Constraint Engine** — persistent rules (session or global scope) injected into every context pack
- **Verification & Trust Layer** — verify, contradict, mark_stale, and retract entities with confidence scoring
- **State Event Ledger** — append-only audit trail of all system state changes
- **Tool Output Normalizer** — ingest raw tool output as structured facts
- **13 MCP tools** — `aesp_start_task`, `aesp_query`, `aesp_write`, `aesp_context_pack`, `aesp_decision_log`, `aesp_verify`, `aesp_constrain`, `aesp_graph_view`, `aesp_status`, `aesp_reindex`, `aesp_session`, `aesp_ingest_tool_result`, `aesp_inspect`
- **CLI** — `init`, `serve`, `query`, `status`, `reindex`, `inspect` commands
- **Two-pass indexing** with entity deduplication, ID remapping, and graceful handling of external dependencies
- **MCP transport** — Content-Length framed and bare JSON line support
- **MCP server instructions** — `instructions` field in initialize response guides agents to call `aesp_start_task` first
