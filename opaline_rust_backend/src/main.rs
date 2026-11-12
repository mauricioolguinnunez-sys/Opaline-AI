use opaline_backend::{db, models, routes, security, state};

use state::AppState;
use axum::{routing::{get, post, put, delete}, Router};
use axum::http::Method;
use tower_http::cors::{AllowOrigin, CorsLayer};
use sqlx::mysql::MySqlPoolOptions;
use mongodb::Client as MongoClient;
use std::net::SocketAddr;
use std::sync::OnceLock;

static WALL_AUDIT_WORKER: OnceLock<(tokio::task::JoinHandle<()>, tokio::sync::watch::Sender<bool>)> = OnceLock::new();
static WALL_AUDIT_RECONCILER: OnceLock<(tokio::task::JoinHandle<()>, tokio::sync::watch::Sender<bool>)> = OnceLock::new();
static WALL_AUDIT_RECOVERY: OnceLock<(tokio::task::JoinHandle<()>, tokio::sync::watch::Sender<bool>)> = OnceLock::new();
static CHAT_PERSIST_WORKER: OnceLock<(tokio::task::JoinHandle<()>, tokio::sync::watch::Sender<bool>)> = OnceLock::new();
static QUOTA_RECOVERY_WORKER: OnceLock<tokio::task::JoinHandle<()>> = OnceLock::new();
static PAYMENT_RECOVERY_WORKER: OnceLock<tokio::task::JoinHandle<()>> = OnceLock::new();
static PAYMENT_WEBHOOK_WORKER: OnceLock<(tokio::task::JoinHandle<()>, tokio::sync::watch::Sender<bool>)> = OnceLock::new();

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

    // Best-effort Mongo initialization. It never deletes existing conversations.
    db::init_mongo(&mongo_db).await.expect("mongo init");

    let state = AppState {
        pool,
        mongo: mongo_db,
        lmstudio_url,
        rate_limiter: security::RateLimiter::new(),
    };

    if std::env::var("OPALINE_WALL_AUDIT_OUTBOX_ENABLED").ok().as_deref() == Some("1") {
        let consumer = opaline_backend::consumers::mongo_wall_audit::MongoWallAuditConsumer::new(state.mongo.clone());
        match consumer.verify_event_id_index().await {
            Ok(()) => {
                let config = opaline_backend::outbox_worker::OutboxWorkerConfig::from_env();
                let (tx, rx) = tokio::sync::watch::channel(false);
                let handle = opaline_backend::outbox_worker::spawn_worker(state.pool.clone(), config, consumer, rx);
                let _ = WALL_AUDIT_WORKER.set((handle, tx));
                tracing::info!("wall audit outbox worker activated");
            }
            Err(error) => {
                tracing::error!(error = %error, "wall audit outbox worker not activated; pending events remain in MySQL");
            }
        }
    } else {
        tracing::info!("wall audit outbox worker disabled");
    }

    if std::env::var("OPALINE_WALL_AUDIT_RECONCILIATION_ENABLED").ok().as_deref() == Some("1") {
        let consumer = opaline_backend::consumers::mongo_wall_audit::MongoWallAuditConsumer::new(state.mongo.clone());
        match consumer.verify_event_id_index().await {
            Ok(()) => {
                let config = opaline_backend::outbox_reconciler::ReconcilerConfig::from_env();
                let (tx, rx) = tokio::sync::watch::channel(false);
                let handle = opaline_backend::outbox_reconciler::spawn_reconciler(state.pool.clone(), state.mongo.clone(), config, rx);
                let _ = WALL_AUDIT_RECONCILER.set((handle, tx));
                tracing::info!("wall audit outbox reconciler activated");
            }
            Err(error) => {
                tracing::error!(error = %error, "wall audit outbox reconciler not activated");
            }
        }
    } else {
        tracing::info!("wall audit outbox reconciler disabled");
    }

    if std::env::var("OPALINE_WALL_AUDIT_RECOVERY_ENABLED").ok().as_deref() == Some("1") {
        let config = opaline_backend::outbox_recovery::RecoveryConfig::from_env();
        let (tx, rx) = tokio::sync::watch::channel(false);
        let handle = opaline_backend::outbox_recovery::spawn_recovery(state.pool.clone(), state.mongo.clone(), config, rx);
        let _ = WALL_AUDIT_RECOVERY.set((handle, tx));
        tracing::info!("wall audit outbox recovery activated");
    } else {
        tracing::info!("wall audit outbox recovery disabled");
    }

    if std::env::var("OPALINE_CHAT_PERSISTENCE_OUTBOX_ENABLED").ok().as_deref() == Some("1") {
        let consumer = opaline_backend::consumers::mongo_chat_persist::MongoChatPersistConsumer {
            pool: state.pool.clone(),
            mongo: state.mongo.clone(),
        };
        let config = opaline_backend::outbox_worker::OutboxWorkerConfig::from_env();
        let (tx, rx) = tokio::sync::watch::channel(false);
        let handle = opaline_backend::outbox_worker::spawn_worker(state.pool.clone(), config, consumer, rx);
        let _ = CHAT_PERSIST_WORKER.set((handle, tx));
        tracing::info!("chat persistence outbox worker activated");
    } else {
        tracing::info!("chat persistence outbox worker disabled");
    }

    if std::env::var("OPALINE_PAYMENT_WEBHOOK_OUTBOX_ENABLED").ok().as_deref() == Some("1") {
        let consumer = opaline_backend::consumers::paypal_webhook::PayPalWebhookConsumer {
            pool: state.pool.clone(),
        };
        let config = opaline_backend::outbox_worker::OutboxWorkerConfig::from_env();
        let (tx, rx) = tokio::sync::watch::channel(false);
        let handle = opaline_backend::outbox_worker::spawn_worker(state.pool.clone(), config, consumer, rx);
        let _ = PAYMENT_WEBHOOK_WORKER.set((handle, tx));
        tracing::info!("payment webhook outbox worker activated");
    } else {
        tracing::info!("payment webhook outbox worker disabled");
    }

    if std::env::var("OPALINE_QUOTA_RECOVERY_ENABLED").ok().as_deref() == Some("1") {
        let pool = state.pool.clone();
        let mongo = state.mongo.clone();
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                let _ = opaline_backend::quota::reap_expired_reservations(&pool).await;
                let _ = opaline_backend::chat_operations::reconcile_operations(&pool, &mongo, true).await;
            }
        });
        let _ = QUOTA_RECOVERY_WORKER.set(handle);
        tracing::info!("quota recovery worker activated");
    } else {
        tracing::info!("quota recovery worker disabled");
    }

    if std::env::var("OPALINE_PAYMENT_RECOVERY_ENABLED").ok().as_deref() == Some("1") {
        let pool = state.pool.clone();
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                let _ = opaline_backend::routes::payments::reconcile_payment_activations(&pool, true).await;
            }
        });
        let _ = PAYMENT_RECOVERY_WORKER.set(handle);
        tracing::info!("payment recovery worker activated");
    } else {
        tracing::info!("payment recovery worker disabled");
    }

    let cors_origins = std::env::var("OPALINE_CORS_ORIGINS").unwrap_or_else(|_| {
        "http://localhost:8000,http://127.0.0.1:8000".to_string()
    });
    let allowed_origins = security::parse_cors_origins(&cors_origins);

    let cors = if allowed_origins.is_empty() {
        CorsLayer::new()
    } else {
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(allowed_origins))
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
            .allow_headers([axum::http::header::AUTHORIZATION, axum::http::header::CONTENT_TYPE])
    };

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/", get(|| async {
            axum::response::Redirect::temporary("http://localhost:8000/frontend/index.php")
        }))
        .nest("/api/v1", routes::router())
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([0,0,0,0], 8090));
    tracing::info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await.unwrap();
}
