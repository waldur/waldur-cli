//! Tests for src/api.rs's `run`: the `api` escape hatch that calls an
//! arbitrary endpoint directly, bypassing every generated command's typed
//! Args/skeleton/schema-validation machinery.

use waldur_cli::output::OutputFormat;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn get_returns_the_raw_response() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/some-new-endpoint/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"uuid": "u1", "name": "Thing One"},
        ])))
        .expect(1)
        .mount(&server)
        .await;

    waldur_cli::api::run(
        &server.uri(),
        Some("t"),
        "GET",
        "/api/some-new-endpoint/",
        None,
        None,
        None,
        false,
        OutputFormat::Json,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn a_path_without_a_leading_slash_still_reaches_the_right_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/some-new-endpoint/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    waldur_cli::api::run(
        &server.uri(),
        Some("t"),
        "get",
        "api/some-new-endpoint/", // no leading slash, lowercase method
        None,
        None,
        None,
        false,
        OutputFormat::Json,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn post_sends_the_inline_request_body_verbatim() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/some-new-endpoint/"))
        .and(body_json(serde_json::json!({"name": "Created via api"})))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "uuid": "new-1", "name": "Created via api"
        })))
        .expect(1)
        .mount(&server)
        .await;

    waldur_cli::api::run(
        &server.uri(),
        Some("t"),
        "POST",
        "/api/some-new-endpoint/",
        Some(r#"{"name": "Created via api"}"#),
        None,
        None,
        false,
        OutputFormat::Json,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn a_body_is_optional_get_and_delete_typically_send_none() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/some-new-endpoint/abc/"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    waldur_cli::api::run(
        &server.uri(),
        Some("t"),
        "DELETE",
        "/api/some-new-endpoint/abc/",
        None,
        None,
        None,
        false,
        OutputFormat::Json,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn jmespath_reshapes_the_response() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/some-new-endpoint/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"uuid": "u1"}, {"uuid": "u2"},
        ])))
        .mount(&server)
        .await;

    // Nothing to assert on stdout here (that's exercised by hand, same as
    // every other command) -- what matters is that a valid expression
    // doesn't error, and an invalid one does (below).
    waldur_cli::api::run(
        &server.uri(),
        Some("t"),
        "GET",
        "/api/some-new-endpoint/",
        None,
        None,
        Some("[].uuid"),
        false,
        OutputFormat::Json,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn an_invalid_jmespath_expression_is_a_clear_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/some-new-endpoint/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .mount(&server)
        .await;

    let err = waldur_cli::api::run(
        &server.uri(),
        Some("t"),
        "GET",
        "/api/some-new-endpoint/",
        None,
        None,
        Some("["), // malformed
        false,
        OutputFormat::Json,
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("invalid --query expression"));
}

#[tokio::test]
async fn dry_run_never_calls_the_server() {
    let server = MockServer::start().await;
    // No Mock mounted at all -- any request reaching the server fails the
    // test via wiremock's default "no matching mock" panic.

    waldur_cli::api::run(
        &server.uri(),
        Some("t"),
        "POST",
        "/api/some-new-endpoint/",
        Some(r#"{"name": "Preview me"}"#),
        None,
        None,
        true, // dry_run
        OutputFormat::Json,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn a_malformed_method_is_rejected_before_any_http_call() {
    let server = MockServer::start().await;
    // No Mock mounted -- a request reaching the server would fail the test.

    let err = waldur_cli::api::run(
        &server.uri(),
        Some("t"),
        "bad method",
        "/api/some-new-endpoint/",
        None,
        None,
        None,
        false,
        OutputFormat::Json,
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("not a valid HTTP method"));
}

#[tokio::test]
async fn a_failing_request_propagates_the_api_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/some-new-endpoint/"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({"detail": "Not found."})))
        .expect(1)
        .mount(&server)
        .await;

    let err = waldur_cli::api::run(
        &server.uri(),
        Some("t"),
        "GET",
        "/api/some-new-endpoint/",
        None,
        None,
        None,
        false,
        OutputFormat::Json,
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("404"));
}
