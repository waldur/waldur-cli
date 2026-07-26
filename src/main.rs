use anyhow::Context;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use waldur_cli::output::{self, OutputFormat};
use waldur_cli::{api, cli, config, http, progress, schema, web};

/// Scriptable CLI for Waldur MasterMind, covering OpenStack resource
/// management and team/organization management. Generated command surface
/// (see waldur/waldur-cli-generator); this file and config.rs/output.rs/
/// pagination.rs/http.rs are hand-written and not touched by generation.
#[derive(Parser, Debug)]
#[command(name = "waldur-cli", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Waldur API base URL. Falls back to the WALDUR_API_URL env var.
    #[arg(long, global = true)]
    api_url: Option<String>,

    /// Waldur API access token. Falls back to the WALDUR_ACCESS_TOKEN env var.
    #[arg(long, global = true)]
    token: Option<String>,

    /// Output format.
    #[arg(long, global = true, value_enum, default_value = "table")]
    format: OutputFormat,

    /// Print request/response tracing (method, URL, status, timing) to stderr
    #[arg(long, global = true)]
    debug: bool,

    /// Preview mutating commands (create/update/delete/provision/terminate)
    /// without executing them: validate the request and print what would be
    /// sent, then exit. No effect on read-only commands.
    #[arg(long, global = true)]
    dry_run: bool,

    /// Named credential profile to use with `login`/`logout` and for
    /// resolving stored credentials. Falls back to the WALDUR_PROFILE env
    /// var, then "default".
    #[arg(long, global = true)]
    profile: Option<String>,

    /// Project (UUID) to scope commands to: applied as a `project_uuid`
    /// filter on resources that support it, and as the `project` on
    /// `provision` orders, unless you specify one explicitly. Falls back to
    /// the WALDUR_PROJECT env var, then the profile's saved default (see
    /// `set-project`).
    #[arg(long, global = true)]
    project: Option<String>,

    /// Base URL of Waldur's web UI (HomePort), used by `get --web`. Falls
    /// back to the WALDUR_HOMEPORT_URL env var, then the `HOMEPORT_URL`
    /// Waldur's own `/api/configuration/` endpoint reports -- only needed if
    /// that's wrong or unreachable for your deployment.
    #[arg(long, global = true)]
    homeport_url: Option<String>,

    /// Seconds to allow each individual HTTP request before giving up
    /// (default 60). Distinct from `provision`/`wait`'s own `--timeout`,
    /// which bounds a whole poll-until-done operation. Falls back to the
    /// WALDUR_HTTP_TIMEOUT env var.
    #[arg(long, global = true, value_name = "SECONDS")]
    http_timeout: Option<u64>,

    /// How many times to retry a request that failed transiently -- a
    /// connection error, a timeout, a 5xx, or a 429 (default 3). Only
    /// applies to replayable requests: a create/provision that may already
    /// have taken effect server-side is never retried. Falls back to the
    /// WALDUR_MAX_RETRIES env var; 0 disables retrying.
    #[arg(long, global = true, value_name = "N")]
    max_retries: Option<u32>,
}

