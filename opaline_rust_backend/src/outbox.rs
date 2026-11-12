use chrono::{DateTime, NaiveDateTime, Utc};
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum OutboxStatus {
    Pending,
    Processing,
    Processed,
    Failed,
}

impl OutboxStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Processing => "processing",
            Self::Processed => "processed",
            Self::Failed => "failed",
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "processing" => Self::Processing,
            "processed" => Self::Processed,
            "failed" => Self::Failed,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NewOutboxEvent {
    pub id: Option<String>,
    pub event_type: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub payload: String,
    pub correlation_id: Option<String>,
    pub available_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct OutboxEvent {
    pub id: String,
    pub event_type: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub payload: String,
    pub status: OutboxStatus,
    pub attempts: i32,
    pub created_at: Option<NaiveDateTime>,
    pub available_at: Option<NaiveDateTime>,
    pub processed_at: Option<NaiveDateTime>,
    pub last_error: Option<String>,
    pub locked_at: Option<NaiveDateTime>,
    pub correlation_id: Option<String>,
    pub claim_token: Option<String>,
}

impl NewOutboxEvent {
    pub fn new(event_type: impl Into<String>, aggregate_type: impl Into<String>, aggregate_id: impl Into<String>, payload: impl Into<String>) -> Self {
        Self {
            id: None,
            event_type: event_type.into(),
            aggregate_type: aggregate_type.into(),
            aggregate_id: aggregate_id.into(),
            payload: payload.into(),
            correlation_id: None,
            available_at: None,
        }
    }
}

fn row_to_event(row: &sqlx::mysql::MySqlRow) -> OutboxEvent {
    OutboxEvent {
        id: row.get("id"),
        event_type: row.get("event_type"),
        aggregate_type: row.get("aggregate_type"),
        aggregate_id: row.get("aggregate_id"),
        payload: row.get("payload"),
        status: OutboxStatus::from_str(&row.get::<String, _>("status")),
        attempts: row.get("attempts"),
        created_at: row.try_get("created_at").ok(),
        available_at: row.try_get("available_at").ok(),
        processed_at: row.try_get("processed_at").ok(),
        last_error: row.try_get("last_error").ok(),
        locked_at: row.try_get("locked_at").ok(),
        correlation_id: row.try_get("correlation_id").ok(),
        claim_token: row.try_get("claim_token").ok(),
    }
}

pub async fn insert<'e, E>(executor: E, event: &NewOutboxEvent) -> Result<String, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::MySql>,
{
    let id = event.id.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
    let available_at = event.available_at.unwrap_or_else(|| Utc::now().naive_utc());
    sqlx::query(
        "INSERT INTO outbox_events (id, event_type, aggregate_type, aggregate_id, payload, status, attempts, available_at, correlation_id) VALUES (?,?,?,?,?,'pending',0,?,?)",
    )
    .bind(&id)
    .bind(&event.event_type)
    .bind(&event.aggregate_type)
    .bind(&event.aggregate_id)
    .bind(&event.payload)
    .bind(available_at)
    .bind(&event.correlation_id)
    .execute(executor)
    .await?;
    Ok(id)
}

pub async fn list_pending(pool: &sqlx::MySqlPool, limit: i64) -> Result<Vec<OutboxEvent>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, event_type, aggregate_type, aggregate_id, CAST(payload AS CHAR) AS payload, status, attempts, created_at, available_at, processed_at, last_error, locked_at, correlation_id FROM outbox_events WHERE status='pending' AND available_at <= UTC_TIMESTAMP() ORDER BY created_at ASC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(row_to_event).collect())
}

pub async fn list_events(
    pool: &sqlx::MySqlPool,
    event_type_prefix: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<OutboxEvent>, sqlx::Error> {
    let pattern = format!("{}%", event_type_prefix);
    let rows = sqlx::query(
        "SELECT id, event_type, aggregate_type, aggregate_id, CAST(payload AS CHAR) AS payload, status, attempts, created_at, available_at, processed_at, last_error, locked_at, correlation_id, claim_token FROM outbox_events WHERE event_type LIKE ? ORDER BY created_at ASC, id ASC LIMIT ? OFFSET ?",
    )
    .bind(pattern)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(row_to_event).collect())
}

