use sqlx::MySqlPool;
use bcrypt::{hash, DEFAULT_COST};

pub async fn init(pool: &MySqlPool) -> anyhow::Result<()> {
    // create tables
    let schema = include_str!("../migrations.sql");
    for stmt in schema.split(';') {
        let s = stmt.trim();
        if s.is_empty() { continue; }
        sqlx::query(s).execute(pool).await?;
    }

    // Additive, non-destructive session hardening. Existing sessions remain in
    // the table, but old plaintext tokens no longer match the new hashed format.
    ensure_session_schema(pool).await?;
    ensure_wall_schema(pool).await?;
    ensure_outbox_schema(pool).await?;
    ensure_commercial_schema(pool).await?;
    ensure_financial_schema(pool).await?;

    // seed plans
    seed_plans(pool).await?;
    backfill_subscription_plan_versions(pool).await?;
    seed_models(pool).await?;

    // Demo accounts and their sample data are development-only. Production must
    // provision administrators through a controlled process and must not seed
    // public credentials or demo payments.
    if dev_user_seeds_enabled() {
        seed_admin(pool).await?;
        seed_demo(pool).await?;
    }
    backfill_subscription_plan_versions(pool).await?;
    Ok(())
}

async fn seed_plans(pool: &MySqlPool) -> anyhow::Result<()> {
    let plans = [
        ("plan-starter","starter","Opaline Starter",49.00,"MXN","mensual",100i32,"[\"100 mensajes/mes\",\"Modelos open source\",\"Sin RAG\"]"),
        ("plan-plus","plus","Opaline Plus",79.00,"MXN","mensual",1000i32,"[\"1000 mensajes/mes\",\"Modelos avanzados\",\"RAG documentos\"]"),
        ("plan-research","research","Opaline Research",129.00,"MXN","mensual",5000i32,"[\"5000 mensajes/mes\",\"Investigacion\",\"RAG completo\"]"),
    ];
    for p in plans {
        sqlx::query("INSERT INTO plans (id,code,name,price,currency,period,messages_per_month,features) VALUES (?,?,?,?,?,?,?,?) ON DUPLICATE KEY UPDATE name=VALUES(name)")
            .bind(p.0).bind(p.1).bind(p.2).bind(p.3).bind(p.4).bind(p.5).bind(p.6).bind(p.7)
            .execute(pool).await?;

        let version_id = format!("{}-v1", p.0);
        sqlx::query("INSERT INTO plan_versions (id,plan_id,version,price,currency,messages_per_month,features,effective_from,status) VALUES (?,?,?,?,?,?,?,NOW(),'active') ON DUPLICATE KEY UPDATE price=VALUES(price), currency=VALUES(currency), messages_per_month=VALUES(messages_per_month), features=VALUES(features), status=VALUES(status)")
            .bind(&version_id).bind(p.0).bind(1i32).bind(p.3).bind(p.4).bind(p.6).bind(p.7)
            .execute(pool).await?;
    }
    Ok(())
}

async fn backfill_subscription_plan_versions(pool: &MySqlPool) -> anyhow::Result<()> {
    sqlx::query("UPDATE subscriptions s JOIN plan_versions pv ON pv.plan_id=s.plan_id AND pv.version=1 SET s.plan_version_id=pv.id WHERE s.plan_version_id IS NULL")
        .execute(pool).await?;
    Ok(())
}

async fn seed_models(pool: &MySqlPool) -> anyhow::Result<()> {
    let models = [
        ("m-mistral","mistralai/ministral-3-3b","Ministral 3B",1i8,1i8),
        ("m-qwen","qwen/qwen3.5-9b","Qwen 3.5 9B",1i8,0i8),
        ("m-gemma","google/gemma-4-e2b","Gemma 4 E2B",1i8,0i8),
    ];
    for m in models {
        sqlx::query("INSERT INTO ai_models (id,code,name,enabled,is_default) VALUES (?,?,?,?,?) ON DUPLICATE KEY UPDATE name=VALUES(name), is_default=VALUES(is_default)")
            .bind(m.0).bind(m.1).bind(m.2).bind(m.3).bind(m.4)
            .execute(pool).await?;
    }
    Ok(())
}