// Flattens the generated `cli::GroupCommand` variants (openstack/team) in
// alongside the hand-written `completions` command, so both sit at the same
// top level without touching generated code.
#[derive(Subcommand, Debug)]
enum Commands {
    #[command(flatten)]
    Group(Box<cli::GroupCommand>),
    /// Generate a shell completion script and print it to stdout
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },
    /// Log in and save the API URL + token to a local config file, so
    /// later commands don't need --token/WALDUR_ACCESS_TOKEN set. Use
    /// --profile to save under a name other than "default"
    Login,
    /// Remove the credentials saved by `login` for the selected profile
    /// (--profile, defaulting to "default")
    Logout,
    /// Show the current user, verifying the active credentials
    Whoami,
    /// Save a default project (UUID) for the selected profile, so
    /// project-scoped commands filter to it and `provision` orders use it
    /// without a `--project` on every invocation
    SetProject {
        /// Project UUID (from `waldur-cli team project list`)
        uuid: String,
    },
    /// Clear the selected profile's saved default project
    UnsetProject,
    /// Print the CLI command schema as JSON — a machine-readable tool
    /// specification that LLM agents can ingest to discover and use this
    /// CLI without parsing `--help` text
    Schema {
        /// Only include commands from this group (e.g. "openstack", "team")
        #[arg(long)]
        group: Option<String>,
        /// Print only command paths and descriptions (minimal output for
        /// context budgets — ~200 tokens for the full CLI)
        #[arg(long)]
        compact: bool,
    },
    /// Update waldur-cli to the latest version published on GitHub
    Update,
    /// Call an arbitrary Waldur API endpoint directly, using the current
    /// --api-url/--token (or --profile's stored credentials) -- an escape
    /// hatch for endpoints this CLI hasn't wired up as a typed command yet,
    /// or for quick one-off debugging. There's no schema to validate a body
    /// against here, so a malformed --request only fails server-side, same
    /// as curl would
    Api {
        /// HTTP method (GET, POST, PUT, PATCH, DELETE; case-insensitive)
        method: String,
        /// API path, relative to --api-url (e.g. /api/customers/) -- a
        /// leading slash is added if you omit it
        path: String,
        /// Request body as inline JSON
        #[arg(long)]
        request: Option<String>,
        /// Read the request body from a JSON or YAML file
        #[arg(long, value_name = "PATH")]
        request_file: Option<std::path::PathBuf>,
        /// Reshape the response with a JMESPath expression
        /// (https://jmespath.org), e.g. '[].uuid'
        #[arg(long)]
        jmespath: Option<String>,
    },
}

/// Same column set `team user get` uses -- whoami is conceptually that,
/// scoped to the caller's own account.
const WHOAMI_COLUMNS: &[&str] = &["uuid", "username", "full_name", "email"];

