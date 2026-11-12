use std::env;
use axum::extract::ConnectInfo;
use std::net::SocketAddr;
use tokio::sync::Mutex;

use mongodb::Client as MongoClient;
use opaline_backend::{db, security::RateLimiter, state::AppState};
use sqlx::mysql::MySqlPoolOptions;
use sqlx::MySqlPool;

pub fn test_database_url() -> String {
    let url = env::var("OPALINE_TEST_DATABASE_URL")
        .expect("OPALINE_TEST_DATABASE_URL must be set to an isolated MySQL test database");
    assert_test_database_url(&url);
    url
}

pub fn assert_test_database_url(url: &str) {
    assert!(
        url.contains("127.0.0.1") || url.contains("localhost"),
        "test database must run on localhost"
    );

    let database = url
        .rsplit('/')
        .next()
        .unwrap_or("")
        .split('?')
        .next()
        .unwrap_or("");
    assert!(!database.is_empty(), "test database name is required");
    let database = database.to_lowercase();
    assert!(
        database.contains("test"),
        "test database name must contain 'test': {database}"
    );
    assert_ne!(database, "opaline", "refusing to use the development database");
}

pub async fn test_pool() -> MySqlPool {
    let url = test_database_url();
    MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("failed to connect to isolated test database")
}

pub async fn setup_state() -> AppState {
    let pool = test_pool().await;
    db::init(&pool).await.expect("failed to initialize test schema");

    // Tests use unique identifiers and must not truncate shared tables while
    // other integration tests may be using the isolated database.

    let mongo_url = env::var("OPALINE_TEST_MONGO_URL")
        .unwrap_or_else(|_| "mongodb://127.0.0.1:27018/opaline_test".to_string());
    let mongo = MongoClient::with_uri_str(&mongo_url)
        .await
        .expect("failed to connect to test MongoDB")
        .database("opaline_test");
    db::init_mongo(&mongo).await.expect("failed to initialize test Mongo");

    AppState {
        pool,
        mongo,
        lmstudio_url: "http://127.0.0.1:1234".to_string(),
        rate_limiter: RateLimiter::new(),
    }
}

pub fn test_connect_info() -> ConnectInfo<SocketAddr> {
    ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 41000)))
}

pub static TEST_DB_MUTEX: Mutex<()> = Mutex::const_new(());

pub async fn test_lock() -> tokio::sync::MutexGuard<'static, ()> {
    TEST_DB_MUTEX.lock().await
}

pub fn unique_email(prefix: &str) -> String {
    format!("{prefix}-{}@example.test", uuid::Uuid::new_v4())
}