async fn seed_demo(pool: &MySqlPool) -> anyhow::Result<()> {
    let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM users WHERE email=?")
        .bind("demo@opaline-ai.com").fetch_optional(pool).await?;
    if exists.is_none() {
        let h = hash("demo1234", DEFAULT_COST).unwrap();
        let id = "demo-user".to_string();
        sqlx::query("INSERT INTO users (id,email,password_hash,name,role,email_verified) VALUES (?,?,?,?, 'client',1)")
            .bind(&id).bind("demo@opaline-ai.com").bind(h).bind("Cliente Demo")
            .execute(pool).await?;
        // active starter subscription
        let sid = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO subscriptions (id,user_id,plan_id,status,start_date,end_date) VALUES (?,?,?, 'active', NOW(), DATE_ADD(NOW(), INTERVAL 1 MONTH))")
            .bind(sid).bind(&id).bind("plan-starter")
            .execute(pool).await?;

        // Sample payment/invoice for the newly seeded demo user only.
        let pay_id = uuid::Uuid::new_v4().to_string();
        let inv_id = uuid::Uuid::new_v4().to_string();
        let folio = format!("OP-{}", &pay_id[..8]);
        sqlx::query("INSERT INTO payments (id,user_id,plan_id,paypal_order_id,amount,currency,status,created_at) VALUES (?,?,?,?,?,?, 'completed', DATE_SUB(NOW(), INTERVAL 3 DAY))")
            .bind(&pay_id).bind(&id).bind("plan-starter").bind(format!("demo-{}", &pay_id[..8])).bind(49.00_f64).bind("MXN")
            .execute(pool).await?;
        sqlx::query("INSERT INTO invoices (id,payment_id,folio) VALUES (?,?,?)")
            .bind(inv_id).bind(&pay_id).bind(folio)
            .execute(pool).await?;
    }
    // Sample payment/invoice are created only for a newly seeded demo user.
    // Existing demo data is never repaired or overwritten on startup.
    Ok(())
}

pub async fn init_mongo(db: &mongodb::Database) -> anyhow::Result<()> {
    let collection = db.collection::<mongodb::bson::Document>("conversations");

    // The unique index is best-effort: if legacy duplicates already exist, startup
    // must not fail and no conversation data is deleted or overwritten.
    let keys = mongodb::bson::doc! { "user_id": 1, "id": 1 };
    let options = mongodb::options::IndexOptions::builder().unique(true).build();
    let model = mongodb::IndexModel::builder().keys(keys).options(options).build();
    if let Err(err) = collection.create_index(model).await {
        eprintln!("warning: could not create unique conversations index: {err}");
    }

    let audit = db.collection::<mongodb::bson::Document>("wall_events");
    let audit_keys = mongodb::bson::doc! { "event_id": 1 };
    let audit_options = mongodb::options::IndexOptions::builder()
        .unique(true)
        .partial_filter_expression(mongodb::bson::doc! { "event_id": { "$exists": true } })
        .build();
    let audit_model = mongodb::IndexModel::builder().keys(audit_keys).options(audit_options).build();
    if let Err(err) = audit.create_index(audit_model).await {
        eprintln!("warning: could not create unique wall audit index: {err}");
    }

    Ok(())
}

async fn ensure_session_schema(pool: &MySqlPool) -> anyhow::Result<()> {
    let has_previous: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = 'sessions' AND column_name = 'previous_refresh_token'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    if has_previous == 0 {
        sqlx::query("ALTER TABLE sessions ADD COLUMN IF NOT EXISTS previous_refresh_token VARCHAR(64) NULL")
            .execute(pool)
            .await?;
    }

    let has_refresh_expiry: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = 'sessions' AND column_name = 'refresh_expires_at'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    if has_refresh_expiry == 0 {
        sqlx::query("ALTER TABLE sessions ADD COLUMN IF NOT EXISTS refresh_expires_at DATETIME NULL")
            .execute(pool)
            .await?;
    }

    let has_rotated_at: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = 'sessions' AND column_name = 'refresh_rotated_at'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    if has_rotated_at == 0 {
        sqlx::query("ALTER TABLE sessions ADD COLUMN IF NOT EXISTS refresh_rotated_at DATETIME NULL")
            .execute(pool)
            .await?;
    }

    Ok(())
}

