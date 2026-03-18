# AESP — Agent Execution & State Protocol

> Agents don't fail because they lack intelligence.
> They fail because they lose state.

**AESP is a local-first context engine for AI coding agents.** It gives your agent persistent memory, structured search, decision history, and active constraints — all via the [Model Context Protocol (MCP)](https://modelcontextprotocol.io).

```
You: "Fix the user notification bug"

Without AESP:  Agent reads 20 files, forgets what it learned, retries the same fix twice.
With AESP:     Agent gets relevant code + past decisions + constraints in one call. Ships the fix.
```

---

## The Problem

Every AI agent today suffers from six fundamental limitations:

1. **Context Amnesia** — Agents lose understanding as conversations grow. Information read 50 messages ago is effectively forgotten as it falls out of the active context window or gets buried in noise.

2. **Flat, Unstructured Memory** — Conversation history is a linear text stream. Agents cannot query it ("what did I learn about the database schema?"). They must re-read, re-parse, and re-discover — wasting tokens and time.

3. **No Learning Within a Session** — When an agent tries an approach that fails, that knowledge exists only in the conversation log. There is no structured record of "tried X, failed because Y, learned Z" that can be efficiently retrieved later in the same task or across tasks.

4. **No Shared Understanding** — In multi-agent systems, agents communicate via message passing — lossy, slow, and unstructured. There is no shared "whiteboard" or ground truth.

5. **No Trust Awareness** — Agents treat all information equally. A fact from a verified API response, a user's casual remark, and the agent's own inference are all just text in the context window. There is no provenance, no confidence tracking, no way to distinguish verified truth from stale assumption.

6. **No Constraint Memory** — Agents receive instructions at the start ("only use verified data", "don't modify the database schema") but these constraints drift out of the context window during long tasks. There is no durable, enforceable constraint system.

## The Solution

AESP provides a **persistent, structured, queryable world model** that sits between AI agents and their operating environment. It is the missing infrastructure primitive — an agent's working memory, externalized and structured.

- **MCP** solved: "How do agents talk to tools?" (the nervous system)
- **AESP** solves: "How do agents understand and remember their environment?" (the working memory)

## Design Philosophy

1. **Local-first** — AESP runs as a local daemon. No cloud dependency. Data never leaves the machine.
2. **Protocol, not product** — AESP is an open spec. Any agent framework can integrate it.
3. **MCP-native** — AESP exposes itself as an MCP server. Any MCP-compatible agent can use it immediately with zero custom integration.
4. **Write-friendly** — Agents should write to the world graph as easily as they read from it. The graph improves as the agent works.
5. **Token-aware** — The Context Compiler understands token budgets and assembles maximally relevant context within constraints.
6. **Schema-driven** — The world graph uses typed schemas. Different domains (code, research, workflows) have different schemas. Schemas are extensible.
7. **Trust-aware** — Every piece of data in the graph carries provenance and verification status. Agents can distinguish verified facts from inferences and stale data.
8. **Constraint-enforced** — Active constraints are persisted and respected by the Context Compiler. Constraints never drift out of context.
9. **Event-sourced** — Every state change is recorded in an append-only event ledger, enabling full replay, debugging, and audit trails.

---

## 30-Second Install

**Prerequisites:** [Rust toolchain](https://rustup.rs/) (stable)

```bash
# Clone and build
git clone https://github.com/SaiAkash0/aesp.git
cd aesp
cargo build --release

# Index your project
./target/release/aesp init /path/to/your/project
```

**Verify it worked:**

```bash
./target/release/aesp status /path/to/your/project
```

You should see entity counts, relationship counts, and a breakdown by type.

---

## Cursor Setup

Add AESP to your Cursor MCP configuration. Create or edit `.cursor/mcp.json` in your project root:

```json
{
  "mcpServers": {
    "aesp": {
      "command": "/absolute/path/to/aesp",
      "args": ["serve", "/absolute/path/to/your/project"]
    }
  }
}
```

**Windows example:**

```json
{
  "mcpServers": {
    "aesp": {
      "command": "C:/tools/aesp.exe",
      "args": ["serve", "C:/Users/you/projects/my-app"]
    }
  }
}
```

**Windows path formats (both valid):**

**Option A (recommended): forward slashes**

```json
{
  "mcpServers": {
    "aesp": {
      "command": "C:/Users/you/Desktop/AESP/target/release/aesp.exe",
      "args": ["serve", "C:/Users/you/projects/your-project"]
    }
  }
}
```

**Option B: backslashes (must be double-escaped in JSON)**

```json
{
  "mcpServers": {
    "aesp": {
      "command": "C:\\\\Users\\\\you\\\\Desktop\\\\AESP\\\\target\\\\release\\\\aesp.exe",
      "args": ["serve", "C:\\\\Users\\\\you\\\\projects\\\\your-project"]
    }
  }
}
```

Restart Cursor. You should see AESP's 13 tools appear in the MCP panel. The agent will automatically call `aesp_start_task` at the beginning of any task.

---

## How It Works

```
┌─────────────────────┐
│  AI Agent (Cursor)   │
│                     │
│  "Fix the auth bug" │
└─────────┬───────────┘
          │ aesp_start_task
          ▼
┌─────────────────────┐
│   AESP MCP Server   │
│                     │
│  Context Compiler   │──▶ Ranked entities matching your task
│  Decision Log       │──▶ Past attempts and learnings
│  Constraint Engine  │──▶ Rules the agent must follow
│  World Graph        │──▶ Full codebase structure
└─────────┬───────────┘
          │
          ▼
┌─────────────────────┐
│   SQLite Database    │
│                     │
│  Entities           │  functions, classes, types, files
│  Relationships      │  imports, calls, contains
│  Annotations        │  agent notes and discoveries
│  Decisions          │  what worked, what didn't
│  Constraints        │  persistent rules
│  Events             │  full audit trail
└─────────────────────┘
```

AESP parses your codebase with [Tree-sitter](https://tree-sitter.github.io/tree-sitter/) (TypeScript and Python supported), builds a structured graph, and serves it over MCP. Everything stays local — no cloud, no external calls, no data leaves your machine.

---

## The 13 MCP Tools

| Tool | What it does |
|------|-------------|
| **`aesp_start_task`** | **Start here.** Gets relevant context, active constraints, and past decisions for your task in one call |
| `aesp_query` | Search the codebase by keyword or natural language |
| `aesp_write` | Add annotations, create entities, or create relationships in the graph |
| `aesp_context_pack` | Get relevance-ranked context within a token budget |
| `aesp_decision_log` | Record or query past decisions and learnings |
| `aesp_verify` | Mark entities as verified, stale, contradicted, or retracted |
| `aesp_constrain` | Add persistent rules the agent must follow |
| `aesp_graph_view` | Explore entity relationships from any starting point |
| `aesp_status` | Check graph statistics — entity counts, relationship counts, verification breakdown |
| `aesp_reindex` | Rebuild or incrementally update the project index |
| `aesp_session` | Start, end, or inspect working sessions |
| `aesp_ingest_tool_result` | Feed raw tool output into the graph as structured facts |
| `aesp_inspect` | Query the event ledger for debugging and auditing |

---

## Example: Start a Task

When you ask the agent to work on something, it calls `aesp_start_task` automatically:

```
Agent calls: aesp_start_task({ task: "fix user notification emails" })

Response:
  session_id: "a1b2c3..."
  message: "AESP Context Loaded."
  tip: "I found 10 relevant entities. The most relevant are:
        app/api/notifications/route.ts,
        lib/email/sender.ts,
        types/index.ts::Notification.
        There is 1 active constraint to respect."
  context_pack: { ...ranked entities, project map, constraints, decisions... }
```

The agent now has everything it needs without reading a single file manually.

## Example: Query the Codebase

```
Agent calls: aesp_query({ query: "authentication middleware" })

Response:
  - lib/auth/requireAuth.ts (score: 0.95)
  - middleware.ts::config (score: 0.72)
  - app/api/auth/[...nextauth]/route.ts (score: 0.68)
```

Results are ranked by FTS5 BM25 relevance — not random.

## Example: Record a Decision

```
Agent calls: aesp_decision_log({
  action: "record",
  task: "fix notification emails",
  attempt: { approach: "Added sendEmail call in PATCH handler" },
  result: { outcome: "failure", evidence: "Emails sent but not persisted to DB" },
  learnings: { root_cause: "Missing await on async DB insert", recommendations: "Always await DB operations in API routes" }
})
```

Next time the agent works on notifications, it will see this decision and avoid the same mistake.

---

## CLI Reference

```bash
aesp init <path>              # Index a project and create .aesp/ directory
aesp serve <path>             # Start the MCP server for a project
aesp query <search> --path <path>  # Search entities from the command line
aesp status <path>            # Show graph statistics
aesp reindex <path>           # Rebuild the project index
aesp inspect <path>           # View the event ledger
```

---

## Supported Languages

| Language | Parser | Entities Extracted |
|----------|--------|--------------------|
| TypeScript / JavaScript | Tree-sitter | Functions, classes, types, interfaces, enums, variables, imports, call relationships |
| Python | Tree-sitter | Functions, classes, imports, call relationships |
| Other files | Generic | File-level entities with metadata |

Adding new language support requires implementing a Tree-sitter parser in `src/parser/treesitter.rs`.

---

## Privacy & Security

- **100% local.** AESP runs entirely on your machine. No cloud services, no telemetry, no external network calls.
- **Data stays in your project.** The `.aesp/` directory contains a SQLite database with the index. Delete it anytime with `rm -rf .aesp/`.
- **Gitignored by default.** The `.aesp/` directory and database files are in `.gitignore` — nothing gets committed to your repo.

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for build instructions, development setup, and contribution guidelines.

---

## License

MIT License. See [LICENSE](LICENSE) for details.

Copyright (c) 2026 Akash
