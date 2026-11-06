use axum::{extract::State, Json, Router, routing::get};
use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;
use sqlx::Row;
use crate::state::AppState;
use crate::models::Plan;
use crate::routes::auth::user_id_from_token;

pub fn subscription_router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_subscription))
        .route("/cancelar-renovacion", axum::routing::post(cancel_renewal))
        .route("/restaurar", axum::routing::post(restore))
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

async fn get_subscription(headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let row = sqlx::query("SELECT s.id, s.status, s.start_date, s.end_date, p.name, p.code, CAST(p.price AS DOUBLE) as price FROM subscriptions s JOIN plans p ON p.id=s.plan_id WHERE s.user_id=? ORDER BY s.created_at DESC LIMIT 1")
        .bind(&uid).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    match row {
        Some(r) => {
            let start: Option<chrono::NaiveDateTime> = r.try_get("start_date").ok();
            let end: Option<chrono::NaiveDateTime> = r.try_get("end_date").ok();
            Ok(Json(serde_json::json!({
                "id": r.get::<String,_>("id"),
                "status": r.get::<String,_>("status"),
                "plan_name": r.get::<String,_>("name"),
                "plan_code": r.get::<String,_>("code"),
                "price": r.get::<f64,_>("price"),
                "start_date": start.map(|d| d.to_string()),
                "end_date": end.map(|d| d.to_string()),
            })))
        }
        None => Ok(Json(serde_json::json!({"status":"none"}))),
    }
}

async fn cancel_renewal(headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    sqlx::query("UPDATE subscriptions SET status='cancelled' WHERE user_id=? AND status IN ('active','grace')")
        .bind(&uid).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({"ok":true})))
}

async fn restore(headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    sqlx::query("UPDATE subscriptions SET status='active' WHERE user_id=? AND status='cancelled'")
        .bind(&uid).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({"ok":true})))
}
