# MCP Server for packtrans-glossary — Design Spec

**Date:** 2026-07-19  
**Status:** Approved (brainstorming)

## Overview

Add a `mcp` subcommand to the `packtrans-glossary` CLI that starts a Model Context Protocol (MCP) server. MCP clients (Cursor, Claude Desktop, VS Code) can call glossary search tools over stdio (default) or streamable HTTP (optional).

This mirrors the existing experimental `serve` HTTP API but speaks MCP instead of REST, exposing glossary query and minimal helper tools.

## Goals

- Enable AI assistants to search Minecraft mod translation glossaries via MCP
- Reuse existing `search_index()` and index-resolution logic — no duplicated Tantivy code
- stdio transport as the primary mode for local MCP client configs
- Optional HTTP transport for remote or network-based clients
- Minimal tool surface: query + lightweight discovery helpers only

## Non-Goals (v1)

- Full parity with `index` and `dict` CLI subcommands (no download/delete/upgrade tools)
- MCP prompts, resources, or completions
- Per-query `index_dir` override (startup flag only)
- Production-grade HTTP deployment (same experimental/local-only stance as `serve`)

## Requirements (from brainstorming)

| Topic | Decision |
|---|---|
| Transport | stdio default; optional `--http` for streamable HTTP |
| Tool scope | `glossary_query` + minimal helpers |
| Index source | `--index-dir` at startup; release auto-download when unset |

## CLI Interface

```
packtrans-glossary mcp [OPTIONS]
```

