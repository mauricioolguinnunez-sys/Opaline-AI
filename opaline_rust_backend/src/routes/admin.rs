use axum::{extract::Path, extract::State, Json, Router, routing::{get, post, put}};
use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;
use sqlx::Row;
use mongodb::bson::doc;
use crate::state::AppState;
use crate::routes::auth::user_id_from_token;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/users", get(list_users))
        .route("/metrics", get(metrics))
        .route("/ai-stats", get(ai_stats))
        .route("/models/:id/toggle", put(toggle_model))
        .route("/expenses", get(list_expenses).post(create_expense))
        .route("/reports/financial", get(financial_report))
}

async fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<String, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let row = sqlx::query("SELECT role FROM users WHERE id=?").bind(&uid).fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let role: String = row.get("role");
    if role != "admin" { return Err(StatusCode::FORBIDDEN); }
    Ok(uid)
}

async fn list_users(headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    require_admin(&state, &headers).await?;
    let rows = sqlx::query("SELECT id,email,name,role,email_verified,created_at FROM users").fetch_all(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(serde_json::json!({
            "id": r.get::<String,_>("id"),
            "email": r.get::<String,_>("email"),
            "name": r.get::<String,_>("name"),
            "role": r.get::<String,_>("role"),
            "email_verified": r.get::<i8,_>("email_verified")==1,
            "created_at": r.get::<chrono::NaiveDateTime,_>("created_at").to_string(),
        }));
    }
    Ok(Json(serde_json::json!(out)))
}

async fn metrics(headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    require_admin(&state, &headers).await?;
    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users").fetch_one(&state.pool).await.unwrap_or(0);
    let subs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM subscriptions WHERE status='active'").fetch_one(&state.pool).await.unwrap_or(0);
    let revenue: String = sqlx::query_scalar::<_, Option<String>>("SELECT CAST(COALESCE(SUM(amount),0) AS CHAR) FROM payments WHERE status='completed'").fetch_one(&state.pool).await.unwrap_or(Some("0".to_string())).unwrap_or("0".to_string());
    Ok(Json(serde_json::json!({"users":users,"active_subscriptions":subs,"revenue":revenue})))
}

async fn ai_stats(headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    require_admin(&state, &headers).await?;
    let total: i64 = state.mongo.collection::<mongodb::bson::Document>("ai_metrics").count_documents(doc!{}).await.unwrap_or(0) as i64;
    let convs: i64 = state.mongo.collection::<mongodb::bson::Document>("conversations").count_documents(doc!{}).await.unwrap_or(0) as i64;
    Ok(Json(serde_json::json!({"ai_requests":total,"conversations":convs})))
}

async fn toggle_model(Path(id): Path<String>, headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    require_admin(&state, &headers).await?;
    sqlx::query("UPDATE ai_models SET enabled = 1 - enabled WHERE id=? OR code=?").bind(&id).bind(&id).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({"ok":true})))
}

#[derive(Deserialize)]
struct ExpenseReq { date: Option<String>, category: String, amount: f64, currency: Option<String>, note: Option<String> }

async fn list_expenses(headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    require_admin(&state, &headers).await?;
    let rows = sqlx::query("SELECT id,date,category,CAST(amount AS CHAR) as amount,currency,note FROM expenses ORDER BY date DESC").fetch_all(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(serde_json::json!({
            "id": r.get::<String,_>("id"),
            "date": r.get::<chrono::NaiveDateTime,_>("date").to_string(),
            "category": r.get::<String,_>("category"),
            "amount": r.get::<String,_>("amount"),
            "currency": r.get::<String,_>("currency"),
            "note": r.get::<Option<String>,_>("note"),
        }));
    }
    Ok(Json(serde_json::json!(out)))
}

async fn create_expense(headers: HeaderMap, State(state): State<AppState>, Json(req): Json<ExpenseReq>) -> Result<Json<serde_json::Value>, StatusCode> {
    require_admin(&state, &headers).await?;
    let id = uuid::Uuid::new_v4().to_string();
    let date = req.date.unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string());
    sqlx::query("INSERT INTO expenses (id,date,category,amount,currency,note) VALUES (?,?,?,?,?,?)")
        .bind(&id).bind(&date).bind(&req.category).bind(&req.amount).bind(req.currency.unwrap_or_else(|| "MXN".to_string())).bind(&req.note)
        .execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({"id":id})))
}

async fn financial_report(headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    require_admin(&state, &headers).await?;
    let revenue: String = sqlx::query_scalar::<_, Option<String>>("SELECT CAST(COALESCE(SUM(amount),0) AS CHAR) FROM payments WHERE status='completed'").fetch_one(&state.pool).await.unwrap_or(Some("0".to_string())).unwrap_or("0".to_string());
    let expenses: String = sqlx::query_scalar::<_, Option<String>>("SELECT CAST(COALESCE(SUM(amount),0) AS CHAR) FROM expenses").fetch_one(&state.pool).await.unwrap_or(Some("0".to_string())).unwrap_or("0".to_string());
    Ok(Json(serde_json::json!({"revenue":revenue,"expenses":expenses})))
}
