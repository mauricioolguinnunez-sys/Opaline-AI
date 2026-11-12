use axum::{extract::{Path, Query, State}, Json, Router, routing::{get, post, put, delete}};
use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;
use sqlx::Row;
use crate::state::AppState;
use crate::routes::auth::user_id_from_token;
use crate::routes::admin::require_admin;
use crate::outbox::NewOutboxEvent;
use uuid::Uuid;
use mongodb::bson::doc;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_posts).post(create_post))
        .route("/stats", get(wall_stats))
        .route("/moderacion", get(list_moderation))
        .route("/:id", put(update_post).delete(delete_post))
        .route("/:id/reportar", post(report_post))
        .route("/:id/moderar", post(moderate_post))
}

#[derive(Deserialize, Default)]
struct ListPostsQuery {
    limit: Option<i64>,
    offset: Option<i64>,
    sort: Option<String>,
}

#[derive(Deserialize)]
pub struct CreatePostReq {
    pub content: String,
    pub rating: Option<i32>,
    pub idempotency_key: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdatePostReq { pub content: String }

#[derive(Deserialize)]
pub struct ReportReq { pub reason: String, pub detail: Option<String> }

#[derive(Deserialize)]
pub struct ModerateReq { pub action: String, pub reason: Option<String> }

fn validate_content(content: &str) -> Result<String, StatusCode> {
    let content = content.trim();
    let length = content.chars().count();
    if length < 20 || length > 1000 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if content.chars().any(|c| c == '<' || c == '>' || (c.is_control() && c != '\n' && c != '\r' && c != '\t')) {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(content.to_string())
}

fn validate_rating(rating: Option<i32>) -> Result<Option<i32>, StatusCode> {
    match rating {
        None => Ok(None),
        Some(value) if (1..=5).contains(&value) => Ok(Some(value)),
        Some(_) => Err(StatusCode::BAD_REQUEST),
    }
}

fn validate_reason(reason: &str) -> Result<String, StatusCode> {
    let reason = reason.trim();
    if reason.is_empty() || reason.chars().count() > 120 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(reason.to_string())
}

fn validate_detail(detail: Option<&str>) -> Result<Option<String>, StatusCode> {
    match detail {
        None => Ok(None),
        Some(value) if value.chars().count() <= 1000 => Ok(Some(value.to_string())),
        Some(_) => Err(StatusCode::BAD_REQUEST),
    }
}

async fn user_can_publish(pool: &sqlx::MySqlPool, uid: &str) -> Result<bool, StatusCode> {
    let row = sqlx::query("SELECT status, end_date FROM subscriptions WHERE user_id=? AND status IN ('active','grace','cancelled') ORDER BY created_at DESC LIMIT 1")
        .bind(uid).fetch_optional(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(match row {
        None => false,
        Some(r) => {
            let status: String = r.get("status");
            let end_date: Option<chrono::NaiveDateTime> = r.try_get("end_date").ok().flatten();
            (status == "active" || status == "grace" || status == "cancelled")
                && end_date.map(|d| d >= chrono::Utc::now().naive_utc()).unwrap_or(true)
        }
    })
}

fn is_duplicate_key(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Database(db) => db.code().as_deref() == Some("1062"),
        _ => false,
    }
}

pub async fn list_posts(State(state): State<AppState>, Query(query): Query<ListPostsQuery>) -> Result<axum::response::Response, StatusCode> {
    let limit = query.limit.unwrap_or(10);
    let offset = query.offset.unwrap_or(0);
    if !(1..=10).contains(&limit) || offset < 0 || offset > 1000 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let order = match query.sort.as_deref().unwrap_or("recent") {
        "recent" => "p.created_at DESC",
        "rating" => "p.rating DESC, p.created_at DESC",
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    let sql = format!(
        "SELECT p.id,p.user_id,p.content,p.rating,p.status,p.created_at,u.name, (SELECT pl.name FROM subscriptions s JOIN plans pl ON pl.id=s.plan_id WHERE s.user_id=p.user_id AND s.status IN ('active','grace','cancelled') AND (s.end_date IS NULL OR s.end_date >= NOW()) ORDER BY s.created_at DESC LIMIT 1) AS plan_name FROM posts p JOIN users u ON u.id=p.user_id WHERE p.status='approved' ORDER BY {} LIMIT ? OFFSET ?",
        order
    );
    let rows = sqlx::query(&sql)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut out = Vec::new();
    for r in rows {
        let plan: Option<String> = r.try_get("plan_name").ok().flatten();
        out.push(serde_json::json!({
            "id": r.get::<String,_>("id"),
            "user_id": r.get::<String,_>("user_id"),
            "alias": r.get::<String,_>("name"),
            "content": r.get::<String,_>("content"),
            "rating": r.get::<Option<i32>,_>("rating"),
            "status": r.get::<String,_>("status"),
            "created_at": r.get::<chrono::NaiveDateTime,_>("created_at").to_string(),
            "plan": plan.unwrap_or_else(|| "Opaline".to_string()),
        }));
    }

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM posts WHERE status='approved'")
        .fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let average: Option<f64> = sqlx::query_scalar("SELECT CAST(AVG(rating) AS DOUBLE) FROM posts WHERE status='approved' AND rating IS NOT NULL")
        .fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let body = serde_json::to_string(&out).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let response = axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")
        .header("X-Total-Count", total.to_string())
        .header("X-Average-Rating", average.map(|v| format!("{:.2}", v)).unwrap_or_default())
        .header(axum::http::header::CACHE_CONTROL, "public, max-age=30")
        .body(body.into())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(response)
}

pub async fn wall_stats(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM posts WHERE status='approved'")
        .fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let average: Option<f64> = sqlx::query_scalar("SELECT CAST(AVG(rating) AS DOUBLE) FROM posts WHERE status='approved' AND rating IS NOT NULL")
        .fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({"total": total, "average_rating": average})))
}

pub async fn create_post(headers: HeaderMap, State(state): State<AppState>, Json(req): Json<CreatePostReq>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;

    let content = validate_content(&req.content)?;
    let rating = validate_rating(req.rating)?;
    if !user_can_publish(&state.pool, &uid).await? {
        return Err(StatusCode::FORBIDDEN);
    }

    let idempotency_key = req.idempotency_key
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    if let Some(key) = &idempotency_key {
        if key.chars().count() > 128 {
            return Err(StatusCode::BAD_REQUEST);
        }
        let existing = sqlx::query("SELECT id FROM posts WHERE user_id=? AND idempotency_key=?")
            .bind(&uid).bind(key).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if let Some(row) = existing {
            let id: String = row.get("id");
            return Ok(Json(serde_json::json!({"id": id, "duplicate": true})));
        }
    }

    let id = Uuid::new_v4().to_string();
    let event_id = Uuid::new_v4().to_string();
    let payload = serde_json::json!({
        "schema_version": 1,
        "post_id": &id,
        "user_id": &uid,
        "content_length": content.chars().count(),
        "rating": rating,
        "idempotency_key": &idempotency_key,
    });

    let mut tx = state.pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("INSERT INTO posts (id,user_id,content,rating,status,idempotency_key) VALUES (?,?,?,?,'approved',?)")
        .bind(&id).bind(&uid).bind(&content).bind(&rating).bind(&idempotency_key)
        .execute(&mut *tx).await.map_err(|e| {
            if is_duplicate_key(&e) {
                StatusCode::CONFLICT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        })?;
    crate::outbox::insert(&mut *tx, &NewOutboxEvent {
        id: Some(event_id),
        event_type: "wall.post.created".to_string(),
        aggregate_type: "post".to_string(),
        aggregate_id: id.clone(),
        payload: payload.to_string(),
        correlation_id: None,
        available_at: None,
    }).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({"id": id})))
}

