//! Auth/connection config: `WALDUR_API_URL` / `WALDUR_ACCESS_TOKEN` env vars
//! (the same names terraform-provider-waldur-generator's CI already uses for
//! the same purpose), overridable with `--api-url`/`--token`.
//!
//! Deliberately no interactive login flow or persisted config file -- this
//! tool is for scripted/agent use, not interactive human use (see the
//! separate, pre-existing waldur-cli/waldur-cli2 TUI projects for that).

use anyhow::{bail, Result};

pub struct Config {
    pub api_url: String,
    pub token: Option<String>,
}

impl Config {
    pub fn resolve(api_url_flag: Option<String>, token_flag: Option<String>) -> Result<Self> {
        let api_url = api_url_flag
            .or_else(|| std::env::var("WALDUR_API_URL").ok())
            .map(|url| url.trim_end_matches('/').to_string());
        let Some(api_url) = api_url else {
            bail!(
                "No API URL given. Pass --api-url or set WALDUR_API_URL, \
                 e.g. https://waldur.example.com"
            );
        };
        let token = token_flag.or_else(|| std::env::var("WALDUR_ACCESS_TOKEN").ok());
        Ok(Self { api_url, token })
    }
}
