use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use clap::Args;
use serde::Deserialize;

use crate::query::{QueryHit, QueryOptions, search_index, validate_http_limit};

#[derive(Args)]
pub struct ServeCommand {
    /// Socket address to bind (for example `127.0.0.1:8080`).
    #[arg(long, default_value = "127.0.0.1:8080")]
    pub bind: String,

    /// Local index root directory; queries `{index_dir}/{lang}` (same layout as `index --out`).
    #[arg(long)]
    pub index_dir: Option<PathBuf>,
}

#[derive(Clone)]
struct AppState {
    index_dir: Option<PathBuf>,
    dict_path: Option<PathBuf>,
}

#[derive(Deserialize)]
struct HttpQueryParams {
    lang: String,
    /// Search text (`q` or `query` query parameter).
    #[serde(alias = "query")]
    q: String,
    limit: Option<usize>,
    #[serde(default)]
    inverse: bool,
}

#[derive(serde::Serialize)]
struct ErrorBody {
    error: String,
}

pub async fn run(cmd: ServeCommand, dict_path: Option<PathBuf>) -> Result<()> {
    let addr: SocketAddr = cmd
        .bind
        .parse()
        .with_context(|| format!("invalid bind address: {}", cmd.bind))?;

    let state = Arc::new(AppState {
        index_dir: cmd.index_dir,
        dict_path,
    });

    let app = Router::new()
        .route("/query", get(query_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind to {addr}"))?;
    eprintln!("listening on http://{addr}");
    axum::serve(listener, app)
        .await
        .context("HTTP server exited with an error")?;
    Ok(())
}

async fn query_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HttpQueryParams>,
) -> Response {
    match handle_query(&state, params).await {
        Ok(hits) => Json(hits).into_response(),
        Err(err) => err.into_response(),
    }
}

async fn handle_query(
    state: &AppState,
    params: HttpQueryParams,
) -> Result<Vec<QueryHit>, ApiError> {
    if params.q.is_empty() {
        return Err(ApiError::bad_request(
            "missing search text (`q` or `query`)",
        ));
    }
    if params.lang.is_empty() {
        return Err(ApiError::bad_request("missing `lang`"));
    }
    let limit =
        validate_http_limit(params.limit).map_err(|e| ApiError::bad_request(e.to_string()))?;
    search_index(QueryOptions {
        query: params.q,
        index_dir: state.index_dir.clone(),
        lang: params.lang,
        limit,
        inverse: params.inverse,
        dict_path: state.dict_path.clone(),
    })
    .map_err(ApiError::internal)
}

enum ApiError {
    BadRequest(String),
    Internal(anyhow::Error),
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }

    fn internal(err: anyhow::Error) -> Self {
        Self::Internal(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::Internal(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
        };
        (status, Json(ErrorBody { error: message })).into_response()
    }
}