fn prompt_line(label: &str) -> anyhow::Result<String> {
    use std::io::Write;
    print!("{label}: ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

async fn login(api_url_flag: Option<String>, token_flag: Option<String>, profile: &str) -> anyhow::Result<()> {
    let api_url = match api_url_flag.or_else(|| std::env::var("WALDUR_API_URL").ok()) {
        Some(url) => url,
        None => prompt_line("Waldur API URL")?,
    };
    let api_url = api_url.trim_end_matches('/').to_string();

    let token = match token_flag.or_else(|| std::env::var("WALDUR_ACCESS_TOKEN").ok()) {
        Some(token) => token,
        None => rpassword::prompt_password("Waldur API token: ")?,
    };

    // Validate before persisting anything, so a typo'd token doesn't get
    // silently saved and only surface as a confusing 401 on some later,
    // unrelated command.
    let me = http::call_one(&api_url, Some(&token), reqwest::Method::GET, "/api/users/me/", None)
        .await
        .context("login failed -- check the API URL and token")?;

    // Preserve an existing default project across a re-login -- refreshing a
    // token shouldn't silently drop the profile's project scope.
    let project = config::load_stored(profile)?.and_then(|c| c.project);
    config::save_stored(
        profile,
        &config::StoredCredentials {
            api_url: api_url.clone(),
            token,
            project,
        },
    )?;
    let who = me.get("username").and_then(|v| v.as_str()).unwrap_or("(unknown user)");
    println!(
        "Logged in to {api_url} as {who} (profile '{profile}'). Credentials saved to {}.",
        config::config_path()?.display()
    );
    Ok(())
}

fn logout(profile: &str) -> anyhow::Result<()> {
    if config::delete_stored(profile)? {
        println!(
            "Logged out of profile '{profile}'; removed from {}",
            config::config_path()?.display()
        );
    } else {
        println!("Profile '{profile}' is not logged in (no stored credentials found).");
    }
    Ok(())
}

async fn whoami(base_url: &str, token: Option<&str>, format: OutputFormat) -> anyhow::Result<()> {
    let me = http::call_one(base_url, token, reqwest::Method::GET, "/api/users/me/", None).await?;
    output::print_result(&me, WHOAMI_COLUMNS, format)
}

/// Prints in the same shape success output uses (plain text for `table`,
/// JSON for `format json`), so a script/agent parsing `--format json` output
/// doesn't also need a separate path for failures. Always goes to stderr,
/// regardless of format, so stdout stays clean on the success path only.
fn print_error(err: &anyhow::Error, format: OutputFormat) {
    match format {
        // Ndjson's errors are one compact object same as json's -- ndjson
        // only changes success-path streaming, not the error shape.
        OutputFormat::Json | OutputFormat::Ndjson => {
            eprintln!("{}", serde_json::json!({ "error": format!("{err:#}") }))
        }
        // Toon is a full/lossless structured format like json (not a
        // curated-columns one like table/tsv), so it gets the same
        // structured error object, just toon-encoded.
        OutputFormat::Toon => {
            let value = serde_json::json!({ "error": format!("{err:#}") });
            match serde_toon::to_string(&value) {
                Ok(toon) => eprintln!("{toon}"),
                Err(_) => eprintln!("Error: {err:#}"),
            }
        }
        // Tsv has no structured-object concept the way json does (flat rows
        // only), so it gets the same plain-text error table gets.
        OutputFormat::Table | OutputFormat::Tsv => eprintln!("Error: {err:#}"),
    }
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    let profile = cli
        .profile
        .clone()
        .or_else(|| std::env::var("WALDUR_PROFILE").ok())
        .unwrap_or_else(|| config::DEFAULT_PROFILE.to_string());

    // Let the order-polling spinner know to stay quiet under --debug (its
    // request trace already reports each poll).
    progress::set_debug(cli.debug);

    web::set_override(cli.homeport_url.clone().or_else(|| std::env::var("WALDUR_HOMEPORT_URL").ok()));

    // A malformed env var is ignored rather than fatal: it's a transport
    // tuning knob, and falling back to the default is friendlier than
    // refusing to run at all. An explicit flag is parsed by clap, so a bad
    // value there still errors properly.
    http::set_transport_options(
        cli.http_timeout
            .or_else(|| std::env::var("WALDUR_HTTP_TIMEOUT").ok().and_then(|v| v.parse().ok())),
        cli.max_retries
            .or_else(|| std::env::var("WALDUR_MAX_RETRIES").ok().and_then(|v| v.parse().ok())),
    );

    if cli.debug {
        // reqwest-tracing records request/response fields (method, url,
        // status, time_elapsed) onto a span rather than firing a discrete
        // event, so span-close events must be turned on explicitly -- a
        // bare fmt() subscriber prints nothing for it otherwise.
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_target(false)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
            .init();
    }

    let command = match cli.command {
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            // Generate into an in-memory buffer rather than stdout directly:
            // clap_complete panics internally on a write error, which a bare
            // `waldur-cli completions bash | head` would trigger via SIGPIPE.
            let mut buf = Vec::new();
            clap_complete::generate(shell, &mut cmd, name, &mut buf);
            use std::io::Write;
            if let Err(err) = std::io::stdout().write_all(&buf) {
                if err.kind() != std::io::ErrorKind::BrokenPipe {
                    return Err(err.into());
                }
            }
            return Ok(());
        }
        Commands::Login => return login(cli.api_url, cli.token, &profile).await,
        Commands::Logout => return logout(&profile),
        Commands::SetProject { uuid } => {
            config::set_project(&profile, &uuid)?;
            println!("Default project for profile '{profile}' set to {uuid}");
            return Ok(());
        }
        Commands::UnsetProject => {
            if config::unset_project(&profile)? {
                println!("Cleared the default project for profile '{profile}'");
            } else {
                println!("Profile '{profile}' had no default project");
            }
            return Ok(());
        }
        Commands::Whoami => {
            let config = config::Config::resolve(cli.api_url, cli.token, cli.project, &profile)?;
            // Surface the active project scope so it's never a silent surprise
            // -- on stderr, so `--format json` stdout stays clean.
            if let Some(project) = &config.project {
                eprintln!("Active project scope: {project}");
            }
            return whoami(&config.api_url, config.token.as_deref(), cli.format).await;
        }
        Commands::Schema { group, compact } => {
            return print_schema(group.as_deref(), compact, cli.format);
        }
        Commands::Update => {
            return tokio::task::spawn_blocking(run_update).await?;
        }
        Commands::Api { method, path, request, request_file, jmespath } => {
            let config = config::Config::resolve(cli.api_url, cli.token, cli.project, &profile)?;
            return api::run(
                &config.api_url,
                config.token.as_deref(),
                &method,
                &path,
                request.as_deref(),
                request_file.as_deref(),
                jmespath.as_deref(),
                cli.dry_run,
                cli.format,
            )
            .await;
        }
        Commands::Group(cmd) => *cmd,
    };

    let config = config::Config::resolve(cli.api_url, cli.token, cli.project, &profile)?;
    cli::dispatch(
        &config.api_url,
        config.token.as_deref(),
        config.project.as_deref(),
        cli.dry_run,
        command,
        cli.format,
    )
    .await
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();
    let format = cli.format;
    if let Err(err) = run(cli).await {
        print_error(&err, format);
        std::process::exit(1);
    }
}

/// The `schema` subcommand: prints the CLI's command schema as JSON,
/// optionally filtered by group and/or trimmed to a compact form.
/// Needs no API credentials — everything is embedded at build time.
fn print_schema(group: Option<&str>, compact: bool, format: OutputFormat) -> anyhow::Result<()> {
    let full: serde_json::Value = serde_json::from_str(schema::CLI_SCHEMA_JSON)
        .context("internal error: CLI_SCHEMA_JSON is not valid JSON")?;

    let value = if compact {
        // Compact mode: groups overview + command path + description only.
        let mut output = serde_json::json!({
            "version": full["version"],
        });

        // Filter groups if --group is set.
        if let Some(groups) = full["groups"].as_array() {
            let filtered: Vec<&serde_json::Value> = match group {
                Some(g) => groups.iter().filter(|gr| gr["name"].as_str() == Some(g)).collect(),
                None => groups.iter().collect(),
            };
            output["groups"] = serde_json::json!(filtered);
        }

        // Commands: path + description + type only.
        if let Some(commands) = full["commands"].as_array() {
            let compact_cmds: Vec<serde_json::Value> = commands
                .iter()
                .filter(|cmd| match group {
                    Some(g) => cmd["path"][0].as_str() == Some(g),
                    None => true,
                })
                .map(|cmd| {
                    serde_json::json!({
                        "path": cmd["path"],
                        "description": cmd["description"],
                        "type": cmd["type"]
                    })
                })
                .collect();
            output["commands"] = serde_json::json!(compact_cmds);
        }

        output
    } else if let Some(g) = group {
        // Full mode, filtered to one group.
        let mut output = serde_json::json!({
            "version": full["version"],
        });

        if let Some(groups) = full["groups"].as_array() {
            let filtered: Vec<&serde_json::Value> = groups
                .iter()
                .filter(|gr| gr["name"].as_str() == Some(g))
                .collect();
            output["groups"] = serde_json::json!(filtered);
        }

        if let Some(commands) = full["commands"].as_array() {
            let filtered: Vec<&serde_json::Value> = commands
                .iter()
                .filter(|cmd| cmd["path"][0].as_str() == Some(g))
                .collect();
            output["commands"] = serde_json::json!(filtered);
        }

        output
    } else {
        // Full unfiltered output.
        full
    };

    // Render in the requested format. The schema is a deeply nested object,
    // so table/tsv don't apply — fall through to JSON for those.
    match format {
        OutputFormat::Toon => println!("{}", serde_toon::to_string(&value)?),
        OutputFormat::Ndjson => { output::print_ndjson_line(&value)?; }
        // json / table / tsv all get pretty JSON — table and tsv have no
        // meaningful columnar representation for a nested schema object.
        _ => println!("{}", serde_json::to_string_pretty(&value)?),
    }

    Ok(())
}

/// Self-update using GitHub releases. Needs `unix-archive = ".tar.gz"` in
/// dist-workspace.toml since self_update doesn't natively support cargo-dist's
/// default .tar.xz
fn run_update() -> anyhow::Result<()> {
    println!("Checking for updates...");

    let status = self_update::backends::github::Update::configure()
        .repo_owner("waldur")
        .repo_name("waldur-cli")
        .bin_name("waldur-cli")
        // cargo-dist archives nest the binary in a `{bin}-{target}/` directory
        // rather than at the archive root, which is what `bin_name` alone
        // configures `self_update` to expect.
        .bin_path_in_archive("waldur-cli-{{ target }}/{{ bin }}")
        .show_download_progress(true)
        .current_version(self_update::cargo_crate_version!())
        .build()?
        .update()?;

    if status.updated() {
        println!(
            "Successfully updated waldur-cli to version {}",
            status.version()
        );
    } else {
        println!("waldur-cli is already up-to-date (version {}).", status.version());
    }

    Ok(())
}
