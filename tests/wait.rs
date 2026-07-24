//! Tests for src/wait.rs's `wait_for`: the generic client-side polling every
//! get-able resource gets, evaluating a --jmespath condition each poll.

use std::sync::atomic::{AtomicUsize, Ordering};
use waldur_cli::output::OutputFormat;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const COLUMNS: &[&str] = &["uuid", "state"];

/// Returns a different `state` on each successive GET.
struct SequencedStates {
    states: Vec<&'static str>,
    call: AtomicUsize,
}

impl Respond for SequencedStates {
    fn respond(&self, _req: &Request) -> ResponseTemplate {
        let i = self.call.fetch_add(1, Ordering::SeqCst).min(self.states.len() - 1);
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "uuid": "abc", "state": self.states[i]
        }))
    }
}

#[tokio::test]
async fn condition_already_true_returns_immediately() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/customers/abc/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "uuid": "abc", "state": "OK"
        })))
        .expect(1)
        .mount(&server)
        .await;

    waldur_cli::wait::wait_for(
        &server.uri(),
        Some("t"),
        "/api/customers/abc/",
        "state=='OK'",
        60,
        3,
        COLUMNS,
        OutputFormat::Json,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn polls_until_condition_becomes_true() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/customers/abc/"))
        .respond_with(SequencedStates {
            states: vec!["creating", "creating", "OK"],
            call: AtomicUsize::new(0),
        })
        .mount(&server)
        .await;

    // Short interval keeps this test fast while still exercising >1 real poll.
    waldur_cli::wait::wait_for(
        &server.uri(),
        Some("t"),
        "/api/customers/abc/",
        "state=='OK'",
        60,
        1,
        COLUMNS,
        OutputFormat::Json,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn presence_check_without_a_boolean_comparison() {
    // The condition doesn't have to be a `==` comparison -- any jmespath
    // result that isn't false/null counts as "met," so a plain field-
    // presence check works too.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/marketplace-orders/ord1/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "uuid": "ord1", "resource_uuid": "res1"
        })))
        .mount(&server)
        .await;

    waldur_cli::wait::wait_for(
        &server.uri(),
        Some("t"),
        "/api/marketplace-orders/ord1/",
        "resource_uuid",
        60,
        3,
        COLUMNS,
        OutputFormat::Json,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn null_result_does_not_count_as_met() {
    let server = MockServer::start().await;
    // resource_uuid is absent -> jmespath projects to null -> not met ->
    // times out (timeout_secs: 0 for a fast test).
    Mock::given(method("GET"))
        .and(path("/api/marketplace-orders/ord1/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "uuid": "ord1"
        })))
        .mount(&server)
        .await;

    let err = waldur_cli::wait::wait_for(
        &server.uri(),
        Some("t"),
        "/api/marketplace-orders/ord1/",
        "resource_uuid",
        0,
        3,
        COLUMNS,
        OutputFormat::Json,
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("timed out"));
}

#[tokio::test]
async fn never_met_times_out_with_a_clear_message() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/customers/abc/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "uuid": "abc", "state": "creating"
        })))
        .mount(&server)
        .await;

    let err = waldur_cli::wait::wait_for(
        &server.uri(),
        Some("t"),
        "/api/customers/abc/",
        "state=='OK'",
        0, // deadline already passed by the time the first response lands
        3,
        COLUMNS,
        OutputFormat::Json,
    )
    .await
    .unwrap_err();

    let msg = err.to_string();
    assert!(msg.contains("timed out"));
    assert!(msg.contains("state=='OK'"));
}

#[tokio::test]
async fn invalid_jmespath_expression_fails_fast_without_waiting_out_the_timeout() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/customers/abc/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"uuid": "abc"})))
        .expect(1) // must not retry a broken expression across multiple polls
        .mount(&server)
        .await;

    let err = waldur_cli::wait::wait_for(
        &server.uri(),
        Some("t"),
        "/api/customers/abc/",
        "[", // malformed
        60,
        3,
        COLUMNS,
        OutputFormat::Json,
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("invalid --query expression"));
}

#[tokio::test]
async fn a_failing_get_propagates_immediately() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/customers/missing/"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({"detail": "Not found."})))
        .expect(1)
        .mount(&server)
        .await;

    let err = waldur_cli::wait::wait_for(
        &server.uri(),
        Some("t"),
        "/api/customers/missing/",
        "state=='OK'",
        60,
        3,
        COLUMNS,
        OutputFormat::Json,
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("404"));
}
