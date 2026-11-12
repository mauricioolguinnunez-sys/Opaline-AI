mod common;

use common::{setup_state, test_lock};
use axum::http::{HeaderMap, StatusCode};
use opaline_backend::routes::outbox_admin::{self, ActionReq};
use opaline_backend::outbox::{self, OutboxStatus};
use mongodb::bson::doc;
use sqlx::Row;

async fn create_admin(state: &opaline_backend::state::AppState) -> (String, String) {
    let email = format!("admin-{}@example.test", uuid::Uuid::new_v4());
    let response = opaline_backend::routes::auth::register(
        common::test_connect_info(),
        axum::extract::State(state.clone()),
        axum::Json(opaline_backend::routes::auth::RegisterReq {
            email: email.clone(),
            password: "Abcdef1!xy".to_string(),
            name: Some("Admin Test".to_string()),
        }),
    )
    .await
    .expect("admin registration should succeed")
    .0;
    sqlx::query("UPDATE users SET role = 'admin' WHERE id = ?")
        .bind(response.user["id"].as_str().unwrap())
        .execute(&state.pool)
        .await
        .expect("admin role update should succeed");
    (response.access_token, response.user["id"].as_str().unwrap().to_string())
}

fn bearer(token: &str, forwarded_for: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {}", token).parse().unwrap(),
    );
    headers.insert("x-forwarded-for", forwarded_for.parse().unwrap());
    headers
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn admin_outbox_requires_authentication_and_role() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    let unauth = outbox_admin::list_discrepancies(
        HeaderMap::new(),
        axum::extract::State(state.clone()),
        axum::extract::Query(Default::default()),
    )
    .await;
    assert_eq!(unauth.unwrap_err(), StatusCode::UNAUTHORIZED);

    let email = format!("user-{}@example.test", uuid::Uuid::new_v4());
    let response = opaline_backend::routes::auth::register(
        common::test_connect_info(),
        axum::extract::State(state.clone()),
        axum::Json(opaline_backend::routes::auth::RegisterReq {
            email,
            password: "Abcdef1!xy".to_string(),
            name: None,
        }),
    )
    .await
    .expect("registration should succeed")
    .0;

    let normal = outbox_admin::list_discrepancies(
        bearer(&response.access_token, "10.0.0.2"),
        axum::extract::State(state.clone()),
        axum::extract::Query(Default::default()),
    )
    .await;
    assert_eq!(normal.unwrap_err(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn admin_can_list_and_inspect_discrepancies() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    let (token, _) = create_admin(&state).await;
    let headers = bearer(&token, "10.0.0.3");

    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let event_id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at) VALUES (?,?,?,?,?,'pending',0,NOW())")
        .bind(&event_id).bind("wall.post.created").bind("post").bind("post-1").bind("{}")
        .execute(&state.pool).await.unwrap();

    let list = outbox_admin::list_discrepancies(
        headers.clone(),
        axum::extract::State(state.clone()),
        axum::extract::Query(Default::default()),
    )
    .await
    .expect("admin should list discrepancies");
    let items = list.0["items"].as_array().expect("items array");
    assert!(items.iter().any(|item| item["id"] == event_id));

    let inspected = outbox_admin::get_event(
        headers.clone(),
        axum::extract::State(state.clone()),
        axum::extract::Path(event_id.clone()),
    )
    .await
    .expect("admin should inspect event");
    assert_eq!(inspected.0["id"], event_id);
    assert_eq!(inspected.0["classification"], "pending_without_document");
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn admin_recovers_pending_event_with_audit_document() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    let (token, _) = create_admin(&state).await;
    let headers = bearer(&token, "10.0.0.4");

    let event_id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at) VALUES (?,?,?,?,?,'pending',0,NOW())")
        .bind(&event_id).bind("wall.post.created").bind("post").bind("post-1").bind("{\"user_id\":\"test-user\"}")
        .execute(&state.pool).await.unwrap();
    state.mongo.collection::<mongodb::bson::Document>("wall_events").insert_one(doc! { "event_id": &event_id }).await.unwrap();

    let result = outbox_admin::run_action(
        axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 45000))),
        headers,
        axum::extract::State(state.clone()),
        axum::extract::Path(event_id.clone()),
        axum::Json(ActionReq {
            action: "recover".to_string(),
            reason: "recover pending audit".to_string(),
            idempotency_key: "recover-pending-1".to_string(),
            correlation_id: None,
        }),
    )
    .await
    .expect("recovery should succeed");
    assert_eq!(result.0["result"], "recovered");

    let event = outbox::get_event(&state.pool, &event_id).await.unwrap().unwrap();
    assert_eq!(event.status, OutboxStatus::Processed);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn admin_recovery_is_idempotent() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    let (token, _) = create_admin(&state).await;
    let headers = bearer(&token, "10.0.0.5");

    let event_id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at) VALUES (?,?,?,?,?,'pending',0,NOW())")
        .bind(&event_id).bind("wall.post.created").bind("post").bind("post-1").bind("{\"user_id\":\"test-user\"}")
        .execute(&state.pool).await.unwrap();
    state.mongo.collection::<mongodb::bson::Document>("wall_events").insert_one(doc! { "event_id": &event_id }).await.unwrap();

    let first = outbox_admin::run_action(
        axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 45000))),
        headers.clone(),
        axum::extract::State(state.clone()),
        axum::extract::Path(event_id.clone()),
        axum::Json(ActionReq {
            action: "recover".to_string(),
            reason: "first recovery".to_string(),
            idempotency_key: "same-key".to_string(),
            correlation_id: None,
        }),
    )
    .await
    .expect("first recovery should succeed");

    let second = outbox_admin::run_action(
        axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 45000))),
        headers,
        axum::extract::State(state.clone()),
        axum::extract::Path(event_id.clone()),
        axum::Json(ActionReq {
            action: "recover".to_string(),
            reason: "first recovery".to_string(),
            idempotency_key: "same-key".to_string(),
            correlation_id: None,
        }),
    )
    .await
    .expect("repeated recovery should be idempotent");
    assert_eq!(first.0["result"], "recovered");
    assert_eq!(second.0["idempotent"], true);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn admin_recovery_rejects_ineligible_events() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    let (token, _) = create_admin(&state).await;
    let headers = bearer(&token, "10.0.0.6");

    let failed_id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at, last_error) VALUES (?,?,?,?,?,'failed',5,NOW(),'invalid payload')")
        .bind(&failed_id).bind("wall.post.created").bind("post").bind("post-1").bind("{}")
        .execute(&state.pool).await.unwrap();

    let result = outbox_admin::run_action(
        axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 45000))),
        headers,
        axum::extract::State(state.clone()),
        axum::extract::Path(failed_id),
        axum::Json(ActionReq {
            action: "recover".to_string(),
            reason: "attempt recovery".to_string(),
            idempotency_key: "failed-recovery".to_string(),
            correlation_id: None,
        }),
    )
    .await;
    assert_eq!(result.unwrap_err(), StatusCode::CONFLICT);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn admin_can_record_manual_review() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    let (token, _) = create_admin(&state).await;
    let headers = bearer(&token, "10.0.0.7");

    let event_id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at) VALUES (?,?,?,?,?,'processed',1,NOW())")
        .bind(&event_id).bind("wall.post.created").bind("post").bind("post-1").bind("{}")
        .execute(&state.pool).await.unwrap();

    let result = outbox_admin::run_action(
        axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 45000))),
        headers.clone(),
        axum::extract::State(state.clone()),
        axum::extract::Path(event_id.clone()),
        axum::Json(ActionReq {
            action: "mark_manual_review".to_string(),
            reason: "manual investigation".to_string(),
            idempotency_key: "manual-review-1".to_string(),
            correlation_id: None,
        }),
    )
    .await
    .expect("manual review should be recorded");
    assert_eq!(result.0["result"], "recorded");

    let history = outbox_admin::get_history(
        headers,
        axum::extract::State(state.clone()),
        axum::extract::Path(event_id),
    )
    .await
    .expect("history should be available");
    let history = history.0.as_array().expect("history array");
    assert!(!history.is_empty());
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn admin_action_requires_reason_and_idempotency_key() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    let (token, _) = create_admin(&state).await;
    let headers = bearer(&token, "10.0.0.8");

    let event_id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at) VALUES (?,?,?,?,?,'pending',0,NOW())")
        .bind(&event_id).bind("wall.post.created").bind("post").bind("post-1").bind("{}")
        .execute(&state.pool).await.unwrap();

    let missing_reason = outbox_admin::run_action(
        axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 45000))),
        headers.clone(),
        axum::extract::State(state.clone()),
        axum::extract::Path(event_id.clone()),
        axum::Json(ActionReq {
            action: "mark_manual_review".to_string(),
            reason: "   ".to_string(),
            idempotency_key: "reason-test".to_string(),
            correlation_id: None,
        }),
    )
    .await;
    assert_eq!(missing_reason.unwrap_err(), StatusCode::BAD_REQUEST);

    let missing_key = outbox_admin::run_action(
        axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 45000))),
        headers,
        axum::extract::State(state.clone()),
        axum::extract::Path(event_id),
        axum::Json(ActionReq {
            action: "mark_manual_review".to_string(),
            reason: "motivo".to_string(),
            idempotency_key: "   ".to_string(),
            correlation_id: None,
        }),
    )
    .await;
    assert_eq!(missing_key.unwrap_err(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn admin_action_rejects_idempotency_key_reuse_with_different_parameters() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    let (token, _) = create_admin(&state).await;
    let headers = bearer(&token, "10.0.0.9");

    let first_event = uuid::Uuid::new_v4().to_string();
    let second_event = uuid::Uuid::new_v4().to_string();
    for event_id in [&first_event, &second_event] {
        sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at) VALUES (?,?,?,?,?,'pending',0,NOW())")
            .bind(event_id).bind("wall.post.created").bind("post").bind("post-1").bind("{}")
            .execute(&state.pool).await.unwrap();
    }

    let _ = outbox_admin::run_action(
        axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 45000))),
        headers.clone(),
        axum::extract::State(state.clone()),
        axum::extract::Path(first_event.clone()),
        axum::Json(ActionReq {
            action: "mark_manual_review".to_string(),
            reason: "primer evento".to_string(),
            idempotency_key: "shared-key".to_string(),
            correlation_id: None,
        }),
    )
    .await
    .expect("first action should succeed");

    let reused = outbox_admin::run_action(
        axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 45000))),
        headers,
        axum::extract::State(state.clone()),
        axum::extract::Path(second_event),
        axum::Json(ActionReq {
            action: "mark_manual_review".to_string(),
            reason: "segundo evento".to_string(),
            idempotency_key: "shared-key".to_string(),
            correlation_id: None,
        }),
    )
    .await;
    assert_eq!(reused.unwrap_err(), StatusCode::CONFLICT);
}
#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn admin_recovers_failed_event_without_document() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    let (token, _) = create_admin(&state).await;
    let headers = bearer(&token, "10.0.0.10");

    let event_id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at, last_error) VALUES (?,?,?,?,?,'failed',1,NOW(),'confirmation failed')")
        .bind(&event_id).bind("wall.post.created").bind("post").bind("post-1").bind("{\"user_id\":\"test-user\"}")
        .execute(&state.pool).await.unwrap();

    let result = outbox_admin::run_action(
        axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 45000))),
        headers,
        axum::extract::State(state.clone()),
        axum::extract::Path(event_id.clone()),
        axum::Json(ActionReq {
            action: "recover".to_string(),
            reason: "recover failed event".to_string(),
            idempotency_key: "recover-failed-without-doc".to_string(),
            correlation_id: None,
        }),
    )
    .await
    .expect("recoverable failed event should be recovered");
    assert_eq!(result.0["result"], "recovered");

    let event = outbox::get_event(&state.pool, &event_id).await.unwrap().unwrap();
    assert_eq!(event.status, OutboxStatus::Processed);

    let count: u64 = state.mongo.collection::<mongodb::bson::Document>("wall_events").count_documents(mongodb::bson::doc! { "event_id": &event_id }).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn admin_does_not_repair_processed_event_without_document() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    let (token, _) = create_admin(&state).await;
    let headers = bearer(&token, "10.0.0.11");

    let event_id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at) VALUES (?,?,?,?,?,'processed',1,NOW())")
        .bind(&event_id).bind("wall.post.created").bind("post").bind("post-1").bind("{\"user_id\":\"test-user\"}")
        .execute(&state.pool).await.unwrap();

    let result = outbox_admin::run_action(
        axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 45000))),
        headers,
        axum::extract::State(state.clone()),
        axum::extract::Path(event_id.clone()),
        axum::Json(ActionReq {
            action: "recover".to_string(),
            reason: "attempt repair".to_string(),
            idempotency_key: "processed-missing".to_string(),
            correlation_id: None,
        }),
    )
    .await;
    assert_eq!(result.unwrap_err(), StatusCode::CONFLICT);

    let event = outbox::get_event(&state.pool, &event_id).await.unwrap().unwrap();
    assert_eq!(event.status, OutboxStatus::Processed);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn admin_does_not_repair_orphan_document() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    let (token, _) = create_admin(&state).await;
    let headers = bearer(&token, "10.0.0.12");

    let orphan = uuid::Uuid::new_v4().to_string();
    state.mongo.collection::<mongodb::bson::Document>("wall_events").insert_one(mongodb::bson::doc! { "event_id": &orphan }).await.unwrap();

    let result = outbox_admin::run_action(
        axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 45000))),
        headers,
        axum::extract::State(state.clone()),
        axum::extract::Path(orphan),
        axum::Json(ActionReq {
            action: "recover".to_string(),
            reason: "attempt orphan repair".to_string(),
            idempotency_key: "orphan-repair".to_string(),
            correlation_id: None,
        }),
    )
    .await;
    assert_eq!(result.unwrap_err(), StatusCode::NOT_FOUND);
}
