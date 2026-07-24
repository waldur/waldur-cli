//! Tests for src/order.rs: the marketplace-order provision/terminate flow,
//! including `poll_order`'s state machine (tested indirectly through
//! `provision`/`terminate`, since `poll_order` itself is private).

use std::sync::atomic::{AtomicUsize, Ordering};
use waldur_cli::output::OutputFormat;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

/// Returns a different order `state` on each successive GET to the order
/// detail endpoint -- a real sequence (e.g. executing, executing, done)
/// rather than relying on wiremock's mock-priority/expiry semantics, which
/// this test suite shouldn't need to depend on for something this central.
struct SequencedOrderStates {
    states: Vec<&'static str>,
    call: AtomicUsize,
}

impl Respond for SequencedOrderStates {
    fn respond(&self, _req: &Request) -> ResponseTemplate {
        let i = self.call.fetch_add(1, Ordering::SeqCst).min(self.states.len() - 1);
        let state = self.states[i];
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "uuid": "ord00000000000000000000000000000",
            "state": state,
            "resource_uuid": "res00000000000000000000000000000",
            "error_message": if state == "erred" { "boom" } else { "" },
        }))
    }
}

#[tokio::test]
async fn provision_dry_run_never_sends_the_order() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/marketplace-orders/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(0)
        .mount(&server)
        .await;

    waldur_cli::order::provision(
        &server.uri(),
        Some("t"),
        r#"{"offering": "o", "project": "p"}"#,
        None,
        true, // dry_run
        true,
        60,
        OutputFormat::Json,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn provision_injects_ambient_project_when_absent() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/marketplace-orders/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "uuid": "ord1", "state": "executing"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/marketplace-orders/ord1/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "uuid": "ord1", "state": "done", "resource_uuid": "res1"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/marketplace-orders/ord1/resource/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"uuid": "res1"})))
        .mount(&server)
        .await;

    waldur_cli::order::provision(
        &server.uri(),
        Some("t"),
        r#"{"offering": "o"}"#, // no "project" key
        Some("proj-uuid"),
        false,
        true,
        60,
        OutputFormat::Json,
    )
    .await
    .unwrap();

    let requests = server.received_requests().await.unwrap();
    let post = requests.iter().find(|r| r.method == wiremock::http::Method::POST).unwrap();
    let body: serde_json::Value = serde_json::from_slice(&post.body).unwrap();
    assert_eq!(body["project"], format!("{}/api/projects/proj-uuid/", server.uri()));
}

#[tokio::test]
async fn provision_explicit_project_in_body_wins_over_ambient_scope() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/marketplace-orders/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "uuid": "ord1", "state": "executing"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/marketplace-orders/ord1/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "uuid": "ord1", "state": "done", "resource_uuid": "res1"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/marketplace-orders/ord1/resource/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"uuid": "res1"})))
        .mount(&server)
        .await;

    let explicit_project = format!("{}/api/projects/explicit/", server.uri());
    waldur_cli::order::provision(
        &server.uri(),
        Some("t"),
        &format!(r#"{{"offering": "o", "project": "{explicit_project}"}}"#),
        Some("ambient-uuid"), // should be ignored -- body already has one
        false,
        true,
        60,
        OutputFormat::Json,
    )
    .await
    .unwrap();

    let requests = server.received_requests().await.unwrap();
    let post = requests.iter().find(|r| r.method == wiremock::http::Method::POST).unwrap();
    let body: serde_json::Value = serde_json::from_slice(&post.body).unwrap();
    assert_eq!(body["project"], explicit_project);
}

#[tokio::test]
async fn provision_no_wait_returns_immediately_without_polling() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/marketplace-orders/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "uuid": "ord1", "state": "executing"
        })))
        .mount(&server)
        .await;
    // Poll endpoint must never be hit -- proves `wait: false` genuinely skips
    // polling rather than polling once anyway.
    Mock::given(method("GET"))
        .and(path("/api/marketplace-orders/ord1/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(0)
        .mount(&server)
        .await;

    waldur_cli::order::provision(
        &server.uri(),
        Some("t"),
        r#"{"offering": "o", "project": "p"}"#,
        None,
        false,
        false, // wait
        60,
        OutputFormat::Json,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn provision_polls_through_executing_to_done() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/marketplace-orders/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "uuid": "ord1", "state": "executing"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/marketplace-orders/ord1/"))
        .respond_with(SequencedOrderStates {
            states: vec!["executing", "done"],
            call: AtomicUsize::new(0),
        })
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/marketplace-orders/ord1/resource/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "uuid": "res1", "name": "my-vpc", "state": "OK"
        })))
        .mount(&server)
        .await;

    // This crosses one real POLL_INTERVAL (3s) since the first poll reports
    // "executing" -- accepted cost for exercising the actual polling loop
    // rather than just its terminal-state branches.
    waldur_cli::order::provision(
        &server.uri(),
        Some("t"),
        r#"{"offering": "o", "project": "p"}"#,
        None,
        false,
        true,
        60,
        OutputFormat::Json,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn provision_erred_order_fails_with_the_server_message() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/marketplace-orders/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "uuid": "ord1", "state": "executing"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/marketplace-orders/ord1/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "uuid": "ord1", "state": "erred", "error_message": "quota exceeded"
        })))
        .mount(&server)
        .await;

    let err = waldur_cli::order::provision(
        &server.uri(),
        Some("t"),
        r#"{"offering": "o", "project": "p"}"#,
        None,
        false,
        true,
        60,
        OutputFormat::Json,
    )
    .await
    .unwrap_err();

    let msg = err.to_string();
    assert!(msg.contains("erred"));
    assert!(msg.contains("quota exceeded"));
}

#[tokio::test]
async fn provision_never_reaching_a_terminal_state_times_out() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/marketplace-orders/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "uuid": "ord1", "state": "executing"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/marketplace-orders/ord1/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "uuid": "ord1", "state": "executing"
        })))
        .mount(&server)
        .await;

    // timeout_secs: 0 -- the deadline has already passed by the time the
    // first poll's response comes back, so this fails fast without waiting
    // out a real POLL_INTERVAL.
    let err = waldur_cli::order::provision(
        &server.uri(),
        Some("t"),
        r#"{"offering": "o", "project": "p"}"#,
        None,
        false,
        true,
        0,
        OutputFormat::Json,
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("timed out"));
}

#[tokio::test]
async fn terminate_dry_run_never_sends_the_request() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/marketplace-resources/res1/terminate/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(0)
        .mount(&server)
        .await;

    waldur_cli::order::terminate(&server.uri(), Some("t"), "res1", None, true, true, 60, OutputFormat::Json)
        .await
        .unwrap();
}

#[tokio::test]
async fn terminate_polls_to_completion() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/marketplace-resources/res1/terminate/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"order_uuid": "ord1"})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/marketplace-orders/ord1/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "uuid": "ord1", "state": "done"
        })))
        .mount(&server)
        .await;

    waldur_cli::order::terminate(&server.uri(), Some("t"), "res1", None, false, true, 60, OutputFormat::Json)
        .await
        .unwrap();
}

#[tokio::test]
async fn terminate_no_wait_skips_polling() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/marketplace-resources/res1/terminate/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"order_uuid": "ord1"})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/marketplace-orders/ord1/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(0)
        .mount(&server)
        .await;

    waldur_cli::order::terminate(&server.uri(), Some("t"), "res1", None, false, false, 60, OutputFormat::Json)
        .await
        .unwrap();
}
