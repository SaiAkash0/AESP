# Contributing to AESP

Thanks for your interest in contributing to AESP. This document covers how to set up the project for development, run tests, and submit changes.

## Prerequisites

- [Rust](https://rustup.rs/) stable toolchain (1.75+)
- Git

No other system dependencies are needed — SQLite is bundled via `rusqlite`, and Tree-sitter grammars are compiled from source.

## Build from Source

```bash
git clone https://github.com/SaiAkash0/aesp.git
cd aesp
cargo build --release
```

The binary is at `./target/release/aesp` (or `aesp.exe` on Windows).

## Run Tests

```bash
cargo test
```

There is a sample TypeScript project in `tests/fixtures/sample_ts_project/` that the integration tests use for indexing.

## Test with a Real Project

```bash
# Index a project
./target/release/aesp init /path/to/your/project

# Check status
./target/release/aesp status /path/to/your/project

# Query something
./target/release/aesp query "authentication" --path /path/to/your/project

# Start the MCP server
./target/release/aesp serve /path/to/your/project
```

## Connect to Cursor

Create `.cursor/mcp.json` in your project root:

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

Restart Cursor. The 13 AESP tools will appear in the MCP panel.

## Project Structure

```
src/
  main.rs           CLI entry point (clap)
  lib.rs            Module re-exports
  config/           Configuration loading (.aesp/config.toml)
  storage/          SQLite storage layer, migrations, query constants
  graph/            Entity, relationship, annotation CRUD + query engine + BFS traversal
  parser/           Tree-sitter parsing (TypeScript, Python, generic fallback)
  indexer/          Two-pass project indexing pipeline
  compiler/         Context compiler with keyword extraction and token budgeting
  decisions/        Decision log system
  verification/     Trust layer (verify, contradict, stale, retract)
  constraints/      Constraint engine
  events/           State event ledger
  normalizer/       Tool output normalizer
  schema/           Schema registry and validation
  mcp/              MCP server (transport, protocol, tool definitions, handlers)
  watcher/          File watcher for incremental indexing
schemas/
  code.json         Code domain schema definition
tests/
  fixtures/         Sample projects for testing
```

## Code Style

- Rust stable, no nightly features
- No `unsafe` blocks
- Standard `rustfmt` formatting
- Use `anyhow::Result` for error propagation in application code
- Use `thiserror` for library-level error types
- Debug logging via `eprintln!` in MCP server code (goes to stderr, visible in Cursor's MCP log)
- Tracing via `tracing::info!` / `tracing::debug!` in CLI and indexer code

## Adding a New Language Parser

1. Add the Tree-sitter grammar crate to `Cargo.toml`
2. Add language detection in `src/parser/languages/mod.rs`
3. Implement the parser in `src/parser/treesitter.rs` following the TypeScript or Python patterns
4. The parser should extract: file entities, function/class/type entities, import relationships, call relationships, and signatures

## Adding a New MCP Tool

1. Add the tool definition in `src/mcp/tools.rs` (with `inputSchema`, `description`, and `required` fields)
2. Add the handler match arm in `src/mcp/handlers/mod.rs`
3. Implement the handler function
4. Wrap the response with `mcp_text_result()` for proper MCP formatting

## Submitting Changes

1. Fork the repo
2. Create a feature branch (`git checkout -b feature/my-change`)
3. Make your changes
4. Run `cargo test` and `cargo clippy`
5. Commit with a clear message
6. Open a pull request

## License

By contributing to AESP, you agree that your contributions will be licensed under the MIT License.
