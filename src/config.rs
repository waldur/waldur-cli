//! Auth/connection config, resolved in priority order:
//! 1. `--api-url`/`--token` flags
//! 2. `WALDUR_API_URL`/`WALDUR_ACCESS_TOKEN` env vars
//! 3. the selected profile in the config file written by `waldur-cli login`
//!    (see [`save_stored`]) -- `--profile`/`WALDUR_PROFILE` picks which one,
//!    defaulting to [`DEFAULT_PROFILE`] if neither is given.
//!
//! No interactive login flow beyond `login` itself -- this tool is primarily
//! for scripted/agent use, but a persisted config file avoids having to
//! pass `--token`/set an env var on every single invocation for
//! interactive/local use.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The profile name used when `--profile`/`WALDUR_PROFILE` isn't given.
pub const DEFAULT_PROFILE: &str = "default";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredentials {
    pub api_url: String,
    pub token: String,
    /// Default project (UUID) this profile is scoped to, set via
    /// `set-project`. Optional and omitted from the file when unset, so
    /// existing credential files (written before this field) still parse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CredentialsFile {
    #[serde(default)]
    profiles: BTreeMap<String, StoredCredentials>,
}

/// `~/.config/waldur-cli/credentials.toml` on Linux (respects
/// `XDG_CONFIG_HOME`), the platform equivalent elsewhere (`~/Library/
/// Application Support/...` on macOS, `%APPDATA%` on Windows) -- see the
/// `directories` crate.
///
/// Filename is deliberately NOT `config.toml`: other local tools sharing the
/// "waldur-cli" application name resolve to this same directory
/// (`directories::ProjectDirs` on Linux keys off the application name only,
/// ignoring qualifier/organization) and use `config.toml` for their own,
/// differently-shaped config. A shared filename there means a `logout` in
/// one tool can silently delete the other's saved credentials.
pub fn config_path() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "waldur-cli")
        .context("could not determine a config directory for this platform")?;
    Ok(dirs.config_dir().join("credentials.toml"))
}

fn load_file() -> Result<CredentialsFile> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(CredentialsFile::default());
    }
    let contents =
        std::fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&contents).with_context(|| {
        format!(
            "failed to parse {} -- run `waldur-cli login` again",
            path.display()
        )
    })
}

fn write_file(file: &CredentialsFile) -> Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let contents = toml::to_string_pretty(file)?;
    std::fs::write(&path, contents).with_context(|| format!("failed to write {}", path.display()))?;

    // The file holds plaintext API tokens -- restrict it to the owner, same
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

pub fn load_stored(profile: &str) -> Result<Option<StoredCredentials>> {
    Ok(load_file()?.profiles.get(profile).cloned())
}

pub fn save_stored(profile: &str, creds: &StoredCredentials) -> Result<()> {
    let mut file = load_file()?;
    file.profiles.insert(profile.to_string(), creds.clone());
    write_file(&file)
}

/// Saves a default project (UUID) onto an existing, logged-in profile.
/// Requires credentials to already exist for the profile -- the project
/// default lives alongside them, so there's nowhere to store it otherwise.
pub fn set_project(profile: &str, project: &str) -> Result<()> {
    let mut creds = load_stored(profile)?.with_context(|| {
        format!("profile '{profile}' has no saved credentials -- run `waldur-cli login` first")
    })?;
    creds.project = Some(project.to_string());
    save_stored(profile, &creds)
}

/// Clears a profile's default project. Returns whether one was set.
pub fn unset_project(profile: &str) -> Result<bool> {
    let Some(mut creds) = load_stored(profile)? else {
        return Ok(false);
    };
    let had = creds.project.take().is_some();
    if had {
        save_stored(profile, &creds)?;
    }
    Ok(had)
}

/// Returns whether the profile existed (and was removed). Deletes the whole
/// file once the last profile is gone, rather than leaving an empty stub.
pub fn delete_stored(profile: &str) -> Result<bool> {
    let mut file = load_file()?;
    let existed = file.profiles.remove(profile).is_some();
    if existed {
        if file.profiles.is_empty() {
            let path = config_path()?;
            std::fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        } else {
            write_file(&file)?;
        }
    }
    Ok(existed)
}

pub struct Config {
    pub api_url: String,
    pub token: Option<String>,
    /// The project (UUID) commands should scope to, if any --
    /// `--project` > `WALDUR_PROJECT` > the profile's saved default.
    pub project: Option<String>,
}

impl Config {
    pub fn resolve(
        api_url_flag: Option<String>,
        token_flag: Option<String>,
        project_flag: Option<String>,
        profile: &str,
    ) -> Result<Self> {
        let api_url_env = std::env::var("WALDUR_API_URL").ok();
        let token_env = std::env::var("WALDUR_ACCESS_TOKEN").ok();
        let project_env = std::env::var("WALDUR_PROJECT").ok();

        // Only touch the config file if flags/env vars didn't already give
        // us everything -- keeps a corrupt/unreadable file from breaking
        // fully-explicit invocations, while still surfacing a clear parse
        // error (rather than silently ignoring it) when it's actually needed.
        let need_stored =
            (api_url_flag.is_none() && api_url_env.is_none()) || (token_flag.is_none() && token_env.is_none());
        let stored = if need_stored { load_stored(profile)? } else { None };

        let api_url = api_url_flag
            .or(api_url_env)
            .or_else(|| stored.as_ref().map(|c| c.api_url.clone()));
        let Some(api_url) = api_url else {
            let hint = if profile == DEFAULT_PROFILE {
                "run `waldur-cli login`".to_string()
            } else {
                format!("run `waldur-cli login --profile {profile}`")
            };
            bail!(
                "No API URL given for profile '{profile}'. Pass --api-url, set WALDUR_API_URL, or {hint}, \
                 e.g. https://waldur.example.com"
            );
        };
        let api_url = api_url.trim_end_matches('/').to_string();

        // The saved default project is best-effort: if we didn't already load
        // the profile (fully-explicit api_url+token) and no --project/env was
        // given, a missing or unreadable file just means "no default", never
        // a hard failure of an otherwise-explicit command.
        let project = project_flag.or(project_env).or_else(|| match &stored {
            Some(creds) => creds.project.clone(),
            None => load_stored(profile).ok().flatten().and_then(|c| c.project),
        });

        let token = token_flag.or(token_env).or_else(|| stored.map(|c| c.token));
        Ok(Self {
            api_url,
            token,
            project,
        })
    }
}
