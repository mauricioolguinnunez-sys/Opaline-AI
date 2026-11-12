use std::pin::Pin;
use std::future::Future;

use mongodb::bson::doc;
use futures::stream::TryStreamExt;
use mongodb::Database;
use serde_json::Value;

use crate::outbox::{OutboxEvent, OutboxStatus};
use crate::outbox_worker::{ConsumerOutcome, EventConsumer};

#[derive(Clone)]
pub struct MongoWallAuditConsumer {
    mongo: Database,
}

impl MongoWallAuditConsumer {
    pub fn new(mongo: Database) -> Self {
        Self { mongo }
    }

    pub async fn verify_event_id_index(&self) -> Result<(), String> {
        let collection = self.mongo.collection::<mongodb::bson::Document>("wall_events");
        let mut cursor = collection
            .list_indexes()
            .await
            .map_err(|error| format!("unable to list wall_events indexes: {error}"))?;

        while let Some(index) = cursor.try_next().await.map_err(|error| format!("unable to read wall_events indexes: {error}"))? {
            let is_event_id_index = index.keys.get("event_id") == Some(&mongodb::bson::Bson::Int32(1))
                && index.keys.len() == 1;
            let is_unique = index.options.as_ref().and_then(|options| options.unique).unwrap_or(false);
            let has_partial_filter = index
                .options
                .as_ref()
                .and_then(|options| options.partial_filter_expression.as_ref())
                .map(|filter| filter.get("event_id").and_then(|value| value.as_document()).and_then(|value| value.get("$exists")).and_then(|value| value.as_bool()) == Some(true))
                .unwrap_or(false);

            if is_event_id_index && is_unique && has_partial_filter {
                return Ok(());
            }
        }

        Err("wall_events does not have a verified unique partial index on event_id".to_string())
    }

    fn required_string(payload: &Value, key: &str) -> Option<String> {
        payload.get(key).and_then(Value::as_str).map(str::to_string)
    }

    fn event_supported(event_type: &str) -> bool {
        matches!(
            event_type,
            "wall.post.created"
                | "wall.post.updated"
                | "wall.post.deleted"
                | "wall.post.reported"
                | "wall.post.auto_hidden"
                | "wall.post.moderated"
        )
    }
}

impl EventConsumer for MongoWallAuditConsumer {
    fn handle(&self, event: std::sync::Arc<OutboxEvent>) -> Pin<Box<dyn Future<Output = ConsumerOutcome> + Send>> {
        let mongo = self.mongo.clone();
        Box::pin(async move {
            if !Self::event_supported(&event.event_type) {
                return ConsumerOutcome::Permanent(format!("unsupported event type: {}", event.event_type));
            }

            let payload: Value = match serde_json::from_str(&event.payload) {
                Ok(value) => value,
                Err(error) => return ConsumerOutcome::Permanent(format!("invalid payload: {error}")),
            };

            let actor_id = Self::required_string(&payload, "actor_user_id")
                .or_else(|| Self::required_string(&payload, "user_id"))
                .or_else(|| Self::required_string(&payload, "reporter_user_id"));

            if actor_id.is_none() {
                return ConsumerOutcome::Permanent("missing actor id".to_string());
            }

            let collection = mongo.collection::<mongodb::bson::Document>("wall_events");

            match collection.find_one(doc! { "event_id": &event.id }).await {
                Ok(Some(_)) => return ConsumerOutcome::Success,
                Ok(None) => {}
                Err(error) => return ConsumerOutcome::Temporary(error.to_string()),
            }

            let created_at = event
                .created_at
                .map(|value| mongodb::bson::DateTime::from_chrono(value.and_utc()))
                .unwrap_or_else(mongodb::bson::DateTime::now);

            let document = doc! {
                "event_id": &event.id,
                "event_type": &event.event_type,
                "aggregate_type": &event.aggregate_type,
                "aggregate_id": &event.aggregate_id,
                "actor_id": actor_id,
                "correlation_id": &event.correlation_id,
                "schema_version": 1i32,
                "created_at": created_at,
                "payload": &event.payload,
            };

            match collection.insert_one(document).await {
                Ok(_) => ConsumerOutcome::Success,
                Err(error) => {
                    let message = error.to_string();
                    if message.contains("E11000") || message.to_lowercase().contains("duplicate key") {
                        // The unique index is the authoritative protection. A duplicate
                        // must correspond to the same event_id; otherwise it is not safe
                        // to treat it as already processed.
                        match collection.find_one(doc! { "event_id": &event.id }).await {
                            Ok(Some(_)) => ConsumerOutcome::Success,
                            Ok(None) => ConsumerOutcome::Temporary(format!(
                                "duplicate key reported but matching event_id was not found: {message}"
                            )),
                            Err(lookup_error) => ConsumerOutcome::Temporary(lookup_error.to_string()),
                        }
                    } else {
                        ConsumerOutcome::Temporary(message)
                    }
                }
            }
        })
    }
}