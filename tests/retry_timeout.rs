//! Tests that the per-request timeout is actually applied. In its own test
//! binary because `set_transport_options` writes a process-global `OnceLock`
//! that can't be reset once set (see tests/retry.rs).

use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn a_stalled_request_times_out_instead_of_hanging() {
    // reqwest's own default is *no* timeout, so without this the call below
    // would wait on the server indefinitely rather than failing.
    waldur_cli::http::set_transport_options(Some(1), Some(0));

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/slow/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({}))
                .set_delay(Duration::from_secs(10)),
        )
        .mount(&server)
        .await;

    let started = std::time::Instant::now();
    let err = waldur_cli::http::call_one(&server.uri(), Some("t"), reqwest::Method::GET, "/api/slow/", None)
        .await
        .unwrap_err();

    // Gave up on its own rather than waiting out the server's 10s delay.
    assert!(
        started.elapsed() < Duration::from_secs(8),
        "expected the 1s timeout to fire, but the call took {:?}",
        started.elapsed()
    );
    assert!(err.to_string().contains("request failed"), "unexpected error: {err}");
}