| Flag | Default | Description |
|---|---|---|
| *(none)* | — | stdio transport (for MCP client configs) |
| `--http` | off | Use streamable HTTP instead of stdio |
| `--host` | `127.0.0.1` | HTTP bind address (only with `--http`) |
| `--port` | `8081` | HTTP port (8081 to avoid clashing with `serve`'s 8080) |
| `--index-dir` | unset | Local index root; release downloads when unset |

Global `--dict-path` applies as today.

### Example Cursor config (stdio)

```json
{
  "mcpServers": {
    "packtrans-glossary": {
      "command": "packtrans-glossary",
      "args": ["mcp"]
    }
  }
}
```

### Logging

All diagnostics go to **stderr**. The stdio transport owns stdout for JSON-RPC. Startup messages and warnings use stderr, consistent with other MCP servers.

## MCP Tools

### `glossary_query`

Search glossary translations. Wraps existing `search_index()`.

| Parameter | Type | Required | Default | Notes |
|---|---|---|---|---|
| `lang` | string | yes | — | e.g. `zh_cn`, `ja_jp` |
| `q` | string | yes | — | search text |
| `limit` | integer | no | `10` | max `50` (same as HTTP `serve`) |
| `inverse` | boolean | no | `false` | search target language → source |

**Returns:** JSON array of `QueryHit` objects (same shape as `query --json`):

```json
[
  {
    "confidence": 12.5,
    "mod_id": "farmersdelight",
    "key": "block.farmersdelight.cooking_pot",
    "source": "Cooking Pot",
    "source_lang": "en_us",
    "target_lang": "zh_cn",
    "target": "厨锅"
  }
]
```

Index resolution uses the startup `--index-dir` (or release auto-download when unset).

### `glossary_list_languages`

List languages available in the **latest release** index.

**Parameters:** none

**Returns:** JSON array of language codes from `languages.json` on GitHub releases:

```json
["zh_cn", "ja_jp", "ko_kr", "de_de"]
```

Reuses `fetch_available_languages` logic in `indexes.rs` (expose as `pub(crate)` or thin public wrapper).

### `glossary_list_installed`

List indexes **currently installed locally** in the data directory (or under `--index-dir` if set).

**Parameters:** none

**Returns:** JSON array from `list_downloaded_indexes()`:

```json
[
  { "lang": "zh_cn", "version": "v1.2.0", "path": "/home/user/.local/share/..." }
]
```

## Architecture

### Approach

**Recommended:** `mcp` subcommand in the existing `packtrans-glossary` crate using the official [`rmcp`](https://github.com/modelcontextprotocol/rust-sdk) crate.

Alternatives considered and rejected:
- **Separate `packtrans-glossary-mcp` crate** — more isolation but requires library extraction; user requested a CLI subcommand
- **`rust-mcp-sdk`** — Axum-native but less official; stdio is primary use case and `rmcp` covers both transports

### Component diagram

```
packtrans-glossary mcp
  ├── McpCommand (clap args)
  ├── Transport
  │     ├── stdio (default) → rmcp::transport::stdio()
  │     └── --http → rmcp StreamableHttpService (Axum)
  └── GlossaryMcpServer (ServerHandler + #[tool_router])
        ├── AppState (shared with serve)
        ├── glossary_query → query::search_index() [spawn_blocking]
        ├── glossary_list_languages → indexes::fetch_available_languages()
        └── glossary_list_installed → indexes::list_downloaded_indexes()
```

### File layout

| File | Role |
|---|---|
| `src/mcp.rs` | `McpCommand`, `GlossaryMcpServer`, tool handlers, transport dispatch |
| `src/app_state.rs` | Shared `AppState` extracted from `serve.rs` (caches, paths) |
| `src/main.rs` | Add `Commands::Mcp`, dispatch to `mcp::run()` |
| `src/serve.rs` | Import shared `AppState` instead of local definition |

### Shared state

Extract `AppState` from `serve.rs` into `app_state.rs`:

- `index_dir: Option<PathBuf>`
- `dict_path: Option<PathBuf>`
- `download_guard: Arc<DownloadCoordinator>`
- `dict_cache: DictionaryCache`
- `index_cache: IndexCache`

Both `serve` and `mcp` construct the same state at startup.

### Blocking work

`search_index()` is synchronous. Run it in `tokio::task::spawn_blocking` inside the `glossary_query` tool handler (same pattern as `serve.rs`'s `query_handler`).

### Dependencies

Add to `crates/packtrans-glossary/Cargo.toml`:

```toml
rmcp = { version = "2", features = [
  "server",
  "macros",
  "schemars",
  "transport-io",
  "transport-streamable-http-server",
] }
schemars = "1"
```

Server info: name `packtrans-glossary`, version from `CARGO_PKG_VERSION`.

Use `rmcp` 2.x (current stable: 2.2.0). Pin to a compatible minor at implementation time.

## Error Handling

### Tool input validation

Return MCP `InvalidParams` / `ErrorData`:

- Empty `lang` or `q`
- `limit` out of range (0 or > 50) — same rules as `validate_http_limit` in `serve.rs`
- Invalid `lang` segment — propagate from `validate_path_segment`

### Operational errors

Return MCP internal error; log details to stderr:

- Index resolution / download failure
- `spawn_blocking` panic

### Network errors (`glossary_list_languages`)

GitHub fetch failure returns error with context (`"failed to fetch available languages"`).

### stdio mode

Never write non-protocol output to stdout.

### HTTP mode

Print experimental/local-only disclaimer on startup stderr (same as `serve`).

## Testing

### Unit tests

- Input validation helpers (limit bounds, empty fields) in `mcp.rs`
- Mirror `serve.rs` `validate_http_limit` tests if shared or duplicated

### Integration tests

Full stdio MCP protocol round-trip is hard in CI. Prefer testing tool handler inner functions directly with constructed `AppState`.

### Manual verification checklist

1. `packtrans-glossary mcp` — add to Cursor MCP config; confirm tools appear
2. `glossary_query` with `lang=zh_cn`, `q=Cooking Pot` — verify hits
3. `glossary_list_languages` — verify non-empty list
4. `glossary_list_installed` — verify local indexes
5. `packtrans-glossary mcp --http --port 8081` — connect via HTTP MCP client

## Documentation

Update `AGENTS.md` with the `mcp` subcommand and stdio Cursor config example.

## Implementation Notes

- Make `fetch_available_languages` accessible from `mcp.rs` (currently private in `indexes.rs`)
- `list_downloaded_indexes` is already `pub`
- Default HTTP port `8081` avoids conflict with `serve` default `8080`
- Consider sharing `validate_http_limit` between `serve.rs` and `mcp.rs` via a small shared helper
