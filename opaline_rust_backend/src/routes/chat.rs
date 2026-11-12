use axum::extract::{Path, Query, State};
use axum::{Json, Router, routing::{get, post, delete}};
use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;
use sqlx::Row;
use crate::state::AppState;
use crate::routes::auth::user_id_from_token;
use crate::quota;
use crate::chat_operations;
use crate::consumers::mongo_chat_persist;
use std::time::Instant;
use uuid::Uuid;
use mongodb::bson::doc;
use futures::stream::TryStreamExt;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/models", get(list_models))
        .route("/status", get(ai_status))
        .route("/conversations", get(list_conversations))
        .route("/chat", post(chat))
        .route("/operations", get(get_operation))
        .route("/operations/:id", get(get_operation_by_id))
        .route("/conversations/:id", delete(delete_conversation))
}

async fn ai_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let url = format!("{}/v1/models", state.lmstudio_url);
    let client = reqwest::Client::new();
    match client.get(&url).timeout(std::time::Duration::from_secs(3)).send().await {
        Ok(r) if r.status().is_success() => Json(serde_json::json!({"online":true,"url":state.lmstudio_url})),
        _ => Json(serde_json::json!({"online":false,"url":state.lmstudio_url})),
    }
}

async fn list_models(State(state): State<AppState>) -> Json<serde_json::Value> {
    let rows = sqlx::query("SELECT code,name,enabled,is_default FROM ai_models WHERE enabled=1").fetch_all(&state.pool).await.unwrap_or_default();
    let mut models = Vec::new();
    let mut default_model = None;
    for r in rows {
        let code: String = r.get("code");
        if r.get::<i8,_>("is_default") == 1 { default_model = Some(code.clone()); }
        models.push(code);
    }
    if default_model.is_none() && !models.is_empty() { default_model = Some(models[0].clone()); }
    Json(serde_json::json!({"models": models, "default": default_model}))
}

async fn list_conversations(headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let coll = state.mongo.collection::<mongodb::bson::Document>("conversations");
    let options = mongodb::options::FindOptions::builder()
        .sort(doc!{"updated_at": -1})
        .build();
    let mut cursor = coll.find(doc!{"user_id": &uid}).with_options(options).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut out = Vec::new();
    while let Some(d) = cursor.try_next().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)? {
        out.push(serde_json::json!({
            "id": d.get_str("id").unwrap_or_default(),
            "title": d.get_str("title").unwrap_or("Conversacion"),
            "last_message": d.get_str("last_message").ok(),
            "updated_at": d.get_str("updated_at").unwrap_or_default(),
            "messages": d.get_array("messages").map(|a| a.iter().map(|b| mongodb::bson::from_bson(b.clone()).unwrap_or(serde_json::json!({}))).collect::<Vec<_>>()).unwrap_or_default(),
        }));
    }
    Ok(Json(serde_json::json!(out)))
}

#[derive(Deserialize)]
struct ChatReq {
    message: String,
    model: Option<String>,
    history: Option<Vec<serde_json::Value>>,
    conversation_id: Option<String>,
    idempotency_key: Option<String>,
}

