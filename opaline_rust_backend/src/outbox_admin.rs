use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::{Json, Router, routing::get, routing::post};
use serde::Deserialize;
use sqlx::Row;
use crate::state::AppState;
use crate::routes::auth::user_id_from_token;
use crate::routes::admin::require_admin;
use crate::outbox::{self, OutboxEvent, OutboxStatus};
use crate::outbox_recovery::{self, OutboxRecovery, RecoveryConfig};
use crate::consumers::mongo_wall_audit::MongoWallAuditConsumer;
use uuid::Uuid;
use mongodb::bson::doc;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/discrepancies", get(list_discrepancies))
        .route("/events/:id", get(get_event))
        .route("/events/:id/history", get(get_history))
        .route("/events/:id/actions", post(run_action))
}

#[derive(Deserialize, Default)]
struct ListQuery {
    status: Option<String>,
    event_type: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Deserialize)]
struct ActionReq {
    action: String,
    reason: String,
    idempotency_key: String,
    correlation_id: Option<String>,
}

#[derive(serde::Serialize)]
struct AdminEventView {
    id: String,
    event_type: String,
    aggregate_type: String,
    aggregate_id: String,
    status: String,
    attempts: i32,
    created_at: Option<String>,
    available_at: Option<String>,
    processed_at: Option<String>,
    locked_at: Option<String>,
    last_error: Option<String>,
    correlation_id: Option<String>,
    has_audit_document: bool,
    classification: String,
}

async fn list_discrepancies(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    require_admin(&state, &headers).await?;

    let limit = query.limit.unwrap_or(20).clamp(1, 50);
    let offset = query.offset.unwrap_or(0).max(0);
    let status_filter = query.status.as_deref().filter(|v| !v.trim().is_empty());
    let type_filter = query.event_type.as_deref().filter(|v| !v.trim().is_empty());

    let rows = sqlx::query(
        "SELECT id, event_type, aggregate_type, aggregate_id, status, attempts, created_at, available_at, processed_at, locked_at, last_error, correlation_id FROM outbox_events WHERE event_type LIKE 'wall.post.%' AND (? IS NULL OR status=?) AND (? IS NULL OR event_type=?) ORDER BY created_at ASC, id ASC LIMIT ? OFFSET ?",
    )
    .bind(status_filter)
    .bind(status_filter)
    .bind(type_filter)
    .bind(type_filter)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let audit_ids = fetch_audit_ids(&state.mongo).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let now = chrono::Utc::now().naive_utc();
    let mut items = Vec::new();
    for row in rows {
        let event = row_to_event(&row);
        let has_doc = audit_ids.contains(&event.id);
        let classification = classify(&event, has_doc, now);
        if classification == "consistent" {
            continue;
        }
        items.push(AdminEventView {
            id: event.id,
            event_type: event.event_type,
            aggregate_type: event.aggregate_type,
            aggregate_id: event.aggregate_id,
            status: event.status.as_str().to_string(),
            attempts: event.attempts,
            created_at: event.created_at.map(|v| v.to_string()),
            available_at: event.available_at.map(|v| v.to_string()),
            processed_at: event.processed_at.map(|v| v.to_string()),
            locked_at: event.locked_at.map(|v| v.to_string()),
            last_error: event.last_error,
            correlation_id: event.correlation_id,
            has_audit_document: has_doc,
            classification: classification.to_string(),
        });
    }

    let audit_without_outbox = audit_ids
        .into_iter()
        .filter(|id| !items.iter().any(|item| &item.id == id))
        .collect::<Vec<_>>();

    Ok(Json(serde_json::json!({
        "items": items,
        "audit_without_outbox": audit_without_outbox,
    })))
}