pub async fn update_post(Path(id): Path<String>, headers: HeaderMap, State(state): State<AppState>, Json(req): Json<UpdatePostReq>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let content = validate_content(&req.content)?;

    let row = sqlx::query("SELECT created_at, status FROM posts WHERE id=? AND user_id=?")
        .bind(&id).bind(&uid).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = row.ok_or(StatusCode::FORBIDDEN)?;
    let created_at: chrono::NaiveDateTime = row.get("created_at");
    let status: String = row.get("status");
    if (status != "approved" && status != "pending") || chrono::Utc::now().naive_utc() - created_at > chrono::Duration::hours(24) {
        return Err(StatusCode::FORBIDDEN);
    }

    let mut tx = state.pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let res = sqlx::query("UPDATE posts SET content=? WHERE id=? AND user_id=? AND status IN ('approved','pending') AND created_at >= DATE_SUB(NOW(), INTERVAL 24 HOUR)")
        .bind(&content).bind(&id).bind(&uid)
        .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if res.rows_affected() == 0 {
        tx.rollback().await.ok();
        return Err(StatusCode::FORBIDDEN);
    }
    crate::outbox::insert(&mut *tx, &NewOutboxEvent {
        id: Some(Uuid::new_v4().to_string()),
        event_type: "wall.post.updated".to_string(),
        aggregate_type: "post".to_string(),
        aggregate_id: id.clone(),
        payload: serde_json::json!({
            "schema_version": 1,
            "post_id": &id,
            "user_id": &uid,
            "content_length": content.chars().count(),
        }).to_string(),
        correlation_id: None,
        available_at: None,
    }).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({"ok": true})))
}