async fn chat(headers: HeaderMap, State(state): State<AppState>, Json(req): Json<ChatReq>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    if req.message.trim().is_empty() || req.message.chars().count() > 2000 { return Err(StatusCode::BAD_REQUEST); }
    let model = req.model.filter(|m| !m.trim().is_empty()).unwrap_or_else(|| "mistralai/ministral-3-3b".to_string());

    let idempotency_key = req.idempotency_key
        .or_else(|| headers.get("x-idempotency-key").and_then(|v| v.to_str().ok()).map(|s| s.to_string()))
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let request_id = idempotency_key.clone();

    // Recover a previously completed operation without re-running inference.
    if let Some(op) = chat_operations::find_by_idempotency(&state.pool, &uid, &idempotency_key).await? {
        if matches!(op.status.as_str(), "inferred" | "persisting" | "persisted") {
            if let Some(resp) = op.response_json {
                let reply = resp.get("reply").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let cid = resp.get("conversation_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if !op.mongo_persisted {
                    let _ = chat_operations::persist_operation(&state.pool, &state.mongo, &op.id).await;
                    if !op.mongo_persisted {
                        let _ = mongo_chat_persist::enqueue_persist(&state.pool, &op.id).await;
                    }
                }
                return Ok(Json(serde_json::json!({"reply": reply, "citations": [], "conversation_id": cid, "idempotency_key": idempotency_key})));
            }
        }
        if op.status == "failed" {
            return Err(StatusCode::CONFLICT);
        }
    }

    let (subscription_id, plan_version_id, period_start, period_end) = quota::resolve_current_subscription(&state.pool, &uid).await?;
    let limits = quota::get_plan_limits(&state.pool, &plan_version_id).await?;

    // Conservative token estimate.
    let mut input_text = req.message.clone();
    if let Some(history) = &req.history {
        for item in history {
            if let Some(content) = item.get("content").and_then(|v| v.as_str()) {
                input_text.push(' ');
                input_text.push_str(content);
            }
        }
    }
    let estimated_input = quota::estimate_tokens(&input_text);
    let estimated_output: i64 = 1200;
    let started = Instant::now();

    let reservation = quota::reserve_quota(
        &state.pool,
        &uid,
        &subscription_id,
        &plan_version_id,
        period_start,
        period_end,
        &limits,
        "chat",
        &model,
        &request_id,
        &idempotency_key,
        estimated_input,
        estimated_output,
    ).await?;

    if reservation.cached {
        if let Some(resp) = reservation.cached_response {
            let reply = resp.get("reply").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let cid = resp.get("conversation_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let op_id = resp.get("operation_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            return Ok(Json(serde_json::json!({"reply": reply, "citations": [], "conversation_id": cid, "idempotency_key": idempotency_key, "operation_id": op_id})));
        }
        return Err(StatusCode::CONFLICT);
    }

    let operation_id = chat_operations::create_operation(
        &state.pool,
        &uid,
        &subscription_id,
        &reservation.usage_period_id,
        &reservation.id,
        &request_id,
        &idempotency_key,
        &model,
        &req.message,
    ).await?;

    // build message history for LM Studio (keep last 16 turns)
    let mut messages: Vec<serde_json::Value> = vec![serde_json::json!({
        "role": "system",
        "content": "Eres Opaline AI, un asistente util, claro y amable. Responde en espanol."
    })];
    if let Some(history) = req.history {
        for item in history.into_iter().rev().take(16).collect::<Vec<_>>().into_iter().rev() {
            let role = item.get("role").and_then(|v| v.as_str()).unwrap_or("user");
            let content = item.get("content").and_then(|v| v.as_str()).unwrap_or("");
            if !content.is_empty() && (role == "user" || role == "assistant") {
                messages.push(serde_json::json!({"role": role, "content": content}));
            }
        }
    }
    messages.push(serde_json::json!({"role": "user", "content": req.message}));
    // call LMStudio
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "temperature": 0.7,
        "max_tokens": 1200
    });
    let url = format!("{}/v1/chat/completions", state.lmstudio_url);
    let resp = client.post(&url)
        .timeout(std::time::Duration::from_secs(180))
        .json(&body)
        .send().await;
    let resp = match resp {
        Ok(r) => r,
        Err(_) => {
            let _ = quota::release_reservation(&state.pool, &reservation.id, true).await;
            let _ = chat_operations::mark_failed(&state.pool, &operation_id, "provider unavailable").await;
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
    };
    if !resp.status().is_success() {
        let _ = quota::release_reservation(&state.pool, &reservation.id, true).await;
        let _ = chat_operations::mark_failed(&state.pool, &operation_id, "provider error").await;
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    let json: serde_json::Value = match resp.json().await {
        Ok(j) => j,
        Err(_) => {
            let _ = quota::release_reservation(&state.pool, &reservation.id, true).await;
            let _ = chat_operations::mark_failed(&state.pool, &operation_id, "invalid provider response").await;
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    let reply = json["choices"][0]["message"]["content"].as_str().unwrap_or("").trim().to_string();
    if reply.is_empty() {
        tracing::warn!("LM Studio returned an empty assistant message: {}", json);
        let _ = quota::release_reservation(&state.pool, &reservation.id, true).await;
        let _ = chat_operations::mark_failed(&state.pool, &operation_id, "empty provider response").await;
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

    let usage = json.get("usage");
    let (actual_input, actual_output, token_source, measurement_status) = if let Some(u) = usage {
        let has_input = u.get("prompt_tokens").and_then(|v| v.as_i64()).is_some();
        let has_output = u.get("completion_tokens").and_then(|v| v.as_i64()).is_some();
        let input = u.get("prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(quota::estimate_tokens(&input_text));
        let output = u.get("completion_tokens").and_then(|v| v.as_i64()).unwrap_or(quota::estimate_tokens(&reply));
        let (source, measurement) = if has_input && has_output { ("provider", "measured") } else { ("estimated", "estimated") };
        (input, output, source, measurement)
    } else {
        (quota::estimate_tokens(&input_text), quota::estimate_tokens(&reply), "estimated", "estimated")
    };
    let model_used = json.get("model").and_then(|v| v.as_str()).unwrap_or(&model).to_string();
    let total_tokens = u64::try_from(actual_input + actual_output).unwrap_or(0) as i64;
    let cid = req.conversation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let response_payload = serde_json::json!({"reply": reply, "citations": [], "conversation_id": cid, "idempotency_key": idempotency_key, "operation_id": operation_id});

    let commit_result = quota::commit_reservation(
        &state.pool,
        &reservation.id,
        actual_input,
        actual_output,
        token_source,
        measurement_status,
        started.elapsed().as_millis() as i64,
        response_payload.clone(),
    ).await;
    if commit_result.is_err() {
        let _ = quota::release_reservation(&state.pool, &reservation.id, true).await;
        let _ = chat_operations::mark_failed(&state.pool, &operation_id, "quota commit failed").await;
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let _ = chat_operations::update_inferred(
        &state.pool,
        &operation_id,
        &model_used,
        &reply,
        &response_payload,
        &cid,
        token_source,
        measurement_status,
        actual_input,
        actual_output,
        total_tokens,
        started.elapsed().as_millis() as i64,
    ).await;

    // Persist conversation durably. If this fails, keep the operation recoverable
    // through Outbox instead of failing the already completed inference.
    let persist_result = chat_operations::persist_operation(&state.pool, &state.mongo, &operation_id).await;
    if persist_result.is_err() {
        let _ = mongo_chat_persist::enqueue_persist(&state.pool, &operation_id).await;
    }

    state.mongo.collection::<mongodb::bson::Document>("ai_metrics").insert_one(doc!{"user_id":&uid,"model":&model_used,"ts":mongodb::bson::DateTime::now()}).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({"reply": reply, "citations": [], "conversation_id": cid, "idempotency_key": idempotency_key, "operation_id": operation_id})))
}



#[derive(serde::Deserialize)]
pub struct OperationQuery {
    pub idempotency_key: Option<String>,
}

pub async fn get_operation(Query(q): Query<OperationQuery>, headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let key = q.idempotency_key.filter(|v| !v.trim().is_empty()).ok_or(StatusCode::BAD_REQUEST)?;
    let op = chat_operations::get_operation_by_key(&state.pool, &uid, &key).await?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(operation_json(&op)))
}

pub async fn get_operation_by_id(Path(id): Path<String>, headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let op = chat_operations::get_operation(&state.pool, &uid, &id).await?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(operation_json(&op)))
}

fn operation_json(op: &chat_operations::ChatOperation) -> serde_json::Value {
    let status = match op.status.as_str() {
        "reserved" => "processing",
        "inferred" => "awaiting_persistence",
        "persisting" => "awaiting_persistence",
        "persisted" => "completed",
        "failed" => "failed_terminal",
        "manual_review" => "manual_review",
        other => other,
    };
    serde_json::json!({
        "operation_id": op.id,
        "idempotency_key": op.idempotency_key,
        "status": status,
        "has_result": op.assistant_reply.is_some(),
        "reply": op.assistant_reply,
        "conversation_id": op.conversation_id,
        "mongo_persisted": op.mongo_persisted,
        "model_requested": op.model_requested,
        "model_used": op.model_used,
    })
}

async fn delete_conversation(Path(id): Path<String>, headers: HeaderMap, State(state): State<AppState>) -> Result<StatusCode, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let coll = state.mongo.collection::<mongodb::bson::Document>("conversations");
    coll.delete_one(doc!{"id": &id, "user_id": &uid}).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

fn owner_filter(user_id: &str, conversation_id: &str) -> mongodb::bson::Document {
    doc! { "id": conversation_id, "user_id": user_id }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_filter_requires_owner_and_id() {
        let filter = owner_filter("user-a", "conv-1");
        assert_eq!(filter.get_str("id").unwrap(), "conv-1");
        assert_eq!(filter.get_str("user_id").unwrap(), "user-a");
    }

    #[test]
    fn conversation_filter_is_not_shared_between_users() {
        let a = owner_filter("user-a", "shared-id");
        let b = owner_filter("user-b", "shared-id");
        assert_ne!(a.get_str("user_id").unwrap(), b.get_str("user_id").unwrap());
    }
}
