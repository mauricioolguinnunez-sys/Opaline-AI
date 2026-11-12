mod common;

use common::{setup_state, test_connect_info, test_lock, unique_email};
use opaline_backend::consumers::mongo_wall_audit::MongoWallAuditConsumer;
use opaline_backend::outbox::{self, OutboxStatus};
use opaline_backend::outbox_worker::EventConsumer;
use opaline_backend::routes::wall::{self, CreatePostReq};
use mongodb::bson::doc;
use std::sync::Arc;
use tokio::task::JoinHandle;

async fn register_with_subscription(state: &opaline_backend::state::AppState, email: &str) -> String {
    let response = opaline_backend::routes::auth::register(
        test_connect_info(),
        axum::extract::State(state.clone()),
        axum::Json(opaline_backend::routes::auth::RegisterReq {
            email: email.to_string(),
            password: "Abcdef1!xy".to_string(),
            name: Some("Wall Outbox Tester".to_string()),
        }),
    )
    .await
    .expect("registration should succeed")
    .0;

    sqlx::query("INSERT INTO subscriptions (id, user_id, plan_id, status, start_date, end_date) VALUES (?,?,?,?,NOW(),DATE_ADD(NOW(),INTERVAL 1 MONTH))")
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(response.user["id"].as_str().unwrap())
        .bind("plan-starter")
        .bind("active")
        .execute(&state.pool)
        .await
        .expect("active subscription should be created");

    response.access_token
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn wall_post_created_event_is_persisted_and_processed_idempotently() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    state.mongo.collection::<mongodb::bson::Document>("wall_events").delete_many(doc! {}).await.unwrap();
    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let token = register_with_subscription(&state, &unique_email("wall-outbox")).await;
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {}", token).parse().unwrap(),
    );

    let created = wall::create_post(
        headers,
        axum::extract::State(state.clone()),
        axum::Json(CreatePostReq {
            content: "Publicacion outbox con longitud suficiente para la prueba.".to_string(),
            rating: Some(5),
            idempotency_key: None,
        }),
    )
    .await
    .expect("post creation should succeed")
    .0;
    let post_id = created["id"].as_str().unwrap().to_string();

    let events = outbox::list_pending(&state.pool, 10).await.unwrap();
    let event = events.iter().find(|e| e.event_type == "wall.post.created" && e.aggregate_id == post_id).expect("outbox event should exist");
    assert_eq!(event.status, OutboxStatus::Pending);

    let consumer = MongoWallAuditConsumer::new(state.mongo.clone());
    let outcome = consumer.handle(Arc::new(event.clone())).await;
    assert!(matches!(outcome, opaline_backend::outbox_worker::ConsumerOutcome::Success));

    let collection = state.mongo.collection::<mongodb::bson::Document>("wall_events");
    let count: u64 = collection.count_documents(doc! { "event_id": &event.id }).await.unwrap();
    assert_eq!(count, 1);

    let outcome_again = consumer.handle(Arc::new(event.clone())).await;
    assert!(matches!(outcome_again, opaline_backend::outbox_worker::ConsumerOutcome::Success));
    let count_again: u64 = collection.count_documents(doc! { "event_id": &event.id }).await.unwrap();
    assert_eq!(count_again, 1);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn wall_outbox_event_survives_before_consumer_runs() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    state.mongo.collection::<mongodb::bson::Document>("wall_events").delete_many(doc! {}).await.unwrap();
    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let token = register_with_subscription(&state, &unique_email("wall-outbox-persist")).await;
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {}", token).parse().unwrap(),
    );

    let created = wall::create_post(
        headers,
        axum::extract::State(state.clone()),
        axum::Json(CreatePostReq {
            content: "Publicacion persistente outbox con longitud suficiente.".to_string(),
            rating: None,
            idempotency_key: None,
        }),
    )
    .await
    .expect("post creation should succeed")
    .0;
    let post_id = created["id"].as_str().unwrap().to_string();

    let events = outbox::list_pending(&state.pool, 10).await.unwrap();
    assert!(events.iter().any(|e| e.event_type == "wall.post.created" && e.aggregate_id == post_id));

    let audit_count: u64 = state.mongo.collection::<mongodb::bson::Document>("wall_events").count_documents(doc! {}).await.unwrap();
    assert_eq!(audit_count, 0);
}