pub async fn delete_post(Path(id): Path<String>, headers: HeaderMap, State(state): State<AppState>) -> Result<StatusCode, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;

    let mut tx = state.pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let res = sqlx::query("UPDATE posts SET status='deleted' WHERE id=? AND user_id=? AND status<>'deleted'")
        .bind(&id).bind(&uid).execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if res.rows_affected() == 0 {
        tx.rollback().await.ok();
        return Err(StatusCode::FORBIDDEN);
    }
    crate::outbox::insert(&mut *tx, &NewOutboxEvent {
        id: Some(Uuid::new_v4().to_string()),
        event_type: "wall.post.deleted".to_string(),
        aggregate_type: "post".to_string(),
        aggregate_id: id.clone(),
        payload: serde_json::json!({
            "schema_version": 1,
            "post_id": &id,
            "user_id": &uid,
        }).to_string(),
        correlation_id: None,
        available_at: None,
    }).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn report_post(Path(id): Path<String>, headers: HeaderMap, State(state): State<AppState>, Json(req): Json<ReportReq>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let reason = validate_reason(&req.reason)?;
    let detail = validate_detail(req.detail.as_deref())?;

    let existing = sqlx::query("SELECT id FROM reports WHERE post_id=? AND user_id=?")
        .bind(&id).bind(&uid).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if existing.is_some() {
        return Ok(Json(serde_json::json!({"ok": true, "duplicate": true})));
    }

    let post = sqlx::query("SELECT status FROM posts WHERE id=?").bind(&id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let post = post.ok_or(StatusCode::NOT_FOUND)?;
    let status: String = post.get("status");
    if status != "approved" && status != "pending" {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut tx = state.pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let rid = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO reports (id,post_id,user_id,reason,detail) VALUES (?,?,?,?,?)")
        .bind(&rid).bind(&id).bind(&uid).bind(&reason).bind(&detail)
        .execute(&mut *tx).await.map_err(|e| {
            if is_duplicate_key(&e) {
                StatusCode::OK
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        })?;

    let cnt: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT user_id) FROM reports WHERE post_id=?").bind(&id).fetch_one(&mut *tx).await.unwrap_or(0);
    let mut hidden = false;
    if cnt >= 3 {
        sqlx::query("UPDATE posts SET status='hidden' WHERE id=? AND status='approved'").bind(&id).execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        hidden = true;
    }

    crate::outbox::insert(&mut *tx, &NewOutboxEvent {
        id: Some(Uuid::new_v4().to_string()),
        event_type: "wall.post.reported".to_string(),
        aggregate_type: "post".to_string(),
        aggregate_id: id.clone(),
        payload: serde_json::json!({
            "schema_version": 1,
            "post_id": &id,
            "reporter_user_id": &uid,
            "reason": &reason,
            "detail": &detail,
            "hidden": hidden,
        }).to_string(),
        correlation_id: None,
        available_at: None,
    }).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if hidden {
        crate::outbox::insert(&mut *tx, &NewOutboxEvent {
            id: Some(Uuid::new_v4().to_string()),
            event_type: "wall.post.auto_hidden".to_string(),
            aggregate_type: "post".to_string(),
            aggregate_id: id.clone(),
            payload: serde_json::json!({
                "schema_version": 1,
                "post_id": &id,
                "reason": "three_distinct_reports",
            }).to_string(),
            correlation_id: None,
            available_at: None,
        }).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if hidden {
        Ok(Json(serde_json::json!({"ok": true, "hidden": true})))
    } else {
        Ok(Json(serde_json::json!({"ok": true})))
    }
}

pub async fn list_moderation(headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    require_admin(&state, &headers).await?;
    let rows = sqlx::query(
        "SELECT p.id,p.user_id,p.content,p.rating,p.status,p.created_at,u.name, (SELECT COUNT(*) FROM reports r WHERE r.post_id=p.id) AS report_count, (SELECT r.reason FROM reports r WHERE r.post_id=p.id ORDER BY r.created_at DESC LIMIT 1) AS last_report_reason FROM posts p JOIN users u ON u.id=p.user_id WHERE p.status IN ('pending','approved','hidden') ORDER BY p.created_at DESC LIMIT 100"
    ).fetch_all(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
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
            "report_count": r.get::<i64,_>("report_count"),
            "last_report_reason": r.try_get::<Option<String>,_>("last_report_reason").ok().flatten(),
        }));
    }
    Ok(Json(serde_json::json!(out)))
}

