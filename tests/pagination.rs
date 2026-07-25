//! Tests for src/pagination.rs's `fetch_all`/`fetch_all_streaming`: the
//! auto-pagination loop every `list` command uses.

use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn items(n: usize, offset: usize) -> Vec<serde_json::Value> {
    (offset..offset + n)
        .map(|i| serde_json::json!({"uuid": format!("{i:03}"), "name": format!("item{i}")}))
        .collect()
}

#[tokio::test]
async fn single_page_returns_all_items() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/customers/"))
        .and(query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).insert_header("X-Result-Count", "3").set_body_json(items(3, 0)))
        .expect(1)
        .mount(&server)
        .await;

    let result = waldur_cli::pagination::fetch_all(&server.uri(), Some("t"), "/api/customers/", &[], None)
        .await
        .unwrap();

    assert_eq!(result.len(), 3);
}

#[tokio::test]
async fn multi_page_concatenates_in_order() {
    let server = MockServer::start().await;
    // page_size is fixed at 300 (MAX_PAGE_SIZE) whenever there's no --limit,
    // so simulate "more than one page" via a smaller reported total than
    // page 1 actually returns... instead, directly control page_size by
    // requesting with a --limit that's still > one page's worth is awkward;
    // simplest: mock page=1 with X-Result-Count reflecting more items than
    // fit on page 1's returned array, forcing a page=2 request.
    Mock::given(method("GET"))
        .and(path("/api/customers/"))
        .and(query_param("page", "1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("X-Result-Count", "5")
                .set_body_json(items(3, 0)),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/customers/"))
        .and(query_param("page", "2"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("X-Result-Count", "5")
                .set_body_json(items(2, 3)),
        )
        .mount(&server)
        .await;

    let result = waldur_cli::pagination::fetch_all(&server.uri(), Some("t"), "/api/customers/", &[], None)
        .await
        .unwrap();

    assert_eq!(result.len(), 5);
    assert_eq!(result[0]["uuid"], "000");
    assert_eq!(result[4]["uuid"], "004");
}

#[tokio::test]
async fn query_params_are_forwarded() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/customers/"))
        .and(query_param("name_exact", "Acme"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("X-Result-Count", "1")
                .set_body_json(items(1, 0)),
        )
        .mount(&server)
        .await;

    let params = vec![("name_exact".to_string(), "Acme".to_string())];
    let result = waldur_cli::pagination::fetch_all(&server.uri(), Some("t"), "/api/customers/", &params, None)
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
}

#[tokio::test]
async fn limit_truncates_without_a_second_request() {
    let server = MockServer::start().await;
    // A single page reports far more available than --limit asks for --
    // fetch_all must stop after the first page, never requesting page 2.
    Mock::given(method("GET"))
        .and(path("/api/customers/"))
        .and(query_param("page", "1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("X-Result-Count", "100")
                .set_body_json(items(10, 0)),
        )
        .expect(1)
        .mount(&server)
        .await;

    let result = waldur_cli::pagination::fetch_all(
        &server.uri(),
        Some("t"),
        "/api/customers/",
        &[],
        Some(3),
    )
    .await
    .unwrap();

    assert_eq!(result.len(), 3);
}

#[tokio::test]
async fn zero_or_negative_limit_short_circuits_without_any_request() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/customers/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(items(1, 0)))
        .expect(0)
        .mount(&server)
        .await;

    let result = waldur_cli::pagination::fetch_all(&server.uri(), Some("t"), "/api/customers/", &[], Some(0))
        .await
        .unwrap();

    assert!(result.is_empty());
}

#[tokio::test]
async fn empty_result_set() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/customers/"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("X-Result-Count", "0")
                .set_body_json(Vec::<serde_json::Value>::new()),
        )
        .mount(&server)
        .await;

    let result = waldur_cli::pagination::fetch_all(&server.uri(), Some("t"), "/api/customers/", &[], None)
        .await
        .unwrap();

    assert!(result.is_empty());
}

#[tokio::test]
async fn error_partway_through_reports_progress_so_far() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/customers/"))
        .and(query_param("page", "1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("X-Result-Count", "10")
                .set_body_json(items(3, 0)),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/customers/"))
        .and(query_param("page", "2"))
        .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
        .mount(&server)
        .await;

    let err = waldur_cli::pagination::fetch_all(&server.uri(), Some("t"), "/api/customers/", &[], None)
        .await
        .unwrap_err();

    let msg = err.to_string();
    assert!(msg.contains("500"));
    // Never silently returns a partial list as if complete -- the error
    // reports exactly how far it got, distinguishing this from "found
    // nothing" or an immediate connection failure.
    assert!(msg.contains("3 of 10"));
}

// -- streaming ---------------------------------------------------------------

#[tokio::test]
async fn streaming_visits_every_item_across_pages() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/customers/"))
        .and(query_param("page", "1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("X-Result-Count", "4")
                .set_body_json(items(2, 0)),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/customers/"))
        .and(query_param("page", "2"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("X-Result-Count", "4")
                .set_body_json(items(2, 2)),
        )
        .mount(&server)
        .await;

    let mut seen = Vec::new();
    waldur_cli::pagination::fetch_all_streaming(&server.uri(), Some("t"), "/api/customers/", &[], None, |item| {
        seen.push(item["uuid"].as_str().unwrap().to_string());
        Ok(true)
    })
    .await
    .unwrap();

    assert_eq!(seen, vec!["000", "001", "002", "003"]);
}

#[tokio::test]
async fn streaming_stops_and_fetches_no_further_pages_when_on_item_returns_false() {
    let server = MockServer::start().await;
    // If the callback signals "stop" partway through page 1, fetch_all_streaming
    // must never request page 2 at all -- this is what makes `| head` on
    // --format ndjson stop pulling data nobody will read.
    Mock::given(method("GET"))
        .and(path("/api/customers/"))
        .and(query_param("page", "1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("X-Result-Count", "10")
                .set_body_json(items(5, 0)),
        )
        .expect(1)
        .mount(&server)
        .await;

    let mut seen = Vec::new();
    waldur_cli::pagination::fetch_all_streaming(&server.uri(), Some("t"), "/api/customers/", &[], None, |item| {
        seen.push(item);
        Ok(seen.len() < 2) // stop after the 2nd item
    })
    .await
    .unwrap();

    assert_eq!(seen.len(), 2);
}

#[tokio::test]
async fn sends_bearer_authorization_header_for_a_personal_access_token() {
    let server = MockServer::start().await;
    // `fetch_all` builds its own Authorization header separately from
    // src/http.rs's call_one -- this would silently 401 a PAT-authenticated
    // `list` if the two ever drifted apart.
    Mock::given(method("GET"))
        .and(path("/api/customers/"))
        .and(header("Authorization", "Bearer w_1735689599_abc123"))
        .respond_with(ResponseTemplate::new(200).insert_header("X-Result-Count", "1").set_body_json(items(1, 0)))
        .mount(&server)
        .await;

    waldur_cli::pagination::fetch_all(&server.uri(), Some("w_1735689599_abc123"), "/api/customers/", &[], None)
        .await
        .unwrap();
}