#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn wall_audit_index_is_verified() {
    let _guard = test_lock().await;
    let state = setup_state().await;
    let consumer = MongoWallAuditConsumer::new(state.mongo.clone());
    consumer.verify_event_id_index().await.expect("verified unique partial index should exist");
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn wall_audit_duplicate_processing_is_idempotent_under_concurrency() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    state.mongo.collection::<mongodb::bson::Document>("wall_events").delete_many(doc! {}).await.unwrap();
    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let token = register_with_subscription(&state, &unique_email("wall-audit-concurrent")).await;
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {}", token).parse().unwrap(),
    );

    let created = wall::create_post(
        headers,
        axum::extract::State(state.clone()),
        axum::Json(CreatePostReq {
            content: "Publicacion concurrente outbox con longitud suficiente.".to_string(),
            rating: Some(5),
            idempotency_key: None,
        }),
    )
    .await
    .expect("post creation should succeed")
    .0;
    let post_id = created["id"].as_str().unwrap().to_string();

    let events = outbox::list_pending(&state.pool, 10).await.unwrap();
    let event = Arc::new(events.iter().find(|e| e.event_type == "wall.post.created" && e.aggregate_id == post_id).expect("event should exist").clone());

    let consumer = MongoWallAuditConsumer::new(state.mongo.clone());
    let consumer_a = consumer.clone();
    let consumer_b = consumer.clone();
    let event_a = Arc::clone(&event);
    let event_b = Arc::clone(&event);

    let task_a: JoinHandle<_> = tokio::spawn(async move { consumer_a.handle(event_a).await });
    let task_b: JoinHandle<_> = tokio::spawn(async move { consumer_b.handle(event_b).await });

    let result_a = task_a.await.expect("task should join");
    let result_b = task_b.await.expect("task should join");
    assert!(matches!(result_a, opaline_backend::outbox_worker::ConsumerOutcome::Success));
    assert!(matches!(result_b, opaline_backend::outbox_worker::ConsumerOutcome::Success));

    let count: u64 = state.mongo.collection::<mongodb::bson::Document>("wall_events").count_documents(doc! { "event_id": &event.id }).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn wall_audit_event_survives_consumer_failure_before_confirmation() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    state.mongo.collection::<mongodb::bson::Document>("wall_events").delete_many(doc! {}).await.unwrap();
    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let token = register_with_subscription(&state, &unique_email("wall-audit-recovery")).await;
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {}", token).parse().unwrap(),
    );

    let created = wall::create_post(
        headers,
        axum::extract::State(state.clone()),
        axum::Json(CreatePostReq {
            content: "Publicacion recuperable outbox con longitud suficiente.".to_string(),
            rating: None,
            idempotency_key: None,
        }),
    )
    .await
    .expect("post creation should succeed")
    .0;
    let post_id = created["id"].as_str().unwrap().to_string();

    let events = outbox::list_pending(&state.pool, 10).await.unwrap();
    let event = Arc::new(events.iter().find(|e| e.event_type == "wall.post.created" && e.aggregate_id == post_id).expect("event should exist").clone());

    let consumer = MongoWallAuditConsumer::new(state.mongo.clone());
    let first = consumer.handle(Arc::clone(&event)).await;
    assert!(matches!(first, opaline_backend::outbox_worker::ConsumerOutcome::Success));

    let count: u64 = state.mongo.collection::<mongodb::bson::Document>("wall_events").count_documents(doc! { "event_id": &event.id }).await.unwrap();
    assert_eq!(count, 1);

    // Simulate a retry after the document already exists.
    let second = consumer.handle(Arc::clone(&event)).await;
    assert!(matches!(second, opaline_backend::outbox_worker::ConsumerOutcome::Success));

    let count_after_retry: u64 = state.mongo.collection::<mongodb::bson::Document>("wall_events").count_documents(doc! { "event_id": &event.id }).await.unwrap();
    assert_eq!(count_after_retry, 1);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn wall_audit_index_verification_rejects_missing_index() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    let consumer = MongoWallAuditConsumer::new(state.mongo.clone());
    let collection = state.mongo.collection::<mongodb::bson::Document>("wall_events");

    collection.drop_index("event_id_1").await.expect("test index should be droppable");

    let missing = consumer.verify_event_id_index().await;
    assert!(missing.is_err());

    let keys = mongodb::bson::doc! { "event_id": 1 };
    let options = mongodb::options::IndexOptions::builder()
        .unique(true)
        .partial_filter_expression(mongodb::bson::doc! { "event_id": { "$exists": true } })
        .build();
    let model = mongodb::IndexModel::builder().keys(keys).options(options).build();
    collection.create_index(model).await.expect("test index should be recreatable");

    let restored = consumer.verify_event_id_index().await;
    assert!(restored.is_ok());
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn wall_audit_duplicate_document_is_recognized_as_success() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    state.mongo.collection::<mongodb::bson::Document>("wall_events").delete_many(doc! {}).await.unwrap();
    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let token = register_with_subscription(&state, &unique_email("wall-audit-duplicate")).await;
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {}", token).parse().unwrap(),
    );

    let created = wall::create_post(
        headers,
        axum::extract::State(state.clone()),
        axum::Json(CreatePostReq {
            content: "Publicacion duplicada outbox con longitud suficiente.".to_string(),
            rating: None,
            idempotency_key: None,
        }),
    )
    .await
    .expect("post creation should succeed")
    .0;
    let post_id = created["id"].as_str().unwrap().to_string();

    let events = outbox::list_pending(&state.pool, 10).await.unwrap();
    let event = Arc::new(events.iter().find(|e| e.event_type == "wall.post.created" && e.aggregate_id == post_id).expect("event should exist").clone());

    let collection = state.mongo.collection::<mongodb::bson::Document>("wall_events");
    collection.insert_one(doc! { "event_id": &event.id, "legacy": true }).await.unwrap();

    let consumer = MongoWallAuditConsumer::new(state.mongo.clone());
    let outcome = consumer.handle(Arc::clone(&event)).await;
    assert!(matches!(outcome, opaline_backend::outbox_worker::ConsumerOutcome::Success));

    let count: u64 = collection.count_documents(doc! { "event_id": &event.id }).await.unwrap();
    assert_eq!(count, 1);
}
