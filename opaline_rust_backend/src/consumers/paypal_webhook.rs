use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::outbox::{OutboxEvent, OutboxStatus};
use crate::outbox_worker::{ConsumerOutcome, EventConsumer};
use crate::routes::payments::process_payment_webhook_event;
use sqlx::MySqlPool;

pub struct PayPalWebhookConsumer {
    pub pool: MySqlPool,
}

impl EventConsumer for PayPalWebhookConsumer {
    fn handle(&self, event: Arc<OutboxEvent>) -> Pin<Box<dyn Future<Output = ConsumerOutcome> + Send>> {
        let pool = self.pool.clone();
        Box::pin(async move {
            if event.event_type != "payment.webhook.received" {
                return ConsumerOutcome::Permanent(format!("unexpected event type: {}", event.event_type));
            }
            let payload: serde_json::Value = match serde_json::from_str(&event.payload) {
                Ok(value) => value,
                Err(error) => return ConsumerOutcome::Permanent(format!("invalid payload: {error}")),
            };
            let webhook_event_id = match payload.get("webhook_event_id").and_then(|v| v.as_str()) {
                Some(id) => id.to_string(),
                None => return ConsumerOutcome::Permanent("missing webhook_event_id".to_string()),
            };

            // Processing does not call PayPal again; it applies only local, verified
            // state transitions. The webhook was already verified before persistence.
            let client = match crate::paypal::PayPalClient::from_env() {
                Ok(client) => client,
                Err(_) => return ConsumerOutcome::Permanent("paypal not configured".to_string()),
            };
            match process_payment_webhook_event(&pool, &client, &webhook_event_id).await {
                Ok(()) => ConsumerOutcome::Success,
                Err(_) => ConsumerOutcome::Temporary("webhook processing failed".to_string()),
            }
        })
    }
}