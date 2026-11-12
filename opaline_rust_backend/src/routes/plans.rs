use axum::{extract::{Path, State}, Json, Router, routing::{get, post}};
use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;
use sqlx::Row;
use crate::state::AppState;
use crate::models::Plan;
use crate::routes::auth::user_id_from_token;
use uuid::Uuid;
use chrono::{Utc, NaiveDateTime};

pub fn subscription_router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_subscription))
        .route("/:id", get(get_subscription_by_id))
        .route("/history", get(list_subscription_history))
        .route("/cancel", post(cancel_renewal))
        .route("/cancelar-renovacion", post(cancel_renewal))
        .route("/restore", post(restore_subscription))
        .route("/restaurar", post(restore_subscription))
}

pub async fn list_plans(State(state): State<AppState>) -> Result<Json<Vec<Plan>>, StatusCode> {
    let rows = sqlx::query("SELECT id,code,name,CAST(price AS DOUBLE) as price,currency,period,messages_per_month,features,active FROM plans WHERE active=1")
        .fetch_all(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut out = Vec::new();
    for r in rows {
        let features_bytes: Vec<u8> = r.get("features");
        let features_str = String::from_utf8_lossy(&features_bytes).to_string();
        let features: Vec<String> = serde_json::from_str(&features_str).unwrap_or_default();
        out.push(Plan {
            id: r.get("id"),
            code: r.get("code"),
            name: r.get("name"),
            price: r.get::<f64,_>("price"),
            currency: r.get("currency"),
            period: r.get("period"),
            messages_per_month: r.get("messages_per_month"),
            features,
            active: r.get::<i8,_>("active") == 1,
        });
    }
    Ok(Json(out))
}

pub async fn list_plan_versions(Path(plan_id): Path<String>, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let rows = sqlx::query("SELECT id,plan_id,version,CAST(price AS DOUBLE) as price,currency,messages_per_month,features,effective_from,effective_to,status FROM plan_versions WHERE plan_id=? ORDER BY version DESC")
        .bind(&plan_id)
        .fetch_all(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut out = Vec::new();
    for r in rows {
        let features_bytes: Vec<u8> = r.get("features");
        let features_str = String::from_utf8_lossy(&features_bytes).to_string();
        let features: Vec<String> = serde_json::from_str(&features_str).unwrap_or_default();
        let effective_from: NaiveDateTime = r.get("effective_from");
        let effective_to: Option<NaiveDateTime> = r.try_get("effective_to").ok();
        out.push(serde_json::json!({
            "id": r.get::<String,_>("id"),
            "plan_id": r.get::<String,_>("plan_id"),
            "version": r.get::<i32,_>("version"),
            "price": r.get::<f64,_>("price"),
            "currency": r.get::<String,_>("currency"),
            "messages_per_month": r.get::<i32,_>("messages_per_month"),
            "features": features,
            "effective_from": effective_from.to_string(),
            "effective_to": effective_to.map(|d| d.to_string()),
            "status": r.get::<String,_>("status"),
        }));
    }
    Ok(Json(serde_json::json!(out)))
}

async fn current_subscription_row(state: &AppState, uid: &str) -> Result<Option<sqlx::mysql::MySqlRow>, StatusCode> {
    // Lazy expiration: a paid period that has ended is marked expired before it
    // is returned, preventing stale active access after restarts.
    let row = sqlx::query("SELECT id,status,end_date FROM subscriptions WHERE user_id=? ORDER BY is_current DESC, created_at DESC LIMIT 1")
        .bind(uid)
        .fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Some(r) = &row {
        let id: String = r.get("id");
        let status: String = r.get("status");
        let end_date: Option<NaiveDateTime> = r.try_get("end_date").ok();
        if matches!(status.as_str(), "active" | "grace" | "cancelled" | "past_due") {
            if let Some(end) = end_date {
                if end < Utc::now().naive_utc() {
                    let mut tx = state.pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    sqlx::query("UPDATE subscriptions SET status='expired', is_current=NULL, terminated_at=COALESCE(terminated_at, NOW()) WHERE id=?")
                        .bind(&id).execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    sqlx::query("INSERT INTO subscription_events (id,subscription_id,event_type,previous_status,new_status,actor_type,reason) VALUES (?,?,?,?,?,?,?)")
                        .bind(Uuid::new_v4().to_string())
                        .bind(&id)
                        .bind("expired")
                        .bind(&status)
                        .bind("expired")
                        .bind("system")
                        .bind("period ended")
                        .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                }
            }
        }
    }
    sqlx::query("SELECT s.id, s.status, s.start_date, s.end_date, s.current_period_start, s.current_period_end, s.cancel_at_period_end, s.auto_renew, s.terminated_at, s.plan_version_id, s.plan_id, p.name, p.code, CAST(p.price AS DOUBLE) as price FROM subscriptions s JOIN plans p ON p.id=s.plan_id WHERE s.user_id=? ORDER BY s.is_current DESC, s.created_at DESC LIMIT 1")
        .bind(uid)
        .fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn subscription_json(r: &sqlx::mysql::MySqlRow) -> serde_json::Value {
    let start: Option<NaiveDateTime> = r.try_get("start_date").ok();
    let end: Option<NaiveDateTime> = r.try_get("end_date").ok();
    let cps: Option<NaiveDateTime> = r.try_get("current_period_start").ok();
    let cpe: Option<NaiveDateTime> = r.try_get("current_period_end").ok();
    let terminated: Option<NaiveDateTime> = r.try_get("terminated_at").ok();
    serde_json::json!({
        "id": r.get::<String,_>("id"),
        "status": r.get::<String,_>("status"),
        "plan_id": r.get::<String,_>("plan_id"),
        "plan_version_id": r.try_get::<Option<String>,_>("plan_version_id").ok().flatten(),
        "plan_name": r.get::<String,_>("name"),
        "plan_code": r.get::<String,_>("code"),
        "price": r.get::<f64,_>("price"),
        "start_date": start.map(|d| d.to_string()),
        "end_date": end.map(|d| d.to_string()),
        "current_period_start": cps.map(|d| d.to_string()),
        "current_period_end": cpe.map(|d| d.to_string()),
        "cancel_at_period_end": r.try_get::<i8,_>("cancel_at_period_end").unwrap_or(0) == 1,
        "auto_renew": r.try_get::<i8,_>("auto_renew").unwrap_or(1) == 1,
        "terminated_at": terminated.map(|d| d.to_string()),
    })
}

pub async fn get_subscription(headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    match current_subscription_row(&state, &uid).await? {
        Some(r) => Ok(Json(subscription_json(&r))),
        None => Ok(Json(serde_json::json!({"status":"none"}))),
    }
}

pub async fn get_subscription_by_id(Path(id): Path<String>, headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let row = sqlx::query("SELECT s.id, s.status, s.start_date, s.end_date, s.current_period_start, s.current_period_end, s.cancel_at_period_end, s.auto_renew, s.terminated_at, s.plan_version_id, s.plan_id, p.name, p.code, CAST(p.price AS DOUBLE) as price FROM subscriptions s JOIN plans p ON p.id=s.plan_id WHERE s.id=? AND s.user_id=?")
        .bind(&id).bind(&uid)
        .fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    match row {
        Some(r) => Ok(Json(subscription_json(&r))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn list_subscription_history(headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let rows = sqlx::query("SELECT s.id, s.status, s.start_date, s.end_date, s.current_period_start, s.current_period_end, s.cancel_at_period_end, s.auto_renew, s.terminated_at, s.plan_version_id, s.plan_id, p.name, p.code, CAST(p.price AS DOUBLE) as price FROM subscriptions s JOIN plans p ON p.id=s.plan_id WHERE s.user_id=? ORDER BY s.created_at DESC")
        .bind(&uid)
        .fetch_all(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(subscription_json(&r));
    }
    Ok(Json(serde_json::json!(out)))
}

pub async fn cancel_renewal(headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let row = sqlx::query("SELECT id,status FROM subscriptions WHERE user_id=? ORDER BY is_current DESC, created_at DESC LIMIT 1")
        .bind(&uid).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = row.ok_or(StatusCode::NOT_FOUND)?;
    let id: String = row.get("id");
    let status: String = row.get("status");
    if !matches!(status.as_str(), "active" | "grace" | "cancelled") {
        return Err(StatusCode::CONFLICT);
    }
    if status == "cancelled" {
        return Ok(Json(serde_json::json!({"ok":true,"status":"cancelled"})));
    }
    let mut tx = state.pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("UPDATE subscriptions SET status='cancelled', cancel_at_period_end=1, auto_renew=0 WHERE id=?")
        .bind(&id).execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("INSERT INTO subscription_events (id,subscription_id,event_type,previous_status,new_status,actor_type,actor_id,reason) VALUES (?,?,?,?,?,?,?,?)")
        .bind(Uuid::new_v4().to_string())
        .bind(&id)
        .bind("renewal_cancelled")
        .bind(&status)
        .bind("cancelled")
        .bind("user")
        .bind(&uid)
        .bind("user requested cancellation")
        .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({"ok":true,"status":"cancelled"})))
}

pub async fn restore_subscription(headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let row = sqlx::query("SELECT id,status,end_date FROM subscriptions WHERE user_id=? ORDER BY is_current DESC, created_at DESC LIMIT 1")
        .bind(&uid).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = row.ok_or(StatusCode::NOT_FOUND)?;
    let id: String = row.get("id");
    let status: String = row.get("status");
    let end_date: Option<NaiveDateTime> = row.try_get("end_date").ok();
    if status != "cancelled" {
        return Err(StatusCode::CONFLICT);
    }
    if let Some(end) = end_date {
        if end < Utc::now().naive_utc() {
            return Err(StatusCode::CONFLICT);
        }
    }
    let mut tx = state.pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("UPDATE subscriptions SET status='active', cancel_at_period_end=0, auto_renew=1 WHERE id=?")
        .bind(&id).execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("INSERT INTO subscription_events (id,subscription_id,event_type,previous_status,new_status,actor_type,actor_id,reason) VALUES (?,?,?,?,?,?,?,?)")
        .bind(Uuid::new_v4().to_string())
        .bind(&id)
        .bind("renewal_restored")
        .bind("cancelled")
        .bind("active")
        .bind("user")
        .bind(&uid)
        .bind("user restored renewal")
        .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({"ok":true,"status":"active"})))
}

// Internal helper for future payment confirmation. It is intentionally not
// exposed as a public route in FASE 2.1.
pub async fn activate_subscription(state: &AppState, subscription_id: &str, user_id: &str, plan_id: &str, actor_id: &str, source: &str) -> Result<(), StatusCode> {
    let mut tx = state.pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // Resolve the contracted plan version, preserving the version chosen at
    // checkout time when it is already present.
    let version_id: Option<String> = sqlx::query_scalar("SELECT plan_version_id FROM subscriptions WHERE id=? AND user_id=?")
        .bind(subscription_id).bind(user_id)
        .fetch_optional(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.flatten();
    let version_id = match version_id {
        Some(v) => v,
        None => {
            sqlx::query_scalar("SELECT id FROM plan_versions WHERE plan_id=? ORDER BY version DESC LIMIT 1")
                .bind(plan_id)
                .fetch_one(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        }
    };

    sqlx::query("UPDATE subscriptions SET is_current=NULL WHERE user_id=? AND is_current=1 AND id<>?")
        .bind(user_id).bind(subscription_id).execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let activation = sqlx::query("UPDATE subscriptions SET status='active', plan_id=?, plan_version_id=?, activation_source=?, is_current=1, auto_renew=1, cancel_at_period_end=0, start_date=COALESCE(start_date,NOW()), current_period_start=COALESCE(current_period_start,NOW()), current_period_end=COALESCE(current_period_end,DATE_ADD(NOW(),INTERVAL 1 MONTH)), end_date=COALESCE(end_date,DATE_ADD(NOW(),INTERVAL 1 MONTH)) WHERE id=? AND user_id=?")
        .bind(plan_id).bind(&version_id).bind(source).bind(subscription_id).bind(user_id)
        .execute(&mut *tx).await;
    match activation {
        Ok(_) => {}
        Err(sqlx::Error::Database(db)) if db.code().as_deref() == Some("1062") => {
            tx.rollback().await.ok();
            return Err(StatusCode::CONFLICT);
        }
        Err(_) => {
            tx.rollback().await.ok();
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }
    sqlx::query("INSERT INTO subscription_events (id,subscription_id,event_type,previous_status,new_status,actor_type,actor_id,reason) VALUES (?,?,?,?,?,?,?,?)")
        .bind(Uuid::new_v4().to_string())
        .bind(subscription_id)
        .bind("activated")
        .bind("pending")
        .bind("active")
        .bind("system")
        .bind(actor_id)
        .bind(source)
        .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}