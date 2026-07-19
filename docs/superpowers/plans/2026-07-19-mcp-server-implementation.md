# MCP Server Implementation Plan

**Date:** 2026-07-19  
**Design spec:** [`docs/superpowers/specs/2026-07-19-mcp-server-design.md`](../specs/2026-07-19-mcp-server-design.md)  
**Branch:** `cursor/mcp-server-impl-7c6e` (off `master`)

## Overview

Implement `packtrans-glossary mcp` — an MCP server exposing three tools over stdio (default) or streamable HTTP (`--http`), reusing existing query/index infrastructure.

## Task checklist

- [ ] **Task 1:** Add dependencies and shared modules
- [ ] **Task 2:** Extract `AppState` and `validate_query_limit`
- [ ] **Task 3:** Expose `fetch_available_languages` from `indexes.rs`
- [ ] **Task 4:** Implement `mcp.rs` (tools + transports)
- [ ] **Task 5:** Wire `mcp` subcommand in `main.rs`
- [ ] **Task 6:** Unit tests
- [ ] **Task 7:** Update `AGENTS.md`
- [ ] **Task 8:** Verify (`cargo test`, `cargo clippy`, manual smoke)

---

## Task 1: Add dependencies

**File:** `crates/packtrans-glossary/Cargo.toml`

Add:

```toml
rmcp = { version = "2.2", features = [
  "server",
  "macros",
  "schemars",
  "transport-io",
  "transport-streamable-http-server",
] }
schemars = "1"
```

Extend tokio features for HTTP server:

```toml
tokio = { version = "1", features = ["macros", "rt-multi-thread", "net", "signal"] }
```

**Verify:** `cargo check -p packtrans-glossary`

---

## Task 2: Extract shared state and validation

### 2a. Create `src/app_state.rs`

Move from `serve.rs`:

```rust
pub struct AppState {
    pub index_dir: Option<PathBuf>,
    pub dict_path: Option<PathBuf>,
    pub download_guard: Arc<DownloadCoordinator>,
    pub dict_cache: DictionaryCache,
    pub index_cache: IndexCache,
}

impl AppState {
    pub fn new(index_dir: Option<PathBuf>, dict_path: Option<PathBuf>) -> Arc<Self> { ... }
}
```

### 2b. Create `src/query_limit.rs`

Extract `validate_http_limit` from `serve.rs` → rename to `validate_query_limit` (shared by serve + mcp):

```rust
pub fn validate_query_limit(limit: Option<usize>) -> Result<usize>
```

Move the existing unit test from `serve.rs` to `query_limit.rs`.

### 2c. Update `serve.rs`

- `use crate::app_state::AppState`
- `use crate::query_limit::validate_query_limit`
- Remove local `AppState` and `validate_http_limit`
- Replace `AppState { ... }` construction with `AppState::new(cmd.index_dir, dict_path)`

**Verify:** `cargo test -p packtrans-glossary serve::`

---

## Task 3: Expose language listing

**File:** `crates/packtrans-glossary/src/indexes.rs`

Add public wrapper (keep `fetch_available_languages` private):

```rust
/// Returns language codes available in the latest release index.
pub fn list_release_languages() -> Result<Vec<String>> {
    let release = fetch_latest_release()?;
    fetch_available_languages(&release.tag_name)
}
```

If `fetch_latest_release` is private, either expose it as `pub(crate)` or inline the latest-release lookup inside `list_release_languages`.

**Verify:** existing `indexes` tests still pass.

---

## Task 4: Implement `src/mcp.rs`

### 4a. CLI args

```rust
#[derive(Args)]
pub struct McpCommand {
    #[arg(long)]
    pub http: bool,
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    #[arg(long, default_value_t = 8081)]
    pub port: u16,
    #[arg(long)]
    pub index_dir: Option<PathBuf>,
}
```

### 4b. Server struct

```rust
#[derive(Clone)]
pub struct GlossaryMcpServer {
    state: Arc<AppState>,
}

#[tool_router(server_handler)]
impl GlossaryMcpServer {
    // tools defined here
}
```

### 4c. Tool: `glossary_query`

Parameter struct:

```rust
#[derive(Debug, Deserialize, JsonSchema)]
struct GlossaryQueryParams {
    lang: String,
    q: String,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    inverse: bool,
}
```

Handler (async):

1. Validate `lang` and `q` non-empty
2. `validate_query_limit(params.limit)?`
3. Build `QueryOptions` from `self.state`
4. `tokio::task::spawn_blocking(|| search_index(options)).await??`
5. Return `CallToolResult` with JSON content (`serde_json::to_string(&hits)?`)

Map validation errors → `ErrorData` with invalid-params code.  
Map operational errors → log to stderr, return internal error message.

