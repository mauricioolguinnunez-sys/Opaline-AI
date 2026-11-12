use axum::{extract::{ConnectInfo, Path, State}, Json, Router, routing::{get, post}};
use axum::http::{HeaderMap, StatusCode, header};
use std::net::SocketAddr;
use axum::body::Bytes;
use axum::response::Response;
use serde::Deserialize;
use sqlx::Row;
use crate::state::AppState;
use crate::routes::auth::user_id_from_token;
use crate::paypal::{PayPalClient, PayPalError};
use crate::outbox;
use sha2::{Digest, Sha256};
use uuid::Uuid;
use chrono::Utc;


fn client_ip(connect: &SocketAddr) -> String {
    connect.ip().to_string()
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/history", get(list_payments))
        .route("/paypal/create", post(create_order))
        .route("/paypal/orders/:order_id", get(get_paypal_order))
        .route("/paypal/orders/:order_id/capture", post(capture_paypal_order))
        .route("/paypal/orders/:order_id/activate", post(activate_paypal_subscription))
        .route("/paypal/capture/:order_id", post(capture_paypal_order))
        .route("/paypal/verify/:order_id", get(get_paypal_order))
        .route("/paypal/webhook", post(paypal_webhook))
}

pub fn invoices_router() -> Router<AppState> {
    Router::new()
        .route("/:id/pdf", get(invoice_pdf))
        .route("/:id/reembolso", post(refund))
}

pub async fn list_payments(headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let rows = sqlx::query("SELECT p.id,CAST(p.amount AS DOUBLE) as amount,p.currency,p.status,p.created_at,pl.name AS plan_name,i.id AS invoice_id,i.folio FROM payments p JOIN plans pl ON pl.id=p.plan_id LEFT JOIN invoices i ON i.payment_id=p.id WHERE p.user_id=? ORDER BY p.created_at DESC LIMIT 50")
        .bind(&uid).fetch_all(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(serde_json::json!({
            "id": r.get::<String,_>("id"),
            "amount": r.get::<f64,_>("amount"),
            "currency": r.get::<String,_>("currency"),
            "status": r.get::<String,_>("status"),
            "created_at": r.get::<chrono::NaiveDateTime,_>("created_at").to_string(),
            "plan_name": r.get::<String,_>("plan_name"),
            "invoice_id": r.try_get::<Option<String>,_>("invoice_id").ok().flatten(),
            "folio": r.try_get::<Option<String>,_>("folio").ok().flatten(),
        }));
    }
    Ok(Json(serde_json::json!(out)))
}

#[derive(Deserialize)]
pub struct CreateReq {
    pub plan_id: String,
    pub idempotency_key: Option<String>,
}

pub async fn create_order(ConnectInfo(connect): ConnectInfo<SocketAddr>, headers: HeaderMap, State(state): State<AppState>, Json(req): Json<CreateReq>) -> Result<Json<serde_json::Value>, StatusCode> {
    let ip = client_ip(&connect);
    if !state.rate_limiter.check(&format!("payments:create:ip:{ip}"), 20, std::time::Duration::from_secs(3600)) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    if !state.rate_limiter.check(&format!("payments:create:user:{uid}"), 10, std::time::Duration::from_secs(3600)) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let idempotency_key = req.idempotency_key
        .or_else(|| headers.get("x-idempotency-key").and_then(|v| v.to_str().ok()).map(|s| s.to_string()))
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    if idempotency_key.chars().count() > 128 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let client = PayPalClient::from_env().map_err(|_| StatusCode::NOT_IMPLEMENTED)?;
    let result = create_paypal_order(&state, &uid, &req.plan_id, &idempotency_key, &client).await?;
    Ok(Json(result))
}

pub async fn get_paypal_order(ConnectInfo(_connect): ConnectInfo<SocketAddr>, headers: HeaderMap, Path(order_id): Path<String>, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let client = PayPalClient::from_env().map_err(|_| StatusCode::NOT_IMPLEMENTED)?;
    let result = fetch_paypal_order(&state, &uid, &order_id, &client).await?;
    Ok(Json(result))
}

pub async fn verify_order(ConnectInfo(connect): ConnectInfo<SocketAddr>, headers: HeaderMap, Path(order_id): Path<String>, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    get_paypal_order(ConnectInfo(connect), headers, Path(order_id), State(state)).await
}

pub async fn invoice_pdf(headers: HeaderMap, Path(id): Path<String>, State(state): State<AppState>) -> Response {
    let t = match headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")) {
        Some(token) => token.to_string(),
        None => {
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .body(serde_json::json!({"error":"Autenticacion requerida."}).to_string().into())
                .unwrap();
        }
    };
    let uid = match user_id_from_token(&state, &t).await {
        Some(uid) => uid,
        None => {
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .body(serde_json::json!({"error":"Sesion invalida o expirada."}).to_string().into())
                .unwrap();
        }
    };

    // find invoice owned by the authenticated user
    let row = sqlx::query("SELECT i.folio, CAST(p.amount AS CHAR) as amount, p.currency, p.status FROM invoices i JOIN payments p ON p.id=i.payment_id WHERE i.id=? AND p.user_id=?")
        .bind(&id)
        .bind(&uid)
        .fetch_optional(&state.pool)
        .await
        .ok()
        .flatten();
    let found = row.is_some();
    let body = if let Some(r) = row {
        let folio: String = r.get("folio");
        let amount: String = r.get("amount");
        let currency: String = r.get("currency");
        let status: String = r.get("status");
        format!("Opaline AI - Comprobante\nFolio: {}\nImporte: {} {}\nEstado: {}\n", folio, amount, currency, status)
    } else {
        "Factura no encontrada".to_string()
    };
    let status = if found {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    };

    // return as text (mobile downloadBytes just needs bytes)
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(body.into())
        .unwrap()
}

