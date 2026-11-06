use axum::{extract::{Path, State}, Json, Router, routing::{get, post}};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use serde::Deserialize;
use sqlx::Row;
use crate::state::AppState;
use crate::routes::auth::user_id_from_token;
use uuid::Uuid;
use chrono::Utc;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/paypal/create", post(create_order))
        .route("/paypal/verify/:order_id", get(verify_order))
}

pub fn invoices_router() -> Router<AppState> {
    Router::new()
        .route("/:id/pdf", get(invoice_pdf))
        .route("/:id/reembolso", post(refund))
}

#[derive(Deserialize)]
struct CreateReq { plan_id: String }

async fn create_order(headers: HeaderMap, State(state): State<AppState>, Json(req): Json<CreateReq>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    // plan
    let plan = sqlx::query("SELECT id,CAST(price AS DOUBLE) as price FROM plans WHERE id=? OR code=?")
        .bind(&req.plan_id).bind(&req.plan_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let plan = plan.ok_or(StatusCode::NOT_FOUND)?;
    let plan_id: String = plan.get("id");
    let price: f64 = plan.get("price");
    let pay_id = Uuid::new_v4().to_string();
    let order_id = pay_id.clone();
    sqlx::query("INSERT INTO payments (id,user_id,plan_id,paypal_order_id,amount,status) VALUES (?,?,?,?,?,'pending')")
        .bind(&pay_id).bind(&uid).bind(&plan_id).bind(&order_id).bind(price)
        .execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // subscription pending
    let sid = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO subscriptions (id,user_id,plan_id,status) VALUES (?,?,?,'pending')")
        .bind(&sid).bind(&uid).bind(&plan_id).execute(&state.pool).await.ok();
    Ok(Json(serde_json::json!({
        "order_id": order_id,
        "approval_url": format!("http://127.0.0.1:8080/paypal/approve?order={}", order_id)
    })))
}

async fn verify_order(Path(order_id): Path<String>, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let row = sqlx::query("SELECT id,user_id,plan_id,amount FROM payments WHERE paypal_order_id=?")
        .bind(&order_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = row.ok_or(StatusCode::NOT_FOUND)?;
    let pay_id: String = row.get("id");
    let uid: String = row.get("user_id");
    let plan_id: String = row.get("plan_id");
    // mark completed
    sqlx::query("UPDATE payments SET status='completed' WHERE id=?").bind(&pay_id).execute(&state.pool).await.ok();
    // activate subscription
    sqlx::query("UPDATE subscriptions SET status='active', start_date=NOW(), end_date=DATE_ADD(NOW(), INTERVAL 1 MONTH) WHERE user_id=? AND plan_id=? AND status='pending'")
        .bind(&uid).bind(&plan_id).execute(&state.pool).await.ok();
    // invoice
    let inv_id = Uuid::new_v4().to_string();
    let folio = format!("OP-{}", &pay_id[..8]);
    sqlx::query("INSERT INTO invoices (id,payment_id,folio) VALUES (?,?,?)").bind(&inv_id).bind(&pay_id).bind(&folio).execute(&state.pool).await.ok();
    let _ = state.mongo.collection::<mongodb::bson::Document>("payment_events").insert_one(mongodb::bson::doc!{"event":"payment_completed","order_id":&order_id,"ts":mongodb::bson::DateTime::now()}).await;
    Ok(Json(serde_json::json!({"status":"completed","invoice_id":inv_id})))
}

async fn invoice_pdf(Path(id): Path<String>, State(state): State<AppState>) -> Response {
    // find invoice
    let row = sqlx::query("SELECT i.folio, CAST(p.amount AS CHAR) as amount, p.currency, p.status FROM invoices i JOIN payments p ON p.id=i.payment_id WHERE i.id=?")
        .bind(&id).fetch_optional(&state.pool).await.ok().flatten();
    let body = if let Some(r) = row {
        let folio: String = r.get("folio");
        let amount: String = r.get("amount");
        let currency: String = r.get("currency");
        let status: String = r.get("status");
        format!("Opaline AI - Comprobante\nFolio: {}\nImporte: {} {}\nEstado: {}\n", folio, amount, currency, status)
    } else {
        "Factura no encontrada".to_string()
    };
    // return as text (mobile downloadBytes just needs bytes)
    Response::builder().header(header::CONTENT_TYPE, "text/plain").body(body.into()).unwrap()
}

async fn refund(Path(id): Path<String>, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    sqlx::query("UPDATE payments SET status='refunded' WHERE id=?").bind(&id).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({"ok":true})))
}
