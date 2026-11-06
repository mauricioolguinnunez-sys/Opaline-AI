use sqlx::MySqlPool;
use mongodb::Database;

#[derive(Clone)]
pub struct AppState {
    pub pool: MySqlPool,
    pub mongo: Database,
    pub lmstudio_url: String,
}
