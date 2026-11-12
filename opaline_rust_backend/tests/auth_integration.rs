mod common;

use common::{setup_state, test_connect_info, unique_email};
use opaline_backend::routes::auth::{self, ChangePwdReq, LoginReq, RefreshReq, RegisterReq};
use opaline_backend::routes::auth::hash_token;
use axum::Json;
use sqlx::Row;
use std::time::Duration;

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn auth_tokens_are_stored_as_hashes() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("hash");
    let password = "Abcdef1!xy";

    let response = auth::register(
        test_connect_info(),
        axum::extract::State(state.clone()),
        Json(RegisterReq {
            email: email.clone(),
            password: password.to_string(),
            name: Some("Test User".to_string()),
        }),
    )
    .await
    .expect("registration should succeed");

    let auth = response.0;
    let user_id = auth.user["id"].as_str().expect("user id");

    let row = sqlx::query("SELECT access_token, refresh_token FROM sessions WHERE user_id = ?")
        .bind(user_id)
        .fetch_one(&state.pool)
        .await
        .expect("session should exist");

    let stored_access: String = row.get("access_token");
    let stored_refresh: String = row.get("refresh_token");

    assert_eq!(stored_access, hash_token(&auth.access_token));
    assert_eq!(stored_refresh, hash_token(&auth.refresh_token));
    assert_ne!(stored_access, auth.access_token);
    assert_ne!(stored_refresh, auth.refresh_token);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn refresh_rotates_tokens_and_invalidates_the_previous_one() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("refresh");
    let password = "Abcdef1!xy";

    let registered = auth::register(
        test_connect_info(),
        axum::extract::State(state.clone()),
        Json(RegisterReq {
            email: email.clone(),
            password: password.to_string(),
            name: None,
        }),
    )
    .await
    .expect("registration should succeed")
    .0;

    let renewed = auth::refresh(
        test_connect_info(),
        axum::extract::State(state.clone()),
        Json(RefreshReq {
            refresh_token: registered.refresh_token.clone(),
        }),
    )
    .await
    .expect("valid refresh should succeed")
    .0;

    assert_ne!(renewed.access_token, registered.access_token);
    assert_ne!(renewed.refresh_token, registered.refresh_token);

    let old_refresh = auth::refresh(
        test_connect_info(),
        axum::extract::State(state.clone()),
        Json(RefreshReq {
            refresh_token: registered.refresh_token,
        }),
    )
    .await;
    assert!(old_refresh.is_err());

    let row = sqlx::query("SELECT refresh_token FROM sessions WHERE user_id = ?")
        .bind(registered.user["id"].as_str().unwrap())
        .fetch_one(&state.pool)
        .await
        .expect("session should remain");
    let stored_refresh: String = row.get("refresh_token");
    assert_eq!(stored_refresh, hash_token(&renewed.refresh_token));
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn concurrent_refresh_keeps_a_consistent_session() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("concurrent");
    let password = "Abcdef1!xy";

    let registered = auth::register(
        test_connect_info(),
        axum::extract::State(state.clone()),
        Json(RegisterReq {
            email,
            password: password.to_string(),
            name: None,
        }),
    )
    .await
    .expect("registration should succeed")
    .0;

    let state_a = state.clone();
    let state_b = state.clone();
    let token_a = registered.refresh_token.clone();
    let token_b = registered.refresh_token.clone();

    let task_a = tokio::spawn(async move {
        auth::refresh(
            test_connect_info(),
            axum::extract::State(state_a),
            Json(RefreshReq {
                refresh_token: token_a,
            }),
        )
        .await
    });
    let task_b = tokio::spawn(async move {
        auth::refresh(
            test_connect_info(),
            axum::extract::State(state_b),
            Json(RefreshReq {
                refresh_token: token_b,
            }),
        )
        .await
    });

    let result_a = task_a.await.expect("task should join");
    let result_b = task_b.await.expect("task should join");
    let successes = [result_a, result_b].iter().filter(|r| r.is_ok()).count();
    assert_eq!(successes, 1, "exactly one concurrent refresh should succeed");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE user_id = ?")
        .bind(registered.user["id"].as_str().unwrap())
        .fetch_one(&state.pool)
        .await
        .expect("count should succeed");
    assert_eq!(count, 1, "concurrent refresh must not create duplicate sessions");
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn replay_after_grace_period_revokes_the_session() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("replay");
    let password = "Abcdef1!xy";

    let registered = auth::register(
        test_connect_info(),
        axum::extract::State(state.clone()),
        Json(RegisterReq {
            email,
            password: password.to_string(),
            name: None,
        }),
    )
    .await
    .expect("registration should succeed")
    .0;

    let _ = auth::refresh(
        test_connect_info(),
        axum::extract::State(state.clone()),
        Json(RefreshReq {
            refresh_token: registered.refresh_token.clone(),
        }),
    )
    .await
    .expect("first refresh should succeed");

    tokio::time::sleep(Duration::from_secs(6)).await;

    let replay = auth::refresh(
        test_connect_info(),
        axum::extract::State(state.clone()),
        Json(RefreshReq {
            refresh_token: registered.refresh_token,
        }),
    )
    .await;
    assert!(replay.is_err());

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE user_id = ?")
        .bind(registered.user["id"].as_str().unwrap())
        .fetch_one(&state.pool)
        .await
        .expect("count should succeed");
    assert_eq!(count, 0, "replayed token should revoke the session");
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn expired_refresh_token_is_rejected() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("expired");
    let password = "Abcdef1!xy";

    let registered = auth::register(
        test_connect_info(),
        axum::extract::State(state.clone()),
        Json(RegisterReq {
            email,
            password: password.to_string(),
            name: None,
        }),
    )
    .await
    .expect("registration should succeed")
    .0;

    sqlx::query("UPDATE sessions SET refresh_expires_at = DATE_SUB(NOW(), INTERVAL 1 DAY) WHERE user_id = ?")
        .bind(registered.user["id"].as_str().unwrap())
        .execute(&state.pool)
        .await
        .expect("expiration update should succeed");

    let result = auth::refresh(
        test_connect_info(),
        axum::extract::State(state.clone()),
        Json(RefreshReq {
            refresh_token: registered.refresh_token,
        }),
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn logout_revokes_the_session_in_mysql() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("logout");
    let password = "Abcdef1!xy";

    let registered = auth::register(
        test_connect_info(),
        axum::extract::State(state.clone()),
        Json(RegisterReq {
            email,
            password: password.to_string(),
            name: None,
        }),
    )
    .await
    .expect("registration should succeed")
    .0;

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {}", registered.access_token).parse().unwrap(),
    );

    let status = auth::logout(headers, axum::extract::State(state.clone()))
        .await
        .expect("logout should succeed");
    assert_eq!(status, axum::http::StatusCode::NO_CONTENT);

    let new_pool = common::test_pool().await;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE user_id = ?")
        .bind(registered.user["id"].as_str().unwrap())
        .fetch_one(&new_pool)
        .await
        .expect("count should succeed");
    assert_eq!(count, 0);
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn change_password_revokes_other_sessions() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("password");
    let password = "Abcdef1!xy";
    let new_password = "Zyxwvu9!ab";

    let first = auth::register(
        test_connect_info(),
        axum::extract::State(state.clone()),
        Json(RegisterReq {
            email: email.clone(),
            password: password.to_string(),
            name: None,
        }),
    )
    .await
    .expect("registration should succeed")
    .0;

    let second = auth::login(
        test_connect_info(),
        axum::extract::State(state.clone()),
        Json(LoginReq {
            email: email.clone(),
            password: password.to_string(),
        }),
    )
    .await
    .expect("second login should succeed")
    .0;

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {}", first.access_token).parse().unwrap(),
    );

    let response = auth::change_password(
        headers,
        axum::extract::State(state.clone()),
        Json(ChangePwdReq {
            current_password: password.to_string(),
            new_password: new_password.to_string(),
        }),
    )
    .await
    .expect("password change should succeed");
    assert_eq!(response.0["ok"], true);

    let old_login = auth::login(
        test_connect_info(),
        axum::extract::State(state.clone()),
        Json(LoginReq {
            email: email.clone(),
            password: password.to_string(),
        }),
    )
    .await;
    assert!(old_login.is_err());

    let new_login = auth::login(
        test_connect_info(),
        axum::extract::State(state.clone()),
        Json(LoginReq {
            email,
            password: new_password.to_string(),
        }),
    )
    .await;
    assert!(new_login.is_ok());

    let revoked_refresh = auth::refresh(
        test_connect_info(),
        axum::extract::State(state.clone()),
        Json(RefreshReq {
            refresh_token: second.refresh_token,
        }),
    )
    .await;
    assert!(revoked_refresh.is_err());
}

#[tokio::test]
#[ignore = "requires OPALINE_TEST_DATABASE_URL pointing to an isolated test database"]
async fn session_state_persists_across_connections() {
    let _guard = common::test_lock().await;
    let state = setup_state().await;
    let email = unique_email("persist");
    let password = "Abcdef1!xy";

    let registered = auth::register(
        test_connect_info(),
        axum::extract::State(state.clone()),
        Json(RegisterReq {
            email,
            password: password.to_string(),
            name: None,
        }),
    )
    .await
    .expect("registration should succeed")
    .0;

    let user_id = registered.user["id"].as_str().unwrap().to_string();
    drop(state);

    let new_pool = common::test_pool().await;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE user_id = ?")
        .bind(user_id)
        .fetch_one(&new_pool)
        .await
        .expect("session count should succeed");
    assert_eq!(count, 1);
}