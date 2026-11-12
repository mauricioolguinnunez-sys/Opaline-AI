use sqlx::MySqlPool;
use mongodb::Database;
use crate::security::RateLimiter;

#[derive(Clone)]
pub struct AppState {
    pub pool: MySqlPool,
    pub mongo: Database,
    pub lmstudio_url: String,
    pub rate_limiter: RateLimiter,
}