async fn ensure_wall_schema(pool: &MySqlPool) -> anyhow::Result<()> {
    let has_idempotency: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = 'posts' AND column_name = 'idempotency_key'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    if has_idempotency == 0 {
        sqlx::query("ALTER TABLE posts ADD COLUMN IF NOT EXISTS idempotency_key VARCHAR(128) NULL")
            .execute(pool)
            .await?;
    }

    // Best-effort unique constraints. If legacy duplicates exist, startup continues.
    let _ = sqlx::query("ALTER TABLE posts ADD UNIQUE INDEX IF NOT EXISTS uq_posts_user_idempotency (user_id, idempotency_key)")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE reports ADD UNIQUE INDEX IF NOT EXISTS uq_reports_post_user (post_id, user_id)")
        .execute(pool)
        .await;

    Ok(())
}

async fn ensure_outbox_schema(pool: &MySqlPool) -> anyhow::Result<()> {
    let has_claim_token: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = 'outbox_events' AND column_name = 'claim_token'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    if has_claim_token == 0 {
        let _ = sqlx::query("ALTER TABLE outbox_events ADD COLUMN IF NOT EXISTS claim_token VARCHAR(36) NULL")
            .execute(pool)
            .await;
    }
    Ok(())
}


async fn ensure_commercial_schema(pool: &MySqlPool) -> anyhow::Result<()> {
    // Plan versioning.
    let has_plan_versions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = 'plan_versions'",
    )
    .fetch_one(pool).await.unwrap_or(0);
    if has_plan_versions == 0 {
        let _ = sqlx::query("CREATE TABLE IF NOT EXISTS plan_versions (id VARCHAR(36) PRIMARY KEY, plan_id VARCHAR(36) NOT NULL, version INT NOT NULL, price DECIMAL(10,2) NOT NULL, currency VARCHAR(8) NOT NULL DEFAULT 'MXN', messages_per_month INT NOT NULL DEFAULT 100, features JSON, effective_from DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP, effective_to DATETIME NULL, status ENUM('draft','active','inactive') NOT NULL DEFAULT 'active', created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE KEY uq_plan_version (plan_id, version), FOREIGN KEY (plan_id) REFERENCES plans(id))")
            .execute(pool).await;
    }

    let has_subscription_events: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = 'subscription_events'",
    )
    .fetch_one(pool).await.unwrap_or(0);
    if has_subscription_events == 0 {
        let _ = sqlx::query("CREATE TABLE IF NOT EXISTS subscription_events (id VARCHAR(36) PRIMARY KEY, subscription_id VARCHAR(36) NOT NULL, event_type VARCHAR(64) NOT NULL, previous_status VARCHAR(32), new_status VARCHAR(32) NOT NULL, actor_type VARCHAR(32) NOT NULL DEFAULT 'system', actor_id VARCHAR(36), reason VARCHAR(240), correlation_id VARCHAR(64), created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP, FOREIGN KEY (subscription_id) REFERENCES subscriptions(id))")
            .execute(pool).await;
    }

    // Additive subscription columns.
    let columns = [
        ("plan_version_id", "ALTER TABLE subscriptions ADD COLUMN IF NOT EXISTS plan_version_id VARCHAR(36) NULL"),
        ("current_period_start", "ALTER TABLE subscriptions ADD COLUMN IF NOT EXISTS current_period_start DATETIME NULL"),
        ("current_period_end", "ALTER TABLE subscriptions ADD COLUMN IF NOT EXISTS current_period_end DATETIME NULL"),
        ("cancel_at_period_end", "ALTER TABLE subscriptions ADD COLUMN IF NOT EXISTS cancel_at_period_end TINYINT(1) NOT NULL DEFAULT 0"),
        ("terminated_at", "ALTER TABLE subscriptions ADD COLUMN IF NOT EXISTS terminated_at DATETIME NULL"),
        ("activation_source", "ALTER TABLE subscriptions ADD COLUMN IF NOT EXISTS activation_source VARCHAR(64) NULL"),
        ("auto_renew", "ALTER TABLE subscriptions ADD COLUMN IF NOT EXISTS auto_renew TINYINT(1) NOT NULL DEFAULT 1"),
        ("is_current", "ALTER TABLE subscriptions ADD COLUMN IF NOT EXISTS is_current TINYINT NULL"),
        ("current_user_id", "ALTER TABLE subscriptions ADD COLUMN IF NOT EXISTS current_user_id VARCHAR(36) GENERATED ALWAYS AS (CASE WHEN is_current = 1 THEN user_id ELSE NULL END) STORED"),
    ];
    for (_name, sql) in columns {
        let _ = sqlx::query(sql).execute(pool).await;
    }

    // Plan version quota limits. Keep messages_per_month for compatibility.
    for sql in [
        "ALTER TABLE plan_versions ADD COLUMN IF NOT EXISTS requests_per_period INT NULL",
        "ALTER TABLE plan_versions ADD COLUMN IF NOT EXISTS input_tokens_per_period INT NULL",
        "ALTER TABLE plan_versions ADD COLUMN IF NOT EXISTS output_tokens_per_period INT NULL",
        "ALTER TABLE plan_versions ADD COLUMN IF NOT EXISTS total_tokens_per_period INT NULL",
    ] {
        let _ = sqlx::query(sql).execute(pool).await;
    }
    let _ = sqlx::query("UPDATE plan_versions SET requests_per_period = COALESCE(requests_per_period, messages_per_month) WHERE requests_per_period IS NULL")
        .execute(pool).await;

    // Usage tables.
    let _ = sqlx::query("CREATE TABLE IF NOT EXISTS usage_periods (id VARCHAR(36) PRIMARY KEY, user_id VARCHAR(36) NOT NULL, subscription_id VARCHAR(36) NOT NULL, plan_version_id VARCHAR(36) NOT NULL, period_start DATETIME NOT NULL, period_end DATETIME NOT NULL, request_limit INT NULL, input_token_limit INT NULL, output_token_limit INT NULL, total_token_limit INT NULL, requests_used INT NOT NULL DEFAULT 0, input_tokens_used INT NOT NULL DEFAULT 0, output_tokens_used INT NOT NULL DEFAULT 0, total_tokens_used INT NOT NULL DEFAULT 0, reserved_requests INT NOT NULL DEFAULT 0, reserved_input_tokens INT NOT NULL DEFAULT 0, reserved_output_tokens INT NOT NULL DEFAULT 0, reserved_total_tokens INT NOT NULL DEFAULT 0, status ENUM('open','closed') NOT NULL DEFAULT 'open', created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, UNIQUE KEY uq_usage_period_user_start (user_id, period_start), FOREIGN KEY (user_id) REFERENCES users(id), FOREIGN KEY (subscription_id) REFERENCES subscriptions(id), FOREIGN KEY (plan_version_id) REFERENCES plan_versions(id))")
        .execute(pool).await;
    let _ = sqlx::query("CREATE TABLE IF NOT EXISTS usage_reservations (id VARCHAR(36) PRIMARY KEY, user_id VARCHAR(36) NOT NULL, subscription_id VARCHAR(36) NOT NULL, usage_period_id VARCHAR(36) NOT NULL, request_id VARCHAR(120) NOT NULL, idempotency_key VARCHAR(128) NOT NULL, service_type VARCHAR(64) NOT NULL DEFAULT 'chat', model VARCHAR(120) NOT NULL, status ENUM('reserved','committed','released','expired','failed') NOT NULL DEFAULT 'reserved', estimated_input_tokens INT NOT NULL DEFAULT 0, estimated_output_tokens INT NOT NULL DEFAULT 0, estimated_total_tokens INT NOT NULL DEFAULT 0, actual_input_tokens INT NULL, actual_output_tokens INT NULL, actual_total_tokens INT NULL, token_source ENUM('provider','local','estimated','unavailable') NULL, measurement_status ENUM('measured','estimated','unavailable','pending') NOT NULL DEFAULT 'pending', response_json JSON NULL, created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP, expires_at DATETIME NOT NULL, committed_at DATETIME NULL, released_at DATETIME NULL, UNIQUE KEY uq_usage_reservation_user_idempotency (user_id, idempotency_key), UNIQUE KEY uq_usage_reservation_user_request (user_id, request_id), FOREIGN KEY (user_id) REFERENCES users(id), FOREIGN KEY (subscription_id) REFERENCES subscriptions(id), FOREIGN KEY (usage_period_id) REFERENCES usage_periods(id))")
        .execute(pool).await;
    let _ = sqlx::query("CREATE TABLE IF NOT EXISTS usage_records (id VARCHAR(36) PRIMARY KEY, user_id VARCHAR(36) NOT NULL, subscription_id VARCHAR(36) NOT NULL, usage_period_id VARCHAR(36) NOT NULL, reservation_id VARCHAR(36) NOT NULL, request_id VARCHAR(120) NOT NULL, provider VARCHAR(64) NOT NULL, model VARCHAR(120) NOT NULL, service_type VARCHAR(64) NOT NULL DEFAULT 'chat', input_tokens INT NOT NULL DEFAULT 0, output_tokens INT NOT NULL DEFAULT 0, total_tokens INT NOT NULL DEFAULT 0, token_source ENUM('provider','local','estimated','unavailable') NOT NULL, measurement_status ENUM('measured','estimated','unavailable','pending') NOT NULL, duration_ms INT NOT NULL DEFAULT 0, status ENUM('committed','failed') NOT NULL DEFAULT 'committed', created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE KEY uq_usage_record_user_request (user_id, request_id), FOREIGN KEY (user_id) REFERENCES users(id), FOREIGN KEY (subscription_id) REFERENCES subscriptions(id), FOREIGN KEY (usage_period_id) REFERENCES usage_periods(id), FOREIGN KEY (reservation_id) REFERENCES usage_reservations(id))")
        .execute(pool).await;


    for sql in [
        "ALTER TABLE usage_reservations ADD COLUMN IF NOT EXISTS recovery_owner VARCHAR(36) NULL",
        "ALTER TABLE usage_reservations ADD COLUMN IF NOT EXISTS recovery_lease_expires_at DATETIME NULL",
    ] {
        let _ = sqlx::query(sql).execute(pool).await;
    }

    let _ = sqlx::query("CREATE TABLE IF NOT EXISTS chat_operations (id VARCHAR(36) PRIMARY KEY, user_id VARCHAR(36) NOT NULL, subscription_id VARCHAR(36) NOT NULL, usage_period_id VARCHAR(36) NOT NULL, reservation_id VARCHAR(36) NOT NULL, request_id VARCHAR(120) NOT NULL, idempotency_key VARCHAR(128) NOT NULL, model_requested VARCHAR(120) NOT NULL, model_used VARCHAR(120) NULL, status ENUM('reserved','inferred','persisting','persisted','failed','manual_review') NOT NULL DEFAULT 'reserved', user_message TEXT NOT NULL, assistant_reply TEXT NULL, response_json JSON NULL, conversation_id VARCHAR(120) NULL, mongo_persisted TINYINT(1) NOT NULL DEFAULT 0, mongo_error TEXT NULL, token_source VARCHAR(32) NULL, measurement_status VARCHAR(32) NULL, actual_input_tokens INT NULL, actual_output_tokens INT NULL, actual_total_tokens INT NULL, duration_ms INT NULL, created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, UNIQUE KEY uq_chat_operation_user_idempotency (user_id, idempotency_key), UNIQUE KEY uq_chat_operation_reservation (reservation_id), FOREIGN KEY (user_id) REFERENCES users(id), FOREIGN KEY (subscription_id) REFERENCES subscriptions(id), FOREIGN KEY (usage_period_id) REFERENCES usage_periods(id), FOREIGN KEY (reservation_id) REFERENCES usage_reservations(id))")
        .execute(pool).await;

    // Best-effort foreign key and indexes.
    let _ = sqlx::query("ALTER TABLE subscriptions ADD CONSTRAINT fk_subscriptions_plan_version FOREIGN KEY (plan_version_id) REFERENCES plan_versions(id)").execute(pool).await;
    // Remove the previous, weaker uniqueness constraint. It allowed multiple
    // historical rows only when is_current was NULL, but it could not express
    // the intended "one current subscription" rule for non-NULL sentinels.
    let _ = sqlx::query("ALTER TABLE subscriptions DROP INDEX IF EXISTS uq_subscriptions_user_current").execute(pool).await;
    let _ = sqlx::query("ALTER TABLE subscriptions ADD UNIQUE INDEX IF NOT EXISTS uq_subscriptions_current_user (current_user_id)").execute(pool).await;
    let _ = sqlx::query("ALTER TABLE subscriptions ADD INDEX IF NOT EXISTS idx_subscriptions_user_current (user_id, is_current)").execute(pool).await;
    let _ = sqlx::query("ALTER TABLE subscription_events ADD INDEX IF NOT EXISTS idx_subscription_events_subscription (subscription_id, created_at)").execute(pool).await;

    // Normalize existing active subscriptions. Only one current subscription per
    // user can be represented by is_current=1; older rows become historical.
    let _ = sqlx::query("UPDATE subscriptions SET is_current=1 WHERE status IN ('active','grace') AND is_current IS NULL").execute(pool).await;
    // If the previous statement created duplicates, collapse them deterministically.
    let _ = sqlx::query("UPDATE subscriptions s JOIN (SELECT user_id, MIN(id) AS keep_id FROM subscriptions WHERE is_current=1 GROUP BY user_id HAVING COUNT(*) > 1) d ON d.user_id=s.user_id SET s.is_current=NULL WHERE s.is_current=1 AND s.id <> d.keep_id").execute(pool).await;

    Ok(())
}


