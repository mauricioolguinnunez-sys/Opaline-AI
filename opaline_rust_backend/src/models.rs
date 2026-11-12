use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct User {
    pub id: String,
    pub email: String,
    pub name: String,
    pub role: String,
    pub email_verified: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Plan {
    pub id: String,
    pub code: String,
    pub name: String,
    pub price: f64,
    pub currency: String,
    pub period: String,
    pub messages_per_month: i32,
    pub features: Vec<String>,
    pub active: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Post {
    pub id: String,
    pub user_id: String,
    pub alias: String,
    pub content: String,
    pub rating: Option<i32>,
    pub status: String,
    pub created_at: String,
    pub plan: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct PlanVersion {
    pub id: String,
    pub plan_id: String,
    pub version: i32,
    pub price: f64,
    pub currency: String,
    pub messages_per_month: i32,
    pub features: Vec<String>,
    pub effective_from: String,
    pub effective_to: Option<String>,
    pub status: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Subscription {
    pub id: String,
    pub user_id: String,
    pub plan_id: String,
    pub plan_version_id: Option<String>,
    pub status: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub current_period_start: Option<String>,
    pub current_period_end: Option<String>,
    pub cancel_at_period_end: bool,
    pub auto_renew: bool,
    pub terminated_at: Option<String>,
    pub activation_source: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum PaymentOrderStatus {
    Created,
    Approved,
    Captured,
    Failed,
    Cancelled,
    Expired,
}

impl PaymentOrderStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Approved => "approved",
            Self::Captured => "captured",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Expired => "expired",
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum PaymentStatus {
    Pending,
    Completed,
    Denied,
    Refunded,
    PartiallyRefunded,
    Reversed,
}

impl PaymentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Denied => "denied",
            Self::Refunded => "refunded",
            Self::PartiallyRefunded => "partially_refunded",
            Self::Reversed => "reversed",
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum WebhookProcessingStatus {
    Received,
    Pending,
    Processed,
    Ignored,
    Failed,
}

impl WebhookProcessingStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Received => "received",
            Self::Pending => "pending",
            Self::Processed => "processed",
            Self::Ignored => "ignored",
            Self::Failed => "failed",
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum RefundStatus {
    Requested,
    Pending,
    Completed,
    Failed,
    Cancelled,
}

impl RefundStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct PaymentOrder {
    pub id: String,
    pub user_id: String,
    pub plan_id: String,
    pub plan_version_id: String,
    pub provider: String,
    pub provider_order_id: String,
    pub idempotency_key: String,
    pub amount: String,
    pub currency: String,
    pub status: PaymentOrderStatus,
    pub description: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct PaymentTransaction {
    pub id: String,
    pub payment_order_id: String,
    pub user_id: String,
    pub provider: String,
    pub provider_transaction_id: String,
    pub amount: String,
    pub currency: String,
    pub status: PaymentStatus,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct PaymentWebhookEvent {
    pub id: String,
    pub provider: String,
    pub provider_event_id: String,
    pub event_type: String,
    pub resource_id: Option<String>,
    pub payload_hash: String,
    pub processing_status: WebhookProcessingStatus,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct PaymentRefund {
    pub id: String,
    pub payment_transaction_id: String,
    pub user_id: String,
    pub provider_refund_id: Option<String>,
    pub idempotency_key: String,
    pub amount: String,
    pub currency: String,
    pub status: RefundStatus,
    pub reason: String,
}
