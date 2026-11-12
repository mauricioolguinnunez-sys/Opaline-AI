use axum::http::StatusCode;
use axum::routing::get;
use axum::{http, Router};
use axum::body::Body;
use http::Request;
use opaline_backend::security::parse_cors_origins;
use tower::ServiceExt;
use tower_http::cors::{AllowOrigin, CorsLayer};

fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(parse_cors_origins(
            "http://localhost:8000,https://app.example.com",
        )))
        .allow_methods([http::Method::GET, http::Method::POST, http::Method::OPTIONS])
        .allow_headers([http::header::AUTHORIZATION, http::header::CONTENT_TYPE])
}

#[tokio::test]
async fn cors_allows_configured_origin() {
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .layer(cors_layer());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .header(http::header::ORIGIN, "http://localhost:8000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(http::header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(),
        "http://localhost:8000"
    );
}

#[tokio::test]
async fn cors_rejects_unconfigured_origin() {
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .layer(cors_layer());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .header(http::header::ORIGIN, "https://evil.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response
        .headers()
        .get(http::header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .is_none());
}

#[tokio::test]
async fn cors_preflight_returns_allowed_methods() {
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .layer(cors_layer());

    let response = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/health")
                .header(http::header::ORIGIN, "https://app.example.com")
                .header(http::header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(response.status().is_success());
    assert_eq!(
        response.headers().get(http::header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(),
        "https://app.example.com"
    );
    assert!(response
        .headers()
        .get(http::header::ACCESS_CONTROL_ALLOW_METHODS)
        .unwrap()
        .to_str()
        .unwrap()
        .contains("GET"));
}

#[tokio::test]
async fn cors_does_not_allow_wildcard_with_credentials() {
    let origins = parse_cors_origins("*");
    assert!(origins.is_empty());
}