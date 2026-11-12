use axum::{extract::{ConnectInfo, State}, Json, Router, routing::{get, post}};
use axum::http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use crate::state::AppState;
use sha2::{Digest, Sha256};
use uuid::Uuid;
use chrono::{Utc, Duration};
use std::net::SocketAddr;
use std::time::Duration as StdDuration;

#[derive(Deserialize)]
pub struct RegisterReq { pub email: String, pub password: String, pub name: Option<String> }

#[derive(Deserialize)]
pub struct LoginReq { pub email: String, pub password: String }

#[derive(Deserialize)]
pub struct RefreshReq { pub refresh_token: String }

#[derive(Deserialize)]
pub struct ChangePwdReq { pub current_password: String, pub new_password: String }

#[derive(Deserialize)]
pub struct ForgotReq { pub email: String }

#[derive(Serialize)]
pub struct AuthResp { pub access_token: String, pub refresh_token: String, pub user: serde_json::Value }

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/refresh", post(refresh))
        .route("/change-password", post(change_password))
        .route("/forgot-password", post(forgot_password))
        .route("/sessions", get(sessions))
}

fn token_from_headers(headers: &HeaderMap) -> Option<String> {
    headers.get("authorization").and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer ")).map(|s| s.to_string())
}

pub async fn user_id_from_token(state: &AppState, token: &str) -> Option<String> {
    let token_hash = hash_token(token);
    let row = sqlx::query("SELECT user_id, expires_at FROM sessions WHERE access_token=?")
        .bind(&token_hash).fetch_optional(&state.pool).await.ok()??;
    let uid: String = row.get("user_id");
    let exp: chrono::NaiveDateTime = row.get("expires_at");
    if exp < Utc::now().naive_utc() { return None; }
    Some(uid)
}

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn valid_password(password: &str) -> bool {
    let chars: Vec<char> = password.chars().collect();
    if chars.len() < 10 || chars.len() > 64 {
        return false;
    }
    let has_lower = chars.iter().any(|c| c.is_lowercase());
    let has_upper = chars.iter().any(|c| c.is_uppercase());
    let has_digit = chars.iter().any(|c| c.is_ascii_digit());
    let has_special = chars.iter().any(|c| !c.is_alphanumeric());
    has_lower && has_upper && has_digit && has_special
}

