mod common;

use common::{setup_state, test_connect_info, unique_email};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use opaline_backend::routes::wall::{self, CreatePostReq, ReportReq, UpdatePostReq};
use opaline_backend::routes::admin::require_admin;
use sqlx::Row;

async fn register_user(state: &opaline_backend::state::AppState, email: &str) -> (String, String) {
    let response = opaline_backend::routes::auth::register(
        test_connect_info(),
        axum::extract::State(state.clone()),
        Json(opaline_backend::routes::auth::RegisterReq {
            email: email.to_string(),
            password: "Abcdef1!xy".to_string(),
            name: Some("Wall Tester".to_string()),
        }),
    )
    .await
    .expect("registration should succeed")
    .0;
    let token = response.access_token;
    let user_id = response.user["id"].as_str().unwrap().to_string();

    sqlx::query("INSERT INTO subscriptions (id, user_id, plan_id, status, start_date, end_date) VALUES (?,?,?,?,NOW(),DATE_ADD(NOW(),INTERVAL 1 MONTH))")
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&user_id)
        .bind("plan-starter")
        .bind("active")
        .execute(&state.pool)
        .await
        .expect("active subscription should be created");

    (token, user_id)
}

fn bearer(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {}", token).parse().unwrap(),
    );
    headers
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn wall_rejects_invalid_content_and_rating() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let (token, _) = register_user(&state, &unique_email("wall-invalid")).await;
    let headers = bearer(&token);

    for content in ["too short", &"a".repeat(1001)] {
        let result = wall::create_post(
            headers.clone(),
            axum::extract::State(state.clone()),
            Json(CreatePostReq {
                content: content.to_string(),
                rating: Some(5),
                idempotency_key: None,
            }),
        )
        .await;
        assert_eq!(result.unwrap_err(), StatusCode::BAD_REQUEST);
    }

    let result = wall::create_post(
        headers.clone(),
        axum::extract::State(state.clone()),
        Json(CreatePostReq {
            content: "<script>alert(1)</script>".to_string() + &"a".repeat(5),
            rating: Some(5),
            idempotency_key: None,
        }),
    )
    .await;
    assert_eq!(result.unwrap_err(), StatusCode::BAD_REQUEST);

    for rating in [Some(0), Some(6)] {
        let result = wall::create_post(
            headers.clone(),
            axum::extract::State(state.clone()),
            Json(CreatePostReq {
                content: "Este contenido tiene la longitud minima requerida.".to_string(),
                rating,
                idempotency_key: None,
            }),
        )
        .await;
        assert_eq!(result.unwrap_err(), StatusCode::BAD_REQUEST);
    }

    let result = wall::create_post(
        headers,
        axum::extract::State(state.clone()),
        Json(CreatePostReq {
            content: "Este contenido tiene la longitud minima requerida.".to_string(),
            rating: None,
            idempotency_key: None,
        }),
    )
    .await;
    assert!(result.is_ok());
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn wall_requires_active_subscription() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("wall-nosub");
    let registered = opaline_backend::routes::auth::register(
        test_connect_info(),
        axum::extract::State(state.clone()),
        Json(opaline_backend::routes::auth::RegisterReq {
            email: email.clone(),
            password: "Abcdef1!xy".to_string(),
            name: None,
        }),
    )
    .await
    .expect("registration should succeed")
    .0;

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {}", registered.access_token).parse().unwrap(),
    );

    let result = wall::create_post(
        headers,
        axum::extract::State(state),
        Json(CreatePostReq {
            content: "Este contenido tiene la longitud minima requerida.".to_string(),
            rating: None,
            idempotency_key: None,
        }),
    )
    .await;
    assert_eq!(result.unwrap_err(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn wall_enforces_ownership_and_edit_window() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let (token_a, _) = register_user(&state, &unique_email("owner-a")).await;
    let (token_b, _) = register_user(&state, &unique_email("owner-b")).await;

    let created = wall::create_post(
        bearer(&token_a),
        axum::extract::State(state.clone()),
        Json(CreatePostReq {
            content: "Publicacion original con longitud suficiente para la prueba.".to_string(),
            rating: Some(5),
            idempotency_key: None,
        }),
    )
    .await
    .expect("owner should create post")
    .0;
    let post_id = created["id"].as_str().unwrap().to_string();

    let foreign_update = wall::update_post(
        axum::extract::Path(post_id.clone()),
        bearer(&token_b),
        axum::extract::State(state.clone()),
        Json(UpdatePostReq {
            content: "Intento de edicion ajena con longitud suficiente.".to_string(),
        }),
    )
    .await;
    assert_eq!(foreign_update.unwrap_err(), StatusCode::FORBIDDEN);

    let foreign_delete = wall::delete_post(
        axum::extract::Path(post_id.clone()),
        bearer(&token_b),
        axum::extract::State(state.clone()),
    )
    .await;
    assert_eq!(foreign_delete.unwrap_err(), StatusCode::FORBIDDEN);

    sqlx::query("UPDATE posts SET created_at = DATE_SUB(NOW(), INTERVAL 25 HOUR) WHERE id = ?")
        .bind(&post_id)
        .execute(&state.pool)
        .await
        .expect("timestamp update should succeed");

    let expired_update = wall::update_post(
        axum::extract::Path(post_id),
        bearer(&token_a),
        axum::extract::State(state.clone()),
        Json(UpdatePostReq {
            content: "Edicion tardia con longitud suficiente para la prueba.".to_string(),
        }),
    )
    .await;
    assert_eq!(expired_update.unwrap_err(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn wall_idempotency_prevents_duplicate_posts() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let (token, _) = register_user(&state, &unique_email("idempotent")).await;
    let headers = bearer(&token);
    let key = uuid::Uuid::new_v4().to_string();
    let content = "Publicacion idempotente con longitud suficiente para la prueba.".to_string();

    let first = wall::create_post(
        headers.clone(),
        axum::extract::State(state.clone()),
        Json(CreatePostReq {
            content: content.clone(),
            rating: Some(4),
            idempotency_key: Some(key.clone()),
        }),
    )
    .await
    .expect("first request should create")
    .0;
    let first_id = first["id"].as_str().unwrap().to_string();

    let second = wall::create_post(
        headers,
        axum::extract::State(state.clone()),
        Json(CreatePostReq {
            content: content.clone(),
            rating: Some(4),
            idempotency_key: Some(key),
        }),
    )
    .await
    .expect("repeated request should be idempotent")
    .0;
    let second_id = second["id"].as_str().unwrap().to_string();

    assert_eq!(first_id, second_id);
    assert_eq!(second["duplicate"], true);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM posts WHERE id = ?")
        .bind(&first_id)
        .fetch_one(&state.pool)
        .await
        .expect("count should succeed");
    assert_eq!(count, 1);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn wall_reports_are_deduplicated_and_hide_after_three_users() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let (owner_token, _) = register_user(&state, &unique_email("report-owner")).await;

    let created = wall::create_post(
        bearer(&owner_token),
        axum::extract::State(state.clone()),
        Json(CreatePostReq {
            content: "Publicacion para reportes con longitud suficiente para la prueba.".to_string(),
            rating: None,
            idempotency_key: None,
        }),
    )
    .await
    .expect("owner should create post")
    .0;
    let post_id = created["id"].as_str().unwrap().to_string();

    let mut reporters = Vec::new();
    for _ in 0..3 {
        let (token, _) = register_user(&state, &unique_email("reporter")).await;
        reporters.push(token);
    }

    for token in &reporters {
        let result = wall::report_post(
            axum::extract::Path(post_id.clone()),
            bearer(token),
            axum::extract::State(state.clone()),
            Json(ReportReq {
                reason: "spam".to_string(),
                detail: Some("detalle".to_string()),
            }),
        )
        .await;
        assert!(result.is_ok());
    }

    let duplicate = wall::report_post(
        axum::extract::Path(post_id.clone()),
        bearer(&reporters[0]),
        axum::extract::State(state.clone()),
        Json(ReportReq {
            reason: "spam".to_string(),
            detail: None,
        }),
    )
    .await
    .expect("duplicate report should be idempotent")
    .0;
    assert_eq!(duplicate["duplicate"], true);

    let report_count: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT user_id) FROM reports WHERE post_id = ?")
        .bind(&post_id)
        .fetch_one(&state.pool)
        .await
        .expect("report count should succeed");
    assert_eq!(report_count, 3);

    let status: String = sqlx::query_scalar("SELECT status FROM posts WHERE id = ?")
        .bind(&post_id)
        .fetch_one(&state.pool)
        .await
        .expect("post should exist");
    assert_eq!(status, "hidden");
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn wall_moderation_requires_admin() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let (user_token, _) = register_user(&state, &unique_email("normal-user")).await;

    let normal_list = wall::list_moderation(
        bearer(&user_token),
        axum::extract::State(state.clone()),
    )
    .await;
    assert_eq!(normal_list.unwrap_err(), StatusCode::FORBIDDEN);

    let normal_moderate = wall::moderate_post(
        axum::extract::Path("00000000-0000-0000-0000-000000000000".to_string()),
        bearer(&user_token),
        axum::extract::State(state.clone()),
        Json(wall::ModerateReq {
            action: "hide".to_string(),
            reason: Some("motivo".to_string()),
        }),
    )
    .await;
    assert_eq!(normal_moderate.unwrap_err(), StatusCode::FORBIDDEN);

    // Create an admin user.
    let admin_email = unique_email("admin");
    let registered = opaline_backend::routes::auth::register(
        test_connect_info(),
        axum::extract::State(state.clone()),
        Json(opaline_backend::routes::auth::RegisterReq {
            email: admin_email.clone(),
            password: "Abcdef1!xy".to_string(),
            name: Some("Admin".to_string()),
        }),
    )
    .await
    .expect("admin registration should succeed")
    .0;
    sqlx::query("UPDATE users SET role = 'admin' WHERE id = ?")
        .bind(registered.user["id"].as_str().unwrap())
        .execute(&state.pool)
        .await
        .expect("admin role update should succeed");

    let admin_list = wall::list_moderation(
        bearer(&registered.access_token),
        axum::extract::State(state.clone()),
    )
    .await;
    assert!(admin_list.is_ok());

    let (_owner_token, _) = register_user(&state, &unique_email("moderated-owner")).await;
    let created = wall::create_post(
        bearer(&_owner_token),
        axum::extract::State(state.clone()),
        Json(CreatePostReq {
            content: "Publicacion para moderacion con longitud suficiente.".to_string(),
            rating: None,
            idempotency_key: None,
        }),
    )
    .await
    .expect("owner should create post")
    .0;
    let post_id = created["id"].as_str().unwrap().to_string();

    let hide_without_reason = wall::moderate_post(
        axum::extract::Path(post_id.clone()),
        bearer(&registered.access_token),
        axum::extract::State(state.clone()),
        Json(wall::ModerateReq {
            action: "hide".to_string(),
            reason: None,
        }),
    )
    .await;
    assert_eq!(hide_without_reason.unwrap_err(), StatusCode::BAD_REQUEST);

    let hide = wall::moderate_post(
        axum::extract::Path(post_id.clone()),
        bearer(&registered.access_token),
        axum::extract::State(state.clone()),
        Json(wall::ModerateReq {
            action: "hide".to_string(),
            reason: Some("motivo de moderacion".to_string()),
        }),
    )
    .await;
    assert!(hide.is_ok());

    let status: String = sqlx::query_scalar("SELECT status FROM posts WHERE id = ?")
        .bind(&post_id)
        .fetch_one(&state.pool)
        .await
        .expect("post should exist");
    assert_eq!(status, "hidden");
}