pub async fn list_events_filtered(
    pool: &sqlx::MySqlPool,
    event_type_prefix: &str,
    status: Option<&str>,
    event_type: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<OutboxEvent>, sqlx::Error> {
    let pattern = format!("{}%", event_type_prefix);
    let rows = sqlx::query(
        "SELECT id, event_type, aggregate_type, aggregate_id, CAST(payload AS CHAR) AS payload, status, attempts, created_at, available_at, processed_at, last_error, locked_at, correlation_id, claim_token FROM outbox_events WHERE event_type LIKE ? AND (? IS NULL OR status=?) AND (? IS NULL OR event_type=?) ORDER BY created_at ASC, id ASC LIMIT ? OFFSET ?",
    )
    .bind(pattern)
    .bind(status)
    .bind(status)
    .bind(event_type)
    .bind(event_type)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(row_to_event).collect())
}

pub async fn get_event(pool: &sqlx::MySqlPool, id: &str) -> Result<Option<OutboxEvent>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, event_type, aggregate_type, aggregate_id, CAST(payload AS CHAR) AS payload, status, attempts, created_at, available_at, processed_at, last_error, locked_at, correlation_id FROM outbox_events WHERE id=?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.as_ref().map(row_to_event))
}

pub async fn claim_pending(
    pool: &sqlx::MySqlPool,
    limit: i64,
    lock_timeout_secs: i64,
) -> Result<Vec<OutboxEvent>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let rows = sqlx::query(
        "SELECT id, event_type, aggregate_type, aggregate_id, CAST(payload AS CHAR) AS payload, status, attempts, created_at, available_at, processed_at, last_error, locked_at, correlation_id FROM outbox_events WHERE status='pending' AND available_at <= UTC_TIMESTAMP() ORDER BY created_at ASC LIMIT ? FOR UPDATE SKIP LOCKED",
    )
    .bind(limit)
    .fetch_all(&mut *tx)
    .await?;

    if rows.is_empty() {
        tx.commit().await?;
        return Ok(Vec::new());
    }

    let mut claimed = Vec::new();
    for row in rows {
        let event = row_to_event(&row);
        let attempts = event.attempts + 1;
        let claim_token = Uuid::new_v4().to_string();
        let result = sqlx::query(
            "UPDATE outbox_events SET status='processing', locked_at=UTC_TIMESTAMP(), claim_token=?, attempts=? WHERE id=? AND status='pending'",
        )
        .bind(&claim_token)
        .bind(attempts)
        .bind(&event.id)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            continue;
        }
        claimed.push(OutboxEvent {
            status: OutboxStatus::Processing,
            attempts,
            locked_at: Some(Utc::now().naive_utc()),
            claim_token: Some(claim_token),
            ..event
        });
    }

    tx.commit().await?;
    Ok(claimed)
}

