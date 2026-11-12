use std::time::Duration;

use chrono::{NaiveDateTime, Utc};
use futures::stream::TryStreamExt;
use mongodb::Database;
use crate::consumers::mongo_wall_audit::MongoWallAuditConsumer;
use crate::outbox::{self, OutboxEvent, OutboxStatus};
use crate::outbox_worker::{ConsumerOutcome, EventConsumer};
use sqlx::MySqlPool;
use tokio::sync::watch;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct RecoveryConfig {
    pub poll_interval: Duration,
    pub batch_size: i64,
    pub pending_age_threshold_secs: i64,
    pub processing_timeout_secs: i64,
    pub max_recoveries_per_cycle: usize,
    pub max_attempts: i32,
    pub retry_base: Duration,
    pub retry_max: Duration,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(60),
            batch_size: 100,
            pending_age_threshold_secs: 300,
            processing_timeout_secs: 300,
            max_recoveries_per_cycle: 10,
            max_attempts: 5,
            retry_base: Duration::from_secs(1),
            retry_max: Duration::from_secs(300),
        }
    }
}

impl RecoveryConfig {
    pub fn from_env() -> Self {
        let mut config = Self::default();
        if let Ok(value) = std::env::var("OPALINE_RECOVERY_POLL_SECS") {
            if let Ok(seconds) = value.parse::<u64>() {
                if seconds > 0 {
                    config.poll_interval = Duration::from_secs(seconds);
                }
            }
        }
        if let Ok(value) = std::env::var("OPALINE_RECOVERY_BATCH_SIZE") {
            if let Ok(size) = value.parse::<i64>() {
                if size > 0 {
                    config.batch_size = size;
                }
            }
        }
        if let Ok(value) = std::env::var("OPALINE_RECOVERY_MAX_PER_CYCLE") {
            if let Ok(limit) = value.parse::<usize>() {
                if limit > 0 {
                    config.max_recoveries_per_cycle = limit;
                }
            }
        }
        config
    }
}

