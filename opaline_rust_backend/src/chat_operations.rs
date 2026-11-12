use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;
use chrono::Utc;
use crate::state::AppState;
use axum::http::StatusCode;
use mongodb::bson::doc;

#[derive(Clone)]
pub struct ChatOperation {
    pub id: String,
    pub user_id: String,
    pub subscription_id: String,
    pub usage_period_id: String,
    pub reservation_id: String,
    pub request_id: String,
    pub idempotency_key: String,
    pub model_requested: String,
    pub model_used: Option<String>,
    pub status: String,
    pub user_message: String,
    pub assistant_reply: Option<String>,
    pub response_json: Option<Value>,
    pub conversation_id: Option<String>,
    pub mongo_persisted: bool,
    pub mongo_error: Option<String>,
    pub token_source: Option<String>,
    pub measurement_status: Option<String>,
    pub actual_input_tokens: Option<i64>,
    pub actual_output_tokens: Option<i64>,
    pub actual_total_tokens: Option<i64>,
    pub duration_ms: Option<i64>,
}

fn row_to_operation(row: &sqlx::mysql::MySqlRow) -> ChatOperation {
    ChatOperation {
        id: row.get("id"),
        user_id: row.get("user_id"),
        subscription_id: row.get("subscription_id"),
        usage_period_id: row.get("usage_period_id"),
        reservation_id: row.get("reservation_id"),
        request_id: row.get("request_id"),
        idempotency_key: row.get("idempotency_key"),
        model_requested: row.get("model_requested"),
        model_used: row.try_get("model_used").ok().flatten(),
        status: row.get("status"),
        user_message: row.get("user_message"),
        assistant_reply: row.try_get("assistant_reply").ok().flatten(),
        response_json: row.try_get("response_json").ok().flatten(),
        conversation_id: row.try_get("conversation_id").ok().flatten(),
        mongo_persisted: row.get::<i8, _>("mongo_persisted") == 1,
        mongo_error: row.try_get("mongo_error").ok().flatten(),
        token_source: row.try_get("token_source").ok().flatten(),
        measurement_status: row.try_get("measurement_status").ok().flatten(),
        actual_input_tokens: row.try_get::<Option<i32>, _>("actual_input_tokens").ok().flatten().map(|v| v as i64),
        actual_output_tokens: row.try_get::<Option<i32>, _>("actual_output_tokens").ok().flatten().map(|v| v as i64),
        actual_total_tokens: row.try_get::<Option<i32>, _>("actual_total_tokens").ok().flatten().map(|v| v as i64),
        duration_ms: row.try_get::<Option<i32>, _>("duration_ms").ok().flatten().map(|v| v as i64),
    }
}

