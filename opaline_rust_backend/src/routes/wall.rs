use axum::{extract::Path, extract::State, Json, Router, routing::{get, post, put, delete}};
use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;
use sqlx::Row;
use crate::state::AppState;
use crate::routes::auth::user_id_from_token;
use uuid::Uuid;
use mongodb::bson::doc;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_posts).post(create_post))
        .route("/:id", put(update_post).delete(delete_post))
        .route("/:id/reportar", post(report_post))
}

#[derive(Deserialize)]
struct CreatePostReq { content: String, rating: Option<i32> }

#[derive(Deserialize)]
struct UpdatePostReq { content: String }

#[derive(Deserialize)]
struct ReportReq { reason: String, detail: Option<String> }

async fn list_posts(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let rows = sqlx::query("SELECT p.id,p.user_id,p.content,p.rating,p.status,p.created_at,u.name FROM posts p JOIN users u ON u.id=p.user_id WHERE p.status='approved' ORDER BY p.created_at DESC")
        .fetch_all(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(serde_json::json!({
            "id": r.get::<String,_>("id"),
            "user_id": r.get::<String,_>("user_id"),
            "alias": r.get::<String,_>("name"),
            "content": r.get::<String,_>("content"),
            "rating": r.get::<Option<i32>,_>("rating"),
            "status": r.get::<String,_>("status"),
            "created_at": r.get::<chrono::NaiveDateTime,_>("created_at").to_string(),
            "plan": "Opaline",
        }));
    }
    Ok(Json(serde_json::json!(out)))
}

async fn create_post(headers: HeaderMap, State(state): State<AppState>, Json(req): Json<CreatePostReq>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    if req.content.len() < 20 || req.content.len() > 1000 { return Err(StatusCode::BAD_REQUEST); }
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO posts (id,user_id,content,rating,status) VALUES (?,?,?,?,'approved')")
        .bind(&id).bind(&uid).bind(&req.content).bind(&req.rating)
        .execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let _ = state.mongo.collection::<mongodb::bson::Document>("wall_events").insert_one(doc!{"event":"post_created","user_id":&uid,"post_id":&id,"ts":mongodb::bson::DateTime::now()}).await;
    Ok(Json(serde_json::json!({"id":id})))
}

async fn update_post(Path(id): Path<String>, headers: HeaderMap, State(state): State<AppState>, Json(req): Json<UpdatePostReq>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    // only owner and within 24h (simplify: owner)
    let res = sqlx::query("UPDATE posts SET content=? WHERE id=? AND user_id=?")
        .bind(&req.content).bind(&id).bind(&uid).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if res.rows_affected() == 0 { return Err(StatusCode::FORBIDDEN); }
    Ok(Json(serde_json::json!({"ok":true})))
}

async fn delete_post(Path(id): Path<String>, headers: HeaderMap, State(state): State<AppState>) -> Result<StatusCode, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let res = sqlx::query("UPDATE posts SET status='deleted' WHERE id=? AND user_id=?")
        .bind(&id).bind(&uid).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if res.rows_affected() == 0 { return Err(StatusCode::FORBIDDEN); }
    Ok(StatusCode::NO_CONTENT)
}

async fn report_post(Path(id): Path<String>, headers: HeaderMap, State(state): State<AppState>, Json(req): Json<ReportReq>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let rid = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO reports (id,post_id,user_id,reason,detail) VALUES (?,?,?,?,?)")
        .bind(&rid).bind(&id).bind(&uid).bind(&req.reason).bind(&req.detail)
        .execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // if 3 reports, hide
    let cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM reports WHERE post_id=?").bind(&id).fetch_one(&state.pool).await.unwrap_or(0);
    if cnt >= 3 {
        sqlx::query("UPDATE posts SET status='hidden' WHERE id=?").bind(&id).execute(&state.pool).await.ok();
    }
    Ok(Json(serde_json::json!({"ok":true})))
}
