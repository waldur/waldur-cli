//! Auth/connection config, resolved in priority order:
//! 1. `--api-url`/`--token` flags
//! 2. `WALDUR_API_URL`/`WALDUR_ACCESS_TOKEN` env vars
//! 3. the config file written by `waldur-cli login` (see [`save_stored`])
//!
//! No interactive login flow beyond `login` itself -- this tool is primarily
//! for scripted/agent use, but a persisted config file avoids
//! having to pass `--token`/set an env var on every single invocation for
//! interactive/local use.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredentials {
    pub api_url: String,
    pub token: String,
}

/// `~/.config/waldur-cli/credentials.toml` on Linux (respects
/// `XDG_CONFIG_HOME`), the platform equivalent elsewhere (`~/Library/
/// Application Support/...` on macOS, `%APPDATA%` on Windows) -- see the
/// `directories` crate.
///
pub fn config_path() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "waldur-cli")
        .context("could not determine a config directory for this platform")?;
    Ok(dirs.config_dir().join("credentials.toml"))
}

pub fn load_stored() -> Result<Option<StoredCredentials>> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let creds: StoredCredentials = toml::from_str(&contents).with_context(|| {
        format!(
            "failed to parse {} -- run `waldur-cli login` again",
            path.display()
        )
    })?;
    Ok(Some(creds))
}

pub fn save_stored(creds: &StoredCredentials) -> Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let contents = toml::to_string_pretty(creds)?;
    std::fs::write(&path, contents)
        .with_context(|| format!("failed to write {}", path.display()))?;

    // The file holds a plaintext API token -- restrict it to the owner, same
    // as ~/.netrc or ~/.aws/credentials. No equivalent lockdown attempted on
    // Windows; NTFS already defaults new files under the user's profile to
    // that user.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to set permissions on {}", path.display()))?;
    }
    Ok(())
}

/// Returns whether a stored config file existed (and was removed).
pub fn delete_stored() -> Result<bool> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(false);
    }
    std::fs::remove_file(&path).with_context(|| format!("failed to remove {}", path.display()))?;
    Ok(true)
}

pub struct Config {
    pub api_url: String,
    pub token: Option<String>,
}

impl Config {
    pub fn resolve(api_url_flag: Option<String>, token_flag: Option<String>) -> Result<Self> {
        let api_url_env = std::env::var("WALDUR_API_URL").ok();
        let token_env = std::env::var("WALDUR_ACCESS_TOKEN").ok();

        // Only touch the config file if flags/env vars didn't already give
        // us everything -- keeps a corrupt/unreadable file from breaking
        // fully-explicit invocations, while still surfacing a clear parse
        // error (rather than silently ignoring it) when it's actually needed.
        let need_stored = (api_url_flag.is_none() && api_url_env.is_none())
            || (token_flag.is_none() && token_env.is_none());
        let stored = if need_stored { load_stored()? } else { None };

        let api_url = api_url_flag
            .or(api_url_env)
            .or_else(|| stored.as_ref().map(|c| c.api_url.clone()));
        let Some(api_url) = api_url else {
            bail!(
                "No API URL given. Pass --api-url, set WALDUR_API_URL, or run \
                 `waldur-cli login`, e.g. https://waldur.example.com"
            );
        };
        let api_url = api_url.trim_end_matches('/').to_string();

        let token = token_flag.or(token_env).or_else(|| stored.map(|c| c.token));
        Ok(Self { api_url, token })
    }
}
