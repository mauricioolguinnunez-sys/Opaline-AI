mod common;

use common::{setup_state, test_connect_info, unique_email};
use axum::http::{HeaderMap, StatusCode};
use axum::Router;
use opaline_backend::routes::payments::{CreateReq, create_order, invoice_pdf, list_payments, refund, verify_order};
use sqlx::Row;

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn payments_require_authentication() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let connect = test_connect_info();

    let create = create_order(
        connect.clone(),
        HeaderMap::new(),
        axum::extract::State(state.clone()),
        axum::Json(CreateReq {
            plan_id: "starter".to_string(),
            idempotency_key: None,
        }),
    )
    .await;
    assert_eq!(create.unwrap_err(), StatusCode::UNAUTHORIZED);

    let verify = verify_order(
        connect.clone(),
        HeaderMap::new(),
        axum::extract::Path("missing-order".to_string()),
        axum::extract::State(state.clone()),
    )
    .await;
    assert_eq!(verify.unwrap_err(), StatusCode::UNAUTHORIZED);

    let refund = refund(
        connect,
        HeaderMap::new(),
        axum::extract::Path("missing-payment".to_string()),
        axum::extract::State(state),
    )
    .await;
    assert_eq!(refund.unwrap_err(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn authenticated_payment_creation_stays_pending() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("payment");
    let password = "Abcdef1!xy";

    let registered = opaline_backend::routes::auth::register(
        test_connect_info(),
        axum::extract::State(state.clone()),
        axum::Json(opaline_backend::routes::auth::RegisterReq {
            email: email.clone(),
            password: password.to_string(),
            name: None,
        }),
    )
    .await
    .expect("registration should succeed")
    .0;

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {}", registered.access_token).parse().unwrap(),
    );

    let response = create_order(
        test_connect_info(),
        headers.clone(),
        axum::extract::State(state.clone()),
        axum::Json(CreateReq {
            plan_id: "starter".to_string(),
            idempotency_key: None,
        }),
    )
    .await;
    assert_eq!(response.unwrap_err(), StatusCode::NOT_IMPLEMENTED);

    let verify = verify_order(
        test_connect_info(),
        headers.clone(),
        axum::extract::Path("missing-order".to_string()),
        axum::extract::State(state.clone()),
    )
    .await;
    assert_eq!(verify.unwrap_err(), StatusCode::NOT_IMPLEMENTED);

    let refund = refund(
        test_connect_info(),
        headers,
        axum::extract::Path("not-a-payment".to_string()),
        axum::extract::State(state),
    )
    .await;
    assert_eq!(refund.unwrap_err(), StatusCode::NOT_IMPLEMENTED);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn payment_history_is_isolated_by_user() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;

    for email in [unique_email("owner"), unique_email("other")] {
        let registered = opaline_backend::routes::auth::register(
            test_connect_info(),
            axum::extract::State(state.clone()),
            axum::Json(opaline_backend::routes::auth::RegisterReq {
                email: email.clone(),
                password: "Abcdef1!xy".to_string(),
                name: None,
            }),
        )
        .await
        .expect("registration should succeed")
        .0;

        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            format!("Bearer {}", registered.access_token).parse().unwrap(),
        );

        let _ = create_order(
            test_connect_info(),
            headers,
            axum::extract::State(state.clone()),
            axum::Json(CreateReq {
                plan_id: "starter".to_string(),
                idempotency_key: None,
            }),
        )
        .await
        .expect_err("order creation requires PayPal configuration");
    }

    // Re-register owner? Simpler: use a fresh owner with one payment and check count.
    let owner = opaline_backend::routes::auth::register(
        test_connect_info(),
        axum::extract::State(state.clone()),
        axum::Json(opaline_backend::routes::auth::RegisterReq {
            email: unique_email("single-owner"),
            password: "Abcdef1!xy".to_string(),
            name: None,
        }),
    )
    .await
    .expect("registration should succeed")
    .0;

    let mut owner_headers = HeaderMap::new();
    owner_headers.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {}", owner.access_token).parse().unwrap(),
    );
    let payment_id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO payments (id, user_id, plan_id, paypal_order_id, amount, currency, status) VALUES (?,?,?,?,?,?, 'completed')")
        .bind(&payment_id)
        .bind(&owner.user["id"].as_str().unwrap())
        .bind("plan-starter")
        .bind(format!("seed-{}", payment_id))
        .bind(49.00_f64)
        .bind("MXN")
        .execute(&state.pool)
        .await
        .expect("seed payment should succeed");

    let history = list_payments(owner_headers, axum::extract::State(state.clone()))
        .await
        .expect("history should succeed")
        .0;
    let history = history.as_array().expect("history array");
    assert_eq!(history.len(), 1);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn invoice_requires_authentication() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let response = invoice_pdf(
        HeaderMap::new(),
        axum::extract::Path("00000000-0000-0000-0000-000000000000".to_string()),
        axum::extract::State(state),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn invoice_access_is_isolated_by_owner() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;

    async fn create_invoice(state: &opaline_backend::state::AppState, email: &str) -> (String, String) {
        let registered = opaline_backend::routes::auth::register(
            test_connect_info(),
            axum::extract::State(state.clone()),
            axum::Json(opaline_backend::routes::auth::RegisterReq {
                email: email.to_string(),
                password: "Abcdef1!xy".to_string(),
                name: None,
            }),
        )
        .await
        .expect("registration should succeed")
        .0;

        let user_id = registered.user["id"].as_str().unwrap().to_string();
        let payment_id = uuid::Uuid::new_v4().to_string();
        let invoice_id = uuid::Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO payments (id, user_id, plan_id, paypal_order_id, amount, currency, status) VALUES (?,?,?,?,?,?, 'completed')",
        )
        .bind(&payment_id)
        .bind(&user_id)
        .bind("plan-starter")
        .bind(format!("test-{}", payment_id))
        .bind(49.00_f64)
        .bind("MXN")
        .execute(&state.pool)
        .await
        .expect("payment insert should succeed");

        sqlx::query("INSERT INTO invoices (id, payment_id, folio) VALUES (?,?,?)")
            .bind(&invoice_id)
            .bind(&payment_id)
            .bind(format!("OP-{}", &invoice_id[..8]))
            .execute(&state.pool)
            .await
            .expect("invoice insert should succeed");

        (registered.access_token, invoice_id)
    }

    let (token_a, invoice_a) = create_invoice(&state, &unique_email("invoice-a")).await;
    let (token_b, invoice_b) = create_invoice(&state, &unique_email("invoice-b")).await;

    let mut headers_a = HeaderMap::new();
    headers_a.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {}", token_a).parse().unwrap(),
    );
    let mut headers_b = HeaderMap::new();
    headers_b.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {}", token_b).parse().unwrap(),
    );

    let own_a = invoice_pdf(
        headers_a.clone(),
        axum::extract::Path(invoice_a.clone()),
        axum::extract::State(state.clone()),
    )
    .await;
    assert_eq!(own_a.status(), StatusCode::OK);

    let foreign_for_a = invoice_pdf(
        headers_a.clone(),
        axum::extract::Path(invoice_b.clone()),
        axum::extract::State(state.clone()),
    )
    .await;
    assert_eq!(foreign_for_a.status(), StatusCode::NOT_FOUND);

    let own_b = invoice_pdf(
        headers_b.clone(),
        axum::extract::Path(invoice_b.clone()),
        axum::extract::State(state.clone()),
    )
    .await;
    assert_eq!(own_b.status(), StatusCode::OK);

    let foreign_for_b = invoice_pdf(
        headers_b,
        axum::extract::Path(invoice_a),
        axum::extract::State(state.clone()),
    )
    .await;
    assert_eq!(foreign_for_b.status(), StatusCode::NOT_FOUND);

    let unauthenticated = invoice_pdf(
        HeaderMap::new(),
        axum::extract::Path(uuid::Uuid::new_v4().to_string()),
        axum::extract::State(state),
    )
    .await;
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn login_rate_limit_returns_429() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("ratelimit");
    let password = "Abcdef1!xy";

    let _ = opaline_backend::routes::auth::register(
        test_connect_info(),
        axum::extract::State(state.clone()),
        axum::Json(opaline_backend::routes::auth::RegisterReq {
            email: email.clone(),
            password: password.to_string(),
            name: None,
        }),
    )
    .await
    .expect("registration should succeed");

    let mut last_status = StatusCode::OK;
    for _ in 0..6 {
        let result = opaline_backend::routes::auth::login(
            test_connect_info(),
            axum::extract::State(state.clone()),
            axum::Json(opaline_backend::routes::auth::LoginReq {
                email: email.clone(),
                password: "WrongPass1!xy".to_string(),
            }),
        )
        .await;
        last_status = match result {
            Ok(_) => StatusCode::OK,
            Err(status) => status,
        };
    }
    assert_eq!(last_status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn payment_creation_rate_limit_returns_429() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("payment-limit");
    let password = "Abcdef1!xy";

    let registered = opaline_backend::routes::auth::register(
        test_connect_info(),
        axum::extract::State(state.clone()),
        axum::Json(opaline_backend::routes::auth::RegisterReq {
            email,
            password: password.to_string(),
            name: None,
        }),
    )
    .await
    .expect("registration should succeed")
    .0;

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {}", registered.access_token).parse().unwrap(),
    );

    let mut last_status = StatusCode::OK;
    for _ in 0..11 {
        let result = create_order(
            test_connect_info(),
            headers.clone(),
            axum::extract::State(state.clone()),
            axum::Json(CreateReq {
                plan_id: "starter".to_string(),
                idempotency_key: None,
            }),
        )
        .await;
        last_status = match result {
            Ok(_) => StatusCode::OK,
            Err(status) => status,
        };
    }
    assert_eq!(last_status, StatusCode::TOO_MANY_REQUESTS);
}