async fn ensure_financial_schema(pool: &MySqlPool) -> anyhow::Result<()> {
    for sql in [
        "ALTER TABLE payment_orders MODIFY COLUMN provider_order_id VARCHAR(120) NULL",
        "ALTER TABLE payment_orders ADD COLUMN IF NOT EXISTS provider_request_id VARCHAR(120) NULL",
        "ALTER TABLE payment_orders ADD COLUMN IF NOT EXISTS description VARCHAR(240) NULL",
        "ALTER TABLE payment_orders ADD COLUMN IF NOT EXISTS updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP",
        "ALTER TABLE payment_orders ADD COLUMN IF NOT EXISTS expires_at DATETIME NULL",
        "ALTER TABLE payment_orders ADD COLUMN IF NOT EXISTS captured_at DATETIME NULL",
        "ALTER TABLE payment_orders ADD COLUMN IF NOT EXISTS activated_subscription_id VARCHAR(36) NULL",
        "ALTER TABLE payment_orders ADD COLUMN IF NOT EXISTS activated_at DATETIME NULL",
        "ALTER TABLE payment_transactions ADD COLUMN IF NOT EXISTS subscription_id VARCHAR(36) NULL",
    ] {
        let _ = sqlx::query(sql).execute(pool).await;
    }
    let _ = sqlx::query("ALTER TABLE payment_webhook_events ADD COLUMN IF NOT EXISTS payload JSON NULL").execute(pool).await;
    let _ = sqlx::query("ALTER TABLE payment_orders ADD UNIQUE INDEX IF NOT EXISTS uq_payment_orders_provider_request (provider_request_id)").execute(pool).await;
    let _ = sqlx::query("ALTER TABLE payment_orders ADD CONSTRAINT fk_payment_orders_activated_subscription FOREIGN KEY (activated_subscription_id) REFERENCES subscriptions(id)").execute(pool).await;
    let _ = sqlx::query("ALTER TABLE payment_transactions ADD CONSTRAINT fk_payment_transactions_subscription FOREIGN KEY (subscription_id) REFERENCES subscriptions(id)").execute(pool).await;
    Ok(())
}