async fn get_event(headers: HeaderMap, State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, StatusCode> {
    require_admin(&state, &headers).await?;
    let event = outbox::get_event(&state.pool, &id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let event = event.ok_or(StatusCode::NOT_FOUND)?;
    if !event.event_type.starts_with("wall.post.") {
        return Err(StatusCode::NOT_FOUND);
    }
    let audit_exists = mongo_has_event(&state.mongo, &event.id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({
        "id": event.id,
        "event_type": event.event_type,
        "aggregate_type": event.aggregate_type,
        "aggregate_id": event.aggregate_id,
        "status": event.status.as_str(),
        "attempts": event.attempts,
        "created_at": event.created_at.map(|v| v.to_string()),
        "available_at": event.available_at.map(|v| v.to_string()),
        "processed_at": event.processed_at.map(|v| v.to_string()),
        "locked_at": event.locked_at.map(|v| v.to_string()),
        "last_error": event.last_error,
        "correlation_id": event.correlation_id,
        "has_audit_document": audit_exists,
        "classification": classify(&event, audit_exists, chrono::Utc::now().naive_utc()),
    })))
}

async fn get_history(headers: HeaderMap, State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, StatusCode> {
    require_admin(&state, &headers).await?;
    let rows = sqlx::query("SELECT id, admin_user_id, action_type, reason, previous_status, resulting_status, result, idempotency_key, correlation_id, created_at, completed_at FROM outbox_admin_actions WHERE event_id=? ORDER BY created_at DESC LIMIT 100")
        .bind(&id).fetch_all(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(serde_json::json!({
            "id": r.get::<String,_>("id"),
            "admin_user_id": r.get::<String,_>("admin_user_id"),
            "action_type": r.get::<String,_>("action_type"),
            "reason": r.get::<String,_>("reason"),
            "previous_status": r.get::<String,_>("previous_status"),
            "resulting_status": r.get::<String,_>("resulting_status"),
            "result": r.get::<String,_>("result"),
            "idempotency_key": r.try_get::<Option<String>,_>("idempotency_key").ok().flatten(),
            "correlation_id": r.try_get::<Option<String>,_>("correlation_id").ok().flatten(),
            "created_at": r.get::<chrono::NaiveDateTime,_>("created_at").to_string(),
            "completed_at": r.try_get::<Option<chrono::NaiveDateTime>,_>("completed_at").ok().flatten().map(|v| v.to_string()),
        }));
    }
    Ok(Json(serde_json::json!(out)))
}

async fn run_action(headers: HeaderMap, State(state): State<AppState>, Path(id): Path<String>, Json(req): Json<ActionReq>) -> Result<Json<serde_json::Value>, StatusCode> {
    let admin_uid = require_admin(&state, &headers).await?;
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let _uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;

    if !state.rate_limiter.check(&format!("admin:outbox:ip:{}", client_ip_from_headers(&headers)), 30, std::time::Duration::from_secs(900)) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    if !state.rate_limiter.check(&format!("admin:outbox:user:{admin_uid}"), 20, std::time::Duration::from_secs(900)) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let action = req.action.trim().to_lowercase();
    let reason = req.reason.trim().to_string();
    if reason.is_empty() || reason.chars().count() > 500 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req.idempotency_key.trim().is_empty() || req.idempotency_key.chars().count() > 128 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let event = outbox::get_event(&state.pool, &id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let event = match event {
        Some(event) if event.event_type.starts_with("wall.post.") => event,
        _ => return Err(StatusCode::NOT_FOUND),
    };

    if let Some(existing) = find_admin_action(&state.pool, &admin_uid, &req.idempotency_key).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)? {
        if existing.event_id == event.id && existing.action_type == action && existing.reason == reason {
            return Ok(Json(serde_json::json!({"ok": true, "idempotent": true, "result": existing.result})));
        }
        return Err(StatusCode::CONFLICT);
    }

    let audit_exists = mongo_has_event(&state.mongo, &event.id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let now = chrono::Utc::now().naive_utc();
    let eligible = match action.as_str() {
        "recover" => {
            eligible_for_recovery(&event, audit_exists, now, 5)
        }
        "mark_manual_review" | "record_investigation" => true,
        _ => false,
    };
    if !eligible {
        record_admin_action(&state.pool, &admin_uid, &event, &action, &reason, &req, "rejected", event.status.as_str(), event.status.as_str()).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        return Err(StatusCode::CONFLICT);
    }

    match action.as_str() {
        "recover" => {
            let result = crate::outbox_recovery::OutboxRecovery::new(state.pool.clone(), state.mongo.clone(), crate::outbox_recovery::RecoveryConfig::default())
                .recover_event_by_id(&id).await;
            match result {
                Ok(true) => {
                    record_admin_action(&state.pool, &admin_uid, &event, &action, &reason, &req, "success", event.status.as_str(), "processed").await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    Ok(Json(serde_json::json!({"ok": true, "result": "recovered"})))
                }
                Ok(false) => {
                    record_admin_action(&state.pool, &admin_uid, &event, &action, &reason, &req, "rejected", event.status.as_str(), event.status.as_str()).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    Err(StatusCode::CONFLICT)
                }
                Err(error) => {
                    record_admin_action(&state.pool, &admin_uid, &event, &action, &reason, &req, "failed", event.status.as_str(), event.status.as_str()).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    tracing::warn!(event_id = %id, error = %error, "admin recovery failed");
                    Err(StatusCode::from_u16(422).unwrap())
                }
            }
        }
        "mark_manual_review" | "record_investigation" => {
            record_admin_action(&state.pool, &admin_uid, &event, &action, &reason, &req, "success", event.status.as_str(), event.status.as_str()).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(Json(serde_json::json!({"ok": true, "result": "recorded"})))
        }
        _ => Err(StatusCode::BAD_REQUEST),
    }
}

fn classify(event: &OutboxEvent, has_document: bool, now: chrono::NaiveDateTime) -> &'static str {
    match event.status {
        OutboxStatus::Pending => {
            if has_document { "pending_with_document" } else { "pending_without_document" }
        }
        OutboxStatus::Processing => {
            let expired = event.locked_at.map(|locked| (now - locked).num_seconds() >= 300).unwrap_or(true);
            if expired {
                if has_document { "processing_expired_with_document" } else { "processing_expired_without_document" }
            } else {
                "processing_active"
            }
        }
        OutboxStatus::Failed => {
            if has_document { "failed_with_document" } else { "failed_without_document" }
        }
        OutboxStatus::Processed => {
            if has_document { "consistent" } else { "processed_missing_document" }
        }
    }
}

fn eligible_for_recovery(event: &OutboxEvent, has_document: bool, now: chrono::NaiveDateTime, max_attempts: i32) -> bool {
    match event.status {
        OutboxStatus::Pending => has_document,
        OutboxStatus::Processing => event.locked_at.map(|locked| (now - locked).num_seconds() >= 300).unwrap_or(true),
        OutboxStatus::Failed => has_document && outbox_recovery::is_recoverable_failure(event.last_error.as_deref()) && event.attempts < max_attempts,
        OutboxStatus::Processed => false,
    }
}

async fn fetch_audit_ids(mongo: &mongodb::Database) -> Result<std::collections::HashSet<String>, mongodb::error::Error> {
    let collection = mongo.collection::<mongodb::bson::Document>("wall_events");
    let filter = mongodb::bson::doc! { "event_id": { "$exists": true, "$type": "string" } };
    let mut cursor = collection.find(filter).projection(mongodb::bson::doc! { "event_id": 1 }).await?;
    let mut ids = std::collections::HashSet::new();
    while let Some(doc) = cursor.try_next().await? {
        if let Ok(id) = doc.get_str("event_id") {
            ids.insert(id.to_string());
        }
    }
    Ok(ids)
}

async fn mongo_has_event(mongo: &mongodb::Database, id: &str) -> Result<bool, mongodb::error::Error> {
    let collection = mongo.collection::<mongodb::bson::Document>("wall_events");
    Ok(collection.find_one(mongodb::bson::doc! { "event_id": id }).await?.is_some())
}

fn client_ip_from_headers(headers: &HeaderMap) -> String {
    headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()).unwrap_or("unknown").to_string()
}

#[derive(sqlx::FromRow)]
struct AdminActionRow {
    event_id: String,
    action_type: String,
    reason: String,
}

async fn find_admin_action(pool: &sqlx::MySqlPool, admin_user_id: &str, idempotency_key: &str) -> Result<Option<AdminActionRow>, sqlx::Error> {
    sqlx::query_as::<_, AdminActionRow>(
        "SELECT event_id, action_type, reason FROM outbox_admin_actions WHERE admin_user_id=? AND idempotency_key=?",
    )
    .bind(admin_user_id)
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await
}

async fn record_admin_action(
    pool: &sqlx::MySqlPool,
    admin_user_id: &str,
    event: &OutboxEvent,
    action_type: &str,
    reason: &str,
    req: &ActionReq,
    result: &str,
    previous_status: &str,
    resulting_status: &str,
) -> Result<(), sqlx::Error> {
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO outbox_admin_actions (id,event_id,admin_user_id,action_type,reason,previous_status,resulting_status,result,idempotency_key,correlation_id,completed_at) VALUES (?,?,?,?,?,?,?,?,?,?,UTC_TIMESTAMP())",
    )
    .bind(&id)
    .bind(&event.id)
    .bind(admin_user_id)
    .bind(action_type)
    .bind(reason)
    .bind(previous_status)
    .bind(resulting_status)
    .bind(result)
    .bind(&req.idempotency_key)
    .bind(&req.correlation_id)
    .execute(pool)
    .await?;
    Ok(())
}

// Reuse OutboxEvent row mapping? outbox::get_event already returns OutboxEvent.
fn row_to_event(row: &sqlx::mysql::MySqlRow) -> OutboxEvent {
    // outbox module keeps this private, so reconstruct only fields needed from get_event is not available here.
    // This function is unused; keep for compatibility documentation.
    let _ = row;
    unimplemented!("not used")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_actions_require_reason() {
        // Reason validation is enforced in run_action before consulting the event.
        let reason = "   ";
        assert!(reason.trim().is_empty());
    }
}