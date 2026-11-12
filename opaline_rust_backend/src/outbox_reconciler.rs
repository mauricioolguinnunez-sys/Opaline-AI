use std::time::Duration;

use chrono::{NaiveDateTime, Utc};
use futures::stream::TryStreamExt;
use mongodb::bson::doc;
use mongodb::Database;
use crate::consumers::mongo_wall_audit::MongoWallAuditConsumer;
use crate::outbox::{self, OutboxEvent};
use sqlx::MySqlPool;
use tokio::sync::watch;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct ReconcilerConfig {
    pub poll_interval: Duration,
    pub batch_size: i64,
    pub pending_age_threshold_secs: i64,
    pub processing_timeout_secs: i64,
    pub max_events_per_cycle: usize,
}

impl Default for ReconcilerConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(60),
            batch_size: 100,
            pending_age_threshold_secs: 300,
            processing_timeout_secs: 300,
            max_events_per_cycle: 1000,
        }
    }
}

impl ReconcilerConfig {
    pub fn from_env() -> Self {
        let mut config = Self::default();
        if let Ok(value) = std::env::var("OPALINE_RECONCILER_POLL_SECS") {
            if let Ok(seconds) = value.parse::<u64>() {
                if seconds > 0 {
                    config.poll_interval = Duration::from_secs(seconds);
                }
            }
        }
        if let Ok(value) = std::env::var("OPALINE_RECONCILER_BATCH_SIZE") {
            if let Ok(size) = value.parse::<i64>() {
                if size > 0 {
                    config.batch_size = size;
                }
            }
        }
        if let Ok(value) = std::env::var("OPALINE_RECONCILER_PENDING_AGE_SECS") {
            if let Ok(seconds) = value.parse::<i64>() {
                if seconds >= 0 {
                    config.pending_age_threshold_secs = seconds;
                }
            }
        }
        if let Ok(value) = std::env::var("OPALINE_RECONCILER_PROCESSING_TIMEOUT_SECS") {
            if let Ok(seconds) = value.parse::<i64>() {
                if seconds >= 0 {
                    config.processing_timeout_secs = seconds;
                }
            }
        }
        if let Ok(value) = std::env::var("OPALINE_RECONCILER_MAX_EVENTS") {
            if let Ok(limit) = value.parse::<usize>() {
                if limit > 0 {
                    config.max_events_per_cycle = limit;
                }
            }
        }
        config
    }
}

#[derive(Debug, Clone, Default)]
pub struct ReconciliationReport {
    pub examined: usize,
    pub pending_old_without_document: Vec<String>,
    pub pending_old_with_document: Vec<String>,
    pub processing_expired_without_document: Vec<String>,
    pub processing_expired_with_document: Vec<String>,
    pub failed_without_document: Vec<String>,
    pub failed_with_document: Vec<String>,
    pub processed_missing_document: Vec<String>,
    pub audit_exists_non_processed: Vec<String>,
    pub audit_without_outbox: Vec<String>,
}

impl ReconciliationReport {
    pub fn has_discrepancies(&self) -> bool {
        !self.pending_old_without_document.is_empty()
            || !self.pending_old_with_document.is_empty()
            || !self.processing_expired_without_document.is_empty()
            || !self.processing_expired_with_document.is_empty()
            || !self.failed_without_document.is_empty()
            || !self.failed_with_document.is_empty()
            || !self.processed_missing_document.is_empty()
            || !self.audit_exists_non_processed.is_empty()
            || !self.audit_without_outbox.is_empty()
    }
}

pub struct OutboxReconciler {
    pool: MySqlPool,
    mongo: Database,
    config: ReconcilerConfig,
    consumer: MongoWallAuditConsumer,
}

