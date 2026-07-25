//! Hand-written: resolving and opening a resource's page in Waldur's web UI
//! (HomePort) for `get --web`. HomePort's own routing isn't part of the
//! OpenAPI schema, so the per-resource path templates live in
//! `commands.toml`'s `[group.resource.web]` config and get embedded as
//! generated consts; this module only handles the two things every one of
//! them needs: finding HomePort's base URL, and opening a browser.

use anyhow::{Context, Result};
use std::sync::OnceLock;

/// Set once in `main` from `--homeport-url`/`WALDUR_HOMEPORT_URL`. `get`'s
/// generated `--web` arm is reached through generated command code, so a
/// process-global is cheaper than threading an override through every
/// generated `run()` signature for something only a handful of resources'
/// `--web` flag ever reads.
static HOMEPORT_OVERRIDE: OnceLock<Option<String>> = OnceLock::new();

pub fn set_override(url: Option<String>) {
    let _ = HOMEPORT_OVERRIDE.set(url);
}

/// Resolves HomePort's base URL (no trailing slash): the `--homeport-url`
/// override if set, otherwise `HOMEPORT_URL` from Waldur's public
/// `/api/configuration/` endpoint -- no auth required, so this works even
/// with an expired/missing token.
pub async fn resolve_homeport_url(base_url: &str, token: Option<&str>) -> Result<String> {
    if let Some(url) = HOMEPORT_OVERRIDE.get().cloned().flatten() {
        return Ok(url.trim_end_matches('/').to_string());
    }
    let config = crate::http::call_one(base_url, token, reqwest::Method::GET, "/api/configuration/", None)
        .await
        .context("fetching /api/configuration/ to resolve HomePort's URL")?;
    let url = config
        .get("HOMEPORT_URL")
        .and_then(|v| v.as_str())
        .context(
            "Waldur's /api/configuration/ response has no HOMEPORT_URL -- pass --homeport-url explicitly",
        )?;
    Ok(url.trim_end_matches('/').to_string())
}

/// Opens `url` in the user's default browser. Non-fatal on failure (e.g. a
/// headless/SSH session with no browser to open) -- the URL is always
/// printed first, so `--web` is still useful even where nothing can
/// actually pop a window.
pub fn open_in_browser(url: &str) {
    println!("Opening {url} in your browser...");
    if let Err(err) = open::that(url) {
        eprintln!("warning: couldn't open a browser automatically: {err}");
    }
}
