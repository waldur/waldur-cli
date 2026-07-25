//! Tests src/web.rs's `--homeport-url` override path in its own file/process
//! (see the note in tests/web.rs) -- `set_override` writes to a
//! process-global `OnceLock` that can only ever be set once per test binary.

#[tokio::test]
async fn override_short_circuits_without_any_http_call() {
    waldur_cli::web::set_override(Some("https://custom.example.com/".to_string()));

    // An unreachable base_url: if resolve_homeport_url tried to actually
    // fetch /api/configuration/ despite the override, this would fail
    // (connection refused) instead of returning cleanly.
    let url = waldur_cli::web::resolve_homeport_url("http://127.0.0.1:1", None).await.unwrap();

    assert_eq!(url, "https://custom.example.com");
}
