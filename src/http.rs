//! Hand-written: single-request get/create/update/delete against a raw REST
//! endpoint, the `pagination.rs`-style counterpart for every verb that isn't
//! `list`: no typed response to drift out of sync with the live API, since
//! nothing downstream of `print_result` ever reads a typed field off the
//! response anyway.

use anyhow::{bail, Context, Result};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::RetryTransientMiddleware;
use reqwest_tracing::TracingMiddleware;
use std::sync::OnceLock;
use std::time::Duration;

/// Per-request wall-clock cap. reqwest's own default is *no* timeout, which
/// leaves an unattended run (CI, an agent) hanging indefinitely on a stalled
/// connection. Generous enough that a slow-but-working API call is never cut
/// off; override with `--http-timeout`.
const DEFAULT_HTTP_TIMEOUT_SECS: u64 = 60;

/// Retries applied to *idempotent* requests only (see `call_one`). Kept low
/// deliberately: this is for riding out a transient blip, not for waiting out
/// a sustained outage -- a human or a job scheduler should decide that.
const DEFAULT_MAX_RETRIES: u32 = 3;

/// Set once in `main` from `--http-timeout`/`--max-retries` (and their env
/// vars). Process-globals for the same reason `web.rs`'s HomePort override
/// is one: the clients are built deep inside generated command code, and
/// threading two transport knobs through every generated `run()` signature
/// would be a lot of churn for something nothing else reads.
static HTTP_TIMEOUT_SECS: OnceLock<u64> = OnceLock::new();
static MAX_RETRIES: OnceLock<u32> = OnceLock::new();

pub fn set_transport_options(timeout_secs: Option<u64>, max_retries: Option<u32>) {
    if let Some(secs) = timeout_secs {
        let _ = HTTP_TIMEOUT_SECS.set(secs);
    }
    if let Some(n) = max_retries {
        let _ = MAX_RETRIES.set(n);
    }
}

fn timeout_secs() -> u64 {
    HTTP_TIMEOUT_SECS.get().copied().unwrap_or(DEFAULT_HTTP_TIMEOUT_SECS)
}

fn max_retries() -> u32 {
    MAX_RETRIES.get().copied().unwrap_or(DEFAULT_MAX_RETRIES)
}

fn base_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs()))
        .build()
        // Only fails on a broken TLS backend, which would break every request
        // anyway -- fall back to the default client so this stays infallible
        // for callers rather than making every call site handle it.
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// The HTTP client for requests that are safe to replay: retries transient
/// failures (connection errors, timeouts, 5xx, 429) with exponential backoff.
pub(crate) fn build_client() -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(max_retries());
    ClientBuilder::new(base_client())
        .with(TracingMiddleware::default())
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}

/// The HTTP client for requests that are *not* safe to replay. A POST/PATCH
/// that times out may still have been applied server-side, so a retry can
/// duplicate it -- and for `provision` that means a second marketplace order
/// someone has to pay for. Timeouts still apply; only the retry is dropped.
pub(crate) fn build_client_no_retry() -> ClientWithMiddleware {
    ClientBuilder::new(base_client())
        .with(TracingMiddleware::default())
        .build()
}

/// Builds the `Authorization` header value for `token`. Personal Access
/// Tokens are self-identifying by their `w_` prefix (Waldur's own
/// `PATAuthentication` middleware uses the same prefix check to route
/// between auth backends) and authenticate via `Bearer`; every other token
/// (from `login`/`--token`/`WALDUR_ACCESS_TOKEN`) is a classic DRF token via
/// `Token`.
pub fn auth_header_value(token: &str) -> String {
    if token.starts_with("w_") {
        format!("Bearer {token}")
    } else {
        format!("Token {token}")
    }
}

/// Sends one request and returns its parsed JSON body (`Value::Null` for an
/// empty body, e.g. DELETE's 204 No Content).
pub async fn call_one(
    base_url: &str,
    token: Option<&str>,
    method: reqwest::Method,
    path: &str,
    json_body: Option<&str>,
) -> Result<serde_json::Value> {
    // GET/PUT/DELETE can be replayed safely; POST/PATCH can't (see
    // `build_client_no_retry`).
    let client = if method.is_idempotent() {
        build_client()
    } else {
        build_client_no_retry()
    };
    let mut req = client.request(method.clone(), format!("{base_url}{path}"));
    if let Some(token) = token {
        req = req.header("Authorization", auth_header_value(token));
    }
    if let Some(body) = json_body {
        req = req.header("Content-Type", "application/json").body(body.to_string());
    }

    let response = req
        .send()
        .await
        .with_context(|| format!("{method} {path} request failed"))?;
    let status = response.status();
    let body_text = response
        .text()
        .await
        .with_context(|| format!("failed to read {method} {path} response body"))?;

    if !status.is_success() {
        bail!("API error {status}: {body_text}");
    }
    if body_text.trim().is_empty() {
        return Ok(serde_json::Value::Null);
    }
    serde_json::from_str(&body_text)
        .with_context(|| format!("failed to parse {method} {path} response body"))
}