pub async fn refund(ConnectInfo(connect): ConnectInfo<SocketAddr>, headers: HeaderMap, Path(id): Path<String>, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let ip = client_ip(&connect);
    if !state.rate_limiter.check(&format!("payments:refund:ip:{ip}"), 20, std::time::Duration::from_secs(900)) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let _uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    if !state.rate_limiter.check(&format!("payments:refund:user:{}", _uid), 10, std::time::Duration::from_secs(900)) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    // Refunds require a real provider confirmation. The previous implementation
    // only changed a local column and reported success.
    if !payment_provider_configured() {
        return Err(StatusCode::NOT_IMPLEMENTED);
    }

    let _ = id;
    let _ = state;
    Err(StatusCode::NOT_IMPLEMENTED)
}



pub async fn create_paypal_order(
    state: &AppState,
    uid: &str,
    plan_id: &str,
    idempotency_key: &str,
    client: &PayPalClient,
) -> Result<serde_json::Value, StatusCode> {
    let (plan_db_id, plan_version_id, amount, currency, plan_name) = resolve_plan_and_version(&state.pool, plan_id).await?;

    // Reuse an existing operation for the same idempotency key.
    let existing = sqlx::query("SELECT id, plan_id, plan_version_id, CAST(amount AS CHAR) AS amount, currency, status, provider_order_id, provider_request_id FROM payment_orders WHERE user_id=? AND idempotency_key=?")
        .bind(uid).bind(idempotency_key)
        .fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Some(row) = existing {
        let existing_plan: String = row.get("plan_id");
        let existing_version: String = row.get("plan_version_id");
        let existing_amount: String = row.get("amount");
        let existing_currency: String = row.get("currency");
        if existing_plan != plan_db_id || existing_version != plan_version_id || existing_amount != amount || existing_currency != currency {
            return Err(StatusCode::CONFLICT);
        }
        let local_id: String = row.get("id");
        let status: String = row.get("status");
        let provider_order_id: Option<String> = row.try_get("provider_order_id").ok().flatten();
        let provider_request_id: Option<String> = row.try_get("provider_request_id").ok().flatten();

        if let Some(provider_order_id) = provider_order_id {
            let paypal_order = client.get_order(&provider_order_id).await.map_err(|_| StatusCode::BAD_GATEWAY)?;
            let provider_status = paypal_order.get("status").and_then(|v| v.as_str()).unwrap_or("CREATED");
            let local_status = map_paypal_order_status(provider_status);
            if local_status != status {
                sqlx::query("UPDATE payment_orders SET status=?, updated_at=NOW() WHERE id=?")
                    .bind(local_status).bind(&local_id).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            }
            let approval_url = extract_approval_url(&paypal_order).unwrap_or_else(|| format!("https://www.sandbox.paypal.com/checkoutnow?token={}", provider_order_id));
            return Ok(serde_json::json!({
                "order_id": local_id,
                "provider_order_id": provider_order_id,
                "status": local_status,
                "provider_status": provider_status,
                "approval_url": approval_url,
                "amount": amount,
                "currency": currency,
                "plan_name": plan_name,
            }));
        }

        // A previous attempt may have created a local row but not received PayPal's response.
        let request_id = provider_request_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let paypal_order = create_remote_order(client, &request_id, &amount, &currency, &plan_name).await?;
        let provider_order_id = paypal_order.get("id").and_then(|v| v.as_str()).ok_or(StatusCode::BAD_GATEWAY)?.to_string();
        let provider_status = paypal_order.get("status").and_then(|v| v.as_str()).unwrap_or("CREATED");
        let local_status = map_paypal_order_status(provider_status);
        sqlx::query("UPDATE payment_orders SET provider_order_id=?, provider_request_id=?, status=?, updated_at=NOW() WHERE id=?")
            .bind(&provider_order_id).bind(&request_id).bind(local_status).bind(&local_id)
            .execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let approval_url = extract_approval_url(&paypal_order).unwrap_or_else(|| format!("https://www.sandbox.paypal.com/checkoutnow?token={}", provider_order_id));
        return Ok(serde_json::json!({
            "order_id": local_id,
            "provider_order_id": provider_order_id,
            "status": local_status,
            "provider_status": provider_status,
            "approval_url": approval_url,
            "amount": amount,
            "currency": currency,
            "plan_name": plan_name,
        }));
    }

    let local_id = Uuid::new_v4().to_string();
    let request_id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO payment_orders (id, user_id, plan_id, plan_version_id, provider, provider_order_id, provider_request_id, idempotency_key, amount, currency, status, description, expires_at) VALUES (?,?,?,?, 'paypal', NULL, ?, ?, ?, ?, 'created', ?, DATE_ADD(NOW(), INTERVAL 3 HOUR))")
        .bind(&local_id)
        .bind(uid)
        .bind(&plan_db_id)
        .bind(&plan_version_id)
        .bind(&request_id)
        .bind(idempotency_key)
        .bind(&amount)
        .bind(&currency)
        .bind(&plan_name)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            if is_duplicate_key(&e) { StatusCode::CONFLICT } else { StatusCode::INTERNAL_SERVER_ERROR }
        })?;

    // If a concurrent request won the insert, reuse its row.
    let row = sqlx::query("SELECT id, provider_order_id, provider_request_id, status FROM payment_orders WHERE user_id=? AND idempotency_key=?")
        .bind(uid).bind(idempotency_key)
        .fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let local_id: String = row.get("id");
    let existing_provider_order: Option<String> = row.try_get("provider_order_id").ok().flatten();
    let existing_request: Option<String> = row.try_get("provider_request_id").ok().flatten();
    let status: String = row.get("status");

    if let Some(provider_order_id) = existing_provider_order {
        let paypal_order = client.get_order(&provider_order_id).await.map_err(|_| StatusCode::BAD_GATEWAY)?;
        let provider_status = paypal_order.get("status").and_then(|v| v.as_str()).unwrap_or("CREATED");
        let approval_url = extract_approval_url(&paypal_order).unwrap_or_else(|| format!("https://www.sandbox.paypal.com/checkoutnow?token={}", provider_order_id));
        return Ok(serde_json::json!({
            "order_id": local_id,
            "provider_order_id": provider_order_id,
            "status": status,
            "provider_status": provider_status,
            "approval_url": approval_url,
            "amount": amount,
            "currency": currency,
            "plan_name": plan_name,
        }));
    }

    let request_id = existing_request.unwrap_or_else(|| Uuid::new_v4().to_string());
    let paypal_order = create_remote_order(client, &request_id, &amount, &currency, &plan_name).await?;
    let provider_order_id = paypal_order.get("id").and_then(|v| v.as_str()).ok_or(StatusCode::BAD_GATEWAY)?.to_string();
    let provider_status = paypal_order.get("status").and_then(|v| v.as_str()).unwrap_or("CREATED");
    let local_status = map_paypal_order_status(provider_status);

    sqlx::query("UPDATE payment_orders SET provider_order_id=?, provider_request_id=?, status=?, updated_at=NOW() WHERE id=?")
        .bind(&provider_order_id)
        .bind(&request_id)
        .bind(local_status)
        .bind(&local_id)
        .execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let approval_url = extract_approval_url(&paypal_order).unwrap_or_else(|| format!("https://www.sandbox.paypal.com/checkoutnow?token={}", provider_order_id));
    Ok(serde_json::json!({
        "order_id": local_id,
        "provider_order_id": provider_order_id,
        "status": local_status,
        "provider_status": provider_status,
        "approval_url": approval_url,
        "amount": amount,
        "currency": currency,
        "plan_name": plan_name,
    }))
}

