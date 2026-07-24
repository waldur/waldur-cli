//! Tests for src/config.rs: credential/project resolution precedence and the
//! on-disk credentials file.
//!
//! Every test here touches process-global state (env vars, and indirectly
//! `$XDG_CONFIG_HOME` for the credentials file) via `directories::ProjectDirs`,
//! which reads `XDG_CONFIG_HOME` itself -- so every test is `#[serial]` and
//! points `XDG_CONFIG_HOME` at its own tempdir, otherwise tests running in
//! parallel (Rust's default) would race on the same env vars/file.

use serial_test::serial;
use waldur_cli::config::{self, Config, StoredCredentials};

/// Points XDG_CONFIG_HOME at a fresh tempdir and clears the env vars
/// Config::resolve reads, so each test starts from a clean, isolated slate
/// regardless of what's actually set in the process running the suite.
struct Isolated {
    _dir: tempfile::TempDir,
}

impl Isolated {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("XDG_CONFIG_HOME", dir.path());
        std::env::remove_var("WALDUR_API_URL");
        std::env::remove_var("WALDUR_ACCESS_TOKEN");
        std::env::remove_var("WALDUR_PROJECT");
        Self { _dir: dir }
    }
}

#[test]
#[serial]
fn flag_wins_over_env_and_stored() {
    let _iso = Isolated::new();
    std::env::set_var("WALDUR_API_URL", "https://env.example.com");
    config::save_stored(
        "default",
        &StoredCredentials {
            api_url: "https://stored.example.com".to_string(),
            token: "stored-token".to_string(),
            project: None,
        },
    )
    .unwrap();

    let cfg = Config::resolve(
        Some("https://flag.example.com".to_string()),
        Some("flag-token".to_string()),
        None,
        "default",
    )
    .unwrap();

    assert_eq!(cfg.api_url, "https://flag.example.com");
    assert_eq!(cfg.token.as_deref(), Some("flag-token"));
}

#[test]
#[serial]
fn env_wins_over_stored_when_no_flag() {
    let _iso = Isolated::new();
    std::env::set_var("WALDUR_API_URL", "https://env.example.com");
    std::env::set_var("WALDUR_ACCESS_TOKEN", "env-token");
    config::save_stored(
        "default",
        &StoredCredentials {
            api_url: "https://stored.example.com".to_string(),
            token: "stored-token".to_string(),
            project: None,
        },
    )
    .unwrap();

    let cfg = Config::resolve(None, None, None, "default").unwrap();

    assert_eq!(cfg.api_url, "https://env.example.com");
    assert_eq!(cfg.token.as_deref(), Some("env-token"));
}

#[test]
#[serial]
fn falls_back_to_stored_profile_when_nothing_else_given() {
    let _iso = Isolated::new();
    config::save_stored(
        "default",
        &StoredCredentials {
            api_url: "https://stored.example.com".to_string(),
            token: "stored-token".to_string(),
            project: None,
        },
    )
    .unwrap();

    let cfg = Config::resolve(None, None, None, "default").unwrap();

    assert_eq!(cfg.api_url, "https://stored.example.com");
    assert_eq!(cfg.token.as_deref(), Some("stored-token"));
}

#[test]
#[serial]
fn missing_api_url_everywhere_is_a_clear_error() {
    let _iso = Isolated::new();
    let err = Config::resolve(None, None, None, "default").unwrap_err();
    assert!(err.to_string().contains("No API URL"));
    assert!(err.to_string().contains("waldur-cli login"));
}

#[test]
#[serial]
fn non_default_profile_error_hint_mentions_the_profile() {
    let _iso = Isolated::new();
    let err = Config::resolve(None, None, None, "staging").unwrap_err();
    assert!(err.to_string().contains("--profile staging"));
}

#[test]
#[serial]
fn api_url_has_trailing_slash_stripped() {
    let _iso = Isolated::new();
    let cfg = Config::resolve(
        Some("https://waldur.example.com/".to_string()),
        Some("t".to_string()),
        None,
        "default",
    )
    .unwrap();
    assert_eq!(cfg.api_url, "https://waldur.example.com");
}

