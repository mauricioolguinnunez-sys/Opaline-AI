use axum::{extract::{Query, State}, Json, Router, routing::get};
use axum::http::{HeaderMap, StatusCode};
use crate::state::AppState;
use crate::routes::auth::user_id_from_token;
use crate::quota;

#[derive(serde::Deserialize)]
struct HistoryQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_usage))
        .route("/history", get(get_history))
        .route("/summary", get(get_summary))
}

async fn uid_from_headers(headers: &HeaderMap, state: &AppState) -> Result<String, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    user_id_from_token(state, &t).await.ok_or(StatusCode::UNAUTHORIZED)
}

async fn get_usage(headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let uid = uid_from_headers(&headers, &state).await?;
    let usage = quota::get_current_usage(&state.pool, &uid).await?;
    Ok(Json(usage))
}

async fn get_history(headers: HeaderMap, Query(q): Query<HistoryQuery>, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let uid = uid_from_headers(&headers, &state).await?;
    let limit = q.limit.unwrap_or(50);
    let offset = q.offset.unwrap_or(0);
    let history = quota::get_usage_history(&state.pool, &uid, limit, offset).await?;
    Ok(Json(history))
}

async fn get_summary(headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let uid = uid_from_headers(&headers, &state).await?;
    let summary = quota::get_usage_summary(&state.pool, &uid).await?;
    Ok(Json(summary))
}