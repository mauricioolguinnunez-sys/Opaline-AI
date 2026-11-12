mod common;

use common::{setup_state, test_connect_info, unique_email};
use axum::http::{HeaderMap, HeaderValue};
use opaline_backend::routes::auth::{self, RegisterReq};
use opaline_backend::quota;
use sqlx::Row;
use uuid::Uuid;
use chrono::Utc;

fn auth_headers(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", HeaderValue::from_str(&format!("Bearer {}", token)).unwrap());
    headers
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn quota_requires_subscription() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("quota-none");
    let password = "Abcdef1!xy";
    let registered = auth::register(test_connect_info(), axum::extract::State(state.clone()), axum::Json(RegisterReq {
        email, password: password.to_string(), name: None,
    })).await.expect("register").0;
    let uid = registered.user["id"].as_str().unwrap();
    let result = quota::resolve_current_subscription(&state.pool, uid).await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn quota_reserve_release_and_usage() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("quota-basic");
    let password = "Abcdef1!xy";
    let registered = auth::register(test_connect_info(), axum::extract::State(state.clone()), axum::Json(RegisterReq {
        email, password: password.to_string(), name: None,
    })).await.expect("register").0;
    let uid = registered.user["id"].as_str().unwrap().to_string();

    let plan_id = "plan-starter";
    let version_id = "plan-starter-v1";
    let sid = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO subscriptions (id,user_id,plan_id,plan_version_id,status,start_date,end_date,current_period_start,current_period_end,is_current,auto_renew) VALUES (?,?,?,?,'active',NOW(),DATE_ADD(NOW(),INTERVAL 1 MONTH),NOW(),DATE_ADD(NOW(),INTERVAL 1 MONTH),1,1)")
        .bind(&sid).bind(&uid).bind(plan_id).bind(version_id)
        .execute(&state.pool).await.expect("subscription");

    let (sub_id, pv_id, period_start, period_end) = quota::resolve_current_subscription(&state.pool, &uid).await.expect("resolve");
    let limits = quota::get_plan_limits(&state.pool, &pv_id).await.expect("limits");
    let reservation = quota::reserve_quota(&state.pool, &uid, &sub_id, &pv_id, period_start, period_end, &limits, "chat", "m", &Uuid::new_v4().to_string(), "key-basic", 10, 20).await.expect("reserve");
    quota::release_reservation(&state.pool, &reservation.id, false).await.expect("release");

    let usage = quota::get_current_usage(&state.pool, &uid).await.expect("usage");
    assert_eq!(usage["requests_used"], 0);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn quota_idempotency_after_commit() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("quota-idem");
    let password = "Abcdef1!xy";
    let registered = auth::register(test_connect_info(), axum::extract::State(state.clone()), axum::Json(RegisterReq {
        email, password: password.to_string(), name: None,
    })).await.expect("register").0;
    let uid = registered.user["id"].as_str().unwrap().to_string();

    let sid = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO subscriptions (id,user_id,plan_id,plan_version_id,status,start_date,end_date,current_period_start,current_period_end,is_current,auto_renew) VALUES (?,?,?,'plan-starter-v1','active',NOW(),DATE_ADD(NOW(),INTERVAL 1 MONTH),NOW(),DATE_ADD(NOW(),INTERVAL 1 MONTH),1,1)")
        .bind(&sid).bind(&uid).bind("plan-starter")
        .execute(&state.pool).await.expect("subscription");

    let (sub_id, pv_id, period_start, period_end) = quota::resolve_current_subscription(&state.pool, &uid).await.expect("resolve");
    let limits = quota::get_plan_limits(&state.pool, &pv_id).await.expect("limits");
    let key = "idem-key-1".to_string();
    let reservation = quota::reserve_quota(&state.pool, &uid, &sub_id, &pv_id, period_start, period_end, &limits, "chat", "m", &key, &key, 10, 20).await.expect("reserve");
    let response = serde_json::json!({"reply": "hola", "conversation_id": "c1"});
    quota::commit_reservation(&state.pool, &reservation.id, 12, 8, "estimated", "estimated", 5, response.clone()).await.expect("commit");

    let second = quota::reserve_quota(&state.pool, &uid, &sub_id, &pv_id, period_start, period_end, &limits, "chat", "m", &key, &key, 10, 20).await.expect("second reserve");
    assert!(second.cached);
    assert_eq!(second.cached_response.unwrap()["reply"], "hola");
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn quota_concurrency_respects_limit() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("quota-conc");
    let password = "Abcdef1!xy";
    let registered = auth::register(test_connect_info(), axum::extract::State(state.clone()), axum::Json(RegisterReq {
        email, password: password.to_string(), name: None,
    })).await.expect("register").0;
    let uid = registered.user["id"].as_str().unwrap().to_string();

    // Create a plan/version with one request per period.
    let plan_id = format!("pt{}", &Uuid::new_v4().to_string()[..8]);
    let version_id = format!("{}-v1", plan_id);
    sqlx::query("INSERT INTO plans (id,code,name,price,currency,period,messages_per_month,features,active) VALUES (?,?,?,?, 'MXN','mensual',1,'[]',1)")
        .bind(&plan_id).bind(&plan_id).bind("Test").bind(1.0_f64)
        .execute(&state.pool).await.expect("plan");
    sqlx::query("INSERT INTO plan_versions (id,plan_id,version,price,currency,messages_per_month,requests_per_period,input_tokens_per_period,output_tokens_per_period,total_tokens_per_period,effective_from,status) VALUES (?,?,1,1.0,'MXN',1,1,NULL,NULL,NULL,NOW(),'active')")
        .bind(&version_id).bind(&plan_id)
        .execute(&state.pool).await.expect("version");

    let sid = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO subscriptions (id,user_id,plan_id,plan_version_id,status,start_date,end_date,current_period_start,current_period_end,is_current,auto_renew) VALUES (?,?,?,?,'active',NOW(),DATE_ADD(NOW(),INTERVAL 1 MONTH),NOW(),DATE_ADD(NOW(),INTERVAL 1 MONTH),1,1)")
        .bind(&sid).bind(&uid).bind(&plan_id).bind(&version_id)
        .execute(&state.pool).await.expect("subscription");

    let (sub_id, pv_id, period_start, period_end) = quota::resolve_current_subscription(&state.pool, &uid).await.expect("resolve");
    let limits = quota::get_plan_limits(&state.pool, &pv_id).await.expect("limits");

    let state_a = state.clone();
    let state_b = state.clone();
    let uid_a = uid.clone();
    let uid_b = uid.clone();
    let sub_a = sub_id.clone();
    let sub_b = sub_id.clone();
    let pv_a = pv_id.clone();
    let pv_b = pv_id.clone();
    let limits_a = limits.clone();
    let limits_b = limits.clone();
    let key_a = Uuid::new_v4().to_string();
    let key_b = Uuid::new_v4().to_string();

    let task_a = tokio::spawn(async move {
        quota::reserve_quota(&state_a.pool, &uid_a, &sub_a, &pv_a, period_start, period_end, &limits_a, "chat", "m", &key_a, &key_a, 1, 1).await
    });
    let task_b = tokio::spawn(async move {
        quota::reserve_quota(&state_b.pool, &uid_b, &sub_b, &pv_b, period_start, period_end, &limits_b, "chat", "m", &key_b, &key_b, 1, 1).await
    });

    let res_a = task_a.await.unwrap();
    let res_b = task_b.await.unwrap();
    let successes = [res_a.is_ok(), res_b.is_ok()].iter().filter(|x| **x).count();
    assert_eq!(successes, 1, "only one concurrent reservation should succeed");
}
