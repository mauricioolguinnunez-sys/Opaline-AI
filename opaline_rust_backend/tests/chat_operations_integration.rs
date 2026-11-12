mod common;

use axum::http::{HeaderMap, HeaderValue};

fn auth_headers(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", HeaderValue::from_str(&format!("Bearer {}", token)).unwrap());
    headers
}

use common::{setup_state, test_connect_info, unique_email};
use opaline_backend::routes::auth::{self, RegisterReq};
use opaline_backend::chat_operations;
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn chat_operation_persists_idempotently() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("chat-op");
    let password = "Abcdef1!xy";
    let registered = auth::register(test_connect_info(), axum::extract::State(state.clone()), axum::Json(RegisterReq {
        email, password: password.to_string(), name: None,
    })).await.expect("register").0;
    let uid = registered.user["id"].as_str().unwrap().to_string();

    let plan_id = "plan-starter";
    let version_id = "plan-starter-v1";
    let sid = Uuid::new_v4().to_string();
    let rid = Uuid::new_v4().to_string();
    let period_id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO subscriptions (id,user_id,plan_id,plan_version_id,status,start_date,end_date,current_period_start,current_period_end,is_current,auto_renew) VALUES (?,?,?,?,'active',NOW(),DATE_ADD(NOW(),INTERVAL 1 MONTH),NOW(),DATE_ADD(NOW(),INTERVAL 1 MONTH),1,1)")
        .bind(&sid).bind(&uid).bind(plan_id).bind(version_id)
        .execute(&state.pool).await.expect("subscription");
    sqlx::query("INSERT INTO usage_periods (id,user_id,subscription_id,plan_version_id,period_start,period_end,request_limit) VALUES (?,?,?,? ,NOW(),DATE_ADD(NOW(),INTERVAL 1 MONTH),100)")
        .bind(&period_id).bind(&uid).bind(&sid).bind(version_id)
        .execute(&state.pool).await.expect("period");
    sqlx::query("INSERT INTO usage_reservations (id,user_id,subscription_id,usage_period_id,request_id,idempotency_key,service_type,model,status,estimated_input_tokens,estimated_output_tokens,estimated_total_tokens,expires_at) VALUES (?,?,?,?,'req1','key1','chat','m','reserved',1,1,2,DATE_ADD(NOW(),INTERVAL 5 MINUTE))")
        .bind(&rid).bind(&uid).bind(&sid).bind(&period_id)
        .execute(&state.pool).await.expect("reservation");

    let op_id = chat_operations::create_operation(&state.pool, &uid, &sid, &period_id, &rid, "req1", "key1", "m", "hello").await.expect("operation");
    chat_operations::update_inferred(&state.pool, &op_id, "m", "reply", &json!({"reply":"reply","conversation_id":"cid1"}), "cid1", "estimated", "estimated", 1, 1, 2, 10).await.expect("inferred");

    chat_operations::persist_operation(&state.pool, &state.mongo, &op_id).await.expect("persist");
    let row = sqlx::query("SELECT mongo_persisted FROM chat_operations WHERE id=?").bind(&op_id).fetch_one(&state.pool).await.unwrap();
    assert_eq!(row.get::<i8,_>("mongo_persisted"), 1);

    // idempotent
    chat_operations::persist_operation(&state.pool, &state.mongo, &op_id).await.expect("persist again");

    let count: i64 = state.mongo.collection::<mongodb::bson::Document>("conversations").count_documents(mongodb::bson::doc!{"id":"cid1","user_id":&uid}).await.unwrap() as i64;
    assert_eq!(count, 1);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn reconcile_chat_operations_reports_pending() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let report = chat_operations::reconcile_operations(&state.pool, &state.mongo, false).await.expect("reconcile");
    assert!(report.get("operations").is_some());
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn operation_can_be_queried_by_id_and_key() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("chat-query");
    let password = "Abcdef1!xy";
    let registered = auth::register(test_connect_info(), axum::extract::State(state.clone()), axum::Json(RegisterReq {
        email, password: password.to_string(), name: None,
    })).await.expect("register").0;
    let token = registered.access_token.clone();
    let uid = registered.user["id"].as_str().unwrap().to_string();

    let plan_id = "plan-starter";
    let version_id = "plan-starter-v1";
    let sid = Uuid::new_v4().to_string();
    let rid = Uuid::new_v4().to_string();
    let period_id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO subscriptions (id,user_id,plan_id,plan_version_id,status,start_date,end_date,current_period_start,current_period_end,is_current,auto_renew) VALUES (?,?,?,?,'active',NOW(),DATE_ADD(NOW(),INTERVAL 1 MONTH),NOW(),DATE_ADD(NOW(),INTERVAL 1 MONTH),1,1)")
        .bind(&sid).bind(&uid).bind(plan_id).bind(version_id)
        .execute(&state.pool).await.expect("subscription");
    sqlx::query("INSERT INTO usage_periods (id,user_id,subscription_id,plan_version_id,period_start,period_end,request_limit) VALUES (?,?,?,? ,NOW(),DATE_ADD(NOW(),INTERVAL 1 MONTH),100)")
        .bind(&period_id).bind(&uid).bind(&sid).bind(version_id)
        .execute(&state.pool).await.expect("period");
    sqlx::query("INSERT INTO usage_reservations (id,user_id,subscription_id,usage_period_id,request_id,idempotency_key,service_type,model,status,estimated_input_tokens,estimated_output_tokens,estimated_total_tokens,expires_at) VALUES (?,?,?,?,'req2','key2','chat','m','reserved',1,1,2,DATE_ADD(NOW(),INTERVAL 5 MINUTE))")
        .bind(&rid).bind(&uid).bind(&sid).bind(&period_id)
        .execute(&state.pool).await.expect("reservation");

    let op_id = chat_operations::create_operation(&state.pool, &uid, &sid, &period_id, &rid, "req2", "key2", "m", "hello").await.expect("operation");
    chat_operations::update_inferred(&state.pool, &op_id, "m", "reply", &serde_json::json!({"reply":"reply","conversation_id":"cid2"}), "cid2", "estimated", "estimated", 1, 1, 2, 10).await.expect("inferred");

    let headers = auth_headers(&token);
    let by_id = opaline_backend::routes::chat::get_operation_by_id(axum::extract::Path(op_id.clone()), headers.clone(), axum::extract::State(state.clone())).await.expect("by id").0;
    assert_eq!(by_id["operation_id"], op_id);
    assert_eq!(by_id["idempotency_key"], "key2");

    let by_key = opaline_backend::routes::chat::get_operation(axum::extract::Query(opaline_backend::routes::chat::OperationQuery { idempotency_key: Some("key2".to_string()) }), headers, axum::extract::State(state.clone())).await.expect("by key").0;
    assert_eq!(by_key["status"], "awaiting_persistence");
}
