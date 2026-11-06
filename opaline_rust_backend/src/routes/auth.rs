use axum::{extract::State, Json, Router, routing::{get, post}};
use axum::http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use crate::state::AppState;
use bcrypt::{hash, verify, DEFAULT_COST};
use uuid::Uuid;
use chrono::{Utc, Duration};

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
    let row = sqlx::query("SELECT user_id, expires_at FROM sessions WHERE access_token=?")
        .bind(token).fetch_optional(&state.pool).await.ok()??;
    let uid: String = row.get("user_id");
    let exp: chrono::NaiveDateTime = row.get("expires_at");
    if exp < Utc::now().naive_utc() { return None; }
    Some(uid)
}

pub async fn register(State(state): State<AppState>, Json(req): Json<RegisterReq>) -> Result<Json<serde_json::Value>, StatusCode> {
    // validate password length
    if req.password.len() < 10 { return Err(StatusCode::BAD_REQUEST); }
    let existing = sqlx::query("SELECT id FROM users WHERE email=?").bind(&req.email).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if existing.is_some() { return Err(StatusCode::CONFLICT); }
    let id = Uuid::new_v4().to_string();
    let h = hash(&req.password, DEFAULT_COST).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let name = req.name.unwrap_or_else(|| req.email.split('@').next().unwrap_or("Usuario").to_string());
    sqlx::query("INSERT INTO users (id,email,password_hash,name,role,email_verified) VALUES (?,?,?,?, 'client',0)")
        .bind(&id).bind(&req.email).bind(&h).bind(&name)
        .execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // mongo audit
    let _ = state.mongo.collection::<mongodb::bson::Document>("auth_events").insert_one(mongodb::bson::doc!{"event":"register","email":&req.email,"ts":mongodb::bson::DateTime::now()}).await;
    Ok(Json(serde_json::json!({"ok":true,"message":"Registrado. Verifica tu correo."})))
}

pub async fn login(State(state): State<AppState>, Json(req): Json<LoginReq>) -> Result<Json<AuthResp>, StatusCode> {
    let row = sqlx::query("SELECT id,password_hash,name,role,email_verified FROM users WHERE email=?")
        .bind(&req.email).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = match row { Some(r) => r, None => return Err(StatusCode::UNAUTHORIZED) };
    let uid: String = row.get("id");
    let hashpw: String = row.get("password_hash");
    let name: String = row.get("name");
    let role: String = row.get("role");
    let verified: i8 = row.get("email_verified");
    if !verify(&req.password, &hashpw).unwrap_or(false) { return Err(StatusCode::UNAUTHORIZED); }
    // create session
    let access = Uuid::new_v4().to_string();
    let refresh = Uuid::new_v4().to_string();
    let exp = Utc::now() + Duration::minutes(15);
    let sid = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO sessions (id,user_id,access_token,refresh_token,expires_at) VALUES (?,?,?,?,?)")
        .bind(&sid).bind(&uid).bind(&access).bind(&refresh).bind(exp.naive_utc())
        .execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let user = serde_json::json!({"id":uid,"email":req.email,"name":name,"role":role,"email_verified":verified==1});
    Ok(Json(AuthResp { access_token: access, refresh_token: refresh, user }))
}

pub async fn logout(headers: HeaderMap, State(state): State<AppState>) -> StatusCode {
    if let Some(t) = token_from_headers(&headers) {
        let _ = sqlx::query("DELETE FROM sessions WHERE access_token=?").bind(&t).execute(&state.pool).await;
    }
    StatusCode::NO_CONTENT
}

pub async fn refresh(State(state): State<AppState>, Json(req): Json<RefreshReq>) -> Result<Json<AuthResp>, StatusCode> {
    let row = sqlx::query("SELECT s.user_id, s.id, u.email,u.name,u.role,u.email_verified FROM sessions s JOIN users u ON u.id=s.user_id WHERE s.refresh_token=?")
        .bind(&req.refresh_token).fetch_optional(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = match row { Some(r) => r, None => return Err(StatusCode::UNAUTHORIZED) };
    let uid: String = row.get("user_id");
    let sid: String = row.get("id");
    let email: String = row.get("email");
    let name: String = row.get("name");
    let role: String = row.get("role");
    let verified: i8 = row.get("email_verified");
    // rotate
    let access = Uuid::new_v4().to_string();
    let refresh = Uuid::new_v4().to_string();
    let exp = Utc::now() + Duration::minutes(15);
    let _ = sqlx::query("DELETE FROM sessions WHERE id=?").bind(&sid).execute(&state.pool).await;
    let newsid = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO sessions (id,user_id,access_token,refresh_token,expires_at) VALUES (?,?,?,?,?)")
        .bind(&newsid).bind(&uid).bind(&access).bind(&refresh).bind(exp.naive_utc())
        .execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let user = serde_json::json!({"id":uid,"email":email,"name":name,"role":role,"email_verified":verified==1});
    Ok(Json(AuthResp { access_token: access, refresh_token: refresh, user }))
}

pub async fn change_password(headers: HeaderMap, State(state): State<AppState>, Json(req): Json<ChangePwdReq>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = token_from_headers(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let row = sqlx::query("SELECT password_hash FROM users WHERE id=?").bind(&uid).fetch_one(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let h: String = row.get("password_hash");
    if !verify(&req.current_password, &h).unwrap_or(false) { return Err(StatusCode::UNAUTHORIZED); }
    let newh = hash(&req.new_password, DEFAULT_COST).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("UPDATE users SET password_hash=? WHERE id=?").bind(&newh).bind(&uid).execute(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // revoke other sessions
    sqlx::query("DELETE FROM sessions WHERE user_id=? AND access_token<>?").bind(&uid).bind(&t).execute(&state.pool).await.ok();
    Ok(Json(serde_json::json!({"ok":true})))
}

pub async fn forgot_password(State(state): State<AppState>, Json(req): Json<ForgotReq>) -> Json<serde_json::Value> {
    // generic response
    let _ = state.mongo.collection::<mongodb::bson::Document>("auth_events").insert_one(mongodb::bson::doc!{"event":"forgot","email":&req.email,"ts":mongodb::bson::DateTime::now()}).await;
    Json(serde_json::json!({"ok":true,"message":"Si el correo existe, se enviara un enlace."}))
}

pub async fn sessions(headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let t = token_from_headers(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user_id_from_token(&state, &t).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let rows = sqlx::query("SELECT id, created_at, expires_at FROM sessions WHERE user_id=?")
        .bind(&uid).fetch_all(&state.pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut list = Vec::new();
    for r in rows {
        list.push(serde_json::json!({
            "id": r.get::<String,_>("id"),
            "created_at": r.get::<chrono::NaiveDateTime,_>("created_at").to_string(),
            "expires_at": r.get::<chrono::NaiveDateTime,_>("expires_at").to_string(),
        }));
    }
    Ok(Json(serde_json::json!({"sessions":list})))
}
