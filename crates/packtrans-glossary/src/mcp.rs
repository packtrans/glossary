use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Router;
use clap::Args;
use packtrans_glossary_core::util;
use rmcp::{
    ErrorData, ServiceExt,
    handler::server::wrapper::Parameters,
    model::ErrorCode,
    schemars, tool, tool_router,
    transport::{
        StreamableHttpService, streamable_http_server::session::local::LocalSessionManager,
    },
};
use serde::Deserialize;

use crate::indexes;
use crate::query::{QueryOptions, search_index, validate_regex_query};
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
    ) -> Result<String, ErrorData> {
        if params.lang.is_empty() {
            return Err(invalid_params("missing `lang`"));
        }
        if params.q.is_empty() {
            return Err(invalid_params("missing search text (`q`)"));
        }
        let limit =
            validate_query_limit(params.limit).map_err(|err| invalid_params(err.to_string()))?;
        validate_regex_query(&params.lang, &params.q, params.inverse, params.regex)
            .map_err(|err| invalid_params(err.to_string()))?;
        // Reject malformed language codes up front so they surface as invalid
        // parameters instead of a later path-resolution failure.
        util::validate_path_segment(&params.lang, "lang")
            .map_err(|err| invalid_params(err.to_string()))?;

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
            .map_err(|err| internal_error(format!("search task panicked: {err}")))?
            .map_err(search_error)?;

        serde_json::to_string(&hits).map_err(|err| internal_error(err.to_string()))
    }

    #[tool(description = "List language codes available in the latest release glossary index")]
    async fn glossary_list_languages(&self) -> Result<String, ErrorData> {
        let guard = Arc::clone(&self.state.download_guard);
        let langs =
            tokio::task::spawn_blocking(move || indexes::list_release_languages(Some(&guard)))
                .await
                .map_err(|err| internal_error(format!("task panicked: {err}")))?
                .map_err(|err| internal_error(err.to_string()))?;

        serde_json::to_string(&langs).map_err(|err| internal_error(err.to_string()))
    }

    #[tool(description = "List glossary indexes currently installed locally")]
    async fn glossary_list_installed(&self) -> Result<String, ErrorData> {
        let index_dir = self.state.index_dir.clone();
        let entries = tokio::task::spawn_blocking(move || {
            indexes::list_downloaded_indexes(index_dir.as_deref())
        })
        .await
        .map_err(|err| internal_error(format!("task panicked: {err}")))?
        .map_err(|err| internal_error(err.to_string()))?;

        serde_json::to_string(&entries).map_err(|err| internal_error(err.to_string()))
    }
}

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
    let service = StreamableHttpService::new(
        move || Ok(server.clone()),
        LocalSessionManager::default().into(),
        Default::default(),
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

fn invalid_params(message: impl Into<Cow<'static, str>>) -> ErrorData {
    ErrorData::invalid_params(message, None)
}

fn internal_error(message: impl Into<Cow<'static, str>>) -> ErrorData {
    let message = message.into();
    eprintln!("internal error: {message}");
    ErrorData::internal_error(message, None)
}

/// Maps a `search_index` failure to an MCP error: input-dependent failures
/// (bad language code, unparseable query) become `invalid_params`, everything
/// else (missing index, download or I/O failure) becomes `internal_error`.
fn search_error(err: anyhow::Error) -> ErrorData {
    let message = err.to_string();
    let code = if message.contains("lang contains invalid path component")
        || message.contains("failed to parse regex query")
        || message.contains("QueryParserError")
    {
        ErrorCode::INVALID_PARAMS
    } else {
        ErrorCode::INTERNAL_ERROR
    };
    if code == ErrorCode::INTERNAL_ERROR {
        eprintln!("internal error: {message}");
    }
    ErrorData::new(code, message, None)
}

#[cfg(test)]
mod tests {
    use rmcp::model::ErrorCode;

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

    #[tokio::test]
    async fn glossary_query_rejects_invalid_input() {
        let server = server();

        let err = server
            .glossary_query(Parameters(params("", "test")))
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("lang"));

        let err = server
            .glossary_query(Parameters(params("zh_cn", "")))
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("q"));

        let mut bad_limit = params("zh_cn", "test");
        bad_limit.limit = Some(0);
        let err = server
            .glossary_query(Parameters(bad_limit))
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("limit"));

        let mut traversal = params("../etc", "test");
        traversal.q = "test".to_string();
        let err = server
            .glossary_query(Parameters(traversal))
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("lang"));
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
    fn search_error_classifies_input_vs_internal() {
        let err = search_error(anyhow::anyhow!(
            "lang contains invalid path component: ../etc"
        ));
        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);

        let err = search_error(anyhow::anyhow!("failed to parse regex query: cook["));
        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);

        let err = search_error(anyhow::anyhow!(
            "QueryParserError: SyntaxError at position 0"
        ));
        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);

        let err = search_error(anyhow::anyhow!(
            "index directory does not exist: indexes/zh_cn"
        ));
        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
    }
}
