use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use chrono::{NaiveDateTime, Utc};
use crate::outbox::{self, OutboxEvent, OutboxStatus};
use sqlx::MySqlPool;
use tokio::sync::watch;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct OutboxWorkerConfig {
    pub poll_interval: Duration,
    pub batch_size: i64,
    pub lock_timeout_secs: i64,
    pub max_attempts: i32,
    pub retry_base: Duration,
    pub retry_max: Duration,
}

impl Default for OutboxWorkerConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(5),
            batch_size: 10,
            lock_timeout_secs: 300,
            max_attempts: 5,
            retry_base: Duration::from_secs(1),
            retry_max: Duration::from_secs(300),
        }
    }
}

impl OutboxWorkerConfig {
    pub fn from_env() -> Self {
        let mut config = Self::default();
        if let Ok(value) = std::env::var("OPALINE_OUTBOX_POLL_MS") {
            if let Ok(ms) = value.parse::<u64>() {
                config.poll_interval = Duration::from_millis(ms.max(100));
            }
        }
        if let Ok(value) = std::env::var("OPALINE_OUTBOX_BATCH_SIZE") {
            if let Ok(size) = value.parse::<i64>() {
                if size > 0 {
                    config.batch_size = size;
                }
            }
        }
        if let Ok(value) = std::env::var("OPALINE_OUTBOX_LOCK_TIMEOUT_SECS") {
            if let Ok(seconds) = value.parse::<i64>() {
                if seconds > 0 {
                    config.lock_timeout_secs = seconds;
                }
            }
        }
        if let Ok(value) = std::env::var("OPALINE_OUTBOX_MAX_ATTEMPTS") {
            if let Ok(attempts) = value.parse::<i32>() {
                if attempts > 0 {
                    config.max_attempts = attempts;
                }
            }
        }
        config
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsumerOutcome {
    Success,
    Temporary(String),
    Permanent(String),
}

pub trait EventConsumer: Send + Sync {
    fn handle(&self, event: Arc<OutboxEvent>) -> Pin<Box<dyn Future<Output = ConsumerOutcome> + Send>>;
}

pub struct OutboxWorker<C> {
    pool: MySqlPool,
    config: OutboxWorkerConfig,
    consumer: Arc<C>,
}

impl<C> OutboxWorker<C>
where
    C: EventConsumer + 'static,
{
    pub fn new(pool: MySqlPool, config: OutboxWorkerConfig, consumer: C) -> Self {
        Self {
            pool,
            config,
            consumer: Arc::new(consumer),
        }
    }

    pub async fn run(self, mut shutdown: watch::Receiver<bool>) {
        loop {
            if *shutdown.borrow() {
                break;
            }

            if let Err(error) = self.process_once().await {
                warn!(error = %error, "outbox worker batch failed");
            }

            tokio::select! {
                _ = tokio::time::sleep(self.config.poll_interval) => {}
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        break;
                    }
                }
            }
        }
    }

    pub async fn process_once(&self) -> Result<(), sqlx::Error> {
        let events = outbox::claim_pending(&self.pool, self.config.batch_size, self.config.lock_timeout_secs).await?;
        for event in events {
            let claim_token = match event.claim_token.clone() {
                Some(token) => token,
                None => {
                    warn!(event_id = %event.id, "claimed outbox event has no claim token");
                    continue;
                }
            };

            info!(
                event_id = %event.id,
                event_type = %event.event_type,
                attempt = event.attempts,
                "processing outbox event"
            );

            let handle = tokio::spawn({
                let consumer = Arc::clone(&self.consumer);
                let event = Arc::new(event.clone());
                async move { consumer.handle(event).await }
            });

            let outcome = match handle.await {
                Ok(outcome) => outcome,
                Err(join_error) if join_error.is_panic() => {
                    ConsumerOutcome::Permanent("consumer panicked".to_string())
                }
                Err(_) => ConsumerOutcome::Temporary("consumer task cancelled".to_string()),
            };

            match outcome {
                ConsumerOutcome::Success => {
                    let updated = outbox::mark_processed(&self.pool, &event.id, &claim_token).await?;
                    if !updated {
                        warn!(event_id = %event.id, "outbox event was not marked processed because its claim is stale");
                    }
                }
                ConsumerOutcome::Temporary(error) => {
                    let delay = outbox::retry_delay(event.attempts as u32, self.config.retry_base, self.config.retry_max);
                    outbox::record_failure(&self.pool, &event.id, &claim_token, &error, delay, self.config.max_attempts).await?;
                }
                ConsumerOutcome::Permanent(error) => {
                    outbox::record_failure(&self.pool, &event.id, &claim_token, &error, Duration::from_secs(0), 1).await?;
                }
            }
        }

        let _ = outbox::release_expired_locks(&self.pool, self.config.lock_timeout_secs).await;
        Ok(())
    }
}

pub fn spawn_worker<C>(
    pool: MySqlPool,
    config: OutboxWorkerConfig,
    consumer: C,
    mut shutdown: watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()>
where
    C: EventConsumer + 'static,
{
    tokio::spawn(async move {
        OutboxWorker::new(pool, config, consumer).run(shutdown).await;
    })
}

pub struct NoopConsumer;

impl EventConsumer for NoopConsumer {
    fn handle(&self, _event: Arc<OutboxEvent>) -> Pin<Box<dyn Future<Output = ConsumerOutcome> + Send>> {
        Box::pin(async { ConsumerOutcome::Permanent("no consumer configured".to_string()) })
    }
}