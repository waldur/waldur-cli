//! Tests for src/web.rs's `resolve_homeport_url`'s default (no
//! `--homeport-url` override) path -- resolving from Waldur's public
//! `/api/configuration/` endpoint. The override path is covered separately
//! in tests/web_override.rs: `set_override` writes to a process-global
//! `OnceLock` that, once set, can't be reset for the life of the test
//! binary, so it can't share a file with tests that expect the unset state.

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn resolves_homeport_url_from_the_configuration_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/configuration/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "HOMEPORT_URL": "https://homeport.example.com/",
            "SITE_NAME": "Waldur",
        })))
        .mount(&server)
        .await;

    let url = waldur_cli::web::resolve_homeport_url(&server.uri(), Some("t")).await.unwrap();

    // Trailing slash trimmed, so callers can always do `{url}{path}` without
    // risking a doubled slash.
    assert_eq!(url, "https://homeport.example.com");
}

#[tokio::test]
async fn errors_clearly_when_the_configuration_response_has_no_homeport_url() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/configuration/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "SITE_NAME": "Waldur",
        })))
        .mount(&server)
        .await;

    let err = waldur_cli::web::resolve_homeport_url(&server.uri(), Some("t")).await.unwrap_err();

    assert!(err.to_string().contains("HOMEPORT_URL"));
    assert!(err.to_string().contains("--homeport-url"));
}
