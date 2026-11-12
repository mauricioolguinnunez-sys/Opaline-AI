mod common;

use common::{setup_state, test_connect_info, unique_email};
use opaline_backend::routes::auth::{self, RegisterReq};
use sqlx::Row;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn financial_model_rejects_duplicates() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("finance");
    let password = "Abcdef1!xy";
    let registered = auth::register(test_connect_info(), axum::extract::State(state.clone()), axum::Json(RegisterReq {
        email, password: password.to_string(), name: None,
    })).await.expect("register").0;
    let uid = registered.user["id"].as_str().unwrap().to_string();

    let order_id = Uuid::new_v4().to_string();
    let provider_order_id = format!("PAY-{}", Uuid::new_v4());
    let idempotency_key = "finance-key-1".to_string();
    sqlx::query("INSERT INTO payment_orders (id,user_id,plan_id,plan_version_id,provider,provider_order_id,idempotency_key,amount,currency,status) VALUES (?,?,?,?, 'paypal',?,?,49.00,'MXN','created')")
        .bind(&order_id).bind(&uid).bind("plan-starter").bind("plan-starter-v1").bind(&provider_order_id).bind(&idempotency_key)
        .execute(&state.pool).await.expect("order");

    // Duplicate provider order id.
    let dup_provider = sqlx::query("INSERT INTO payment_orders (id,user_id,plan_id,plan_version_id,provider,provider_order_id,idempotency_key,amount,currency,status) VALUES (?,?,?,?, 'paypal',?,?,49.00,'MXN','created')")
        .bind(Uuid::new_v4().to_string()).bind(&uid).bind("plan-starter").bind("plan-starter-v1").bind(&provider_order_id).bind("finance-key-2")
        .execute(&state.pool).await;
    assert!(dup_provider.is_err());

    // Duplicate idempotency key for same user.
    let dup_key = sqlx::query("INSERT INTO payment_orders (id,user_id,plan_id,plan_version_id,provider,provider_order_id,idempotency_key,amount,currency,status) VALUES (?,?,?,?, 'paypal',?,?,49.00,'MXN','created')")
        .bind(Uuid::new_v4().to_string()).bind(&uid).bind("plan-starter").bind("plan-starter-v1").bind(format!("PAY-{}", Uuid::new_v4())).bind(&idempotency_key)
        .execute(&state.pool).await;
    assert!(dup_key.is_err());

    let transaction_id = Uuid::new_v4().to_string();
    let capture_id = format!("CAP-{}", Uuid::new_v4());
    sqlx::query("INSERT INTO payment_transactions (id,payment_order_id,user_id,provider,provider_transaction_id,amount,currency,status) VALUES (?,?,?,?,?,?, 'MXN','completed')")
        .bind(&transaction_id).bind(&order_id).bind(&uid).bind("paypal").bind(&capture_id).bind(49.00_f64)
        .execute(&state.pool).await.expect("transaction");

    // Duplicate capture id.
    let dup_capture = sqlx::query("INSERT INTO payment_transactions (id,payment_order_id,user_id,provider,provider_transaction_id,amount,currency,status) VALUES (?,?,?,?,?,?, 'MXN','completed')")
        .bind(Uuid::new_v4().to_string()).bind(&order_id).bind(&uid).bind("paypal").bind(&capture_id).bind(49.00_f64)
        .execute(&state.pool).await;
    assert!(dup_capture.is_err());

    let event_id = format!("EVT-{}", Uuid::new_v4());
    sqlx::query("INSERT INTO payment_webhook_events (id,provider,provider_event_id,event_type,payload_hash,processing_status) VALUES (?,?,?,?,'hash','received')")
        .bind(Uuid::new_v4().to_string()).bind("paypal").bind(&event_id).bind("PAYMENT.CAPTURE.COMPLETED")
        .execute(&state.pool).await.expect("webhook");
    let dup_event = sqlx::query("INSERT INTO payment_webhook_events (id,provider,provider_event_id,event_type,payload_hash,processing_status) VALUES (?,?,?,?,'hash','received')")
        .bind(Uuid::new_v4().to_string()).bind("paypal").bind(&event_id).bind("PAYMENT.CAPTURE.COMPLETED")
        .execute(&state.pool).await;
    assert!(dup_event.is_err());

    let refund_key = "refund-key-1".to_string();
    sqlx::query("INSERT INTO payment_refunds (id,payment_transaction_id,user_id,idempotency_key,amount,currency,status,reason) VALUES (?,?,?,? ,20.00,'MXN','requested','test')")
        .bind(Uuid::new_v4().to_string()).bind(&transaction_id).bind(&uid).bind(&refund_key)
        .execute(&state.pool).await.expect("refund");
    let dup_refund = sqlx::query("INSERT INTO payment_refunds (id,payment_transaction_id,user_id,idempotency_key,amount,currency,status,reason) VALUES (?,?,?,? ,10.00,'MXN','requested','test')")
        .bind(Uuid::new_v4().to_string()).bind(&transaction_id).bind(&uid).bind(&refund_key)
        .execute(&state.pool).await;
    assert!(dup_refund.is_err());

    // Verify monetary precision is stored as decimal.
    let amount: String = sqlx::query_scalar("SELECT CAST(amount AS CHAR) FROM payment_orders WHERE id=?")
        .bind(&order_id).fetch_one(&state.pool).await.expect("amount");
    assert_eq!(amount, "49.00");
}