#[derive(Debug, Clone)]
pub struct RecoverySkip {
    pub event_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct RecoveryReport {
    pub examined: usize,
    pub recovered: Vec<String>,
    pub skipped: Vec<RecoverySkip>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecoveryAction {
    Recover,
    ManualReview,
}

pub struct OutboxRecovery {
    pool: MySqlPool,
    mongo: Database,
    config: RecoveryConfig,
    consumer: MongoWallAuditConsumer,
}

impl OutboxRecovery {
    pub fn new(pool: MySqlPool, mongo: Database, config: RecoveryConfig) -> Self {
        let consumer = MongoWallAuditConsumer::new(mongo.clone());
        Self {
            pool,
            mongo,
            config,
            consumer,
        }
    }

    pub async fn verify_dependencies(&self) -> Result<(), String> {
        self.consumer.verify_event_id_index().await
    }

    pub async fn recover_once(&self) -> anyhow::Result<RecoveryReport> {
        self.verify_dependencies().await.map_err(anyhow::Error::msg)?;

        let mut report = RecoveryReport::default();
        let now = Utc::now().naive_utc();
        let mut offset = 0i64;
        let mut events = Vec::new();

        while events.len() < self.config.max_recoveries_per_cycle + 100 {
            let remaining = (self.config.max_recoveries_per_cycle + 100 - events.len()) as i64;
            let limit = self.config.batch_size.min(remaining);
            if limit <= 0 {
                break;
            }
            let batch = outbox::list_events(&self.pool, "wall.post.", limit, offset).await?;
            if batch.is_empty() {
                break;
            }
            events.extend(batch.clone());
            if batch.len() < limit as usize {
                break;
            }
            offset += limit;
        }

        let audit_ids: std::collections::HashSet<String> = self.fetch_audit_event_ids().await?.into_iter().collect();
        for audit_id in &audit_ids {
            if !events.iter().any(|event| event.id == *audit_id) {
                report.skipped.push(RecoverySkip {
                    event_id: audit_id.clone(),
                    reason: "historical_or_orphaned_document".to_string(),
                });
            }
        }

        report.examined = events.len();

        let mut recovered_count = 0usize;
        for event in events {
            if recovered_count >= self.config.max_recoveries_per_cycle {
                report.skipped.push(RecoverySkip {
                    event_id: event.id.clone(),
                    reason: "recovery limit reached".to_string(),
                });
                continue;
            }

            let has_document = audit_ids.contains(&event.id);
            let action = self.classify(&event, has_document, now);

            match action {
                RecoveryAction::Recover => {
                    match self.recover_event(&event).await {
                        Ok(true) => {
                            recovered_count += 1;
                            report.recovered.push(event.id.clone());
                        }
                        Ok(false) => {
                            report.skipped.push(RecoverySkip {
                                event_id: event.id.clone(),
                                reason: "claim not acquired or stale".to_string(),
                            });
                        }
                        Err(error) => {
                            report.skipped.push(RecoverySkip {
                                event_id: event.id.clone(),
                                reason: format!("recovery failed: {error}"),
                            });
                        }
                    }
                }
                RecoveryAction::ManualReview => {
                    report.skipped.push(RecoverySkip {
                        event_id: event.id.clone(),
                        reason: self.manual_reason(&event, has_document),
                    });
                }
            }
        }

        Ok(report)
    }

    fn classify(&self, event: &OutboxEvent, has_document: bool, now: NaiveDateTime) -> RecoveryAction {
        match event.status {
            outbox::OutboxStatus::Pending => {
                if has_document {
                    RecoveryAction::Recover
                } else if event.created_at.map(|created| (now - created).num_seconds() >= self.config.pending_age_threshold_secs).unwrap_or(true) {
                    // Pending events without an audit document are retried by the normal
                    // worker path; recovery does not create missing audits.
                    RecoveryAction::ManualReview
                } else {
                    RecoveryAction::ManualReview
                }
            }
            outbox::OutboxStatus::Processing => {
                let expired = event
                    .locked_at
                    .map(|locked| (now - locked).num_seconds() >= self.config.processing_timeout_secs)
                    .unwrap_or(true);
                if expired {
                    RecoveryAction::Recover
                } else {
                    RecoveryAction::ManualReview
                }
            }
            outbox::OutboxStatus::Failed => {
                if has_document && is_recoverable_failure(event.last_error.as_deref()) && event.attempts < self.config.max_attempts {
                    RecoveryAction::Recover
                } else {
                    RecoveryAction::ManualReview
                }
            }
            outbox::OutboxStatus::Processed => RecoveryAction::ManualReview,
        }
    }

    fn manual_reason(&self, event: &OutboxEvent, has_document: bool) -> String {
        match event.status {
            outbox::OutboxStatus::Pending if !has_document => "pending without audit document requires worker retry".to_string(),
            outbox::OutboxStatus::Processing => "processing lock is still active".to_string(),
            outbox::OutboxStatus::Failed if !has_document => "failed without audit document requires manual review".to_string(),
            outbox::OutboxStatus::Failed => "failed event has no demonstrable recoverable cause".to_string(),
            outbox::OutboxStatus::Processed if !has_document => "processed event missing audit document requires manual review".to_string(),
            _ => "not eligible for automatic recovery".to_string(),
        }
    }

    async fn recover_event(&self, event: &OutboxEvent) -> Result<bool, String> {
        let claim = outbox::claim_for_recovery(
            &self.pool,
            &event.id,
            self.config.processing_timeout_secs,
            self.config.max_attempts,
        )
        .await
        .map_err(|error| error.to_string())?;

        let claim = match claim {
            Some(claim) => claim,
            None => return Ok(false),
        };

        let outcome = self.consumer.handle(std::sync::Arc::new(event.clone())).await;
        match outcome {
            crate::outbox_worker::ConsumerOutcome::Success => {
                let updated = outbox::mark_processed(&self.pool, &event.id, &claim.1)
                    .await
                    .map_err(|error| error.to_string())?;
                if !updated {
                    return Err("stale claim could not confirm recovery".to_string());
                }
                Ok(true)
            }
            crate::outbox_worker::ConsumerOutcome::Temporary(error) => {
                let delay = outbox::retry_delay(claim.0.attempts as u32, self.config.retry_base, self.config.retry_max);
                outbox::record_failure(
                    &self.pool,
                    &event.id,
                    &claim.1,
                    &error,
                    delay,
                    self.config.max_attempts,
                )
                .await
                .map_err(|error| error.to_string())?;
                Err(format!("temporary failure: {error}"))
            }
            crate::outbox_worker::ConsumerOutcome::Permanent(error) => {
                outbox::record_failure(
                    &self.pool,
                    &event.id,
                    &claim.1,
                    &error,
                    std::time::Duration::from_secs(0),
                    1,
                )
                .await
                .map_err(|error| error.to_string())?;
                Err(format!("permanent failure: {error}"))
            }
        }
    }

    async fn fetch_audit_event_ids(&self) -> Result<std::collections::HashSet<String>, mongodb::error::Error> {
        let collection = self.mongo.collection::<mongodb::bson::Document>("wall_events");
        let filter = mongodb::bson::doc! { "event_id": { "$exists": true, "$type": "string" } };
        let mut ids = std::collections::HashSet::new();
        let mut skip = 0i64;
        loop {
            let mut cursor = collection
                .find(filter.clone())
                .projection(mongodb::bson::doc! { "event_id": 1 })
                .sort(mongodb::bson::doc! { "_id": 1 })
                .skip(skip as u64)
                .limit(self.config.batch_size as i64)
                .await?;
            let mut batch_count = 0usize;
            while let Some(document) = cursor.try_next().await? {
                if let Some(id) = document.get_str("event_id").ok() {
                    ids.insert(id.to_string());
                }
                batch_count += 1;
            }
            if batch_count < self.config.batch_size as usize {
                break;
            }
            skip += self.config.batch_size;
            if ids.len() >= self.config.max_recoveries_per_cycle.saturating_mul(100) {
                break;
            }
        }
        Ok(ids)
    }

    pub async fn recover_event_by_id(&self, id: &str) -> Result<bool, String> {
        let event = outbox::get_event(&self.pool, id).await.map_err(|error| error.to_string())?;
        let event = event.ok_or_else(|| "event not found".to_string())?;
        if !event.event_type.starts_with("wall.post.") {
            return Err("event type is not supported by this recovery service".to_string());
        }

        let has_document = self
            .mongo
            .collection::<mongodb::bson::Document>("wall_events")
            .find_one(mongodb::bson::doc! { "event_id": &event.id })
            .await
            .map_err(|error| error.to_string())?
            .is_some();

        let now = chrono::Utc::now().naive_utc();
        let eligible = match event.status {
            outbox::OutboxStatus::Pending => has_document,
            outbox::OutboxStatus::Processing => event
                .locked_at
                .map(|locked| (now - locked).num_seconds() >= self.config.processing_timeout_secs)
                .unwrap_or(true),
            outbox::OutboxStatus::Failed => {
                is_recoverable_failure(event.last_error.as_deref())
                    && event.attempts < self.config.max_attempts
            }
            outbox::OutboxStatus::Processed => false,
        };

        if !eligible {
            return Ok(false);
        }

        self.recover_event(&event).await
    }

    pub async fn run(&self, mut shutdown: watch::Receiver<bool>) {
        loop {
            if *shutdown.borrow() {
                break;
            }

            match self.recover_once().await {
                Ok(report) => {
                    if report.recovered.is_empty() && report.skipped.is_empty() {
                        info!(examined = report.examined, "outbox recovery cycle completed without discrepancies");
                    } else {
                        info!(
                            examined = report.examined,
                            recovered = report.recovered.len(),
                            skipped = report.skipped.len(),
                            "outbox recovery cycle completed"
                        );
                    }
                }
                Err(error) => {
                    warn!(error = %error, "outbox recovery cycle failed");
                }
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
}

pub fn spawn_reconciler(
    pool: MySqlPool,
    mongo: Database,
    config: RecoveryConfig,
    mut shutdown: watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let reconciler = OutboxRecovery::new(pool, mongo, config);
        if let Err(error) = reconciler.verify_dependencies().await {
            warn!(error = %error, "outbox recovery not activated");
            return;
        }
        reconciler.run(shutdown).await;
    })
}

pub fn spawn_recovery(
    pool: MySqlPool,
    mongo: Database,
    config: RecoveryConfig,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let recovery = OutboxRecovery::new(pool, mongo, config);
        if let Err(error) = recovery.verify_dependencies().await {
            warn!(error = %error, "outbox recovery not activated");
            return;
        }
        recovery.run(shutdown).await;
    })
}

pub fn is_recoverable_failure(last_error: Option<&str>) -> bool {
    let Some(error) = last_error else {
        return false;
    };
    let error = error.to_lowercase();
    error.contains("confirmation")
        || error.contains("claim")
        || error.contains("stale")
        || error.contains("fencing")
        || error.contains("timeout")
        || error.contains("connection")
        || error.contains("worker")
        || error.contains("interrupt")
        || error.contains("shutdown")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recoverable_failures_are_explicitly_classified() {
        assert!(is_recoverable_failure(Some("confirmation failed")));
        assert!(is_recoverable_failure(Some("stale claim")));
        assert!(!is_recoverable_failure(Some("invalid payload")));
        assert!(!is_recoverable_failure(None));
    }
}