pub async fn moderate_post(Path(id): Path<String>, headers: HeaderMap, State(state): State<AppState>, Json(req): Json<ModerateReq>) -> Result<Json<serde_json::Value>, StatusCode> {
    let admin_uid = require_admin(&state, &headers).await?;
    let action = req.action.trim().to_lowercase();
    let reason = req.reason.map(|r| r.trim().to_string()).filter(|r| !r.is_empty());
    if action != "approve" && action != "hide" && action != "delete" {
        return Err(StatusCode::BAD_REQUEST);
    }
    if (action == "hide" || action == "delete") && reason.is_none() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let row = sqlx::query("SELECT status FROM posts WHERE id=?").bind(&id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = row.ok_or(StatusCode::NOT_FOUND)?;
    let current_status: String = row.get("status");
    if current_status == "deleted" {
        return Err(StatusCode::BAD_REQUEST);
    }

    let new_status = match action.as_str() {
        "approve" => "approved",
        "hide" => "hidden",
        "delete" => "deleted",
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    let mut tx = state.pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("UPDATE posts SET status=? WHERE id=?").bind(new_status).bind(&id).execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    crate::outbox::insert(&mut *tx, &NewOutboxEvent {
        id: Some(Uuid::new_v4().to_string()),
        event_type: "wall.post.moderated".to_string(),
        aggregate_type: "post".to_string(),
        aggregate_id: id.clone(),
        payload: serde_json::json!({
            "schema_version": 1,
            "post_id": &id,
            "actor_user_id": &admin_uid,
            "action": &action,
            "reason": &reason,
            "previous_status": &current_status,
            "new_status": new_status,
        }).to_string(),
        correlation_id: None,
        available_at: None,
    }).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({"ok": true})))
}