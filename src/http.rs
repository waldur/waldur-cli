//! Hand-written: single-request get/create/update/delete against a raw REST
//! endpoint, the `pagination.rs`-style counterpart for every verb that isn't
//! `list`: no typed response to drift out of sync with the live API, since
//! nothing downstream of `print_result` ever reads a typed field off the
//! response anyway.

use anyhow::{bail, Context, Result};

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
    let client = crate::pagination::build_client();
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