#[test]
#[serial]
fn project_precedence_flag_then_env_then_stored_default() {
    let _iso = Isolated::new();
    config::save_stored(
        "default",
        &StoredCredentials {
            api_url: "https://x.example.com".to_string(),
            token: "t".to_string(),
            project: Some("stored-project".to_string()),
        },
    )
    .unwrap();

    // Stored default alone.
    let cfg = Config::resolve(None, None, None, "default").unwrap();
    assert_eq!(cfg.project.as_deref(), Some("stored-project"));

    // Env overrides stored.
    std::env::set_var("WALDUR_PROJECT", "env-project");
    let cfg = Config::resolve(None, None, None, "default").unwrap();
    assert_eq!(cfg.project.as_deref(), Some("env-project"));

    // Flag overrides both.
    let cfg = Config::resolve(None, None, Some("flag-project".to_string()), "default").unwrap();
    assert_eq!(cfg.project.as_deref(), Some("flag-project"));

    std::env::remove_var("WALDUR_PROJECT");
}

#[test]
#[serial]
fn fully_explicit_call_never_touches_a_corrupt_stored_file() {
    let iso = Isolated::new();
    let path = config::config_path().unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "this is not valid toml {{{").unwrap();

    // api_url + token both given explicitly -- Config::resolve must not even
    // attempt to read the (corrupt) stored file for those two fields.
    let cfg = Config::resolve(
        Some("https://flag.example.com".to_string()),
        Some("flag-token".to_string()),
        None,
        "default",
    )
    .unwrap();
    assert_eq!(cfg.api_url, "https://flag.example.com");
    drop(iso);
}

#[test]
#[serial]
fn set_and_unset_project_round_trip() {
    let _iso = Isolated::new();
    config::save_stored(
        "default",
        &StoredCredentials {
            api_url: "https://x.example.com".to_string(),
            token: "t".to_string(),
            project: None,
        },
    )
    .unwrap();

    config::set_project("default", "proj-1").unwrap();
    assert_eq!(
        config::load_stored("default").unwrap().unwrap().project.as_deref(),
        Some("proj-1")
    );

    let had = config::unset_project("default").unwrap();
    assert!(had);
    assert_eq!(config::load_stored("default").unwrap().unwrap().project, None);

    // Unsetting again reports nothing was set, rather than erroring.
    let had_again = config::unset_project("default").unwrap();
    assert!(!had_again);
}

#[test]
#[serial]
fn set_project_without_prior_login_fails_clearly() {
    let _iso = Isolated::new();
    let err = config::set_project("default", "proj-1").unwrap_err();
    assert!(err.to_string().contains("no saved credentials"));
}

#[test]
#[serial]
fn delete_stored_removes_file_when_last_profile_gone() {
    let _iso = Isolated::new();
    config::save_stored(
        "default",
        &StoredCredentials {
            api_url: "https://x.example.com".to_string(),
            token: "t".to_string(),
            project: None,
        },
    )
    .unwrap();

    let existed = config::delete_stored("default").unwrap();
    assert!(existed);
    assert!(config::load_stored("default").unwrap().is_none());
    assert!(!config::config_path().unwrap().exists());

    // Deleting a profile that was never there just reports false.
    let existed_again = config::delete_stored("default").unwrap();
    assert!(!existed_again);
}

#[test]
#[serial]
fn multiple_profiles_are_independent() {
    let _iso = Isolated::new();
    config::save_stored(
        "prod",
        &StoredCredentials {
            api_url: "https://prod.example.com".to_string(),
            token: "prod-token".to_string(),
            project: None,
        },
    )
    .unwrap();
    config::save_stored(
        "staging",
        &StoredCredentials {
            api_url: "https://staging.example.com".to_string(),
            token: "staging-token".to_string(),
            project: None,
        },
    )
    .unwrap();

    let prod = Config::resolve(None, None, None, "prod").unwrap();
    let staging = Config::resolve(None, None, None, "staging").unwrap();
    assert_eq!(prod.api_url, "https://prod.example.com");
    assert_eq!(staging.api_url, "https://staging.example.com");

    // Deleting one profile leaves the other intact.
    config::delete_stored("prod").unwrap();
    assert!(config::load_stored("prod").unwrap().is_none());
    assert!(config::load_stored("staging").unwrap().is_some());
}

#[cfg(unix)]
#[test]
#[serial]
fn credentials_file_is_owner_only_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let _iso = Isolated::new();
    config::save_stored(
        "default",
        &StoredCredentials {
            api_url: "https://x.example.com".to_string(),
            token: "secret-token".to_string(),
            project: None,
        },
    )
    .unwrap();

    let path = config::config_path().unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}
