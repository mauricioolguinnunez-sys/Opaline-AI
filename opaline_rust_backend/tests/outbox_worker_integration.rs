mod common;

use common::{setup_state, test_lock};
use opaline_backend::outbox::{self, NewOutboxEvent, OutboxStatus};
use opaline_backend::outbox_worker::{ConsumerOutcome, EventConsumer, OutboxWorker, OutboxWorkerConfig};
use sqlx::Row;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::watch;

struct TestConsumer {
    effects: Arc<Mutex<Vec<String>>>,
    temporary_failures: Arc<Mutex<usize>>,
}

impl TestConsumer {
    fn new() -> Self {
        Self {
            effects: Arc::new(Mutex::new(Vec::new())),
            temporary_failures: Arc::new(Mutex::new(0)),
        }
    }
}

impl EventConsumer for TestConsumer {
    fn handle(&self, event: Arc<outbox::OutboxEvent>) -> Pin<Box<dyn std::future::Future<Output = ConsumerOutcome> + Send>> {
        let effects = Arc::clone(&self.effects);
        let temporary_failures = Arc::clone(&self.temporary_failures);
        Box::pin(async move {
            let mut effects = effects.lock().unwrap();
            if effects.contains(&event.aggregate_id) {
                return ConsumerOutcome::Success;
            }
            let mut failures = temporary_failures.lock().unwrap();
            if *failures > 0 {
                *failures -= 1;
                return ConsumerOutcome::Temporary("temporary failure".to_string());
            }
            effects.push(event.aggregate_id.clone());
            ConsumerOutcome::Success
        })
    }
}

struct PanicConsumer;

impl EventConsumer for PanicConsumer {
    fn handle(&self, _event: Arc<outbox::OutboxEvent>) -> Pin<Box<dyn std::future::Future<Output = ConsumerOutcome> + Send>> {
        Box::pin(async { panic!("consumer panic") })
    }
}

struct PermanentConsumer;

impl EventConsumer for PermanentConsumer {
    fn handle(&self, _event: Arc<outbox::OutboxEvent>) -> Pin<Box<dyn std::future::Future<Output = ConsumerOutcome> + Send>> {
        Box::pin(async { ConsumerOutcome::Permanent("permanent failure".to_string()) })
    }
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn worker_processes_pending_event() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let id = outbox::insert(&state.pool, &outbox::NewOutboxEvent::new("test.success", "aggregate", "one", "{}"))
        .await
        .unwrap();

    let consumer = TestConsumer::new();
    let effects = Arc::clone(&consumer.effects);
    let worker = opaline_backend::outbox_worker::OutboxWorker::new(state.pool.clone(), OutboxWorkerConfig::default(), consumer);
    worker.process_once().await.unwrap();

    let event = outbox::get_event(&state.pool, &id).await.unwrap().unwrap();
    assert_eq!(event.status, OutboxStatus::Processed);
    assert_eq!(effects.lock().unwrap().as_slice(), ["one"]);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn worker_retries_temporary_failure() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let consumer = TestConsumer::new();
    *consumer.temporary_failures.lock().unwrap() = 1;
    let effects = Arc::clone(&consumer.effects);

    let id = outbox::insert(&state.pool, &outbox::NewOutboxEvent::new("test.retry", "aggregate", "retry-one", "{}"))
        .await
        .unwrap();

    let worker = opaline_backend::outbox_worker::OutboxWorker::new(
        state.pool.clone(),
        OutboxWorkerConfig {
            retry_base: Duration::from_secs(0),
            retry_max: Duration::from_secs(0),
            ..OutboxWorkerConfig::default()
        },
        consumer,
    );

    worker.process_once().await.unwrap();
    let after_first = outbox::get_event(&state.pool, &id).await.unwrap().unwrap();
    assert_eq!(after_first.status, OutboxStatus::Pending);
    assert_eq!(after_first.attempts, 1);
    assert!(after_first.last_error.is_some());

