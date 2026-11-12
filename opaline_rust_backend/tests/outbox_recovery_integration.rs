mod common;

use common::{setup_state, test_lock};
use opaline_backend::outbox::{self, OutboxStatus};
use opaline_backend::outbox_recovery::{OutboxRecovery, RecoveryConfig};
use mongodb::bson::doc;

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn recovery_processes_pending_event_with_existing_audit_document() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    state.mongo.collection::<mongodb::bson::Document>("wall_events").delete_many(doc! {}).await.unwrap();
    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at) VALUES (?,?,?,?,?,'pending',0,NOW())")
        .bind(&id).bind("wall.post.created").bind("post").bind("post-1").bind("{\"user_id\":\"test-user\"}")
        .execute(&state.pool).await.unwrap();
    state.mongo.collection::<mongodb::bson::Document>("wall_events").insert_one(doc! { "event_id": &id }).await.unwrap();

    let recovery = OutboxRecovery::new(state.pool.clone(), state.mongo.clone(), RecoveryConfig::default());
    recovery.verify_dependencies().await.unwrap();
    let report = recovery.recover_once().await.unwrap();

    assert!(report.recovered.contains(&id));
    let event = outbox::get_event(&state.pool, &id).await.unwrap().unwrap();
    assert_eq!(event.status, OutboxStatus::Processed);
    let count: u64 = state.mongo.collection::<mongodb::bson::Document>("wall_events").count_documents(doc! { "event_id": &id }).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn recovery_processes_expired_processing_event() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    state.mongo.collection::<mongodb::bson::Document>("wall_events").delete_many(doc! {}).await.unwrap();
    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at, locked_at, claim_token) VALUES (?,?,?,?,?,'processing',1,NOW(), DATE_SUB(NOW(), INTERVAL 10 MINUTE), 'old-claim')")
        .bind(&id).bind("wall.post.created").bind("post").bind("post-2").bind("{\"user_id\":\"test-user\"}")
        .execute(&state.pool).await.unwrap();
    state.mongo.collection::<mongodb::bson::Document>("wall_events").insert_one(doc! { "event_id": &id }).await.unwrap();

    let recovery = OutboxRecovery::new(state.pool.clone(), state.mongo.clone(), RecoveryConfig {
        processing_timeout_secs: 0,
        ..RecoveryConfig::default()
    });
    recovery.verify_dependencies().await.unwrap();
    let report = recovery.recover_once().await.unwrap();

    assert!(report.recovered.contains(&id));
    let event = outbox::get_event(&state.pool, &id).await.unwrap().unwrap();
    assert_eq!(event.status, OutboxStatus::Processed);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn recovery_processes_expired_processing_event_without_document() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    state.mongo.collection::<mongodb::bson::Document>("wall_events").delete_many(doc! {}).await.unwrap();
    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at, locked_at, claim_token) VALUES (?,?,?,?,?,'processing',1,NOW(), DATE_SUB(NOW(), INTERVAL 10 MINUTE), 'old-claim')")
        .bind(&id).bind("wall.post.created").bind("post").bind("post-3").bind("{\"user_id\":\"test-user\"}")
        .execute(&state.pool).await.unwrap();

    let recovery = OutboxRecovery::new(state.pool.clone(), state.mongo.clone(), RecoveryConfig {
        processing_timeout_secs: 0,
        ..RecoveryConfig::default()
    });
    recovery.verify_dependencies().await.unwrap();
    let report = recovery.recover_once().await.unwrap();

    assert!(report.recovered.contains(&id));
    let event = outbox::get_event(&state.pool, &id).await.unwrap().unwrap();
    assert_eq!(event.status, OutboxStatus::Processed);
    let count: u64 = state.mongo.collection::<mongodb::bson::Document>("wall_events").count_documents(doc! { "event_id": &id }).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn recovery_skips_active_processing_lock() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    state.mongo.collection::<mongodb::bson::Document>("wall_events").delete_many(doc! {}).await.unwrap();
    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at, locked_at, claim_token) VALUES (?,?,?,?,?,'processing',1,NOW(), NOW(), 'active-claim')")
        .bind(&id).bind("wall.post.created").bind("post").bind("post-4").bind("{\"user_id\":\"test-user\"}")
        .execute(&state.pool).await.unwrap();

    let recovery = OutboxRecovery::new(state.pool.clone(), state.mongo.clone(), RecoveryConfig::default());
    recovery.verify_dependencies().await.unwrap();
    let report = recovery.recover_once().await.unwrap();

    assert!(!report.recovered.contains(&id));
    assert!(report.skipped.iter().any(|s| s.event_id == id && s.reason == "processing lock is still active"));
    let event = outbox::get_event(&state.pool, &id).await.unwrap().unwrap();
    assert_eq!(event.status, OutboxStatus::Processing);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn recovery_recovers_failed_event_with_recoverable_cause() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    state.mongo.collection::<mongodb::bson::Document>("wall_events").delete_many(doc! {}).await.unwrap();
    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at, last_error) VALUES (?,?,?,?,?,'failed',1,NOW(),'confirmation failed')")
        .bind(&id).bind("wall.post.created").bind("post").bind("post-5").bind("{\"user_id\":\"test-user\"}")
        .execute(&state.pool).await.unwrap();
    state.mongo.collection::<mongodb::bson::Document>("wall_events").insert_one(doc! { "event_id": &id }).await.unwrap();

    let recovery = OutboxRecovery::new(state.pool.clone(), state.mongo.clone(), RecoveryConfig::default());
    recovery.verify_dependencies().await.unwrap();
    let report = recovery.recover_once().await.unwrap();

    assert!(report.recovered.contains(&id));
    let event = outbox::get_event(&state.pool, &id).await.unwrap().unwrap();
    assert_eq!(event.status, OutboxStatus::Processed);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn recovery_requires_manual_review_for_failed_event_without_document() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    state.mongo.collection::<mongodb::bson::Document>("wall_events").delete_many(doc! {}).await.unwrap();
    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at, last_error) VALUES (?,?,?,?,?,'failed',1,NOW(),'confirmation failed')")
        .bind(&id).bind("wall.post.created").bind("post").bind("post-6").bind("{\"user_id\":\"test-user\"}")
        .execute(&state.pool).await.unwrap();

    let recovery = OutboxRecovery::new(state.pool.clone(), state.mongo.clone(), RecoveryConfig::default());
    recovery.verify_dependencies().await.unwrap();
    let report = recovery.recover_once().await.unwrap();

    assert!(!report.recovered.contains(&id));
    assert!(report.skipped.iter().any(|s| s.event_id == id && s.reason == "failed without audit document requires manual review"));
    let event = outbox::get_event(&state.pool, &id).await.unwrap().unwrap();
    assert_eq!(event.status, OutboxStatus::Failed);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn recovery_requires_manual_review_for_processed_event_without_document() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    state.mongo.collection::<mongodb::bson::Document>("wall_events").delete_many(doc! {}).await.unwrap();
    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at) VALUES (?,?,?,?,?,'processed',1,NOW())")
        .bind(&id).bind("wall.post.created").bind("post").bind("post-7").bind("{\"user_id\":\"test-user\"}")
        .execute(&state.pool).await.unwrap();

    let recovery = OutboxRecovery::new(state.pool.clone(), state.mongo.clone(), RecoveryConfig::default());
    recovery.verify_dependencies().await.unwrap();
    let report = recovery.recover_once().await.unwrap();

    assert!(!report.recovered.contains(&id));
    assert!(report.skipped.iter().any(|s| s.event_id == id && s.reason == "processed event missing audit document requires manual review"));
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn recovery_classifies_mongo_document_without_outbox_event() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    state.mongo.collection::<mongodb::bson::Document>("wall_events").delete_many(doc! {}).await.unwrap();
    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let orphan = uuid::Uuid::new_v4().to_string();
    state.mongo.collection::<mongodb::bson::Document>("wall_events").insert_one(doc! { "event_id": &orphan }).await.unwrap();

    let recovery = OutboxRecovery::new(state.pool.clone(), state.mongo.clone(), RecoveryConfig::default());
    recovery.verify_dependencies().await.unwrap();
    let report = recovery.recover_once().await.unwrap();

    assert!(report.skipped.iter().any(|s| s.event_id == orphan && s.reason == "historical_or_orphaned_document"));
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn recovery_respects_cycle_limit() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    state.mongo.collection::<mongodb::bson::Document>("wall_events").delete_many(doc! {}).await.unwrap();
    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let mut ids = Vec::new();
    for _ in 0..2 {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at) VALUES (?,?,?,?,?,'pending',0,NOW())")
            .bind(&id).bind("wall.post.created").bind("post").bind("post-limit").bind("{\"user_id\":\"test-user\"}")
            .execute(&state.pool).await.unwrap();
        state.mongo.collection::<mongodb::bson::Document>("wall_events").insert_one(doc! { "event_id": &id }).await.unwrap();
        ids.push(id);
    }

    let recovery = OutboxRecovery::new(state.pool.clone(), state.mongo.clone(), RecoveryConfig {
        max_recoveries_per_cycle: 1,
        ..RecoveryConfig::default()
    });
    recovery.verify_dependencies().await.unwrap();
    let report = recovery.recover_once().await.unwrap();

    assert_eq!(report.recovered.len(), 1);
    assert!(report.skipped.iter().any(|s| s.reason == "recovery limit reached"));
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn recovery_concurrency_does_not_duplicate_recovery() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    state.mongo.collection::<mongodb::bson::Document>("wall_events").delete_many(doc! {}).await.unwrap();
    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at) VALUES (?,?,?,?,?,'pending',0,NOW())")
        .bind(&id).bind("wall.post.created").bind("post").bind("post-concurrent").bind("{\"user_id\":\"test-user\"}")
        .execute(&state.pool).await.unwrap();
    state.mongo.collection::<mongodb::bson::Document>("wall_events").insert_one(doc! { "event_id": &id }).await.unwrap();

    let recovery_a = OutboxRecovery::new(state.pool.clone(), state.mongo.clone(), RecoveryConfig::default());
    let recovery_b = OutboxRecovery::new(state.pool.clone(), state.mongo.clone(), RecoveryConfig::default());

    let task_a = tokio::spawn(async move { recovery_a.recover_once().await });
    let task_b = tokio::spawn(async move { recovery_b.recover_once().await });

    let report_a = task_a.await.unwrap().unwrap();
    let report_b = task_b.await.unwrap().unwrap();

    let recovered_total = report_a.recovered.len() + report_b.recovered.len();
    assert!(recovered_total <= 1, "only one recovery should succeed");
    let count: u64 = state.mongo.collection::<mongodb::bson::Document>("wall_events").count_documents(doc! { "event_id": &id }).await.unwrap();
    assert_eq!(count, 1);
    let event = outbox::get_event(&state.pool, &id).await.unwrap().unwrap();
    assert_eq!(event.status, OutboxStatus::Processed);
}