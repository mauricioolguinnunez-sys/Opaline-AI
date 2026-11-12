mod common;

use common::{setup_state, test_lock};
use opaline_backend::outbox_reconciler::{OutboxReconciler, ReconciliationReport, ReconcilerConfig};
use mongodb::bson::doc;

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn reconciler_classifies_pending_events() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    let without_id = uuid::Uuid::new_v4().to_string();
    let with_id = uuid::Uuid::new_v4().to_string();

    sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at) VALUES (?,?,?,?,?,'pending',0,NOW())")
        .bind(&without_id).bind("wall.post.created").bind("post").bind("post-1").bind("{}")
        .execute(&state.pool).await.unwrap();
    sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at) VALUES (?,?,?,?,?,'pending',0,NOW())")
        .bind(&with_id).bind("wall.post.created").bind("post").bind("post-2").bind("{}")
        .execute(&state.pool).await.unwrap();

    state.mongo.collection::<mongodb::bson::Document>("wall_events").insert_one(doc! { "event_id": &with_id }).await.unwrap();

    let reconciler = OutboxReconciler::new(state.pool.clone(), state.mongo.clone(), ReconcilerConfig {
        pending_age_threshold_secs: 0,
        ..ReconcilerConfig::default()
    });
    reconciler.verify_dependencies().await.unwrap();
    let report = reconciler.reconcile_once().await.unwrap();

    assert!(report.pending_old_without_document.contains(&without_id));
    assert!(report.pending_old_with_document.contains(&with_id));
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn reconciler_classifies_processing_failed_and_processed_events() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    let processing_without = uuid::Uuid::new_v4().to_string();
    let processing_with = uuid::Uuid::new_v4().to_string();
    let failed_without = uuid::Uuid::new_v4().to_string();
    let failed_with = uuid::Uuid::new_v4().to_string();
    let processed_missing = uuid::Uuid::new_v4().to_string();

    for (id, status) in [
        (&processing_without, "processing"),
        (&processing_with, "processing"),
        (&failed_without, "failed"),
        (&failed_with, "failed"),
        (&processed_missing, "processed"),
    ] {
        sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at, locked_at) VALUES (?,?,?,?,?,?,0,NOW(), DATE_SUB(NOW(), INTERVAL 10 MINUTE))")
            .bind(id).bind("wall.post.created").bind("post").bind("post-x").bind("{}").bind(status)
            .execute(&state.pool).await.unwrap();
    }

    for id in [&processing_with, &failed_with] {
        state.mongo.collection::<mongodb::bson::Document>("wall_events").insert_one(doc! { "event_id": id }).await.unwrap();
    }

    let reconciler = OutboxReconciler::new(state.pool.clone(), state.mongo.clone(), ReconcilerConfig {
        processing_timeout_secs: 0,
        ..ReconcilerConfig::default()
    });
    reconciler.verify_dependencies().await.unwrap();
    let report = reconciler.reconcile_once().await.unwrap();

    assert!(report.processing_expired_without_document.contains(&processing_without));
    assert!(report.processing_expired_with_document.contains(&processing_with));
    assert!(report.failed_without_document.contains(&failed_without));
    assert!(report.failed_with_document.contains(&failed_with));
    assert!(report.processed_missing_document.contains(&processed_missing));
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn reconciler_classifies_audit_documents() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    let non_processed = uuid::Uuid::new_v4().to_string();
    let orphan = uuid::Uuid::new_v4().to_string();

    sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at) VALUES (?,?,?,?,?,'pending',0,NOW())")
        .bind(&non_processed).bind("wall.post.created").bind("post").bind("post-1").bind("{}")
        .execute(&state.pool).await.unwrap();

    state.mongo.collection::<mongodb::bson::Document>("wall_events").insert_one(doc! { "event_id": &non_processed }).await.unwrap();
    state.mongo.collection::<mongodb::bson::Document>("wall_events").insert_one(doc! { "event_id": &orphan }).await.unwrap();

    let reconciler = OutboxReconciler::new(state.pool.clone(), state.mongo.clone(), ReconcilerConfig::default());
    reconciler.verify_dependencies().await.unwrap();
    let report = reconciler.reconcile_once().await.unwrap();

    assert!(report.audit_exists_non_processed.contains(&non_processed));
    assert!(report.audit_without_outbox.contains(&orphan));
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn reconciler_excludes_non_wall_events() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at) VALUES (?,?,?,?,?,'pending',0,NOW())")
        .bind(&id).bind("auth.login").bind("user").bind("user-1").bind("{}")
        .execute(&state.pool).await.unwrap();

    let reconciler = OutboxReconciler::new(state.pool.clone(), state.mongo.clone(), ReconcilerConfig {
        pending_age_threshold_secs: 0,
        ..ReconcilerConfig::default()
    });
    reconciler.verify_dependencies().await.unwrap();
    let report = reconciler.reconcile_once().await.unwrap();

    assert!(!report.pending_old_without_document.contains(&id));
    assert!(!report.pending_old_with_document.contains(&id));
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL and MongoDB pointing to isolated test databases"]
async fn reconciler_is_read_only() {
    let _guard = test_lock().await;
    let state = setup_state().await;

    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at, claim_token) VALUES (?,?,?,?,?,'processing',2,NOW(),'claim-token')")
        .bind(&id).bind("wall.post.created").bind("post").bind("post-1").bind("{}")
        .execute(&state.pool).await.unwrap();

    let before_status: String = sqlx::query_scalar("SELECT status FROM outbox_events WHERE id = ?")
        .bind(&id).fetch_one(&state.pool).await.unwrap();
    let before_attempts: i32 = sqlx::query_scalar("SELECT attempts FROM outbox_events WHERE id = ?")
        .bind(&id).fetch_one(&state.pool).await.unwrap();
    let before_claim: Option<String> = sqlx::query_scalar("SELECT claim_token FROM outbox_events WHERE id = ?")
        .bind(&id).fetch_one(&state.pool).await.unwrap();
    let mongo_before: u64 = state.mongo.collection::<mongodb::bson::Document>("wall_events").count_documents(mongodb::bson::doc! {}).await.unwrap();

    let reconciler = OutboxReconciler::new(state.pool.clone(), state.mongo.clone(), ReconcilerConfig {
        processing_timeout_secs: 0,
        ..ReconcilerConfig::default()
    });
    reconciler.verify_dependencies().await.unwrap();
    let report = reconciler.reconcile_once().await.unwrap();
    assert!(report.processing_expired_without_document.contains(&id));

    let after_status: String = sqlx::query_scalar("SELECT status FROM outbox_events WHERE id = ?")
        .bind(&id).fetch_one(&state.pool).await.unwrap();
    let after_attempts: i32 = sqlx::query_scalar("SELECT attempts FROM outbox_events WHERE id = ?")
        .bind(&id).fetch_one(&state.pool).await.unwrap();
    let after_claim: Option<String> = sqlx::query_scalar("SELECT claim_token FROM outbox_events WHERE id = ?")
        .bind(&id).fetch_one(&state.pool).await.unwrap();
    let mongo_after: u64 = state.mongo.collection::<mongodb::bson::Document>("wall_events").count_documents(mongodb::bson::doc! {}).await.unwrap();

    assert_eq!(before_status, after_status);
    assert_eq!(before_attempts, after_attempts);
    assert_eq!(before_claim, after_claim);
    assert_eq!(mongo_before, mongo_after);
}