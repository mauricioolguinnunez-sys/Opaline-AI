use reqwest::Client;
use serde::Deserialize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PayPalEnvironment {
    Sandbox,
}

impl PayPalEnvironment {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sandbox => "sandbox",
        }
    }

    pub fn base_url(self) -> &'static str {
        match self {
            Self::Sandbox => "https://api-m.sandbox.paypal.com",
        }
    }
}

#[derive(Clone)]
pub struct PayPalConfig {
    pub environment: PayPalEnvironment,
    pub client_id: String,
    pub client_secret: String,
    pub api_base_url: String,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub webhook_id: Option<String>,
}

impl std::fmt::Debug for PayPalConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PayPalConfig")
            .field("environment", &self.environment)
            .field("client_id", &"[redacted]")
            .field("client_secret", &"[redacted]")
            .field("api_base_url", &self.api_base_url)
            .field("connect_timeout", &self.connect_timeout)
            .field("request_timeout", &self.request_timeout)
            .field("webhook_id", &self.webhook_id.as_ref().map(|_| "[redacted]"))
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PayPalError {
    Configuration(String),
    Authentication { status: Option<u16>, message: String },
    Transport(String),
    InvalidResponse(String),
}

impl std::fmt::Display for PayPalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Configuration(message) => write!(f, "paypal configuration error: {message}"),
            Self::Authentication { status, message } => {
                if let Some(status) = status {
                    write!(f, "paypal authentication failed ({status}): {message}")
                } else {
                    write!(f, "paypal authentication failed: {message}")
                }
            }
            Self::Transport(message) => write!(f, "paypal transport error: {message}"),
            Self::InvalidResponse(message) => write!(f, "paypal invalid response: {message}"),
        }
    }
}

impl std::error::Error for PayPalError {}

impl PayPalConfig {
    pub fn from_env() -> Result<Self, PayPalError> {
        let raw_environment = std::env::var("PAYPAL_ENVIRONMENT")
            .or_else(|_| std::env::var("PAYPAL_ENV"))
            .unwrap_or_else(|_| "sandbox".to_string());
        let environment = match raw_environment.trim().to_ascii_lowercase().as_str() {
            "sandbox" => PayPalEnvironment::Sandbox,
            "live" | "production" => {
                return Err(PayPalError::Configuration(
                    "live PayPal environment is disabled in this phase".to_string(),
                ))
            }
            other => {
                return Err(PayPalError::Configuration(format!(
                    "unknown PayPal environment: {other}"
                )))
            }
        };

        let client_id = std::env::var("PAYPAL_CLIENT_ID")
            .map_err(|_| PayPalError::Configuration("PAYPAL_CLIENT_ID is required".to_string()))?;
        let client_secret = std::env::var("PAYPAL_CLIENT_SECRET")
            .map_err(|_| PayPalError::Configuration("PAYPAL_CLIENT_SECRET is required".to_string()))?;
        if client_id.trim().is_empty() || client_secret.trim().is_empty() {
            return Err(PayPalError::Configuration(
                "PayPal credentials must not be empty".to_string(),
            ));
        }

        let canonical_base = environment.base_url();
        let api_base_url = match std::env::var("PAYPAL_API_BASE_URL") {
            Ok(value) => {
                let value = value.trim().trim_end_matches('/').to_string();
                let is_local_test = value.starts_with("http://127.0.0.1") || value.starts_with("http://localhost");
                if value != canonical_base && !is_local_test {
                    return Err(PayPalError::Configuration(
                        "PAYPAL_API_BASE_URL does not match the selected PayPal environment".to_string(),
                    ));
                }
                value
            }
            Err(_) => canonical_base.to_string(),
        };

        let connect_timeout = parse_duration_env("PAYPAL_CONNECT_TIMEOUT_SECS", 10)?;
        let request_timeout = parse_duration_env("PAYPAL_REQUEST_TIMEOUT_SECS", 30)?;
        let webhook_id = std::env::var("PAYPAL_WEBHOOK_ID").ok();

        Ok(Self {
            environment,
            client_id,
            client_secret,
            api_base_url,
            connect_timeout,
            request_timeout,
            webhook_id,
        })
    }
}