### 4d. Tool: `glossary_list_languages`

No parameters. Call `indexes::list_release_languages()` in `spawn_blocking`. Return JSON array.

### 4e. Tool: `glossary_list_installed`

No parameters. Call `indexes::list_downloaded_indexes(state.index_dir.as_deref())` in `spawn_blocking`. Return JSON array (serialize `IndexEntry`; add `Serialize` derive if missing).

### 4f. `run()` dispatch

```rust
pub async fn run(cmd: McpCommand, dict_path: Option<PathBuf>) -> Result<()> {
    let server = GlossaryMcpServer {
        state: AppState::new(cmd.index_dir, dict_path),
    };

    if cmd.http {
        run_http(server, &cmd.host, cmd.port).await
    } else {
        run_stdio(server).await
    }
}
```

**stdio:**

```rust
async fn run_stdio(server: GlossaryMcpServer) -> Result<()> {
    eprintln!("packtrans-glossary MCP server (stdio)");
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
```

**HTTP:**

```rust
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager,
    tower::StreamableHttpService,
};

async fn run_http(server: GlossaryMcpServer, host: &str, port: u16) -> Result<()> {
    let bind_addr = format!("{host}:{port}");
    let service = StreamableHttpService::new(
        move || Ok(server.clone()),
        LocalSessionManager::default().into(),
        Default::default(),
    );
    let app = Router::new().nest_service("/mcp", service);
    let listener = TcpListener::bind(&bind_addr).await?;
    eprintln!("listening on http://{bind_addr}/mcp");
    eprintln!("note: MCP HTTP mode is experimental and for local use only");
    axum::serve(listener, app)
        .with_graceful_shutdown(async { tokio::signal::ctrl_c().await.ok(); })
        .await?;
    Ok(())
}
```

**Note:** If `GlossaryMcpServer` cannot be `Clone` due to rmcp internals, use `Arc<GlossaryMcpServer>` in the factory closure instead.

**Verify:** `cargo build -p packtrans-glossary`

---

## Task 5: Wire subcommand in `main.rs`

```rust
mod app_state;
mod mcp;
mod query_limit;

// In Commands enum:
/// Start an MCP server for glossary tools (stdio or HTTP).
Mcp(mcp::McpCommand),

// In match:
Commands::Mcp(cmd) => {
    let rt = tokio::runtime::Runtime::new().context("failed to start async runtime")?;
    rt.block_on(mcp::run(cmd, cli.dict_path))
}
```

**Verify:** `packtrans-glossary mcp --help` shows flags.

---

## Task 6: Unit tests

**File:** `src/mcp.rs` (or `query_limit.rs`)

- Reuse/locate `validate_query_limit` tests (moved in Task 2)
- Add tests for MCP param validation helpers if extracted (e.g. `validate_glossary_query_params`)

Optional: test `list_release_languages` with mocked HTTP is out of scope; rely on manual check.

**Verify:** `cargo test -p packtrans-glossary`

---

## Task 7: Update `AGENTS.md`

Add under "Running the CLIs":

```sh
# MCP server (stdio — for Cursor/Claude Desktop MCP config)
cargo run --bin packtrans-glossary -- mcp

# MCP server (streamable HTTP)
cargo run --bin packtrans-glossary -- mcp --http --port 8081
```

Include Cursor stdio config JSON from design spec.

---

## Task 8: Verification

```sh
cargo fmt -- --check
cargo clippy -p packtrans-glossary -- -D warnings
cargo test -p packtrans-glossary
cargo build --release -p packtrans-glossary
```

**Manual smoke (if index cached):**

```sh
# Terminal 1 — stdio (will block; confirm no panic on startup)
packtrans-glossary mcp

# Terminal 2 — HTTP
packtrans-glossary mcp --http --port 8081
# curl -s http://127.0.0.1:8081/mcp  (expect MCP protocol response, not 404)
```

Full MCP tool invocation requires an MCP client (Cursor config or MCP Inspector).

---

## Risk notes

| Risk | Mitigation |
|---|---|
| `rmcp` 2.x API differs from examples | Pin `2.2`; adjust imports if `StreamableHttpService` path changed |
| `IndexEntry` lacks `Serialize` | Add `#[derive(Serialize)]` |
| Stateful HTTP sessions | Use `LocalSessionManager` per rmcp examples; document `/mcp` endpoint path |
| Blocking index download on first query | Same behavior as `serve`; acceptable for v1 |
| `GlossaryMcpServer` Clone for HTTP factory | Use `Arc` wrapper if needed |

## Out of scope (defer)

- MCP prompts/resources
- Index download/delete MCP tools
- Integration test harness for full stdio protocol
- Publishing separate MCP-only binary
