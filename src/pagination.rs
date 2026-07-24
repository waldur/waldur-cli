//! Hand-written: auto-fetches every page of a `list` endpoint.
//!
//! Separate from `http.rs`'s single-request `call_one` because pagination is
//! genuinely list-specific: it reads Waldur's pagination header
//! (`X-Result-Count`, emitted by `waldur_core.core.pagination.
//! LinkHeaderPagination`, the default pagination class for every list
//! endpoint) across as many page requests as it takes to know when the
//! complete result set has been fetched -- a loop with its own state
//! (`page`, `total`, accumulated/streamed items) that `call_one`'s one
//! request/one response shape has no need for.

use anyhow::{bail, Context, Result};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_tracing::TracingMiddleware;

/// Waldur's `LinkHeaderPagination.max_page_size` -- the largest page we can
/// ask for, to minimize round trips.
const MAX_PAGE_SIZE: i64 = 300;

/// Hard safety net, independent of `--limit`: stops a runaway fetch (a
/// server/client bug reporting a total that's never actually reached)
/// rather than looping indefinitely. Generous enough to never trigger for
/// a real result set (10_000 * 300 = 3,000,000 items).
const MAX_PAGES: i64 = 10_000;

pub(crate) fn build_client() -> ClientWithMiddleware {
    ClientBuilder::new(reqwest::Client::new())
        .with(TracingMiddleware::default())
        .build()
}

/// Fetches every page of `path` (a list endpoint), merging them into one
/// JSON array. A thin buffering wrapper over `fetch_all_streaming` -- see
/// that function for the pagination/limit/error semantics, which apply
/// unchanged here.
pub async fn fetch_all(
    base_url: &str,
    token: Option<&str>,
    path: &str,
    query_params: &[(String, String)],
    limit: Option<i64>,
) -> Result<Vec<serde_json::Value>> {
    let mut items = Vec::new();
    fetch_all_streaming(base_url, token, path, query_params, limit, |item| {
        items.push(item);
        Ok(true)
    })
    .await?;
    Ok(items)
}

/// Fetches every page of `path` (a list endpoint), calling `on_item` for
/// each item as its page arrives rather than buffering the complete result
/// set first -- what `--format ndjson` uses for `list` to start printing
/// after the first page instead of waiting for all of them. `query_params`
/// should contain every filter the caller asked for except page/page_size,
/// which this function controls itself.
///
/// `on_item` returns whether to keep going: `Ok(true)` continues, `Ok(false)`
/// stops immediately (used to stop fetching further pages once a downstream
/// reader has hung up, e.g. `| head` -- no point paying for requests nobody
/// will read), `Err` propagates as this function's own error.
///
/// `limit`, if given, stops once that many items have been emitted (never
/// emitting past it, even if the last page fetched overshoots) -- both to
/// bound how long a huge list takes/how much memory it uses, and to bound
/// the damage if a page fails partway through a very long fetch (better to
/// ask for only what's actually needed than to redo hundreds of requests
/// after a late-page failure). A failure always surfaces as an error either
/// way (never silently stops as if the result were complete -- that would
/// reintroduce exactly the "looks complete but isn't" problem
/// auto-pagination exists to avoid), but the error message reports how much
/// had been emitted so far, so a real partial-progress failure is
/// distinguishable from "found nothing" or an immediate connection error.
pub async fn fetch_all_streaming(
    base_url: &str,
    token: Option<&str>,
    path: &str,
    query_params: &[(String, String)],
    limit: Option<i64>,
    mut on_item: impl FnMut(serde_json::Value) -> Result<bool>,
) -> Result<()> {
    if let Some(limit) = limit {
        if limit <= 0 {
            return Ok(());
        }
    }

    let client = build_client();
    let mut sent: i64 = 0;
    let mut page = 1i64;
    let mut total: Option<i64> = None;
    // Don't request a full 300-item page just to immediately truncate it
    // down to a small --limit.
    let page_size = limit.map(|l| l.min(MAX_PAGE_SIZE)).unwrap_or(MAX_PAGE_SIZE);

    loop {
        let mut params = query_params.to_vec();
        params.push(("page".to_string(), page.to_string()));
        params.push(("page_size".to_string(), page_size.to_string()));

        let mut req = client.get(format!("{base_url}{path}")).query(&params);
        if let Some(token) = token {
            req = req.header("Authorization", format!("Token {token}"));
        }
        let response = req.send().await.with_context(|| {
            format!("pagination request failed on page {page} (fetched {sent} item(s) before this)")
        })?;
        let status = response.status();

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let progress = match total {
                Some(total) => format!("{sent} of {total}"),
                None => sent.to_string(),
            };
            bail!("API error {status} on page {page} (fetched {progress} item(s) before this failed): {body}");
        }

        if total.is_none() {
            total = response
                .headers()
                .get("x-result-count")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<i64>().ok());
        }

        // Matches rs-client's own generated code: .text() + serde_json::
        // from_str rather than reqwest's `.json()`, so we don't need
        // reqwest's "json" feature enabled just for this one call site.
        let body_text = response.text().await.with_context(|| {
            format!("failed to read page {page} body (fetched {sent} item(s) before this)")
        })?;
        let items: Vec<serde_json::Value> = serde_json::from_str(&body_text).with_context(|| {
            format!("failed to parse page {page} body (fetched {sent} item(s) before this)")
        })?;
        let got = items.len() as i64;

        for item in items {
            if let Some(limit) = limit {
                if sent >= limit {
                    break;
                }
            }
            if !on_item(item)? {
                return Ok(());
            }
            sent += 1;
        }

        if let Some(limit) = limit {
            if sent >= limit {
                break;
            }
        }

        let done = match total {
            Some(total) => sent >= total,
            // Shouldn't happen against a real Waldur instance -- every list
            // endpoint uses LinkHeaderPagination by default -- but fall back
            // to stopping once a page comes back short of what was asked
            // for, rather than looping forever.
            None => got < page_size,
        };
        if done || got == 0 {
            break;
        }

        page += 1;
        if page > MAX_PAGES {
            bail!(
                "stopped after {MAX_PAGES} pages ({sent} items) without reaching the reported \
                 total ({total:?}) -- this looks like a server or client bug rather than a \
                 real result set"
            );
        }
    }

    Ok(())
}
