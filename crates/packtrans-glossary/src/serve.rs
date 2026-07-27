use std::path::PathBuf;
use std::sync::Arc;

use crate::query::{QueryHit, QueryOptions, search_index, validate_regex_query};
use crate::{app_state::AppState, query::validate_query_limit};
use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::rejection::QueryRejection,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use clap::Args;
use packtrans_glossary_core::util;
use serde::Deserialize;

#[derive(Args)]
pub struct ServeCommand {
    /// Host address to bind.
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// TCP port to bind.
    #[arg(long, default_value_t = 8080)]
    pub port: u16,

    /// Local index root directory; queries `{index_dir}/{lang}` (same layout as `index --out`).
    #[arg(long)]
    pub index_dir: Option<PathBuf>,
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
    #[serde(default)]
    regex: bool,
}

#[derive(serde::Serialize)]
struct ErrorBody {
    error: String,
}

pub async fn run(cmd: ServeCommand, dict_path: Option<PathBuf>) -> Result<()> {
    let bind_addr = format!("{}:{}", cmd.host, cmd.port);

    let state = AppState::new(cmd.index_dir, dict_path);

    let app = Router::new()
        .route("/query", get(query_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .with_context(|| format!("failed to bind to {bind_addr}"))?;
    eprintln!("listening on http://{bind_addr}");
    eprintln!(
        "note: serve is experimental and for local use only; it is not intended for production or many parallel requests"
    );
    axum::serve(listener, app)
        .await
        .context("HTTP server exited with an error")?;
    Ok(())
}

async fn query_handler(
    State(state): State<Arc<AppState>>,
    params: Result<Query<HttpQueryParams>, QueryRejection>,
) -> Response {
    let Query(params) = match params {
        Ok(query) => query,
        Err(rejection) => return ApiError::bad_request(rejection.to_string()).into_response(),
    };

    match handle_query(state, params).await {
        Ok(hits) => Json(hits).into_response(),
        Err(err) => err.into_response(),
    }
}

async fn handle_query(
    state: Arc<AppState>,
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
        validate_query_limit(params.limit).map_err(|e| ApiError::bad_request(e.to_string()))?;
    validate_regex_query(&params.lang, &params.q, params.inverse, params.regex)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    util::validate_path_segment(&params.lang, "lang")
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    let options = QueryOptions {
        query: params.q,
        index_dir: state.index_dir.clone(),
        lang: params.lang,
        limit,
        inverse: params.inverse,
        regex: params.regex,
        dict_path: state.dict_path.clone(),
        download_guard: Some(Arc::clone(&state.download_guard)),
        dict_cache: Some(state.dict_cache.clone()),
        index_cache: Some(state.index_cache.clone()),
    };

    tokio::task::spawn_blocking(move || search_index(options))
        .await
        .map_err(|e| ApiError::internal_logged(anyhow::anyhow!("search task panicked: {e}")))?
        .map_err(ApiError::internal_logged)
}

enum ApiError {
    BadRequest(String),
    Internal,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }

    fn internal_logged(err: anyhow::Error) -> Self {
        eprintln!("internal error: {err:?}");
        Self::Internal
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal server error".to_string(),
            ),
        };
        (status, Json(ErrorBody { error: message })).into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::extract::Query;
    use axum::http::Uri;

    use super::*;

    #[test]
    fn http_regex_parameter_defaults_and_parses() {
        let Query(params) = Query::<HttpQueryParams>::try_from_uri(&Uri::from_static(
            "/query?lang=en_us&q=cook.*&regex=true",
        ))
        .unwrap();
        assert!(params.regex);

        let Query(params) =
            Query::<HttpQueryParams>::try_from_uri(&Uri::from_static("/query?lang=en_us&q=cook.*"))
                .unwrap();
        assert!(!params.regex);
    }
}
