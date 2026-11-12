mod common;

use common::{setup_state, test_connect_info, unique_email};
use opaline_backend::routes::auth::{self, RegisterReq};
use axum::http::{HeaderMap, HeaderValue};
use opaline_backend::routes::payments::process_payment_webhook_event;
use opaline_backend::paypal::{PayPalClient, PayPalConfig, PayPalEnvironment};
use serde_json::json;
use sqlx::Row;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn paypal_webhook_verifies_persists_and_deduplicates() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;

    std::env::set_var("PAYPAL_ENVIRONMENT", "sandbox");
    std::env::set_var("PAYPAL_CLIENT_ID", "client");
    std::env::set_var("PAYPAL_CLIENT_SECRET", "secret");
    std::env::set_var("PAYPAL_WEBHOOK_ID", "WH-123");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    std::env::set_var("PAYPAL_API_BASE_URL", format!("http://{}", addr));

    let verify_count = Arc::new(AtomicUsize::new(0));
    let verify_count_clone = verify_count.clone();
    let app = axum::Router::new()
        .route("/v1/oauth2/token", axum::routing::post(|| async {
            axum::Json(json!({"access_token":"test-token","token_type":"Bearer","expires_in":3600}))
        }))
        .route("/v1/notifications/verify-webhook-signature", axum::routing::post(move |req: axum::http::Request<axum::body::Body>| {
            let counter = verify_count_clone.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                let body_bytes = axum::body::to_bytes(req.into_body(), 1_048_576).await.unwrap_or_default();
                let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap_or_default();
                let sig = body.get("transmission_sig").and_then(|v| v.as_str()).unwrap_or("");
                if sig == "bad" {
                    axum::Json(json!({"verification_status":"FAILURE"}))
                } else {
                    axum::Json(json!({"verification_status":"SUCCESS"}))
                }
            }
        }));
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap(); });

    let event_id = format!("EVT-{}", Uuid::new_v4());
    let body = json!({
        "id": event_id,
        "event_type": "CHECKOUT.ORDER.APPROVED",
        "resource": {"id": "NO-SUCH-ORDER"}
    });
    let raw = serde_json::to_vec(&body).unwrap();

    let mut headers = HeaderMap::new();
    headers.insert("PAYPAL-TRANSMISSION-ID", HeaderValue::from_str("tx-1").unwrap());
    headers.insert("PAYPAL-CERT-URL", HeaderValue::from_str("https://example.com/cert").unwrap());
    headers.insert("PAYPAL-AUTH-ALGO", HeaderValue::from_str("SHA256withRSA").unwrap());
    headers.insert("PAYPAL-TRANSMISSION-SIG", HeaderValue::from_str("sig").unwrap());
    headers.insert("PAYPAL-TRANSMISSION-TIME", HeaderValue::from_str("2026-01-01T00:00:00Z").unwrap());

    use axum::body::Bytes;
    let first = opaline_backend::routes::payments::paypal_webhook(
        common::test_connect_info(),
        axum::extract::State(state.clone()),
        headers.clone(),
        Bytes::from(raw.clone()),
    ).await;
    assert!(first.is_ok());

    let second = opaline_backend::routes::payments::paypal_webhook(
        common::test_connect_info(),
        axum::extract::State(state.clone()),
        headers.clone(),
        Bytes::from(raw.clone()),
    ).await;
    assert!(second.is_ok());

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM payment_webhook_events WHERE provider_event_id=?")
        .bind(&event_id).fetch_one(&state.pool).await.unwrap();
    assert_eq!(count, 1);

    let mut bad_headers = headers.clone();
    bad_headers.insert("PAYPAL-TRANSMISSION-SIG", HeaderValue::from_str("bad").unwrap());
    let bad = opaline_backend::routes::payments::paypal_webhook(
        common::test_connect_info(),
        axum::extract::State(state.clone()),
        bad_headers,
        Bytes::from(raw.clone()),
    ).await;
    assert!(bad.is_err());

    std::env::remove_var("PAYPAL_ENVIRONMENT");
    std::env::remove_var("PAYPAL_CLIENT_ID");
    std::env::remove_var("PAYPAL_CLIENT_SECRET");
    std::env::remove_var("PAYPAL_API_BASE_URL");
    server.abort();
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn paypal_webhook_capture_event_activates_subscription() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("webhook-activate");
    let password = "Abcdef1!xy";
    let registered = auth::register(test_connect_info(), axum::extract::State(state.clone()), axum::Json(RegisterReq {
        email, password: password.to_string(), name: None,
    })).await.expect("register").0;
    let uid = registered.user["id"].as_str().unwrap().to_string();

    let order_id = Uuid::new_v4().to_string();
    let provider_order_id = format!("PAYPAL-{}", Uuid::new_v4());
    let capture_id = format!("CAP-{}", Uuid::new_v4());
    sqlx::query("INSERT INTO payment_orders (id,user_id,plan_id,plan_version_id,provider,provider_order_id,idempotency_key,amount,currency,status) VALUES (?,?,?,'plan-starter-v1','paypal',?,'wh-key',49.00,'MXN','approved')")
        .bind(&order_id).bind(&uid).bind("plan-starter").bind(&provider_order_id)
        .execute(&state.pool).await.expect("order");

    std::env::set_var("PAYPAL_ENVIRONMENT", "sandbox");
    std::env::set_var("PAYPAL_CLIENT_ID", "client");
    std::env::set_var("PAYPAL_CLIENT_SECRET", "secret");
    std::env::set_var("PAYPAL_WEBHOOK_ID", "WH-123");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    std::env::set_var("PAYPAL_API_BASE_URL", format!("http://{}", addr));

    let app = axum::Router::new()
        .route("/v1/oauth2/token", axum::routing::post(|| async {
            axum::Json(json!({"access_token":"test-token","token_type":"Bearer","expires_in":3600}))
        }))
        .route("/v1/notifications/verify-webhook-signature", axum::routing::post(|| async {
            axum::Json(json!({"verification_status":"SUCCESS"}))
        }))
        .route("/v2/checkout/orders/:id", axum::routing::get(move |axum::extract::Path(id): axum::extract::Path<String>| async move {
            axum::Json(json!({
                "id": id,
                "status": "COMPLETED",
                "purchase_units": [{"amount": {"value": "49.00", "currency_code": "MXN"}, "payments": {"captures": [{"id": format!("CAP-{}", id), "status": "COMPLETED", "amount": {"value": "49.00", "currency_code": "MXN"}}]}}]
            }))
        }));
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap(); });

    let event_id = format!("EVT-{}", Uuid::new_v4());
    let body = json!({
        "id": event_id,
        "event_type": "PAYMENT.CAPTURE.COMPLETED",
        "resource": {
            "id": capture_id,
            "amount": {"value": "49.00", "currency_code": "MXN"},
            "supplementary_data": {"related_ids": {"order_id": provider_order_id}}
        }
    });
    let raw = serde_json::to_vec(&body).unwrap();

    let mut headers = HeaderMap::new();
    headers.insert("PAYPAL-TRANSMISSION-ID", HeaderValue::from_str("tx-2").unwrap());
    headers.insert("PAYPAL-CERT-URL", HeaderValue::from_str("https://example.com/cert").unwrap());
    headers.insert("PAYPAL-AUTH-ALGO", HeaderValue::from_str("SHA256withRSA").unwrap());
    headers.insert("PAYPAL-TRANSMISSION-SIG", HeaderValue::from_str("sig").unwrap());
    headers.insert("PAYPAL-TRANSMISSION-TIME", HeaderValue::from_str("2026-01-01T00:00:00Z").unwrap());

    use axum::body::Bytes;
    let result = opaline_backend::routes::payments::paypal_webhook(
        common::test_connect_info(),
        axum::extract::State(state.clone()),
        headers,
        Bytes::from(raw),
    ).await;
    assert!(result.is_ok());

    let sub_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM subscriptions WHERE user_id=? AND status='active' AND is_current=1")
        .bind(&uid).fetch_one(&state.pool).await.unwrap();
    assert_eq!(sub_count, 1);

    std::env::remove_var("PAYPAL_ENVIRONMENT");
    std::env::remove_var("PAYPAL_CLIENT_ID");
    std::env::remove_var("PAYPAL_CLIENT_SECRET");
    std::env::remove_var("PAYPAL_API_BASE_URL");
    server.abort();
}
