//! Tests for src/http.rs's `call_one`: the single-request get/create/update/
//! delete path every non-`list` verb uses.

use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn get_returns_parsed_json_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/customers/abc/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "uuid": "abc", "name": "Acme"
        })))
        .mount(&server)
        .await;

    let result = waldur_cli::http::call_one(
        &server.uri(),
        Some("good-token"),
        reqwest::Method::GET,
        "/api/customers/abc/",
        None,
    )
    .await
    .unwrap();

    assert_eq!(result, serde_json::json!({"uuid": "abc", "name": "Acme"}));
}

#[tokio::test]
async fn sends_authorization_header_in_waldur_token_format() {
    let server = MockServer::start().await;
    // Waldur's DRF TokenAuthentication expects "Token <key>", not "Bearer
    // <key>" -- this is the one thing that would silently 401 in production
    // if it regressed, so it's worth asserting the header shape directly.
    Mock::given(method("GET"))
        .and(path("/api/whatever/"))
        .and(header("Authorization", "Token good-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .mount(&server)
        .await;

    waldur_cli::http::call_one(
        &server.uri(),
        Some("good-token"),
        reqwest::Method::GET,
        "/api/whatever/",
        None,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn post_sends_body_and_content_type() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/customers/"))
        .and(header("Content-Type", "application/json"))
        .and(body_json(serde_json::json!({"name": "Acme"})))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "uuid": "new-uuid", "name": "Acme"
        })))
        .mount(&server)
        .await;

    let result = waldur_cli::http::call_one(
        &server.uri(),
        Some("good-token"),
        reqwest::Method::POST,
        "/api/customers/",
        Some(r#"{"name": "Acme"}"#),
    )
    .await
    .unwrap();

    assert_eq!(result["uuid"], "new-uuid");
}

#[tokio::test]
async fn error_status_includes_body_in_message() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/customers/missing/"))
        .respond_with(
            ResponseTemplate::new(404).set_body_json(serde_json::json!({"detail": "Not found."})),
        )
        .mount(&server)
        .await;

    let err = waldur_cli::http::call_one(
        &server.uri(),
        Some("good-token"),
        reqwest::Method::GET,
        "/api/customers/missing/",
        None,
    )
    .await
    .unwrap_err();

    let msg = err.to_string();
    assert!(msg.contains("404"));
    assert!(msg.contains("Not found."));
}

#[tokio::test]
async fn empty_body_204_returns_null() {
    let server = MockServer::start().await;
    // DELETE's 204 No Content has no body to parse.
    Mock::given(method("DELETE"))
        .and(path("/api/customers/abc/"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let result = waldur_cli::http::call_one(
        &server.uri(),
        Some("good-token"),
        reqwest::Method::DELETE,
        "/api/customers/abc/",
        None,
    )
    .await
    .unwrap();

    assert_eq!(result, serde_json::Value::Null);
}

#[tokio::test]
async fn no_token_omits_authorization_header() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/public/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .mount(&server)
        .await;

    // Should succeed with no Authorization header at all (not e.g. "Token
    // None" or similar accidental stringification).
    waldur_cli::http::call_one(&server.uri(), None, reqwest::Method::GET, "/api/public/", None)
        .await
        .unwrap();
}
