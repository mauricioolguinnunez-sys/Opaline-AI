use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::chat_operations;
use crate::outbox::{self, OutboxEvent, OutboxStatus};
use crate::outbox_worker::ConsumerOutcome;
use mongodb::Database;
use sqlx::MySqlPool;

pub struct MongoChatPersistConsumer {
    pub pool: MySqlPool,
    pub mongo: Database,
}

impl crate::outbox_worker::EventConsumer for MongoChatPersistConsumer {
    fn handle(&self, event: Arc<OutboxEvent>) -> Pin<Box<dyn Future<Output = ConsumerOutcome> + Send>> {
        let pool = self.pool.clone();
        let mongo = self.mongo.clone();
        Box::pin(async move {
            if event.event_type != "chat.operation.persist" {
                return ConsumerOutcome::Permanent("unexpected event type".to_string());
            }
            let operation_id = match serde_json::from_str::<serde_json::Value>(&event.payload) {
                Ok(v) => v.get("operation_id").and_then(|x| x.as_str()).map(|s| s.to_string()),
                Err(_) => None,
            };
            let operation_id = match operation_id {
                Some(id) => id,
                None => return ConsumerOutcome::Permanent("missing operation_id".to_string()),
            };
            match chat_operations::persist_operation(&pool, &mongo, &operation_id).await {
                Ok(()) => ConsumerOutcome::Success,
                Err(_) => ConsumerOutcome::Temporary("chat persistence failed".to_string()),
            }
        })
    }
}

// Helper to enqueue persistence for a pending operation.
pub async fn enqueue_persist(pool: &MySqlPool, operation_id: &str) -> Result<String, sqlx::Error> {
    let payload = serde_json::json!({"operation_id": operation_id}).to_string();
    outbox::insert(pool, &outbox::NewOutboxEvent {
        id: None,
        event_type: "chat.operation.persist".to_string(),
        aggregate_type: "chat_operation".to_string(),
        aggregate_id: operation_id.to_string(),
        payload,
        correlation_id: None,
        available_at: None,
    }).await
}