pub async fn find_by_idempotency(pool: &sqlx::MySqlPool, uid: &str, key: &str) -> Result<Option<ChatOperation>, StatusCode> {
    let row = sqlx::query("SELECT id,user_id,subscription_id,usage_period_id,reservation_id,request_id,idempotency_key,model_requested,model_used,status,user_message,assistant_reply,response_json,conversation_id,mongo_persisted,mongo_error,token_source,measurement_status,actual_input_tokens,actual_output_tokens,actual_total_tokens,duration_ms FROM chat_operations WHERE user_id=? AND idempotency_key=?")
        .bind(uid).bind(key)
        .fetch_optional(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(row.as_ref().map(row_to_operation))
}

pub async fn get_operation(pool: &sqlx::MySqlPool, uid: &str, id: &str) -> Result<Option<ChatOperation>, StatusCode> {
    let row = sqlx::query("SELECT id,user_id,subscription_id,usage_period_id,reservation_id,request_id,idempotency_key,model_requested,model_used,status,user_message,assistant_reply,response_json,conversation_id,mongo_persisted,mongo_error,token_source,measurement_status,actual_input_tokens,actual_output_tokens,actual_total_tokens,duration_ms FROM chat_operations WHERE id=? AND user_id=?")
        .bind(id).bind(uid)
        .fetch_optional(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(row.as_ref().map(row_to_operation))
}

pub async fn get_operation_by_key(pool: &sqlx::MySqlPool, uid: &str, key: &str) -> Result<Option<ChatOperation>, StatusCode> {
    find_by_idempotency(pool, uid, key).await
}

pub async fn create_operation(
    pool: &sqlx::MySqlPool,
    uid: &str,
    subscription_id: &str,
    usage_period_id: &str,
    reservation_id: &str,
    request_id: &str,
    idempotency_key: &str,
    model_requested: &str,
    user_message: &str,
) -> Result<String, StatusCode> {
    let id = Uuid::new_v4().to_string();
    let result = sqlx::query("INSERT INTO chat_operations (id,user_id,subscription_id,usage_period_id,reservation_id,request_id,idempotency_key,model_requested,user_message,status) VALUES (?,?,?,?,?,?,?,?,?, 'reserved')")
        .bind(&id).bind(uid).bind(subscription_id).bind(usage_period_id).bind(reservation_id)
        .bind(request_id).bind(idempotency_key).bind(model_requested).bind(user_message)
        .execute(pool).await;
    match result {
        Ok(_) => Ok(id),
        Err(sqlx::Error::Database(db)) if db.code().as_deref() == Some("1062") => {
            // Existing operation; return its id.
            let row = sqlx::query("SELECT id FROM chat_operations WHERE user_id=? AND idempotency_key=?")
                .bind(uid).bind(idempotency_key)
                .fetch_one(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(row.get("id"))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn update_inferred(
    pool: &sqlx::MySqlPool,
    operation_id: &str,
    model_used: &str,
    assistant_reply: &str,
    response_json: &Value,
    conversation_id: &str,
    token_source: &str,
    measurement_status: &str,
    actual_input_tokens: i64,
    actual_output_tokens: i64,
    actual_total_tokens: i64,
    duration_ms: i64,
) -> Result<(), StatusCode> {
    sqlx::query("UPDATE chat_operations SET status='inferred', model_used=?, assistant_reply=?, response_json=?, conversation_id=?, token_source=?, measurement_status=?, actual_input_tokens=?, actual_output_tokens=?, actual_total_tokens=?, duration_ms=?, mongo_persisted=0, mongo_error=NULL, updated_at=NOW() WHERE id=?")
        .bind(model_used)
        .bind(assistant_reply)
        .bind(response_json.to_string())
        .bind(conversation_id)
        .bind(token_source)
        .bind(measurement_status)
        .bind(actual_input_tokens as i32)
        .bind(actual_output_tokens as i32)
        .bind(actual_total_tokens as i32)
        .bind(duration_ms as i32)
        .bind(operation_id)
        .execute(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

pub async fn mark_failed(pool: &sqlx::MySqlPool, operation_id: &str, error: &str) -> Result<(), StatusCode> {
    sqlx::query("UPDATE chat_operations SET status='failed', mongo_error=? , updated_at=NOW() WHERE id=?")
        .bind(error)
        .bind(operation_id)
        .execute(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

pub async fn claim_persistence(pool: &sqlx::MySqlPool, operation_id: &str) -> Result<bool, StatusCode> {
    let result = sqlx::query("UPDATE chat_operations SET status='persisting', updated_at=NOW() WHERE id=? AND mongo_persisted=0")
        .bind(operation_id)
        .execute(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(result.rows_affected() == 1)
}

pub async fn mark_persisted(pool: &sqlx::MySqlPool, operation_id: &str) -> Result<(), StatusCode> {
    sqlx::query("UPDATE chat_operations SET status='persisted', mongo_persisted=1, mongo_error=NULL, updated_at=NOW() WHERE id=?")
        .bind(operation_id)
        .execute(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

pub async fn mark_persistence_failed(pool: &sqlx::MySqlPool, operation_id: &str, error: &str) -> Result<(), StatusCode> {
    sqlx::query("UPDATE chat_operations SET status='inferred', mongo_persisted=0, mongo_error=?, updated_at=NOW() WHERE id=?")
        .bind(error)
        .bind(operation_id)
        .execute(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

pub async fn persist_operation(pool: &sqlx::MySqlPool, mongo: &mongodb::Database, operation_id: &str) -> Result<(), StatusCode> {
    let row = sqlx::query("SELECT id,user_id,conversation_id,user_message,assistant_reply,response_json,mongo_persisted FROM chat_operations WHERE id=?")
        .bind(operation_id)
        .fetch_optional(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = match row {
        Some(r) => r,
        None => return Err(StatusCode::NOT_FOUND),
    };
    let mongo_persisted: i8 = row.get("mongo_persisted");
    if mongo_persisted == 1 {
        return Ok(());
    }
    let claimed = claim_persistence(pool, operation_id).await?;
    if !claimed {
        return Ok(());
    }

    let uid: String = row.get("user_id");
    let cid: String = row.try_get("conversation_id").ok().flatten().unwrap_or_else(|| Uuid::new_v4().to_string());
    let user_message: String = row.get("user_message");
    let assistant_reply: String = row.try_get("assistant_reply").ok().flatten().unwrap_or_default();
    let coll = mongo.collection::<mongodb::bson::Document>("conversations");

    let now = Utc::now().to_rfc3339();
    let user_msg = serde_json::json!({"role":"user","content":user_message,"created_at":now,"operation_id":operation_id});
    let asst_msg = serde_json::json!({"role":"assistant","content":assistant_reply,"created_at":now,"operation_id":operation_id});

    let result = async {
        let existing = coll.find_one(doc!{"id": &cid, "user_id": &uid}).await.map_err(|e| e.to_string())?;
        if let Some(doc_existing) = existing {
            let already = doc_existing.get_array("operation_ids")
                .map(|a| a.iter().any(|v| v.as_str() == Some(operation_id)))
                .unwrap_or(false);
            if !already {
                coll.update_one(
                    doc!{"id": &cid, "user_id": &uid},
                    doc!{"$push":{"messages":{"$each":[mongodb::bson::to_bson(&user_msg).map_err(|e| e.to_string())?, mongodb::bson::to_bson(&asst_msg).map_err(|e| e.to_string())?]}}, "$addToSet":{"operation_ids": operation_id}, "$set":{"last_message": &assistant_reply, "updated_at": &now}},
                ).await.map_err(|e| e.to_string())?;
            }
        } else {
            coll.insert_one(doc!{
                "id": &cid,
                "user_id": &uid,
                "title": if user_message.chars().count() > 34 { format!("{}...", user_message.chars().take(34).collect::<String>()) } else { user_message.clone() },
                "last_message": &assistant_reply,
                "updated_at": &now,
                "operation_ids": [operation_id],
                "messages": [mongodb::bson::to_bson(&user_msg).map_err(|e| e.to_string())?, mongodb::bson::to_bson(&asst_msg).map_err(|e| e.to_string())?]
            }).await.map_err(|e| e.to_string())?;
        }
        Ok::<(), String>(())
    }.await;

    match result {
        Ok(()) => {
            mark_persisted(pool, operation_id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(())
        }
        Err(e) => {
            mark_persistence_failed(pool, operation_id, &e).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Err(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}

pub async fn list_pending_operations(pool: &sqlx::MySqlPool, limit: i64) -> Result<Vec<ChatOperation>, StatusCode> {
    let rows = sqlx::query("SELECT id,user_id,subscription_id,usage_period_id,reservation_id,request_id,idempotency_key,model_requested,model_used,status,user_message,assistant_reply,response_json,conversation_id,mongo_persisted,mongo_error,token_source,measurement_status,actual_input_tokens,actual_output_tokens,actual_total_tokens,duration_ms FROM chat_operations WHERE mongo_persisted=0 AND status IN ('inferred','persisting') ORDER BY created_at ASC LIMIT ?")
        .bind(limit)
        .fetch_all(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(rows.iter().map(row_to_operation).collect())
}

pub async fn reconcile_operations(pool: &sqlx::MySqlPool, mongo: &mongodb::Database, apply: bool) -> Result<serde_json::Value, StatusCode> {
    let rows = sqlx::query("SELECT id FROM chat_operations WHERE mongo_persisted=0 ORDER BY created_at ASC LIMIT 100")
        .fetch_all(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut report = Vec::new();
    for r in rows {
        let id: String = r.get("id");
        if apply {
            let result = persist_operation(pool, mongo, &id).await;
            report.push(serde_json::json!({"operation_id": id, "applied": true, "result": if result.is_ok() { "persisted" } else { "failed" }}));
        } else {
            report.push(serde_json::json!({"operation_id": id, "applied": false, "result": "pending"}));
        }
    }
    Ok(serde_json::json!({"operations": report}))
}