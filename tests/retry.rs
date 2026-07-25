//! Tests for src/http.rs's transient-failure retrying. The retry policy is
//! deliberately asymmetric -- replayable requests are retried, POST/PATCH are
//! not -- so these cover both halves, plus the "don't retry a client error"
//! case that would otherwise turn one clear 400 into four.
//!
//! Timeout behaviour lives in tests/retry_timeout.rs instead: it has to call
//! `set_transport_options`, which writes a process-global `OnceLock` that
//! can't be reset, so it needs its own test binary.

use std::sync::atomic::{AtomicUsize, Ordering};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

/// Fails with `status` for the first `fail_times` calls, then succeeds with
/// `body` (an array for list endpoints, an object for everything else).
struct FailThenSucceed {
    calls: AtomicUsize,
    fail_times: usize,
    status: u16,
    body: serde_json::Value,
}

impl FailThenSucceed {
    fn new(fail_times: usize, status: u16) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            fail_times,
            status,
            body: serde_json::json!({"recovered": true}),
        }
    }

    fn with_body(mut self, body: serde_json::Value) -> Self {
        self.body = body;
        self
    }
}

impl Respond for FailThenSucceed {
    fn respond(&self, _req: &Request) -> ResponseTemplate {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        if n < self.fail_times {
            ResponseTemplate::new(self.status)
        } else {
            ResponseTemplate::new(200).set_body_json(self.body.clone())
        }
    }
}

#[tokio::test]
async fn get_retries_a_transient_5xx_and_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/things/abc/"))
        .respond_with(FailThenSucceed::new(1, 503))
        // 1 failed attempt + 1 successful retry.
        .expect(2)
        .mount(&server)
        .await;

    let result = waldur_cli::http::call_one(
        &server.uri(),
        Some("t"),
        reqwest::Method::GET,
        "/api/things/abc/",
        None,
    )
    .await
    .unwrap();

    assert_eq!(result, serde_json::json!({"recovered": true}));
}

#[tokio::test]
async fn get_retries_a_429_rate_limit() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/things/"))
        .respond_with(FailThenSucceed::new(1, 429))
        .expect(2)
        .mount(&server)
        .await;

    waldur_cli::http::call_one(&server.uri(), Some("t"), reqwest::Method::GET, "/api/things/", None)
        .await
        .unwrap();
}

#[tokio::test]
async fn post_is_never_retried_even_on_a_transient_5xx() {
    let server = MockServer::start().await;
    // The safety-critical case: a POST that 503s may still have been applied
    // server-side, so replaying it can duplicate the effect -- for
    // `provision` that's a second marketplace order someone pays for. Exactly
    // one attempt, then surface the error.
    Mock::given(method("POST"))
        .and(path("/api/things/"))
        .respond_with(ResponseTemplate::new(503).set_body_string("upstream unavailable"))
        .expect(1)
        .mount(&server)
        .await;

    let err = waldur_cli::http::call_one(
        &server.uri(),
        Some("t"),
        reqwest::Method::POST,
        "/api/things/",
        Some(r#"{"name":"x"}"#),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("503"));
}

#[tokio::test]
async fn a_client_error_is_not_retried() {
    let server = MockServer::start().await;
    // A 400 is the caller's fault and will fail identically every time --
    // retrying only delays the error message and quadruples the load.
    Mock::given(method("GET"))
        .and(path("/api/things/bogus/"))
        .respond_with(ResponseTemplate::new(400).set_body_string("bad request"))
        .expect(1)
        .mount(&server)
        .await;

    let err = waldur_cli::http::call_one(
        &server.uri(),
        Some("t"),
        reqwest::Method::GET,
        "/api/things/bogus/",
        None,
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("400"));
}

#[tokio::test]
async fn list_pagination_also_retries() {
    let server = MockServer::start().await;
    // fetch_all builds its own client separately from call_one -- a `list`
    // is the longest-running, most retry-worthy thing the CLI does, so it
    // would be the worst path to leave unprotected.
    Mock::given(method("GET"))
        .and(path("/api/things/"))
        .respond_with(
            FailThenSucceed::new(1, 502).with_body(serde_json::json!([{"uuid": "abc"}])),
        )
        .expect(2)
        .mount(&server)
        .await;

    let items = waldur_cli::pagination::fetch_all(&server.uri(), Some("t"), "/api/things/", &[], None)
        .await
        .unwrap();

    assert_eq!(items.len(), 1);
}