async fn hash_password(password: String) -> Result<String, StatusCode> {
    tokio::task::spawn_blocking(move || bcrypt::hash(password, bcrypt::DEFAULT_COST))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn verify_password(password: String, password_hash: String) -> Result<bool, StatusCode> {
    tokio::task::spawn_blocking(move || bcrypt::verify(password, &password_hash).unwrap_or(false))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn client_ip(connect: &SocketAddr) -> String {
    connect.ip().to_string()
}

pub async fn register(
    ConnectInfo(connect): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    Json(req): Json<RegisterReq>,
) -> Result<Json<AuthResp>, StatusCode> {
    let ip = client_ip(&connect);
    if !state.rate_limiter.check(&format!("register:ip:{ip}"), 10, StdDuration::from_secs(3600)) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let email = req.email.trim().to_lowercase();
    if email.is_empty() || !valid_password(&req.password) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let existing: Option<(String,)> = sqlx::query_as("SELECT id FROM users WHERE email=?")
        .bind(&email).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if existing.is_some() {
        return Err(StatusCode::CONFLICT);
    }

    let name = req.name.unwrap_or_else(|| "Usuario Opaline".to_string());
    let password_hash = hash_password(req.password).await?;
    let uid = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO users (id,email,password_hash,name,role,email_verified) VALUES (?,?,?,?, 'client',0)")
        .bind(&uid).bind(&email).bind(&password_hash).bind(&name)
        .execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (access, refresh, sid) = create_session(&state, &uid).await?;
    let user = serde_json::json!({"id":uid,"email":email,"name":name,"role":"client","email_verified":false});
    Ok(Json(AuthResp { access_token: access, refresh_token: refresh, user }))
}

pub async fn login(
    ConnectInfo(connect): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    Json(req): Json<LoginReq>,
) -> Result<Json<AuthResp>, StatusCode> {
    let ip = client_ip(&connect);
    let email = req.email.trim().to_lowercase();
    if email.is_empty() || req.password.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    if !state.rate_limiter.check(&format!("login:ip:{ip}"), 20, StdDuration::from_secs(900)) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    if !state.rate_limiter.check(&format!("login:email:{email}"), 5, StdDuration::from_secs(900)) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let row = sqlx::query("SELECT id,password_hash,name,role,email_verified FROM users WHERE email=?")
        .bind(&email).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = match row { Some(r) => r, None => return Err(StatusCode::UNAUTHORIZED) };
    let uid: String = row.get("id");
    let hashpw: String = row.get("password_hash");
    let name: String = row.get("name");
    let role: String = row.get("role");
    let verified: i8 = row.get("email_verified");

    if !verify_password(req.password, hashpw).await? {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let (access, refresh, _sid) = create_session(&state, &uid).await?;
    let user = serde_json::json!({"id":uid,"email":email,"name":name,"role":role,"email_verified":verified==1});
    Ok(Json(AuthResp { access_token: access, refresh_token: refresh, user }))
}

pub async fn logout(headers: HeaderMap, State(state): State<AppState>) -> Result<StatusCode, StatusCode> {
    if let Some(t) = token_from_headers(&headers) {
        let token_hash = hash_token(&t);
        sqlx::query("DELETE FROM sessions WHERE access_token=?")
            .bind(&token_hash)
            .execute(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn refresh(
    ConnectInfo(connect): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    Json(req): Json<RefreshReq>,
) -> Result<Json<AuthResp>, StatusCode> {
    let ip = client_ip(&connect);
    if !state.rate_limiter.check(&format!("refresh:ip:{ip}"), 60, StdDuration::from_secs(900)) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let presented_hash = hash_token(&req.refresh_token);
    let row = sqlx::query("SELECT id,user_id,refresh_token,previous_refresh_token,refresh_expires_at,refresh_rotated_at FROM sessions WHERE refresh_token=? OR previous_refresh_token=?")
        .bind(&presented_hash)
        .bind(&presented_hash)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = match row { Some(r) => r, None => return Err(StatusCode::UNAUTHORIZED) };

    let sid: String = row.get("id");
    let uid: String = row.get("user_id");
    let current_hash: String = row.get("refresh_token");
    let previous_hash: Option<String> = row.try_get("previous_refresh_token").ok().flatten();
    let refresh_expires: Option<chrono::NaiveDateTime> = row.try_get("refresh_expires_at").ok().flatten();

    let rotated_at: Option<chrono::NaiveDateTime> = row.try_get("refresh_rotated_at").ok().flatten();

    // A token presented immediately after a legitimate rotation can be a
    // concurrent duplicate rather than theft. Revoke only when the rotation is
    // not recent, avoiding false positives while preserving replay protection.
    if previous_hash.as_deref() == Some(presented_hash.as_str()) {
        let recent_duplicate = rotated_at
            .map(|rotated| (Utc::now().naive_utc() - rotated).num_seconds().abs() <= 5)
            .unwrap_or(false);
        if !recent_duplicate {
            sqlx::query("DELETE FROM sessions WHERE id=?")
                .bind(&sid)
                .execute(&state.pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
        return Err(StatusCode::UNAUTHORIZED);
    }

    if current_hash != presented_hash {
        return Err(StatusCode::UNAUTHORIZED);
    }

    if refresh_expires.map(|exp| exp < Utc::now().naive_utc()).unwrap_or(true) {
        sqlx::query("DELETE FROM sessions WHERE id=?")
            .bind(&sid)
            .execute(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        return Err(StatusCode::UNAUTHORIZED);
    }

    let row = sqlx::query("SELECT email,name,role,email_verified FROM users WHERE id=?")
        .bind(&uid).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = row.ok_or(StatusCode::UNAUTHORIZED)?;
    let email: String = row.get("email");
    let name: String = row.get("name");
    let role: String = row.get("role");
    let verified: i8 = row.get("email_verified");

    let access = Uuid::new_v4().to_string();
    let refresh = Uuid::new_v4().to_string();
    let access_exp = Utc::now() + Duration::minutes(15);
    let refresh_exp = Utc::now() + Duration::days(7);

    let rotated_now = Utc::now().naive_utc();
    let result = sqlx::query(
        "UPDATE sessions SET access_token=?, expires_at=?, refresh_token=?, previous_refresh_token=?, refresh_expires_at=?, refresh_rotated_at=? WHERE id=? AND refresh_token=?",
    )
    .bind(hash_token(&access))
    .bind(access_exp.naive_utc())
    .bind(hash_token(&refresh))
    .bind(&current_hash)
    .bind(refresh_exp.naive_utc())
    .bind(rotated_now)
    .bind(&sid)
    .bind(&current_hash)
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() != 1 {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let user = serde_json::json!({"id":uid,"email":email,"name":name,"role":role,"email_verified":verified==1});
    Ok(Json(AuthResp { access_token: access, refresh_token: refresh, user }))
}

pub async fn change_password(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<ChangePwdReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !valid_password(&req.new_password) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let t = token_from_headers(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;

    if !state.rate_limiter.check(&format!("change-password:user:{uid}"), 10, StdDuration::from_secs(3600)) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let row = sqlx::query("SELECT password_hash FROM users WHERE id=?")
        .bind(&uid).fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let h: String = row.get("password_hash");
    if !verify_password(req.current_password, h).await? {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let newh = hash_password(req.new_password).await?;
    sqlx::query("UPDATE users SET password_hash=? WHERE id=?")
        .bind(&newh).bind(&uid).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let token_hash = hash_token(&t);
    sqlx::query("DELETE FROM sessions WHERE user_id=? AND access_token<>?")
        .bind(&uid).bind(&token_hash).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({"ok":true})))
}

pub async fn forgot_password(
    ConnectInfo(connect): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    Json(req): Json<ForgotReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let ip = client_ip(&connect);
    let email = req.email.trim().to_lowercase();
    if !state.rate_limiter.check(&format!("forgot:ip:{ip}"), 10, StdDuration::from_secs(900))
        || !state.rate_limiter.check(&format!("forgot:email:{email}"), 5, StdDuration::from_secs(900))
    {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let _ = state.mongo.collection::<mongodb::bson::Document>("auth_events")
        .insert_one(mongodb::bson::doc!{"event":"forgot","email":&email,"ts":mongodb::bson::DateTime::now()}).await;
    Ok(Json(serde_json::json!({"ok":true,"message":"Si el correo existe, se enviara un enlace."})))
}

pub async fn sessions(headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = token_from_headers(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let rows = sqlx::query("SELECT id, created_at, expires_at, refresh_expires_at FROM sessions WHERE user_id=? AND (refresh_expires_at IS NULL OR refresh_expires_at > UTC_TIMESTAMP())")
        .bind(&uid).fetch_all(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut list = Vec::new();
    for r in rows {
        list.push(serde_json::json!({
            "id": r.get::<String,_>("id"),
            "created_at": r.get::<chrono::NaiveDateTime,_>("created_at").to_string(),
            "expires_at": r.get::<chrono::NaiveDateTime,_>("expires_at").to_string(),
            "refresh_expires_at": r.try_get::<Option<chrono::NaiveDateTime>,_>("refresh_expires_at").ok().flatten().map(|v| v.to_string()),
        }));
    }
    Ok(Json(serde_json::json!({"sessions":list})))
}

async fn create_session(state: &AppState, uid: &str) -> Result<(String, String, String), StatusCode> {
    let access = Uuid::new_v4().to_string();
    let refresh = Uuid::new_v4().to_string();
    let sid = Uuid::new_v4().to_string();
    let access_exp = Utc::now() + Duration::minutes(15);
    let refresh_exp = Utc::now() + Duration::days(7);
    sqlx::query("INSERT INTO sessions (id,user_id,access_token,refresh_token,expires_at,refresh_expires_at,previous_refresh_token,refresh_rotated_at) VALUES (?,?,?,?,?,?,NULL,NULL)")
        .bind(&sid)
        .bind(uid)
        .bind(hash_token(&access))
        .bind(hash_token(&refresh))
        .bind(access_exp.naive_utc())
        .bind(refresh_exp.naive_utc())
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((access, refresh, sid))
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_hash_is_deterministic_and_sensitive() {
        let a = hash_token("token-a");
        let b = hash_token("token-b");
        assert_eq!(a, hash_token("token-a"));
        assert_ne!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn password_validation_requires_all_classes() {
        assert!(valid_password("Abcdef1!xy"));
        assert!(!valid_password("short1!A"));
        assert!(!valid_password("alllowercase1!"));
        assert!(!valid_password("ALLUPPERCASE1!"));
        assert!(!valid_password("NoSpecial1234"));
        assert!(!valid_password("NoDigit!abcd"));
        assert!(!valid_password(""));
    }
}