impl OutboxReconciler {
    pub fn new(pool: MySqlPool, mongo: Database, config: ReconcilerConfig) -> Self {
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

    pub async fn reconcile_once(&self) -> anyhow::Result<ReconciliationReport> {
        self.verify_dependencies().await.map_err(anyhow::Error::msg)?;

        let now = Utc::now().naive_utc();
        let mut report = ReconciliationReport::default();
        let mut examined = 0usize;
        let mut offset = 0i64;

        let mut mysql_events = Vec::new();
        while examined < self.config.max_events_per_cycle {
            let remaining = (self.config.max_events_per_cycle - examined) as i64;
            let limit = self.config.batch_size.min(remaining);
            if limit <= 0 {
                break;
            }
            let batch = outbox::list_events(&self.pool, "wall.post.", limit, offset).await?;
            if batch.is_empty() {
                break;
            }
            let batch_len = batch.len();
            examined += batch_len;
            mysql_events.extend(batch);
            if batch_len < limit as usize {
                break;
            }
            offset += limit;
        }

        let audit_ids = self.fetch_audit_event_ids().await?;
        let mysql_ids: std::collections::HashSet<String> = mysql_events.iter().map(|event| event.id.clone()).collect();

        for event in &mysql_events {
            let has_document = audit_ids.contains(&event.id);
            match event.status {
                outbox::OutboxStatus::Pending => {
                    if event.created_at.map(|created| (now - created).num_seconds() >= self.config.pending_age_threshold_secs).unwrap_or(true) {
                        if has_document {
                            report.pending_old_with_document.push(event.id.clone());
                        } else {
                            report.pending_old_without_document.push(event.id.clone());
                        }
                    }
                }
                outbox::OutboxStatus::Processing => {
                    let expired = event
                        .locked_at
                        .map(|locked| (now - locked).num_seconds() >= self.config.processing_timeout_secs)
                        .unwrap_or(true);
                    if expired {
                        if has_document {
                            report.processing_expired_with_document.push(event.id.clone());
                        } else {
                            report.processing_expired_without_document.push(event.id.clone());
                        }
                    }
                }
                outbox::OutboxStatus::Failed => {
                    if has_document {
                        report.failed_with_document.push(event.id.clone());
                    } else {
                        report.failed_without_document.push(event.id.clone());
                    }
                }
                outbox::OutboxStatus::Processed => {
                    if !has_document {
                        report.processed_missing_document.push(event.id.clone());
                    }
                }
            }
        }

        for audit_id in audit_ids {
            if !mysql_ids.contains(&audit_id) {
                report.audit_without_outbox.push(audit_id.clone());
            } else if let Some(event) = mysql_events.iter().find(|event| event.id == audit_id) {
                if event.status != outbox::OutboxStatus::Processed {
                    report.audit_exists_non_processed.push(audit_id);
                }
            }
        }

        report.examined = mysql_events.len();
        if report.has_discrepancies() {
            for id in &report.pending_old_without_document {
                warn!(event_id = %id, discrepancy = "pending_old_without_document", "outbox reconciliation discrepancy");
            }
            for id in &report.pending_old_with_document {
                warn!(event_id = %id, discrepancy = "pending_old_with_document", "outbox reconciliation discrepancy");
            }
            for id in &report.processing_expired_without_document {
                warn!(event_id = %id, discrepancy = "processing_expired_without_document", "outbox reconciliation discrepancy");
            }
            for id in &report.processing_expired_with_document {
                warn!(event_id = %id, discrepancy = "processing_expired_with_document", "outbox reconciliation discrepancy");
            }
            for id in &report.failed_without_document {
                warn!(event_id = %id, discrepancy = "failed_without_document", "outbox reconciliation discrepancy");
            }
            for id in &report.failed_with_document {
                warn!(event_id = %id, discrepancy = "failed_with_document", "outbox reconciliation discrepancy");
            }
            for id in &report.processed_missing_document {
                warn!(event_id = %id, discrepancy = "processed_missing_document", "outbox reconciliation discrepancy");
            }
            for id in &report.audit_exists_non_processed {
                warn!(event_id = %id, discrepancy = "audit_exists_non_processed", "outbox reconciliation discrepancy");
            }
            for id in &report.audit_without_outbox {
                warn!(event_id = %id, discrepancy = "audit_without_outbox", "outbox reconciliation discrepancy");
            }
        } else {
            info!(examined = report.examined, "outbox reconciliation completed without discrepancies");
        }

        Ok(report)
    }

    async fn fetch_audit_event_ids(&self) -> Result<Vec<String>, mongodb::error::Error> {
        let collection = self.mongo.collection::<mongodb::bson::Document>("wall_events");
        let filter = mongodb::bson::doc! { "event_id": { "$exists": true, "$type": "string" } };
        let mut ids = Vec::new();
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
                    ids.push(id.to_string());
                }
                batch_count += 1;
            }
            if batch_count < self.config.batch_size as usize {
                break;
            }
            skip += self.config.batch_size;
            if ids.len() >= self.config.max_events_per_cycle {
                break;
            }
        }
        Ok(ids)
    }

    pub async fn run(&self, mut shutdown: watch::Receiver<bool>) {
        loop {
            if *shutdown.borrow() {
                break;
            }

            match self.reconcile_once().await {
                Ok(report) => {
                    info!(
                        examined = report.examined,
                        discrepancies = report.has_discrepancies(),
                        "outbox reconciliation cycle completed"
                    );
                }
                Err(error) => {
                    warn!(error = %error, "outbox reconciliation cycle failed");
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
    config: ReconcilerConfig,
    mut shutdown: watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let reconciler = OutboxReconciler::new(pool, mongo, config);
        if let Err(error) = reconciler.verify_dependencies().await {
            warn!(error = %error, "outbox reconciler not activated");
            return;
        }
        reconciler.run(shutdown).await;
    })
}