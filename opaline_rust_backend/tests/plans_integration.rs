mod common;

use common::{setup_state, test_connect_info, unique_email};
use axum::http::{HeaderMap, HeaderValue};
use opaline_backend::routes::auth::{self, RegisterReq};
use opaline_backend::routes::plans;
use sqlx::Row;
use uuid::Uuid;

fn auth_headers(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", HeaderValue::from_str(&format!("Bearer {}", token)).unwrap());
    headers
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn plans_are_listed() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let response = plans::list_plans(axum::extract::State(state)).await.expect("plans should load");
    let plans = response.0;
    assert!(!plans.is_empty());
    assert!(plans.iter().any(|p| p.code == "starter"));
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn subscription_none_for_new_user() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("plan-none");
    let password = "Abcdef1!xy";
    let registered = auth::register(test_connect_info(), axum::extract::State(state.clone()), axum::Json(RegisterReq {
        email, password: password.to_string(), name: None,
    })).await.expect("register").0;
    let response = plans::get_subscription(auth_headers(&registered.access_token), axum::extract::State(state)).await.expect("subscription query");
    assert_eq!(response.0["status"], "none");
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn cancel_renewal_keeps_access_until_period_end() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("plan-cancel");
    let password = "Abcdef1!xy";
    let registered = auth::register(test_connect_info(), axum::extract::State(state.clone()), axum::Json(RegisterReq {
        email: email.clone(), password: password.to_string(), name: None,
    })).await.expect("register").0;
    let uid = registered.user["id"].as_str().unwrap().to_string();
    let sid = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO subscriptions (id,user_id,plan_id,plan_version_id,status,start_date,end_date,current_period_start,current_period_end,is_current,auto_renew) VALUES (?,?,?,?, 'active', NOW(), DATE_ADD(NOW(), INTERVAL 1 MONTH), NOW(), DATE_ADD(NOW(), INTERVAL 1 MONTH), 1, 1)")
        .bind(&sid).bind(&uid).bind("plan-starter").bind("plan-starter-v1")
        .execute(&state.pool).await.expect("subscription insert");

    let response = plans::cancel_renewal(auth_headers(&registered.access_token), axum::extract::State(state.clone())).await.expect("cancel");
    assert_eq!(response.0["status"], "cancelled");

    let current = plans::get_subscription(auth_headers(&registered.access_token), axum::extract::State(state.clone())).await.expect("get").0;
    assert_eq!(current["status"], "cancelled");
    assert_eq!(current["cancel_at_period_end"], true);

    let restored = plans::restore_subscription(auth_headers(&registered.access_token), axum::extract::State(state.clone())).await.expect("restore").0;
    assert_eq!(restored["status"], "active");
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn expired_subscription_is_marked_expired() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("plan-expired");
    let password = "Abcdef1!xy";
    let registered = auth::register(test_connect_info(), axum::extract::State(state.clone()), axum::Json(RegisterReq {
        email: email.clone(), password: password.to_string(), name: None,
    })).await.expect("register").0;
    let uid = registered.user["id"].as_str().unwrap().to_string();
    let sid = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO subscriptions (id,user_id,plan_id,plan_version_id,status,start_date,end_date,is_current) VALUES (?,?,?,?,'active', DATE_SUB(NOW(), INTERVAL 2 MONTH), DATE_SUB(NOW(), INTERVAL 1 MONTH), 1)")
        .bind(&sid).bind(&uid).bind("plan-starter").bind("plan-starter-v1")
        .execute(&state.pool).await.expect("subscription insert");

    let current = plans::get_subscription(auth_headers(&registered.access_token), axum::extract::State(state.clone())).await.expect("get").0;
    assert_eq!(current["status"], "expired");

    let db_status: String = sqlx::query_scalar("SELECT status FROM subscriptions WHERE id=?")
        .bind(&sid).fetch_one(&state.pool).await.expect("status");
    assert_eq!(db_status, "expired");
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn concurrent_activation_keeps_one_current_subscription() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("plan-concurrent");
    let password = "Abcdef1!xy";
    let registered = auth::register(test_connect_info(), axum::extract::State(state.clone()), axum::Json(RegisterReq {
        email: email.clone(), password: password.to_string(), name: None,
    })).await.expect("register").0;
    let uid = registered.user["id"].as_str().unwrap().to_string();

    let sid_a = Uuid::new_v4().to_string();
    let sid_b = Uuid::new_v4().to_string();
    for sid in [&sid_a, &sid_b] {
        sqlx::query("INSERT INTO subscriptions (id,user_id,plan_id,plan_version_id,status) VALUES (?,?,?,'plan-starter-v1','pending')")
            .bind(sid).bind(&uid).bind("plan-starter")
            .execute(&state.pool).await.expect("subscription insert");
    }

    let state_a = state.clone();
    let state_b = state.clone();
    let uid_a = uid.clone();
    let uid_b = uid.clone();
    let task_a = tokio::spawn(async move {
        plans::activate_subscription(&state_a, &sid_a, &uid_a, "plan-starter", &uid_a, "test").await
    });
    let task_b = tokio::spawn(async move {
        plans::activate_subscription(&state_b, &sid_b, &uid_b, "plan-starter", &uid_b, "test").await
    });
    let _ = task_a.await.unwrap();
    let _ = task_b.await.unwrap();

    let current_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM subscriptions WHERE user_id=? AND is_current=1")
        .bind(&uid).fetch_one(&state.pool).await.expect("count");
    assert_eq!(current_count, 1, "only one subscription may be current");
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn get_subscription_by_id_is_owner_scoped() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email_a = unique_email("plan-owner-a");
    let email_b = unique_email("plan-owner-b");
    let password = "Abcdef1!xy";
    let user_a = auth::register(test_connect_info(), axum::extract::State(state.clone()), axum::Json(RegisterReq {
        email: email_a, password: password.to_string(), name: None,
    })).await.expect("register a").0;
    let user_b = auth::register(test_connect_info(), axum::extract::State(state.clone()), axum::Json(RegisterReq {
        email: email_b, password: password.to_string(), name: None,
    })).await.expect("register b").0;
    let uid_a = user_a.user["id"].as_str().unwrap().to_string();
    let sid = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO subscriptions (id,user_id,plan_id,plan_version_id,status,is_current) VALUES (?,?,?,'plan-starter-v1','active',1)")
        .bind(&sid).bind(&uid_a).bind("plan-starter")
        .execute(&state.pool).await.expect("subscription insert");

    let result = plans::get_subscription_by_id(axum::extract::Path(sid.clone()), auth_headers(&user_b.access_token), axum::extract::State(state)).await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn subscription_history_is_available() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("plan-history");
    let password = "Abcdef1!xy";
    let registered = auth::register(test_connect_info(), axum::extract::State(state.clone()), axum::Json(RegisterReq {
        email, password: password.to_string(), name: None,
    })).await.expect("register").0;
    let uid = registered.user["id"].as_str().unwrap().to_string();
    let sid = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO subscriptions (id,user_id,plan_id,plan_version_id,status) VALUES (?,?,?,'plan-starter-v1','pending')")
        .bind(&sid).bind(&uid).bind("plan-starter")
        .execute(&state.pool).await.expect("subscription insert");

    let response = plans::list_subscription_history(auth_headers(&registered.access_token), axum::extract::State(state)).await.expect("history");
    let history = response.0;
    assert!(history.as_array().unwrap().len() >= 1);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn multiple_historical_subscriptions_coexist() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("plan-history-many");
    let password = "Abcdef1!xy";
    let registered = auth::register(test_connect_info(), axum::extract::State(state.clone()), axum::Json(RegisterReq {
        email, password: password.to_string(), name: None,
    })).await.expect("register").0;
    let uid = registered.user["id"].as_str().unwrap().to_string();

    for i in 0..3 {
        let sid = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO subscriptions (id,user_id,plan_id,plan_version_id,status,is_current) VALUES (?,?,?,'plan-starter-v1','expired',NULL)")
            .bind(&sid).bind(&uid).bind("plan-starter")
            .execute(&state.pool).await.expect("subscription insert");
        let _ = i;
    }

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM subscriptions WHERE user_id=?")
        .bind(&uid).fetch_one(&state.pool).await.expect("count");
    assert_eq!(count, 3);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn database_rejects_two_current_subscriptions() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("plan-db-unique");
    let password = "Abcdef1!xy";
    let registered = auth::register(test_connect_info(), axum::extract::State(state.clone()), axum::Json(RegisterReq {
        email, password: password.to_string(), name: None,
    })).await.expect("register").0;
    let uid = registered.user["id"].as_str().unwrap().to_string();

    let sid_a = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO subscriptions (id,user_id,plan_id,plan_version_id,status,is_current) VALUES (?,?,?,'plan-starter-v1','active',1)")
        .bind(&sid_a).bind(&uid).bind("plan-starter")
        .execute(&state.pool).await.expect("first current subscription");

    let sid_b = Uuid::new_v4().to_string();
    let result = sqlx::query("INSERT INTO subscriptions (id,user_id,plan_id,plan_version_id,status,is_current) VALUES (?,?,?,'plan-starter-v1','active',1)")
        .bind(&sid_b).bind(&uid).bind("plan-starter")
        .execute(&state.pool).await;
    assert!(result.is_err(), "MySQL must reject a second current subscription");
}
