mod common;

use common::setup_state;
use opaline_backend::outbox::{self, NewOutboxEvent, OutboxStatus};
use sqlx::Row;

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn outbox_persists_and_retrieves_pending_events() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let event = NewOutboxEvent::new("wall.post_created", "post", "post-1", "{\"ok\":true}");
    let id = outbox::insert(&state.pool, &event).await.expect("insert should succeed");

    let pending = outbox::list_pending(&state.pool, 10).await.expect("list should succeed");
    assert!(pending.iter().any(|e| e.id == id));
    assert_eq!(pending[0].status, OutboxStatus::Pending);

    let stored = outbox::get_event(&state.pool, &id).await.expect("get should succeed").unwrap();
    assert_eq!(stored.event_type, "wall.post_created");
    assert_eq!(stored.aggregate_id, "post-1");
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn outbox_insert_is_atomic_with_business_write() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let email = format!("outbox-{}@example.test", uuid::Uuid::new_v4());
    let mut tx = state.pool.begin().await.expect("transaction should start");
    sqlx::query("INSERT INTO users (id,email,password_hash,name,role,email_verified) VALUES (?,?,?,?, 'client',1)")
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&email)
        .bind("hash")
        .bind("Outbox Test")
        .execute(&mut *tx)
        .await
        .expect("user insert should succeed");

    let event = NewOutboxEvent::new("user.created", "user", "user-1", "{\"ok\":true}");
    outbox::insert(&mut *tx, &event).await.expect("outbox insert should succeed");
    tx.commit().await.expect("commit should succeed");

    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = ?")
        .bind(&email)
        .fetch_one(&state.pool)
        .await
        .expect("user count should succeed");
    let event_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbox_events WHERE event_type = 'user.created'")
        .fetch_one(&state.pool)
        .await
        .expect("event count should succeed");
    assert_eq!(user_count, 1);
    assert_eq!(event_count, 1);

    let email = format!("rollback-{}@example.test", uuid::Uuid::new_v4());
    let mut tx = state.pool.begin().await.expect("transaction should start");
    sqlx::query("INSERT INTO users (id,email,password_hash,name,role,email_verified) VALUES (?,?,?,?, 'client',1)")
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&email)
        .bind("hash")
        .bind("Rollback Test")
        .execute(&mut *tx)
        .await
        .expect("user insert should succeed");
    let event = NewOutboxEvent::new("user.rollback", "user", "user-2", "{\"ok\":true}");
    outbox::insert(&mut *tx, &event).await.expect("outbox insert should succeed");
    tx.rollback().await.expect("rollback should succeed");

    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = ?")
        .bind(&email)
        .fetch_one(&state.pool)
        .await
        .expect("user count should succeed");
    let event_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbox_events WHERE event_type = 'user.rollback'")
        .fetch_one(&state.pool)
        .await
        .expect("event count should succeed");
    assert_eq!(user_count, 0);
    assert_eq!(event_count, 0);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn outbox_claim_marks_events_processing() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let id = outbox::insert(&state.pool, &NewOutboxEvent::new("test.claim", "aggregate", "1", "{}"))
        .await
        .expect("insert should succeed");

    let claimed = outbox::claim_pending(&state.pool, 10, 60).await.expect("claim should succeed");
    assert!(claimed.iter().any(|e| e.id == id));
    let claimed_event = claimed.iter().find(|e| e.id == id).expect("claimed event");
    assert_eq!(claimed_event.status, OutboxStatus::Processing);
    assert_eq!(claimed_event.attempts, 1);

    let stored = outbox::get_event(&state.pool, &id).await.unwrap().unwrap();
    assert_eq!(stored.status, OutboxStatus::Processing);
    assert!(stored.locked_at.is_some());

    let pending = outbox::list_pending(&state.pool, 10).await.unwrap();
    assert!(pending.is_empty());
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn outbox_mark_processed_and_failure_are_persisted() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let processed_id = outbox::insert(&state.pool, &NewOutboxEvent::new("test.processed", "aggregate", "1", "{}"))
        .await
        .unwrap();
    let processed_claim = outbox::claim_pending(&state.pool, 10, 60).await.unwrap().into_iter().find(|e| e.id == processed_id).unwrap().claim_token.unwrap();
    assert!(outbox::mark_processed(&state.pool, &processed_id, &processed_claim).await.unwrap());

    let processed = outbox::get_event(&state.pool, &processed_id).await.unwrap().unwrap();
    assert_eq!(processed.status, OutboxStatus::Processed);
    assert!(processed.processed_at.is_some());
    assert!(processed.locked_at.is_none());

    let failed_id = outbox::insert(&state.pool, &NewOutboxEvent::new("test.failed", "aggregate", "2", "{}"))
        .await
        .unwrap();
    let failed_claim = outbox::claim_pending(&state.pool, 10, 60).await.unwrap().into_iter().find(|e| e.id == failed_id).unwrap().claim_token.unwrap();
    outbox::record_failure(&state.pool, &failed_id, &failed_claim, "temporary failure", std::time::Duration::from_secs(0), 3)
        .await
        .expect("failure should be recorded");

    let failed = outbox::get_event(&state.pool, &failed_id).await.unwrap().unwrap();
    assert_eq!(failed.status, OutboxStatus::Pending);
    assert_eq!(failed.last_error.as_deref(), Some("temporary failure"));
    assert!(failed.locked_at.is_none());
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn outbox_expired_lock_can_be_recovered() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let id = outbox::insert(&state.pool, &NewOutboxEvent::new("test.lock", "aggregate", "1", "{}"))
        .await
        .unwrap();
    assert!(outbox::claim_pending(&state.pool, 10, 60).await.unwrap().iter().any(|e| e.id == id));

    sqlx::query("UPDATE outbox_events SET locked_at = DATE_SUB(NOW(), INTERVAL 10 MINUTE) WHERE id = ?")
        .bind(&id)
        .execute(&state.pool)
        .await
        .unwrap();

    let released = outbox::release_expired_locks(&state.pool, 60).await.unwrap();
    assert_eq!(released, 1);

    let claimed = outbox::claim_pending(&state.pool, 10, 60).await.unwrap();
    assert!(claimed.iter().any(|e| e.id == id));
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn outbox_claim_concurrency_does_not_duplicate_claims() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    for i in 0..5 {
        outbox::insert(&state.pool, &NewOutboxEvent::new("test.concurrent", "aggregate", &i.to_string(), "{}"))
            .await
            .unwrap();
    }

    let state_a = state.clone();
    let state_b = state.clone();
    let task_a = tokio::spawn(async move { outbox::claim_pending(&state_a.pool, 10, 60).await });
    let task_b = tokio::spawn(async move { outbox::claim_pending(&state_b.pool, 10, 60).await });

    let mut claimed_ids = Vec::new();
    if let Ok(Ok(rows)) = task_a.await {
        claimed_ids.extend(rows.into_iter().map(|e| e.id));
    }
    if let Ok(Ok(rows)) = task_b.await {
        claimed_ids.extend(rows.into_iter().map(|e| e.id));
    }

    let mut unique = claimed_ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(claimed_ids.len(), unique.len(), "an event must not be claimed twice");
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn outbox_duplicate_event_id_is_rejected() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let duplicate_id = uuid::Uuid::new_v4().to_string();
    let mut event = NewOutboxEvent::new("test.duplicate", "aggregate", "1", "{}");
    event.id = Some(duplicate_id.clone());

    outbox::insert(&state.pool, &event).await.expect("first insert should succeed");
    let second = outbox::insert(&state.pool, &event).await;
    assert!(second.is_err(), "duplicate event id must be rejected");
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn outbox_state_persists_across_connections() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    sqlx::query("DELETE FROM outbox_admin_actions").execute(&state.pool).await.unwrap();
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let id = outbox::insert(&state.pool, &NewOutboxEvent::new("test.persist", "aggregate", "1", "{}"))
        .await
        .unwrap();

    drop(state);
    let new_pool = common::test_pool().await;
    let stored = outbox::get_event(&new_pool, &id).await.unwrap();
    assert!(stored.is_some());
}