mod state;
mod db;
mod models;
mod routes;

use state::AppState;
use axum::{routing::{get, post, put, delete}, Router};
use tower_http::{cors::CorsLayer, services::ServeDir};
use sqlx::mysql::MySqlPoolOptions;
use mongodb::Client as MongoClient;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://opaline:opaline@127.0.0.1:3307/opaline".to_string());
    let mongo_url = std::env::var("MONGO_URL")
        .unwrap_or_else(|_| "mongodb://127.0.0.1:27018".to_string());
    let lmstudio_url = std::env::var("LMSTUDIO_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:1234".to_string());

    let pool = MySqlPoolOptions::new()
        .max_connections(10)
        .connect(&db_url).await.expect("mysql connect");

    // run migrations (embedded)
    db::init(&pool).await.expect("db init");

    let mongo_client = MongoClient::with_uri_str(&mongo_url).await.expect("mongo connect");
    let mongo_db = mongo_client.database("opaline");

    let state = AppState { pool, mongo: mongo_db, lmstudio_url };

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .nest("/api/v1", routes::router())
        .fallback_service(ServeDir::new("/home/mauricioon/Documentos/Opaline_build/Opaline_AI_Web_Prototipo/frontend"))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([0,0,0,0], 8090));
    tracing::info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