fn dev_user_seeds_enabled() -> bool {
    dev_user_seeds_enabled_from(std::env::var("OPALINE_DEV_SEEDS").ok().as_deref())
}

fn dev_user_seeds_enabled_from(value: Option<&str>) -> bool {
    matches!(
        value.map(str::trim).map(str::to_ascii_lowercase).as_deref(),
        Some("1" | "true" | "yes" | "on")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_user_seeds_require_explicit_opt_in() {
        assert!(!dev_user_seeds_enabled_from(None));
        assert!(!dev_user_seeds_enabled_from(Some("")));
        assert!(!dev_user_seeds_enabled_from(Some("false")));
        assert!(!dev_user_seeds_enabled_from(Some("production")));
        assert!(dev_user_seeds_enabled_from(Some("true")));
        assert!(dev_user_seeds_enabled_from(Some(" 1 ")));
        assert!(dev_user_seeds_enabled_from(Some("YES")));
    }
}

async fn seed_admin(pool: &MySqlPool) -> anyhow::Result<()> {
    let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM users WHERE email=?")
        .bind("admin@opaline-ai.com").fetch_optional(pool).await?;
    if exists.is_none() {
        let h = hash("admin123", DEFAULT_COST).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO users (id,email,password_hash,name,role,email_verified) VALUES (?,?,?,?,?,1)")
            .bind(id).bind("admin@opaline-ai.com").bind(h).bind("Administrador").bind("admin")
            .execute(pool).await?;
    }
    Ok(())
}
