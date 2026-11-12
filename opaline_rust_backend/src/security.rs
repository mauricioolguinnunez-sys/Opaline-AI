use axum::http::HeaderValue;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct RateLimiter {
    inner: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Returns true when the request is allowed.
    pub fn check(&self, key: &str, limit: u32, window: Duration) -> bool {
        let mut guard = self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let now = Instant::now();
        let entries = guard.entry(key.to_string()).or_default();
        entries.retain(|timestamp| now.duration_since(*timestamp) < window);

        if entries.len() >= limit as usize {
            false
        } else {
            entries.push(now);
            true
        }
    }
}

pub fn parse_cors_origins(value: &str) -> Vec<HeaderValue> {
    let mut origins = Vec::new();
    for origin in value.split(',') {
        let origin = origin.trim();
        if origin.is_empty() {
            continue;
        }
        // CORS origins must be absolute HTTP(S) origins. This also rejects
        // malformed or wildcard values such as "*" from being treated as valid.
        if !origin.starts_with("http://") && !origin.starts_with("https://") {
            continue;
        }
        if origin.chars().any(char::is_whitespace) {
            continue;
        }
        if let Ok(value) = HeaderValue::from_str(origin) {
            origins.push(value);
        }
    }
    origins
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn allows_requests_until_limit() {
        let limiter = RateLimiter::new();
        assert!(limiter.check("k", 2, Duration::from_secs(60)));
        assert!(limiter.check("k", 2, Duration::from_secs(60)));
        assert!(!limiter.check("k", 2, Duration::from_secs(60)));
    }

    #[test]
    fn separates_keys() {
        let limiter = RateLimiter::new();
        assert!(limiter.check("a", 1, Duration::from_secs(60)));
        assert!(limiter.check("b", 1, Duration::from_secs(60)));
        assert!(!limiter.check("a", 1, Duration::from_secs(60)));
    }

    #[test]
    fn is_shared_between_clones() {
        let limiter = RateLimiter::new();
        let clone = limiter.clone();
        assert!(limiter.check("k", 1, Duration::from_secs(60)));
        assert!(!clone.check("k", 1, Duration::from_secs(60)));
    }

    #[test]
    fn parses_exact_cors_origins() {
        let origins = parse_cors_origins("http://localhost:8000, https://app.example.com ,invalid");
        assert_eq!(origins.len(), 2);
        assert_eq!(origins[0].to_str().unwrap(), "http://localhost:8000");
        assert_eq!(origins[1].to_str().unwrap(), "https://app.example.com");
    }

    #[test]
    fn empty_cors_value_yields_no_origins() {
        assert!(parse_cors_origins("").is_empty());
        assert!(parse_cors_origins("   ").is_empty());
    }

    #[test]
    fn concurrent_checks_do_not_exceed_limit() {
        let limiter = RateLimiter::new();
        let mut handles = Vec::new();
        for _ in 0..8 {
            let l = limiter.clone();
            handles.push(thread::spawn(move || l.check("k", 3, Duration::from_secs(60))));
        }
        let allowed = handles.into_iter().map(|h| h.join().unwrap()).filter(|v| *v).count();
        assert_eq!(allowed, 3);
    }
}