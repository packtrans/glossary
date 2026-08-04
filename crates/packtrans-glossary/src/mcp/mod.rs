use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Router;
use clap::Args;
use packtrans_glossary_core::util;
use rmcp::{
    ServiceExt,
    handler::server::wrapper::{Json, Parameters},
    model::{CallToolResult, ContentBlock},
    schemars, tool, tool_router,
    transport::{
        StreamableHttpServerConfig, StreamableHttpService,
        streamable_http_server::session::local::LocalSessionManager,
    },
};
use serde::Deserialize;

use crate::index;
use crate::util::progress;
use crate::query::{
    QueryHit, QueryOptions, SearchFailureKind, classify_search_failure, search_index,
    validate_regex_query,
};
use crate::{app_state::AppState, query::validate_query_limit};

#[derive(Args)]
pub struct McpCommand {
    /// Use streamable HTTP instead of stdio.
    #[arg(long)]
    pub http: bool,

    /// HTTP bind address (only with `--http`).
    #[arg(long, default_value = "127.0.0.1", requires = "http")]
    pub host: String,

    /// HTTP port (only with `--http`).
    #[arg(long, default_value_t = 8081, requires = "http")]
    pub port: u16,

    /// Local index root directory; queries `{index_dir}/{lang}` (same layout as `index --out`).
    #[arg(long)]
    pub index_dir: Option<PathBuf>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GlossaryQueryParams {
    /// Target language code (e.g. `zh_cn`, `ja_jp`).
    lang: String,
    /// Search text.
    q: String,
    /// Maximum number of results (default 10, max 50).
    #[serde(default)]
    limit: Option<usize>,
    /// Search target-language text and return source-language matches.
    #[serde(default)]
    inverse: bool,
    /// Interpret `q` as a regular expression matching indexed terms.
    #[serde(default)]
    regex: bool,
}

// MCP requires tool outputSchema root type "object" — wrap Vec outputs.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct QueryHitsOutput {
    hits: Vec<QueryHit>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct LanguagesOutput {
    languages: Vec<String>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct InstalledIndexesOutput {
    indexes: Vec<InstalledIndex>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
struct InstalledIndex {
    lang: String,
    version: String,
}

#[derive(Clone)]
pub struct GlossaryMcpServer {
    state: Arc<AppState>,
}

#[tool_router(server_handler)]
impl GlossaryMcpServer {
    #[tool(description = "Search Minecraft mod glossary translations")]
    async fn glossary_query(
        &self,
        Parameters(params): Parameters<GlossaryQueryParams>,
    ) -> Result<Json<QueryHitsOutput>, CallToolResult> {
        if params.lang.is_empty() {
            return Err(tool_error("missing `lang`"));
        }
        if params.q.is_empty() {
            return Err(tool_error("missing search text (`q`)"));
        }
        let limit =
            validate_query_limit(params.limit).map_err(|err| tool_error(err.to_string()))?;
        validate_regex_query(&params.lang, &params.q, params.inverse, params.regex)
            .map_err(|err| tool_error(err.to_string()))?;
        util::validate_path_segment(&params.lang, "lang")
            .map_err(|err| tool_error(err.to_string()))?;

        let options = QueryOptions {
            query: params.q,
            index_dir: self.state.index_dir.clone(),
            lang: params.lang,
            limit,
            inverse: params.inverse,
            regex: params.regex,
            dict_path: self.state.dict_path.clone(),
            download_guard: Some(Arc::clone(&self.state.download_guard)),
            dict_cache: Some(self.state.dict_cache.clone()),
            index_cache: Some(self.state.index_cache.clone()),
        };

        let hits = tokio::task::spawn_blocking(move || search_index(options))
            .await
            .map_err(|err| tool_error(format!("search task panicked: {err}")))?
            .map_err(map_search_failure)?;

        Ok(Json(QueryHitsOutput { hits }))
    }

    #[tool(description = "List language codes available in the latest release glossary index")]
    async fn glossary_list_languages(&self) -> Result<Json<LanguagesOutput>, CallToolResult> {
        let guard = Arc::clone(&self.state.download_guard);
        let langs =
            tokio::task::spawn_blocking(move || index::list_release_languages(Some(&guard)))
                .await
                .map_err(|err| tool_error(format!("task panicked: {err}")))?
                .map_err(|err| tool_error(err.to_string()))?;

        Ok(Json(LanguagesOutput { languages: langs }))
    }

    #[tool(description = "List glossary indexes currently installed locally")]
    async fn glossary_list_installed(&self) -> Result<Json<InstalledIndexesOutput>, CallToolResult> {
        let index_dir = self.state.index_dir.clone();
        let entries = tokio::task::spawn_blocking(move || {
            index::list_downloaded_indexes(index_dir.as_deref()).map(|entries| {
                entries
                    .into_iter()
                    .map(|entry| InstalledIndex {
                        lang: entry.lang,
                        version: entry.version,
                    })
                    .collect::<Vec<_>>()
            })
        })
        .await
        .map_err(|err| tool_error(format!("task panicked: {err}")))?
        .map_err(|err| tool_error(err.to_string()))?;

        Ok(Json(InstalledIndexesOutput { indexes: entries }))
    }
}

pub async fn run(cmd: McpCommand, dict_path: Option<PathBuf>) -> Result<()> {
    progress::set_suppress_spinners(true);

    let server = GlossaryMcpServer {
        state: AppState::new(cmd.index_dir, dict_path),
    };

    if cmd.http {
        run_http(server, &cmd.host, cmd.port).await
    } else {
        run_stdio(server).await
    }
}

async fn run_stdio(server: GlossaryMcpServer) -> Result<()> {
    eprintln!("packtrans-glossary MCP server (stdio)");
    let service = server
        .serve(rmcp::transport::stdio())
        .await
        .context("failed to start MCP stdio server")?;
    service
        .waiting()
        .await
        .context("MCP stdio server exited with an error")?;
    Ok(())
}

async fn run_http(server: GlossaryMcpServer, host: &str, port: u16) -> Result<()> {
    let bind_addr = format!("{host}:{port}");
    let is_loopback_bind = matches!(host, "127.0.0.1" | "localhost" | "::1");
    let is_wildcard_bind = host == "0.0.0.0" || host == "::";

    if !is_loopback_bind && !is_wildcard_bind {
        eprintln!(
            "warning: MCP HTTP is intended for loopback use; binding to {host} may reject clients unless the Host header matches allowed_hosts"
        );
    }
    if is_wildcard_bind {
        eprintln!(
            "warning: binding to {host}; clients must send a loopback Host header (127.0.0.1, localhost, or ::1)"
        );
    }

    let config = if is_loopback_bind || is_wildcard_bind {
        StreamableHttpServerConfig::default()
    } else {
        StreamableHttpServerConfig::default().with_allowed_hosts([
            "localhost",
            "127.0.0.1",
            "::1",
            host,
            &format!("{host}:{port}"),
        ])
    };

    let service = StreamableHttpService::new(
        move || Ok(server.clone()),
        LocalSessionManager::default().into(),
        config,
    );
    let app = Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .with_context(|| format!("failed to bind to {bind_addr}"))?;
    eprintln!("listening on http://{bind_addr}/mcp");
    eprintln!(
        "note: MCP HTTP mode is experimental and for local use only; it is not intended for production or many parallel requests"
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await
        .context("MCP HTTP server exited with an error")?;
    Ok(())
}

fn tool_error(message: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(message.into())])
}

fn map_search_failure(err: anyhow::Error) -> CallToolResult {
    let kind = classify_search_failure(&err);
    let message = if kind == SearchFailureKind::MissingIndex {
        format!("{err}. Install an index with: packtrans-glossary index download --lang <lang>")
    } else {
        err.to_string()
    };
    if kind == SearchFailureKind::Internal {
        eprintln!("internal error: {message}");
    }
    CallToolResult::error(vec![ContentBlock::text(message)])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> GlossaryMcpServer {
        GlossaryMcpServer {
            state: AppState::new(None, None),
        }
    }

    fn params(lang: &str, q: &str) -> GlossaryQueryParams {
        GlossaryQueryParams {
            lang: lang.to_string(),
            q: q.to_string(),
            limit: None,
            inverse: false,
            regex: false,
        }
    }

    fn tool_error_text(result: CallToolResult) -> String {
        result
            .content
            .first()
            .and_then(|block| block.as_text())
            .map(|text| text.text.clone())
            .unwrap_or_default()
    }

    fn expect_tool_error(result: Result<Json<QueryHitsOutput>, CallToolResult>) -> CallToolResult {
        match result {
            Err(err) => err,
            Ok(_) => panic!("expected tool error"),
        }
    }

    #[tokio::test]
    async fn glossary_query_rejects_invalid_input() {
        let server = server();

        let err = expect_tool_error(server.glossary_query(Parameters(params("", "test"))).await);
        assert_eq!(err.is_error, Some(true));
        assert!(tool_error_text(err).contains("lang"));

        let err = expect_tool_error(server.glossary_query(Parameters(params("zh_cn", ""))).await);
        assert_eq!(err.is_error, Some(true));
        assert!(tool_error_text(err).contains("q"));

        let mut bad_limit = params("zh_cn", "test");
        bad_limit.limit = Some(0);
        let err = expect_tool_error(server.glossary_query(Parameters(bad_limit)).await);
        assert_eq!(err.is_error, Some(true));
        assert!(tool_error_text(err).contains("limit"));

        let err = expect_tool_error(
            server
                .glossary_query(Parameters(params("../etc", "test")))
                .await,
        );
        assert_eq!(err.is_error, Some(true));
        assert!(tool_error_text(err).contains("lang"));
    }

    #[tokio::test]
    async fn glossary_query_rejects_inverse_regex_for_cjk() {
        let server = server();
        let mut p = params("zh_cn", "test");
        p.inverse = true;
        p.regex = true;
        let err = expect_tool_error(server.glossary_query(Parameters(p)).await);
        assert_eq!(err.is_error, Some(true));
        assert!(tool_error_text(err).contains("regex"));
    }

    #[test]
    fn glossary_query_params_regex_defaults_and_parses() {
        let params: GlossaryQueryParams =
            serde_json::from_str(r#"{"lang":"en_us","q":"cook.*","regex":true}"#).unwrap();
        assert!(params.regex);

        let params: GlossaryQueryParams =
            serde_json::from_str(r#"{"lang":"en_us","q":"cook.*"}"#).unwrap();
        assert!(!params.regex);
    }

    #[test]
    fn map_search_failure_classifies_missing_index() {
        let result = map_search_failure(anyhow::anyhow!(
            "index directory does not exist: indexes/zh_cn"
        ));
        assert_eq!(result.is_error, Some(true));
        let text = tool_error_text(result);
        assert!(text.contains("index directory does not exist"));
        assert!(text.contains("index download"));
    }
}