    worker.process_once().await.unwrap();
    let after_second = outbox::get_event(&state.pool, &id).await.unwrap().unwrap();
    assert_eq!(after_second.status, OutboxStatus::Processed);
    assert_eq!(effects.lock().unwrap().as_slice(), ["retry-one"]);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn worker_marks_permanent_failure() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let id = outbox::insert(&state.pool, &outbox::NewOutboxEvent::new("test.permanent", "aggregate", "permanent-one", "{}"))
        .await
        .unwrap();

    let worker = opaline_backend::outbox_worker::OutboxWorker::new(state.pool.clone(), OutboxWorkerConfig::default(), PermanentConsumer);
    worker.process_once().await.unwrap();

    let event = outbox::get_event(&state.pool, &id).await.unwrap().unwrap();
    assert_eq!(event.status, OutboxStatus::Failed);
    assert!(event.last_error.as_deref() == Some("permanent failure"));
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn worker_handles_consumer_panic() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let id = outbox::insert(&state.pool, &outbox::NewOutboxEvent::new("test.panic", "aggregate", "panic-one", "{}"))
        .await
        .unwrap();

    let worker = opaline_backend::outbox_worker::OutboxWorker::new(state.pool.clone(), OutboxWorkerConfig::default(), PanicConsumer);
    worker.process_once().await.unwrap();

    let event = outbox::get_event(&state.pool, &id).await.unwrap().unwrap();
    assert_eq!(event.status, OutboxStatus::Failed);
    assert!(event.last_error.as_deref().unwrap_or("").contains("panicked"));
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn worker_idempotency_does_not_duplicate_effect() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let consumer = TestConsumer::new();
    let effects = Arc::clone(&consumer.effects);
    let id = outbox::insert(&state.pool, &outbox::NewOutboxEvent::new("test.idempotent", "aggregate", "effect-one", "{}"))
        .await
        .unwrap();

    let worker = opaline_backend::outbox_worker::OutboxWorker::new(state.pool.clone(), OutboxWorkerConfig::default(), consumer);

    // Simulate an effect that completed but whose confirmation did not persist.
    effects.lock().unwrap().push("effect-one".to_string());
    let claim = outbox::claim_pending(&state.pool, 10, 60).await.unwrap().into_iter().find(|e| e.id == id).unwrap().claim_token.unwrap();
    outbox::record_failure(&state.pool, &id, &claim, "confirmation failed", std::time::Duration::from_secs(0), 3)
        .await
        .unwrap();

    worker.process_once().await.unwrap();

    let event = outbox::get_event(&state.pool, &id).await.unwrap().unwrap();
    assert_eq!(event.status, OutboxStatus::Processed);
    assert_eq!(effects.lock().unwrap().len(), 1);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn worker_shutdown_is_graceful() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let (tx, rx) = watch::channel(false);
    let worker = opaline_backend::outbox_worker::OutboxWorker::new(
        state.pool.clone(),
        OutboxWorkerConfig {
            poll_interval: Duration::from_millis(10),
            ..OutboxWorkerConfig::default()
        },
        TestConsumer::new(),
    );

    let handle = tokio::spawn(async move { worker.run(rx).await });
    tokio::time::sleep(Duration::from_millis(25)).await;
    tx.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(2), handle).await.expect("worker should stop").expect("worker task should join");
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn worker_stale_claim_cannot_overwrite_new_claim() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    sqlx::query("DELETE FROM outbox_events").execute(&state.pool).await.unwrap();

    let id = outbox::insert(&state.pool, &outbox::NewOutboxEvent::new("test.stale", "aggregate", "stale-one", "{}"))
        .await
        .unwrap();

    let first_claim = outbox::claim_pending(&state.pool, 10, 60).await.unwrap().into_iter().find(|e| e.id == id).unwrap().claim_token.unwrap();

    sqlx::query("UPDATE outbox_events SET locked_at = DATE_SUB(NOW(), INTERVAL 10 MINUTE), claim_token = 'new-claim-token' WHERE id = ?")
        .bind(&id)
        .execute(&state.pool)
        .await
        .unwrap();

    let updated = outbox::mark_processed(&state.pool, &id, &first_claim).await.unwrap();
    assert!(!updated, "stale claim must not mark the event as processed");

    let status: String = sqlx::query_scalar("SELECT status FROM outbox_events WHERE id = ?")
        .bind(&id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(status, "processing");
}