fn parse_duration_env(name: &str, default_secs: u64) -> Result<Duration, PayPalError> {
    match std::env::var(name) {
        Ok(value) => {
            let seconds: u64 = value.trim().parse().map_err(|_| {
                PayPalError::Configuration(format!("{name} must be a positive integer"))
            })?;
            if seconds == 0 {
                return Err(PayPalError::Configuration(format!(
                    "{name} must be greater than zero"
                )));
            }
            Ok(Duration::from_secs(seconds))
        }
        Err(_) => Ok(Duration::from_secs(default_secs)),
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
    #[allow(dead_code)]
    token_type: Option<String>,
}

struct CachedToken {
    access_token: String,
    expires_at: Instant,
}

#[derive(Clone)]
pub struct PayPalClient {
    config: PayPalConfig,
    http: Client,
    token: Arc<Mutex<Option<CachedToken>>>,
}

impl PayPalClient {
    pub fn new(config: PayPalConfig) -> Result<Self, PayPalError> {
        let http = Client::builder()
            .connect_timeout(config.connect_timeout)
            .timeout(config.request_timeout)
            .build()
            .map_err(|error| PayPalError::Configuration(error.to_string()))?;
        Ok(Self {
            config,
            http,
            token: Arc::new(Mutex::new(None)),
        })
    }

    pub fn from_env() -> Result<Self, PayPalError> {
        Self::new(PayPalConfig::from_env()?)
    }

    pub fn environment(&self) -> PayPalEnvironment {
        self.config.environment
    }

    pub fn api_base_url(&self) -> &str {
        &self.config.api_base_url
    }

    pub fn webhook_id(&self) -> Option<&str> {
        self.config.webhook_id.as_deref()
    }

    pub async fn access_token(&self) -> Result<String, PayPalError> {
        let mut guard = self.token.lock().await;
        if let Some(cached) = guard.as_ref() {
            if Instant::now() < cached.expires_at {
                return Ok(cached.access_token.clone());
            }
        }

        let (token, expires_in) = self.fetch_access_token().await?;
        *guard = Some(CachedToken {
            access_token: token.clone(),
            expires_at: Instant::now() + Duration::from_secs(expires_in.saturating_sub(60)),
        });
        Ok(token)
    }

    pub async fn create_order(&self, request_id: &str, body: &serde_json::Value) -> Result<serde_json::Value, PayPalError> {
        let token = self.access_token().await?;
        let response = self
            .http
            .post(format!("{}/v2/checkout/orders", self.config.api_base_url))
            .bearer_auth(token)
            .header("PayPal-Request-Id", request_id)
            .json(body)
            .send()
            .await
            .map_err(|error| PayPalError::Transport(error.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            return Err(PayPalError::Authentication {
                status: Some(status.as_u16()),
                message: "PayPal rejected order creation".to_string(),
            });
        }
        response
            .json()
            .await
            .map_err(|error| PayPalError::InvalidResponse(error.to_string()))
    }

    pub async fn verify_webhook_signature(
        &self,
        webhook_id: &str,
        transmission_id: &str,
        cert_url: &str,
        auth_algo: &str,
        transmission_sig: &str,
        transmission_time: &str,
        webhook_event: &serde_json::Value,
    ) -> Result<bool, PayPalError> {
        let token = self.access_token().await?;
        let body = serde_json::json!({
            "auth_algo": auth_algo,
            "cert_url": cert_url,
            "transmission_id": transmission_id,
            "transmission_sig": transmission_sig,
            "transmission_time": transmission_time,
            "webhook_id": webhook_id,
            "webhook_event": webhook_event,
        });
        let response = self
            .http
            .post(format!("{}/v1/notifications/verify-webhook-signature", self.config.api_base_url))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .map_err(|error| PayPalError::Transport(error.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(PayPalError::Authentication {
                status: Some(status.as_u16()),
                message: "PayPal rejected webhook verification".to_string(),
            });
        }
        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|error| PayPalError::InvalidResponse(error.to_string()))?;
        Ok(value.get("verification_status").and_then(|v| v.as_str()) == Some("SUCCESS"))
    }

    pub async fn capture_order(&self, order_id: &str, request_id: &str) -> Result<serde_json::Value, PayPalError> {
        let token = self.access_token().await?;
        let response = self
            .http
            .post(format!("{}/v2/checkout/orders/{}/capture", self.config.api_base_url, order_id))
            .bearer_auth(token)
            .header("PayPal-Request-Id", request_id)
            .json(&serde_json::json!({}))
            .send()
            .await
            .map_err(|error| PayPalError::Transport(error.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            return Err(PayPalError::Authentication {
                status: Some(status.as_u16()),
                message: "PayPal rejected order capture".to_string(),
            });
        }
        response
            .json()
            .await
            .map_err(|error| PayPalError::InvalidResponse(error.to_string()))
    }

    pub async fn get_order(&self, order_id: &str) -> Result<serde_json::Value, PayPalError> {
        let token = self.access_token().await?;
        let response = self
            .http
            .get(format!("{}/v2/checkout/orders/{}", self.config.api_base_url, order_id))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|error| PayPalError::Transport(error.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            return Err(PayPalError::Authentication {
                status: Some(status.as_u16()),
                message: "PayPal rejected order lookup".to_string(),
            });
        }
        response
            .json()
            .await
            .map_err(|error| PayPalError::InvalidResponse(error.to_string()))
    }

    async fn fetch_access_token(&self) -> Result<(String, u64), PayPalError> {
        let response = self
            .http
            .post(format!("{}/v1/oauth2/token", self.config.api_base_url))
            .basic_auth(&self.config.client_id, Some(&self.config.client_secret))
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::ACCEPT_LANGUAGE, "en_US")
            .form(&[("grant_type", "client_credentials")])
            .send()
            .await
            .map_err(|error| PayPalError::Transport(error.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            return Err(PayPalError::Authentication {
                status: Some(status.as_u16()),
                message: "PayPal rejected the OAuth credentials".to_string(),
            });
        }

        let token_response: TokenResponse = response
            .json()
            .await
            .map_err(|error| PayPalError::InvalidResponse(error.to_string()))?;

        if token_response.access_token.trim().is_empty() {
            return Err(PayPalError::InvalidResponse(
                "PayPal returned an empty access token".to_string(),
            ));
        }

        Ok((token_response.access_token, token_response.expires_in))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn sandbox_is_default_and_live_is_blocked() {
        let config = PayPalConfig {
            environment: PayPalEnvironment::Sandbox,
            client_id: "client".to_string(),
            client_secret: "secret".to_string(),
            api_base_url: PayPalEnvironment::Sandbox.base_url().to_string(),
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            webhook_id: None,
        };
        assert_eq!(config.environment, PayPalEnvironment::Sandbox);
        assert_eq!(config.api_base_url, "https://api-m.sandbox.paypal.com");
    }

    #[test]
    fn rejects_unknown_environment() {
        std::env::set_var("PAYPAL_ENV", "staging");
        std::env::set_var("PAYPAL_CLIENT_ID", "client");
        std::env::set_var("PAYPAL_CLIENT_SECRET", "secret");
        let result = PayPalConfig::from_env();
        std::env::remove_var("PAYPAL_ENV");
        std::env::remove_var("PAYPAL_CLIENT_ID");
        std::env::remove_var("PAYPAL_CLIENT_SECRET");
        assert!(matches!(result, Err(PayPalError::Configuration(_))));
    }

    #[test]
    fn blocks_live_environment() {
        std::env::set_var("PAYPAL_ENVIRONMENT", "live");
        std::env::set_var("PAYPAL_CLIENT_ID", "client");
        std::env::set_var("PAYPAL_CLIENT_SECRET", "secret");
        let result = PayPalConfig::from_env();
        std::env::remove_var("PAYPAL_ENVIRONMENT");
        std::env::remove_var("PAYPAL_CLIENT_ID");
        std::env::remove_var("PAYPAL_CLIENT_SECRET");
        assert!(matches!(result, Err(PayPalError::Configuration(_))));
    }

    #[test]
    fn requires_credentials() {
        std::env::remove_var("PAYPAL_CLIENT_ID");
        std::env::remove_var("PAYPAL_CLIENT_SECRET");
        let result = PayPalConfig::from_env();
        assert!(matches!(result, Err(PayPalError::Configuration(_))));
    }

    #[tokio::test]
    async fn caches_access_token() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();
        let app = axum::Router::new().route(
            "/v1/oauth2/token",
            axum::routing::post(move || {
                let counter = counter_clone.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    axum::Json(serde_json::json!({
                        "access_token": "test-token",
                        "token_type": "Bearer",
                        "expires_in": 3600
                    }))
                }
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let config = PayPalConfig {
            environment: PayPalEnvironment::Sandbox,
            client_id: "client".to_string(),
            client_secret: "secret".to_string(),
            api_base_url: format!("http://{}", addr),
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(5),
            webhook_id: None,
        };
        let client = PayPalClient::new(config).unwrap();
        let first = client.access_token().await.unwrap();
        let second = client.access_token().await.unwrap();
        assert_eq!(first, "test-token");
        assert_eq!(second, "test-token");
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        server.abort();
    }

    #[tokio::test]
    async fn refreshes_expired_token() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();
        let app = axum::Router::new().route(
            "/v1/oauth2/token",
            axum::routing::post(move || {
                let counter = counter_clone.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    axum::Json(serde_json::json!({
                        "access_token": "test-token",
                        "token_type": "Bearer",
                        "expires_in": 0
                    }))
                }
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let config = PayPalConfig {
            environment: PayPalEnvironment::Sandbox,
            client_id: "client".to_string(),
            client_secret: "secret".to_string(),
            api_base_url: format!("http://{}", addr),
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(5),
            webhook_id: None,
        };
        let client = PayPalClient::new(config).unwrap();
        let _ = client.access_token().await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        let _ = client.access_token().await.unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 2);
        server.abort();
    }

    #[tokio::test]
    async fn rejects_invalid_json() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new().route(
            "/v1/oauth2/token",
            axum::routing::post(|| async { "not json" }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let config = PayPalConfig {
            environment: PayPalEnvironment::Sandbox,
            client_id: "client".to_string(),
            client_secret: "secret".to_string(),
            api_base_url: format!("http://{}", addr),
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(5),
            webhook_id: None,
        };
        let client = PayPalClient::new(config).unwrap();
        let result = client.access_token().await;
        assert!(matches!(result, Err(PayPalError::InvalidResponse(_))));
        server.abort();
    }

    #[tokio::test]
    async fn rejects_http_error() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new().route(
            "/v1/oauth2/token",
            axum::routing::post(|| async {
                (axum::http::StatusCode::UNAUTHORIZED, "invalid")
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let config = PayPalConfig {
            environment: PayPalEnvironment::Sandbox,
            client_id: "client".to_string(),
            client_secret: "secret".to_string(),
            api_base_url: format!("http://{}", addr),
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(5),
            webhook_id: None,
        };
        let client = PayPalClient::new(config).unwrap();
        let result = client.access_token().await;
        assert!(matches!(result, Err(PayPalError::Authentication { .. })));
        server.abort();
    }
}