pub async fn mark_processed(pool: &sqlx::MySqlPool, id: &str, claim_token: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE outbox_events SET status='processed', processed_at=UTC_TIMESTAMP(), locked_at=NULL, claim_token=NULL WHERE id=? AND status='processing' AND claim_token=?",
    )
    .bind(id)
    .bind(claim_token)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn record_failure(
    pool: &sqlx::MySqlPool,
    id: &str,
    claim_token: &str,
    error: &str,
    retry_delay: std::time::Duration,
    max_attempts: i32,
) -> Result<(), sqlx::Error> {
    let attempts: i32 = sqlx::query_scalar("SELECT attempts FROM outbox_events WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .unwrap_or(0);

    let status = if attempts >= max_attempts {
        OutboxStatus::Failed.as_str()
    } else {
        OutboxStatus::Pending.as_str()
    };

    let error = if error.chars().count() > 4000 {
        error.chars().take(4000).collect()
    } else {
        error.to_string()
    };

    let retry_seconds = retry_delay.as_secs() as i64;
    sqlx::query(
        "UPDATE outbox_events SET status=?, last_error=?, available_at=DATE_ADD(UTC_TIMESTAMP(), INTERVAL ? SECOND), locked_at=NULL, claim_token=NULL WHERE id=? AND claim_token=?",
    )
    .bind(status)
    .bind(error)
    .bind(retry_seconds)
    .bind(id)
    .bind(claim_token)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn claim_for_recovery(
    pool: &sqlx::MySqlPool,
    id: &str,
    lock_timeout_secs: i64,
    max_attempts: i32,
) -> Result<Option<(OutboxEvent, String)>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query(
        "SELECT id, event_type, aggregate_type, aggregate_id, CAST(payload AS CHAR) AS payload, status, attempts, created_at, available_at, processed_at, last_error, locked_at, correlation_id, claim_token FROM outbox_events WHERE id = ? FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(row) = row else {
        tx.commit().await?;
        return Ok(None);
    };

    let event = row_to_event(&row);
    let now = chrono::Utc::now().naive_utc();
    let eligible = match event.status {
        OutboxStatus::Pending => event.available_at.map(|available| available <= now).unwrap_or(true),
        OutboxStatus::Processing => event
            .locked_at
            .map(|locked| (now - locked).num_seconds() >= lock_timeout_secs)
            .unwrap_or(true),
        OutboxStatus::Failed => event.attempts < max_attempts,
        OutboxStatus::Processed => false,
    };

    if !eligible {
        tx.commit().await?;
        return Ok(None);
    }

    let claim_token = uuid::Uuid::new_v4().to_string();
    let attempts = event.attempts + 1;
    let result = sqlx::query(
        "UPDATE outbox_events SET status='processing', locked_at=UTC_TIMESTAMP(), claim_token=?, attempts=?, available_at=UTC_TIMESTAMP() WHERE id=? AND status=?",
    )
    .bind(&claim_token)
    .bind(attempts)
    .bind(id)
    .bind(event.status.as_str())
    .execute(&mut *tx)
    .await?;

    if result.rows_affected() != 1 {
        tx.commit().await?;
        return Ok(None);
    }

    tx.commit().await?;
    Ok(Some((
        OutboxEvent {
            status: OutboxStatus::Processing,
            attempts,
            locked_at: Some(now),
            claim_token: Some(claim_token.clone()),
            ..event
        },
        claim_token,
    )))
}

pub async fn release_expired_locks(pool: &sqlx::MySqlPool, lock_timeout_secs: i64) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE outbox_events SET status='pending', locked_at=NULL, claim_token=NULL, available_at=UTC_TIMESTAMP() WHERE status='processing' AND locked_at IS NOT NULL AND locked_at < (UTC_TIMESTAMP() - INTERVAL ? SECOND)",
    )
    .bind(lock_timeout_secs)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

pub fn retry_delay(attempt: u32, base: std::time::Duration, max: std::time::Duration) -> std::time::Duration {
    let exponent = attempt.saturating_sub(1);
    let multiplier = 2u32.saturating_pow(exponent);
    base.saturating_mul(multiplier).min(max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_delay_uses_exponential_backoff() {
        let base = std::time::Duration::from_secs(1);
        let max = std::time::Duration::from_secs(60);
        assert_eq!(retry_delay(1, base, max), std::time::Duration::from_secs(1));
        assert_eq!(retry_delay(2, base, max), std::time::Duration::from_secs(2));
        assert_eq!(retry_delay(3, base, max), std::time::Duration::from_secs(4));
        assert_eq!(retry_delay(10, base, max), max);
    }

    #[test]
    fn status_round_trips() {
        assert_eq!(OutboxStatus::from_str("pending"), OutboxStatus::Pending);
        assert_eq!(OutboxStatus::from_str("processing"), OutboxStatus::Processing);
        assert_eq!(OutboxStatus::from_str("processed"), OutboxStatus::Processed);
        assert_eq!(OutboxStatus::from_str("failed"), OutboxStatus::Failed);
        assert_eq!(OutboxStatus::from_str("unknown"), OutboxStatus::Pending);
    }
}