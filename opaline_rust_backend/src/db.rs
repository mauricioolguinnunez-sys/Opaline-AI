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
    // seed plans
    seed_plans(pool).await?;
    seed_models(pool).await?;
    seed_admin(pool).await?;
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
    }
    Ok(())
}

async fn seed_models(pool: &MySqlPool) -> anyhow::Result<()> {
    let models = [
        ("m-qwen","qwen/qwen3.5-9b","Qwen 3.5 9B",1i8,1i8),
        ("m-mistral","mistralai/ministral-3-3b","Ministral 3B",1i8,0i8),
        ("m-gemma","google/gemma-4-e2b","Gemma 4 E2B",1i8,0i8),
    ];
    for m in models {
        sqlx::query("INSERT INTO ai_models (id,code,name,enabled,is_default) VALUES (?,?,?,?,?) ON DUPLICATE KEY UPDATE name=VALUES(name)")
            .bind(m.0).bind(m.1).bind(m.2).bind(m.3).bind(m.4)
            .execute(pool).await?;
    }
    Ok(())
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
