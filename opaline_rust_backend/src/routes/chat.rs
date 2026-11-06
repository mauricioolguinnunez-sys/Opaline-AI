use axum::{extract::Path, extract::State, Json, Router, routing::{get, post, delete}};
use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;
use sqlx::Row;
use crate::state::AppState;
use crate::routes::auth::user_id_from_token;
use uuid::Uuid;
use mongodb::bson::doc;
use futures::stream::TryStreamExt;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/models", get(list_models))
        .route("/conversations", get(list_conversations))
        .route("/chat", post(chat))
        .route("/conversations/:id", delete(delete_conversation))
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
    let mut cursor = coll.find(doc!{"user_id": &uid}).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
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
}

async fn chat(headers: HeaderMap, State(state): State<AppState>, Json(req): Json<ChatReq>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    if req.message.trim().is_empty() || req.message.len() > 2000 { return Err(StatusCode::BAD_REQUEST); }
    let model = req.model.unwrap_or_else(|| "qwen/qwen3.5-9b".to_string());
    // quota check (simple): count messages this month
    // call LMStudio
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role":"system","content":"Eres Opaline AI, asistente util en espanol."},
            {"role":"user","content": req.message}
        ],
        "temperature": 0.7,
        "max_tokens": 512
    });
    let url = format!("{}/v1/chat/completions", state.lmstudio_url);
    let resp = client.post(&url).json(&body).send().await.map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    if !resp.status().is_success() { return Err(StatusCode::SERVICE_UNAVAILABLE); }
    let json: serde_json::Value = resp.json().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let reply = json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string();
    // store conversation in mongo
    let cid = req.conversation_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let title = if req.message.len() > 30 { format!("{}...", &req.message[..30]) } else { req.message.clone() };
    let now = chrono::Utc::now().to_rfc3339();
    let user_msg = serde_json::json!({"role":"user","content":req.message,"created_at":now});
    let asst_msg = serde_json::json!({"role":"assistant","content":reply,"created_at":now});
    let coll = state.mongo.collection::<mongodb::bson::Document>("conversations");
    // upsert: if exists append, else create
    let existing = coll.find_one(doc!{"id": &cid, "user_id": &uid}).await.ok().flatten();
    if existing.is_some() {
        let _ = coll.update_one(doc!{"id": &cid}, doc!{"$push":{"messages":{"$each":[mongodb::bson::to_bson(&user_msg).unwrap(), mongodb::bson::to_bson(&asst_msg).unwrap()]}}, "$set":{"last_message": &reply, "updated_at": &now}}).await;
    } else {
        let _ = coll.insert_one(doc!{
            "id": &cid,
            "user_id": &uid,
            "title": title,
            "last_message": &reply,
            "updated_at": &now,
            "messages": [mongodb::bson::to_bson(&user_msg).unwrap(), mongodb::bson::to_bson(&asst_msg).unwrap()]
        }).await;
    }
    // audit metrics
    let _ = state.mongo.collection::<mongodb::bson::Document>("ai_metrics").insert_one(doc!{"user_id":&uid,"model":&model,"ts":mongodb::bson::DateTime::now()}).await;
    Ok(Json(serde_json::json!({"reply": reply, "citations": [], "conversation_id": cid})))
}

async fn delete_conversation(Path(id): Path<String>, headers: HeaderMap, State(state): State<AppState>) -> Result<StatusCode, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let coll = state.mongo.collection::<mongodb::bson::Document>("conversations");
    coll.delete_one(doc!{"id": &id, "user_id": &uid}).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}
