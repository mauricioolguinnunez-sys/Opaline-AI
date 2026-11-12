use axum::{routing::{get, post, put, delete}, Router};
use crate::state::AppState;

pub mod auth;
pub mod plans;
pub mod payments;
pub mod chat;
pub mod wall;
pub mod usage;
pub mod admin;
pub mod outbox_admin;

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::router())
        .route("/planes", get(plans::list_plans))
        .route("/planes/:id/versions", get(plans::list_plan_versions))
        .nest("/suscripcion", plans::subscription_router())
        .nest("/payments", payments::router())
        .nest("/facturas", payments::invoices_router())
        .nest("/ai", chat::router())
        .nest("/muro", wall::router())
        .nest("/usage", usage::router())
        .nest("/admin", admin::router())
        .nest("/admin/outbox", outbox_admin::router())
}