pub async fn fetch_paypal_order(
    state: &AppState,
    uid: &str,
    order_id: &str,
    client: &PayPalClient,
) -> Result<serde_json::Value, StatusCode> {
    let row = sqlx::query("SELECT id, user_id, plan_id, plan_version_id, provider, provider_order_id, CAST(amount AS CHAR) AS amount, currency, status FROM payment_orders WHERE id=? AND user_id=?")
        .bind(order_id).bind(uid)
        .fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let local_id: String = row.get("id");
    let provider: String = row.get("provider");
    let provider_order_id: Option<String> = row.try_get("provider_order_id").ok().flatten();
    let local_status: String = row.get("status");
    let amount: String = row.get("amount");
    let currency: String = row.get("currency");

    let Some(provider_order_id) = provider_order_id else {
        return Ok(serde_json::json!({
            "order_id": local_id,
            "status": local_status,
            "provider_status": null,
            "synchronized": false,
        }));
    };

    let paypal_order = client.get_order(&provider_order_id).await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    let provider_order_id_remote: String = paypal_order.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if provider_order_id_remote != provider_order_id {
        return Err(StatusCode::BAD_GATEWAY);
    }
    let provider_status = paypal_order.get("status").and_then(|v| v.as_str()).unwrap_or("CREATED");
    let remote_amount = paypal_order
        .get("purchase_units")
        .and_then(|v| v.get(0))
        .and_then(|v| v.get("amount"))
        .and_then(|v| v.get("value"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let remote_currency = paypal_order
        .get("purchase_units")
        .and_then(|v| v.get(0))
        .and_then(|v| v.get("amount"))
        .and_then(|v| v.get("currency_code"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if remote_amount != amount || remote_currency != currency {
        return Err(StatusCode::BAD_GATEWAY);
    }

    let mapped = map_paypal_order_status(provider_status);
    if mapped != local_status {
        sqlx::query("UPDATE payment_orders SET status=?, updated_at=NOW() WHERE id=? AND user_id=?")
            .bind(mapped).bind(&local_id).bind(uid)
            .execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(serde_json::json!({
        "order_id": local_id,
        "provider": provider,
        "provider_order_id": provider_order_id,
        "status": mapped,
        "provider_status": provider_status,
        "amount": amount,
        "currency": currency,
        "synchronized": true,
    }))
}


pub async fn capture_paypal_order(
    ConnectInfo(_connect): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(order_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let client = PayPalClient::from_env().map_err(|_| StatusCode::NOT_IMPLEMENTED)?;
    let result = capture_paypal_order_service(&state, &uid, &order_id, &client).await?;
    Ok(Json(result))
}

pub async fn capture_paypal_order_service(
    state: &AppState,
    uid: &str,
    order_id: &str,
    client: &PayPalClient,
) -> Result<serde_json::Value, StatusCode> {
    let local = sqlx::query("SELECT id, user_id, provider, provider_order_id, CAST(amount AS CHAR) AS amount, currency, status FROM payment_orders WHERE id=? AND user_id=?")
        .bind(order_id).bind(uid)
        .fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let local_id: String = local.get("id");
    let provider: String = local.get("provider");
    let provider_order_id: Option<String> = local.try_get("provider_order_id").ok().flatten();
    let local_status: String = local.get("status");
    let amount: String = local.get("amount");
    let currency: String = local.get("currency");

    let provider_order_id = provider_order_id.ok_or(StatusCode::CONFLICT)?;

    // Already captured.
    let existing = sqlx::query("SELECT id, provider_transaction_id, status, CAST(amount AS CHAR) AS amount, currency FROM payment_transactions WHERE payment_order_id=? AND status='completed' ORDER BY created_at DESC LIMIT 1")
        .bind(&local_id)
        .fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Some(row) = existing {
        let tx_id: String = row.get("id");
        let capture_id: String = row.get("provider_transaction_id");
        let tx_amount: String = row.get("amount");
        let tx_currency: String = row.get("currency");
        return Ok(serde_json::json!({
            "order_id": local_id,
            "transaction_id": tx_id,
            "provider_order_id": provider_order_id,
            "provider_transaction_id": capture_id,
            "status": "captured",
            "amount": tx_amount,
            "currency": tx_currency,
            "idempotent": true,
        }));
    }

    // Verify eligibility with PayPal.
    let paypal_order = client.get_order(&provider_order_id).await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    let remote_order_id = paypal_order.get("id").and_then(|v| v.as_str()).unwrap_or("");
    if remote_order_id != provider_order_id {
        return Err(StatusCode::BAD_GATEWAY);
    }
    let provider_status = paypal_order.get("status").and_then(|v| v.as_str()).unwrap_or("");
    if provider_status != "APPROVED" {
        return Err(StatusCode::CONFLICT);
    }
    let remote_amount = paypal_order.get("purchase_units").and_then(|v| v.get(0)).and_then(|v| v.get("amount")).and_then(|v| v.get("value")).and_then(|v| v.as_str()).unwrap_or("");
    let remote_currency = paypal_order.get("purchase_units").and_then(|v| v.get(0)).and_then(|v| v.get("amount")).and_then(|v| v.get("currency_code")).and_then(|v| v.as_str()).unwrap_or("");
    if remote_amount != amount || remote_currency != currency {
        return Err(StatusCode::CONFLICT);
    }

    // Lock the order and reuse or create a pending transaction.
    let mut tx = state.pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let locked = sqlx::query("SELECT status FROM payment_orders WHERE id=? AND user_id=? FOR UPDATE")
        .bind(&local_id).bind(uid)
        .fetch_optional(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let locked_status: String = locked.ok_or(StatusCode::NOT_FOUND)?.get("status");
    if locked_status == "captured" {
        tx.commit().await.ok();
        // Fall back to the completed transaction if it exists.
        if let Some(row) = sqlx::query("SELECT id, provider_transaction_id, CAST(amount AS CHAR) AS amount, currency FROM payment_transactions WHERE payment_order_id=? AND status='completed' ORDER BY created_at DESC LIMIT 1")
            .bind(&local_id).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)? {
            let tx_id: String = row.get("id");
            let capture_id: String = row.get("provider_transaction_id");
            let tx_amount: String = row.get("amount");
            let tx_currency: String = row.get("currency");
            return Ok(serde_json::json!({
                "order_id": local_id,
                "transaction_id": tx_id,
                "provider_order_id": provider_order_id,
                "provider_transaction_id": capture_id,
                "status": "captured",
                "amount": tx_amount,
                "currency": tx_currency,
                "idempotent": true,
            }));
        }
        return Err(StatusCode::CONFLICT);
    }

    let pending = sqlx::query("SELECT id, provider_transaction_id FROM payment_transactions WHERE payment_order_id=? AND status='pending' ORDER BY created_at DESC LIMIT 1")
        .bind(&local_id).fetch_optional(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let (pending_id, request_id) = if let Some(row) = pending {
        (Some(row.get::<String,_>("id")), row.get::<String,_>("provider_transaction_id"))
    } else {
        let request_id = Uuid::new_v4().to_string();
        let pending_id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO payment_transactions (id, payment_order_id, user_id, provider, provider_transaction_id, amount, currency, status) VALUES (?,?,?,?,?,?,?,'pending')")
            .bind(&pending_id)
            .bind(&local_id)
            .bind(uid)
            .bind(&provider)
            .bind(&request_id)
            .bind(&amount)
            .bind(&currency)
            .execute(&mut *tx).await.map_err(|e| {
                if is_duplicate_key(&e) { StatusCode::CONFLICT } else { StatusCode::INTERNAL_SERVER_ERROR }
            })?;
        (Some(pending_id), request_id)
    };
    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Execute capture outside the database transaction.
    let capture_response = client.capture_order(&provider_order_id, &request_id).await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    let capture = capture_response
        .get("purchase_units")
        .and_then(|v| v.get(0))
        .and_then(|v| v.get("payments"))
        .and_then(|v| v.get("captures"))
        .and_then(|v| v.get(0))
        .ok_or(StatusCode::BAD_GATEWAY)?;
    let capture_id = capture.get("id").and_then(|v| v.as_str()).ok_or(StatusCode::BAD_GATEWAY)?.to_string();
    let capture_status = capture.get("status").and_then(|v| v.as_str()).unwrap_or("PENDING");
    let capture_amount = capture.get("amount").and_then(|v| v.get("value")).and_then(|v| v.as_str()).unwrap_or(&amount).to_string();
    let capture_currency = capture.get("amount").and_then(|v| v.get("currency_code")).and_then(|v| v.as_str()).unwrap_or(&currency).to_string();
    if capture_amount != amount || capture_currency != currency {
        return Err(StatusCode::BAD_GATEWAY);
    }

    let local_capture_status = match capture_status {
        "COMPLETED" => "completed",
        "PENDING" => "pending",
        "DENIED" | "FAILED" => "denied",
        _ => "pending",
    };
    let transaction_id = pending_id.clone();

    // Persist the capture result.
    let mut tx = state.pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Some(pending_id) = pending_id {
        sqlx::query("UPDATE payment_transactions SET provider_transaction_id=?, status=?, amount=?, currency=?, captured_at=CASE WHEN ?='completed' THEN NOW() ELSE captured_at END, updated_at=NOW() WHERE id=?")
            .bind(&capture_id)
            .bind(local_capture_status)
            .bind(&capture_amount)
            .bind(&capture_currency)
            .bind(local_capture_status)
            .bind(&pending_id)
            .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    if local_capture_status == "completed" {
        sqlx::query("UPDATE payment_orders SET status='captured', captured_at=NOW(), updated_at=NOW() WHERE id=? AND user_id=?")
            .bind(&local_id).bind(uid)
            .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        outbox::insert(&mut *tx, &outbox::NewOutboxEvent {
            id: None,
            event_type: "payment.capture.completed".to_string(),
            aggregate_type: "payment_order".to_string(),
            aggregate_id: local_id.clone(),
            payload: serde_json::json!({"payment_order_id": local_id, "transaction_id": capture_id, "amount": capture_amount, "currency": capture_currency}).to_string(),
            correlation_id: None,
            available_at: None,
        }).await;
    } else if local_capture_status == "denied" {
        sqlx::query("UPDATE payment_orders SET status='failed', updated_at=NOW() WHERE id=? AND user_id=?")
            .bind(&local_id).bind(uid)
            .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if local_capture_status == "completed" {
        activate_subscription_from_payment(&state.pool, &local_id, uid, uid, "payment_capture").await?;
    }

    let response_status = if local_capture_status == "completed" { "captured" } else { local_capture_status };
    Ok(serde_json::json!({
        "order_id": local_id,
        "provider_order_id": provider_order_id,
        "transaction_id": transaction_id,
        "provider_transaction_id": capture_id,
        "status": response_status,
        "provider_status": capture_status,
        "amount": capture_amount,
        "currency": capture_currency,
    }))
}

async fn resolve_plan_and_version(pool: &sqlx::MySqlPool, plan_id: &str) -> Result<(String, String, String, String, String), StatusCode> {
    let row = sqlx::query("SELECT p.id, p.name, pv.id AS version_id, CAST(pv.price AS CHAR) AS price, pv.currency FROM plans p JOIN plan_versions pv ON pv.plan_id=p.id WHERE (p.id=? OR p.code=?) AND p.active=1 AND pv.status='active' ORDER BY pv.version DESC LIMIT 1")
        .bind(plan_id).bind(plan_id)
        .fetch_optional(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok((
        row.get("id"),
        row.get("version_id"),
        row.get("price"),
        row.get("currency"),
        row.get("name"),
    ))
}

async fn create_remote_order(client: &PayPalClient, request_id: &str, amount: &str, currency: &str, description: &str) -> Result<serde_json::Value, StatusCode> {
    let body = serde_json::json!({
        "intent": "CAPTURE",
        "purchase_units": [{
            "amount": {
                "currency_code": currency,
                "value": amount,
            },
            "description": description,
        }],
        "application_context": {
            "return_url": "http://127.0.0.1:8000/frontend/cuenta.php",
            "cancel_url": "http://127.0.0.1:8000/frontend/planes.php",
        }
    });
    client.create_order(request_id, &body).await.map_err(|_| StatusCode::BAD_GATEWAY)
}

fn map_paypal_order_status(status: &str) -> &'static str {
    match status {
        "CREATED" | "SAVED" => "created",
        "APPROVED" => "approved",
        "COMPLETED" => "captured",
        "VOIDED" => "cancelled",
        _ => "created",
    }
}

fn extract_approval_url(order: &serde_json::Value) -> Option<String> {
    order.get("links")?.as_array()?.iter().find_map(|link| {
        if link.get("rel").and_then(|v| v.as_str()) == Some("approve") {
            link.get("href").and_then(|v| v.as_str()).map(|s| s.to_string())
        } else {
            None
        }
    })
}

fn is_duplicate_key(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Database(db) => db.code().as_deref() == Some("1062"),
        _ => false,
    }
}


pub async fn paypal_webhook(
    ConnectInfo(connect): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, StatusCode> {
    let ip = client_ip(&connect);
    if !state.rate_limiter.check(&format!("payments:webhook:ip:{ip}"), 120, std::time::Duration::from_secs(300)) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    if body.len() > 1_048_576 {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

    let event: serde_json::Value = serde_json::from_slice(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
    let provider_event_id = event.get("id").and_then(|v| v.as_str()).ok_or(StatusCode::BAD_REQUEST)?.to_string();
    let event_type = event.get("event_type").and_then(|v| v.as_str()).ok_or(StatusCode::BAD_REQUEST)?.to_string();
    let resource_id = event
        .get("resource")
        .and_then(|v| v.get("supplementary_data"))
        .and_then(|v| v.get("related_ids"))
        .and_then(|v| v.get("order_id"))
        .and_then(|v| v.as_str())
        .or_else(|| {
            event
                .get("resource")
                .and_then(|v| v.get("id"))
                .and_then(|v| v.as_str())
        })
        .map(|s| s.to_string());

    let transmission_id = headers.get("PAYPAL-TRANSMISSION-ID").and_then(|v| v.to_str().ok()).ok_or(StatusCode::BAD_REQUEST)?;
    let cert_url = headers.get("PAYPAL-CERT-URL").and_then(|v| v.to_str().ok()).ok_or(StatusCode::BAD_REQUEST)?;
    let auth_algo = headers.get("PAYPAL-AUTH-ALGO").and_then(|v| v.to_str().ok()).ok_or(StatusCode::BAD_REQUEST)?;
    let transmission_sig = headers.get("PAYPAL-TRANSMISSION-SIG").and_then(|v| v.to_str().ok()).ok_or(StatusCode::BAD_REQUEST)?;
    let transmission_time = headers.get("PAYPAL-TRANSMISSION-TIME").and_then(|v| v.to_str().ok()).ok_or(StatusCode::BAD_REQUEST)?;

    let client = PayPalClient::from_env().map_err(|_| StatusCode::NOT_IMPLEMENTED)?;
    let webhook_id = client.webhook_id().ok_or(StatusCode::NOT_IMPLEMENTED)?;
    let verified = client
        .verify_webhook_signature(webhook_id, transmission_id, cert_url, auth_algo, transmission_sig, transmission_time, &event)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    if !verified {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut hasher = Sha256::new();
    hasher.update(&body);
    let payload_hash = format!("{:x}", hasher.finalize());

    let persist_result = persist_payment_webhook_event(
        &state.pool,
        "paypal",
        &provider_event_id,
        &event_type,
        resource_id.as_deref(),
        &String::from_utf8_lossy(&body),
        &payload_hash,
    )
    .await;
    let (internal_id, inserted) = match persist_result {
        Ok(v) => v,
        Err(e) => return Err(e),
    };

    if inserted {
        outbox::insert(
            &state.pool,
            &outbox::NewOutboxEvent {
                id: None,
                event_type: "payment.webhook.received".to_string(),
                aggregate_type: "payment_webhook_event".to_string(),
                aggregate_id: internal_id.clone(),
                payload: serde_json::json!({"webhook_event_id": internal_id}).to_string(),
                correlation_id: None,
                available_at: None,
            },
        )
        .await;
    }

    // Best-effort synchronous processing. Outbox remains the durable retry path.
    let proc = process_payment_webhook_event(&state.pool, &client, &internal_id).await;
    Ok(StatusCode::OK)
}

pub async fn persist_payment_webhook_event(
    pool: &sqlx::MySqlPool,
    provider: &str,
    provider_event_id: &str,
    event_type: &str,
    resource_id: Option<&str>,
    payload: &str,
    payload_hash: &str,
) -> Result<(String, bool), StatusCode> {
    let id = Uuid::new_v4().to_string();
    let result = sqlx::query("INSERT INTO payment_webhook_events (id, provider, provider_event_id, event_type, resource_id, payload, payload_hash, processing_status) VALUES (?,?,?,?,?,?,?, 'received')")
        .bind(&id)
        .bind(provider)
        .bind(provider_event_id)
        .bind(event_type)
        .bind(resource_id)
        .bind(payload)
        .bind(payload_hash)
        .execute(pool)
        .await;
    match result {
        Ok(_) => Ok((id, true)),
        Err(sqlx::Error::Database(db)) => {
            if db.code().as_deref() == Some("1062")
                || db.code().as_deref() == Some("23000")
                || db.message().contains("Duplicate entry")
            {
                let row = sqlx::query("SELECT id FROM payment_webhook_events WHERE provider=? AND provider_event_id=?")
                    .bind(provider).bind(provider_event_id)
                    .fetch_one(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                Ok((row.get("id"), false))
            } else {
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn process_payment_webhook_event(
    pool: &sqlx::MySqlPool,
    client: &PayPalClient,
    internal_event_id: &str,
) -> Result<(), StatusCode> {
    let row = sqlx::query("SELECT id, provider, provider_event_id, event_type, resource_id, processing_status FROM payment_webhook_events WHERE id=?")
        .bind(internal_event_id)
        .fetch_optional(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = match row {
        Some(r) => r,
        None => return Err(StatusCode::NOT_FOUND),
    };
    let status: String = row.get("processing_status");
    if matches!(status.as_str(), "processed" | "ignored") {
        return Ok(());
    }
    let event_type: String = row.get("event_type");
    let resource_id: Option<String> = row.try_get("resource_id").ok().flatten();
    let event_amount: Option<String> = None;
    let event_currency: Option<String> = None;

    // Update the local order only from a verified PayPal resource, never from the
    // webhook payload alone.
    if let Some(resource_id) = resource_id {
        if let Ok(order) = client.get_order(&resource_id).await {
            let provider_status = order.get("status").and_then(|v| v.as_str()).unwrap_or("CREATED");
            let mapped = map_paypal_order_status(provider_status);
            let remote_amount = order
                .get("purchase_units")
                .and_then(|v| v.get(0))
                .and_then(|v| v.get("amount"))
                .and_then(|v| v.get("value"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let remote_currency = order
                .get("purchase_units")
                .and_then(|v| v.get(0))
                .and_then(|v| v.get("amount"))
                .and_then(|v| v.get("currency_code"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let webhook_capture_id = order.get("purchase_units").and_then(|v| v.get(0)).and_then(|v| v.get("payments")).and_then(|v| v.get("captures")).and_then(|v| v.get(0)).and_then(|v| v.get("id")).and_then(|v| v.as_str()).map(|s| s.to_string());
            let webhook_capture_amount = order.get("purchase_units").and_then(|v| v.get(0)).and_then(|v| v.get("payments")).and_then(|v| v.get("captures")).and_then(|v| v.get(0)).and_then(|v| v.get("amount")).and_then(|v| v.get("value")).and_then(|v| v.as_str()).map(|s| s.to_string());
            let webhook_capture_currency = order.get("purchase_units").and_then(|v| v.get(0)).and_then(|v| v.get("payments")).and_then(|v| v.get("captures")).and_then(|v| v.get(0)).and_then(|v| v.get("amount")).and_then(|v| v.get("currency_code")).and_then(|v| v.as_str()).map(|s| s.to_string());
            let capture_id = order.get("purchase_units").and_then(|v| v.get(0)).and_then(|v| v.get("payments")).and_then(|v| v.get("captures")).and_then(|v| v.get(0)).and_then(|v| v.get("id")).and_then(|v| v.as_str()).map(|s| s.to_string());
            let local_row = sqlx::query("SELECT id, user_id, CAST(amount AS CHAR) AS amount, currency, status FROM payment_orders WHERE provider_order_id=?")
                .bind(&resource_id)
                .fetch_optional(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            if let Some(local) = local_row {
                let local_id: String = local.get("id");
                let local_amount: String = local.get("amount");
                let local_currency: String = local.get("currency");
                let local_status: String = local.get("status");
                let amount_matches = webhook_capture_amount.as_deref().unwrap_or(remote_amount) == local_amount;
                let currency_matches = webhook_capture_currency.as_deref().unwrap_or(remote_currency) == local_currency;
                if remote_amount == local_amount && remote_currency == local_currency && amount_matches && currency_matches {
                    if event_type == "PAYMENT.CAPTURE.COMPLETED" {
                        if let Some(capture_id) = webhook_capture_id.as_deref() {
                            let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM payment_transactions WHERE provider_transaction_id=?")
                                .bind(capture_id).fetch_optional(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                            if exists.is_none() {
                                sqlx::query("INSERT INTO payment_transactions (id, payment_order_id, user_id, provider, provider_transaction_id, amount, currency, status, captured_at) VALUES (?,?,?,?,?,?,?,'completed',NOW())")
                                    .bind(Uuid::new_v4().to_string())
                                    .bind(&local_id)
                                    .bind(local.get::<String,_>("user_id"))
                                    .bind("paypal")
                                    .bind(capture_id)
                                    .bind(&local_amount)
                                    .bind(&local_currency)
                                    .execute(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                            }
                        }

                    }
                    let should_update = matches!(event_type.as_str(), "CHECKOUT.ORDER.APPROVED" | "CHECKOUT.ORDER.VOIDED" | "PAYMENT.CAPTURE.COMPLETED" | "PAYMENT.CAPTURE.DENIED")
                        && mapped != local_status;
                    if should_update {
                        sqlx::query("UPDATE payment_orders SET status=?, updated_at=NOW() WHERE provider_order_id=?")
                            .bind(mapped).bind(&resource_id)
                            .execute(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    }
                    if event_type == "PAYMENT.CAPTURE.COMPLETED" && mapped == "captured" {
                        let uid: String = local.get("user_id");
                        activate_subscription_from_payment(pool, &local_id, &uid, "paypal_webhook", "webhook_capture").await?;
                    }
                }
            }
        }
    }

    let supported = matches!(event_type.as_str(), "CHECKOUT.ORDER.APPROVED" | "CHECKOUT.ORDER.VOIDED" | "PAYMENT.CAPTURE.COMPLETED" | "PAYMENT.CAPTURE.DENIED" | "PAYMENT.CAPTURE.REFUNDED" | "PAYMENT.CAPTURE.REVERSED");

    let final_status = if supported { "processed" } else { "ignored" };
    sqlx::query("UPDATE payment_webhook_events SET processing_status=?, processed_at=NOW(), retry_count=retry_count+1 WHERE id=?")
        .bind(final_status).bind(internal_event_id)
        .execute(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

pub async fn reconcile_payment_orders(
    pool: &sqlx::MySqlPool,
    client: &PayPalClient,
    apply: bool,
) -> Result<serde_json::Value, StatusCode> {
    let rows = sqlx::query("SELECT id, user_id, provider_order_id, amount, currency, status, activated_subscription_id FROM payment_orders WHERE provider_order_id IS NOT NULL")
        .fetch_all(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut report = Vec::new();
    for row in rows {
        let id: String = row.get("id");
        let uid: String = row.get("user_id");
        let provider_order_id: String = row.get("provider_order_id");
        let amount: String = row.get("amount");
        let currency: String = row.get("currency");
        let status: String = row.get("status");
        let activated: Option<String> = row.try_get("activated_subscription_id").ok().flatten();
        let remote = client.get_order(&provider_order_id).await;
        match remote {
            Ok(order) => {
                let remote_status = order.get("status").and_then(|v| v.as_str()).unwrap_or("CREATED");
                let mapped = map_paypal_order_status(remote_status);
                let remote_amount = order.get("purchase_units").and_then(|v| v.get(0)).and_then(|v| v.get("amount")).and_then(|v| v.get("value")).and_then(|v| v.as_str()).unwrap_or("");
                let remote_currency = order.get("purchase_units").and_then(|v| v.get(0)).and_then(|v| v.get("amount")).and_then(|v| v.get("currency_code")).and_then(|v| v.as_str()).unwrap_or("");
                let mismatch = mapped != status || remote_amount != amount || remote_currency != currency;
                if apply && mismatch && remote_amount == amount && remote_currency == currency {
                    sqlx::query("UPDATE payment_orders SET status=?, updated_at=NOW() WHERE id=?")
                        .bind(mapped).bind(&id).execute(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                }
                if apply && (mapped == "captured" || status == "captured") && activated.is_none() {
                    let _ = activate_subscription_from_payment(pool, &id, &uid, "reconciliation", "reconciliation").await;
                }
                report.push(serde_json::json!({
                    "order_id": id,
                    "provider_order_id": provider_order_id,
                    "local_status": status,
                    "provider_status": remote_status,
                    "amount_match": remote_amount == amount,
                    "currency_match": remote_currency == currency,
                    "mismatch": mismatch,
                    "applied": apply && mismatch && remote_amount == amount && remote_currency == currency,
                }));
            }
            Err(_) => {
                report.push(serde_json::json!({
                    "order_id": id,
                    "provider_order_id": provider_order_id,
                    "local_status": status,
                    "provider_status": "unavailable",
                    "mismatch": true,
                    "applied": false,
                }));
            }
        }
    }
    Ok(serde_json::json!({"orders": report}))
}


pub async fn activate_subscription_from_payment(
    pool: &sqlx::MySqlPool,
    order_id: &str,
    uid: &str,
    actor_id: &str,
    source: &str,
) -> Result<String, StatusCode> {
    let order = sqlx::query("SELECT id, user_id, plan_id, plan_version_id, provider_order_id, amount, currency, status, activated_subscription_id FROM payment_orders WHERE id=? AND user_id=?")
        .bind(order_id).bind(uid)
        .fetch_optional(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let order_id_db: String = order.get("id");
    let plan_id: String = order.get("plan_id");
    let plan_version_id: String = order.get("plan_version_id");
    let order_status: String = order.get("status");
    let activated: Option<String> = order.try_get("activated_subscription_id").ok().flatten();

    if let Some(subscription_id) = activated {
        return Ok(subscription_id);
    }

    // A captured payment must exist before activation.
    let transaction = sqlx::query("SELECT id FROM payment_transactions WHERE payment_order_id=? AND status='completed' ORDER BY created_at DESC LIMIT 1")
        .bind(&order_id_db)
        .fetch_optional(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let transaction = transaction.ok_or(StatusCode::CONFLICT)?;
    let transaction_id: String = transaction.get("id");

    if order_status != "captured" {
        return Err(StatusCode::CONFLICT);
    }

    let plan_period: String = sqlx::query_scalar("SELECT period FROM plans WHERE id=?")
        .bind(&plan_id).fetch_one(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut tx = pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // Lock the order and all user subscriptions to serialize activations.
    let locked = sqlx::query("SELECT activated_subscription_id FROM payment_orders WHERE id=? AND user_id=? FOR UPDATE")
        .bind(&order_id_db).bind(uid)
        .fetch_optional(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let activated: Option<String> = locked.try_get("activated_subscription_id").ok().flatten();
    if let Some(subscription_id) = activated {
        tx.commit().await.ok();
        return Ok(subscription_id);
    }

    let _ = sqlx::query("SELECT id FROM subscriptions WHERE user_id=? FOR UPDATE")
        .bind(uid).fetch_all(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("UPDATE subscriptions SET is_current=NULL WHERE user_id=? AND is_current=1")
        .bind(uid).execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let subscription_id = Uuid::new_v4().to_string();
    let interval_sql = match plan_period.as_str() {
        "anual" | "annual" | "yearly" => "INTERVAL 1 YEAR",
        _ => "INTERVAL 1 MONTH",
    };
    let insert_sql = format!(
        "INSERT INTO subscriptions (id,user_id,plan_id,plan_version_id,status,start_date,end_date,current_period_start,current_period_end,is_current,auto_renew,activation_source) VALUES (?,?,?,?,'active',NOW(),DATE_ADD(NOW(),{}),NOW(),DATE_ADD(NOW(),{}),1,1,?)",
        interval_sql, interval_sql
    );
    sqlx::query(&insert_sql)
        .bind(&subscription_id)
        .bind(uid)
        .bind(&plan_id)
        .bind(&plan_version_id)
        .bind(source)
        .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    sqlx::query("UPDATE payment_orders SET activated_subscription_id=?, activated_at=NOW(), updated_at=NOW() WHERE id=? AND user_id=?")
        .bind(&subscription_id).bind(&order_id_db).bind(uid)
        .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("UPDATE payment_transactions SET subscription_id=?, updated_at=NOW() WHERE id=?")
        .bind(&subscription_id).bind(&transaction_id)
        .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("INSERT INTO subscription_events (id,subscription_id,event_type,previous_status,new_status,actor_type,actor_id,reason) VALUES (?,?, 'activated', NULL, 'active', 'system', ?, ?)")
        .bind(Uuid::new_v4().to_string())
        .bind(&subscription_id)
        .bind(actor_id)
        .bind(source)
        .execute(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    outbox::insert(&mut *tx, &outbox::NewOutboxEvent {
        id: None,
        event_type: "subscription.activated".to_string(),
        aggregate_type: "subscription".to_string(),
        aggregate_id: subscription_id.clone(),
        payload: serde_json::json!({"subscription_id": subscription_id, "payment_order_id": order_id_db, "user_id": uid, "plan_id": plan_id, "plan_version_id": plan_version_id}).to_string(),
        correlation_id: None,
        available_at: None,
    }).await;
    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(subscription_id)
}


pub async fn activate_paypal_subscription(
    ConnectInfo(connect): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(order_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let ip = client_ip(&connect);
    if !state.rate_limiter.check(&format!("payments:activate:ip:{ip}"), 20, std::time::Duration::from_secs(900)) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    let t = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string()).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    if !state.rate_limiter.check(&format!("payments:activate:user:{uid}"), 10, std::time::Duration::from_secs(900)) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    let subscription_id = activate_subscription_from_payment(&state.pool, &order_id, &uid, &uid, "client_request").await?;
    Ok(Json(serde_json::json!({"order_id": order_id, "subscription_id": subscription_id, "status": "active"})))
}


pub async fn reconcile_payment_activations(pool: &sqlx::MySqlPool, apply: bool) -> Result<serde_json::Value, StatusCode> {
    let rows = sqlx::query("SELECT id, user_id, status, activated_subscription_id FROM payment_orders WHERE status='captured'")
        .fetch_all(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut report = Vec::new();
    for row in rows {
        let id: String = row.get("id");
        let uid: String = row.get("user_id");
        let activated: Option<String> = row.try_get("activated_subscription_id").ok().flatten();
        if activated.is_none() && apply {
            let result = activate_subscription_from_payment(pool, &id, &uid, "reconciliation", "reconciliation").await;
            report.push(serde_json::json!({"order_id": id, "applied": true, "result": if result.is_ok() { "activated" } else { "failed" }}));
        } else {
            report.push(serde_json::json!({"order_id": id, "applied": false, "result": if activated.is_some() { "already_activated" } else { "pending" }}));
        }
    }
    Ok(serde_json::json!({"orders": report}))
}

pub async fn reap_pending_payment_activations(pool: &sqlx::MySqlPool, apply: bool) -> Result<serde_json::Value, StatusCode> {
    reconcile_payment_activations(pool, apply).await
}

fn payment_provider_configured() -> bool {
    // PayPal is not integrated yet. Keeping this as an explicit false makes the
    // disabled state testable and prevents accidental re-enabling.
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payment_provider_is_not_configured() {
        assert!(!payment_provider_configured());
    }
}
