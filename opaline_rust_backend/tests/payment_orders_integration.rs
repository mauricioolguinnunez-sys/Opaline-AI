mod common;

use common::{setup_state, test_connect_info, unique_email};
use opaline_backend::routes::auth::{self, RegisterReq};
use opaline_backend::routes::payments::{capture_paypal_order_service, create_paypal_order, fetch_paypal_order};
use opaline_backend::paypal::{PayPalClient, PayPalConfig, PayPalEnvironment};
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn paypal_order_create_and_query() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("pay");
    let password = "Abcdef1!xy";
    let registered = auth::register(test_connect_info(), axum::extract::State(state.clone()), axum::Json(RegisterReq {
        email, password: password.to_string(), name: None,
    })).await.expect("register").0;
    let uid = registered.user["id"].as_str().unwrap().to_string();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let create_count = Arc::new(AtomicUsize::new(0));
    let get_count = Arc::new(AtomicUsize::new(0));
    let create_count_clone = create_count.clone();
    let get_count_clone = get_count.clone();
    let app = axum::Router::new()
        .route("/v1/oauth2/token", axum::routing::post(|| async {
            axum::Json(serde_json::json!({"access_token":"test-token","token_type":"Bearer","expires_in":3600}))
        }))
        .route("/v2/checkout/orders", axum::routing::post(move || {
            let counter = create_count_clone.clone();
            async move {
                let n = counter.fetch_add(1, Ordering::SeqCst) + 1;
                eprintln!("MOCK CREATE HIT n={}", n);
                let id = format!("PAYPAL-ORDER-{}-{}", uuid::Uuid::new_v4(), n);
                axum::Json(json!({
                    "id": id,
                    "status": "CREATED",
                    "links": [{"rel":"approve","href":format!("https://www.sandbox.paypal.com/checkoutnow?token={}", id)}]
                }))
            }
        }))
        .route("/v2/checkout/orders/:id", axum::routing::get(move |axum::extract::Path(id): axum::extract::Path<String>| {
            let counter = get_count_clone.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                axum::Json(json!({
                    "id": id,
                    "status": "APPROVED",
                    "purchase_units": [{"amount": {"value": "49.00", "currency_code": "MXN"}}]
                }))
            }
        }));
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap(); });

    let config = PayPalConfig {
        environment: PayPalEnvironment::Sandbox,
        client_id: "client".to_string(),
        client_secret: "secret".to_string(),
        api_base_url: format!("http://{}", addr),
        connect_timeout: std::time::Duration::from_secs(5),
        request_timeout: std::time::Duration::from_secs(5),
        webhook_id: None,
    };
    let client = PayPalClient::new(config).unwrap();

    let key = "order-key-1".to_string();
    let first = create_paypal_order(&state, &uid, "plan-starter", &key, &client).await.expect("create");
    let provider_order_id = first["provider_order_id"].as_str().unwrap().to_string();
    assert_eq!(first["status"], "created");
    assert!(first["approval_url"].as_str().unwrap().contains(&provider_order_id));

    // Idempotent retry.
    let second = create_paypal_order(&state, &uid, "plan-starter", &key, &client).await.expect("retry");
    assert_eq!(second["provider_order_id"], provider_order_id);
    assert_eq!(create_count.load(Ordering::SeqCst), 1, "PayPal create must not be called twice");

    let order_id = first["order_id"].as_str().unwrap().to_string();
    let queried = fetch_paypal_order(&state, &uid, &order_id, &client).await.expect("query");
    assert_eq!(queried["provider_status"], "APPROVED");
    assert_eq!(queried["status"], "approved");
    assert_eq!(get_count.load(Ordering::SeqCst), 2);

    // Another user cannot query the order.
    let other_email = unique_email("pay-other");
    let other = auth::register(test_connect_info(), axum::extract::State(state.clone()), axum::Json(RegisterReq {
        email: other_email, password: "Abcdef1!xy".to_string(), name: None,
    })).await.expect("register other").0;
    let other_uid = other.user["id"].as_str().unwrap();
    let forbidden = fetch_paypal_order(&state, other_uid, &order_id, &client).await;
    assert!(forbidden.is_err());

    server.abort();
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn paypal_order_can_be_captured_once() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("capture");
    let password = "Abcdef1!xy";
    let registered = auth::register(test_connect_info(), axum::extract::State(state.clone()), axum::Json(RegisterReq {
        email, password: password.to_string(), name: None,
    })).await.expect("register").0;
    let uid = registered.user["id"].as_str().unwrap().to_string();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = axum::Router::new()
        .route("/v1/oauth2/token", axum::routing::post(|| async {
            axum::Json(json!({"access_token":"test-token","token_type":"Bearer","expires_in":3600}))
        }))
        .route("/v2/checkout/orders/:id", axum::routing::get(|axum::extract::Path(id): axum::extract::Path<String>| async move {
            axum::Json(json!({
                "id": id,
                "status": "APPROVED",
                "purchase_units": [{"amount": {"value": "49.00", "currency_code": "MXN"}}]
            }))
        }))
        .route("/v2/checkout/orders/:id/capture", axum::routing::post(|axum::extract::Path(id): axum::extract::Path<String>| async move {
            axum::Json(json!({
                "id": id,
                "status": "COMPLETED",
                "purchase_units": [{"payments": {"captures": [{"id": format!("CAP-{}", id), "status": "COMPLETED", "amount": {"value": "49.00", "currency_code": "MXN"}}]}}]
            }))
        }));
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap(); });

    std::env::set_var("PAYPAL_ENVIRONMENT", "sandbox");
    std::env::set_var("PAYPAL_CLIENT_ID", "client");
    std::env::set_var("PAYPAL_CLIENT_SECRET", "secret");
    std::env::set_var("PAYPAL_API_BASE_URL", format!("http://{}", addr));

    let config = PayPalConfig {
        environment: PayPalEnvironment::Sandbox,
        client_id: "client".to_string(),
        client_secret: "secret".to_string(),
        api_base_url: format!("http://{}", addr),
        connect_timeout: std::time::Duration::from_secs(5),
        request_timeout: std::time::Duration::from_secs(5),
        webhook_id: None,
    };
    let client = PayPalClient::new(config).unwrap();

    // Seed an approved local order.
    let order_id = Uuid::new_v4().to_string();
    let provider_order_id = format!("PAYPAL-{}", Uuid::new_v4());
    sqlx::query("INSERT INTO payment_orders (id,user_id,plan_id,plan_version_id,provider,provider_order_id,idempotency_key,amount,currency,status) VALUES (?,?,?,'plan-starter-v1','paypal',?,'capture-key',49.00,'MXN','approved')")
        .bind(&order_id).bind(&uid).bind("plan-starter").bind(&provider_order_id)
        .execute(&state.pool).await.expect("order");

    let first = capture_paypal_order_service(&state, &uid, &order_id, &client).await.expect("capture");
    assert_eq!(first["status"], "captured");
    let tx_id = first["transaction_id"].as_str().unwrap().to_string();

    // Idempotent retry.
    let second = capture_paypal_order_service(&state, &uid, &order_id, &client).await.expect("retry");
    assert_eq!(second["status"], "captured");
    assert_eq!(second["transaction_id"], tx_id);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM payment_transactions WHERE payment_order_id=? AND status='completed'")
        .bind(&order_id).fetch_one(&state.pool).await.unwrap();
    assert_eq!(count, 1);

    let sub_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM subscriptions WHERE user_id=? AND status='active' AND is_current=1")
        .bind(&uid).fetch_one(&state.pool).await.unwrap();
    assert_eq!(sub_count, 1);
    let sub_version: String = sqlx::query_scalar("SELECT plan_version_id FROM subscriptions WHERE user_id=? AND is_current=1")
        .bind(&uid).fetch_one(&state.pool).await.unwrap();
    assert_eq!(sub_version, "plan-starter-v1");

    std::env::remove_var("PAYPAL_ENVIRONMENT");
    std::env::remove_var("PAYPAL_CLIENT_ID");
    std::env::remove_var("PAYPAL_CLIENT_SECRET");
    std::env::remove_var("PAYPAL_API_BASE_URL");
    server.abort();
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn payment_activation_is_idempotent_under_concurrency() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("activate-conc");
    let password = "Abcdef1!xy";
    let registered = auth::register(test_connect_info(), axum::extract::State(state.clone()), axum::Json(RegisterReq {
        email, password: password.to_string(), name: None,
    })).await.expect("register").0;
    let uid = registered.user["id"].as_str().unwrap().to_string();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = axum::Router::new()
        .route("/v1/oauth2/token", axum::routing::post(|| async {
            axum::Json(json!({"access_token":"test-token","token_type":"Bearer","expires_in":3600}))
        }))
        .route("/v2/checkout/orders/:id", axum::routing::get(|axum::extract::Path(id): axum::extract::Path<String>| async move {
            axum::Json(json!({
                "id": id,
                "status": "APPROVED",
                "purchase_units": [{"amount": {"value": "49.00", "currency_code": "MXN"}}]
            }))
        }))
        .route("/v2/checkout/orders/:id/capture", axum::routing::post(|axum::extract::Path(id): axum::extract::Path<String>| async move {
            axum::Json(json!({
                "id": id,
                "status": "COMPLETED",
                "purchase_units": [{"payments": {"captures": [{"id": format!("CAP-{}", id), "status": "COMPLETED", "amount": {"value": "49.00", "currency_code": "MXN"}}]}}]
            }))
        }));
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap(); });

    std::env::set_var("PAYPAL_ENVIRONMENT", "sandbox");
    std::env::set_var("PAYPAL_CLIENT_ID", "client");
    std::env::set_var("PAYPAL_CLIENT_SECRET", "secret");
    std::env::set_var("PAYPAL_API_BASE_URL", format!("http://{}", addr));

    let config = PayPalConfig {
        environment: PayPalEnvironment::Sandbox,
        client_id: "client".to_string(),
        client_secret: "secret".to_string(),
        api_base_url: format!("http://{}", addr),
        connect_timeout: std::time::Duration::from_secs(5),
        request_timeout: std::time::Duration::from_secs(5),
        webhook_id: None,
    };
    let client = PayPalClient::new(config).unwrap();

    let order_id = Uuid::new_v4().to_string();
    let provider_order_id = format!("PAYPAL-{}", Uuid::new_v4());
    sqlx::query("INSERT INTO payment_orders (id,user_id,plan_id,plan_version_id,provider,provider_order_id,idempotency_key,amount,currency,status) VALUES (?,?,?,'plan-starter-v1','paypal',?,'conc-key',49.00,'MXN','approved')")
        .bind(&order_id).bind(&uid).bind("plan-starter").bind(&provider_order_id)
        .execute(&state.pool).await.expect("order");

    let state_a = state.clone();
    let state_b = state.clone();
    let uid_a = uid.clone();
    let uid_b = uid.clone();
    let order_a = order_id.clone();
    let order_b = order_id.clone();
    let client_a = client.clone();
    let client_b = client.clone();

    let task_a = tokio::spawn(async move {
        capture_paypal_order_service(&state_a, &uid_a, &order_a, &client_a).await
    });
    let task_b = tokio::spawn(async move {
        capture_paypal_order_service(&state_b, &uid_b, &order_b, &client_b).await
    });

    let res_a = task_a.await.unwrap();
    let res_b = task_b.await.unwrap();
    assert!(res_a.is_ok() || res_b.is_ok());

    let sub_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM subscriptions WHERE user_id=? AND status='active' AND is_current=1")
        .bind(&uid).fetch_one(&state.pool).await.unwrap();
    assert_eq!(sub_count, 1);

    let tx_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM payment_transactions WHERE payment_order_id=? AND status='completed'")
        .bind(&order_id).fetch_one(&state.pool).await.unwrap();
    assert!(tx_count >= 1);

    std::env::remove_var("PAYPAL_ENVIRONMENT");
    std::env::remove_var("PAYPAL_CLIENT_ID");
    std::env::remove_var("PAYPAL_CLIENT_SECRET");
    std::env::remove_var("PAYPAL_API_BASE_URL");
    server.abort();
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn captured_payment_can_be_reconciled_to_subscription() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("reconcile");
    let password = "Abcdef1!xy";
    let registered = auth::register(test_connect_info(), axum::extract::State(state.clone()), axum::Json(RegisterReq {
        email, password: password.to_string(), name: None,
    })).await.expect("register").0;
    let uid = registered.user["id"].as_str().unwrap().to_string();

    let order_id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO payment_orders (id,user_id,plan_id,plan_version_id,provider,provider_order_id,idempotency_key,amount,currency,status) VALUES (?,?,?,'plan-starter-v1','paypal',?,'recon-key',49.00,'MXN','captured')")
        .bind(&order_id).bind(&uid).bind("plan-starter").bind(format!("PAYPAL-{}", Uuid::new_v4()))
        .execute(&state.pool).await.expect("order");
    let tx_id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO payment_transactions (id,payment_order_id,user_id,provider,provider_transaction_id,amount,currency,status) VALUES (?,?,?,'paypal',?,49.00,'MXN','completed')")
        .bind(&tx_id).bind(&order_id).bind(&uid).bind(format!("CAP-{}", Uuid::new_v4()))
        .execute(&state.pool).await.expect("transaction");

    let report = opaline_backend::routes::payments::reconcile_payment_activations(&state.pool, true).await.expect("reconcile");
    assert!(report.get("orders").is_some());

    let sub_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM subscriptions WHERE user_id=? AND status='active' AND is_current=1")
        .bind(&uid).fetch_one(&state.pool).await.unwrap();
    assert_eq!(sub_count, 1);
}
