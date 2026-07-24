mod cli;
mod commands;
mod config;
mod filter;
mod http;
mod order;
mod output;
mod pagination;
mod progress;
mod query;
mod request;

use anyhow::Context;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use output::OutputFormat;
use waldur_client::HttpClient;

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
}

/// Same column set `team user get` uses -- whoami is conceptually that,
/// scoped to the caller's own account.
const WHOAMI_COLUMNS: &[&str] = &["uuid", "username", "full_name", "email"];

/// Waldur's DRF TokenAuthentication expects the literal "Token <key>" format
/// (not "Bearer <key>" -- that's only for the separate OIDC/PAT auth
/// schemes). rs-client's ApiKey auth mode sends this value verbatim with no
/// prefix, so we supply Waldur's own format here.
fn build_client(api_url: String, token: Option<&str>) -> HttpClient {
    let mut client = HttpClient::new().with_base_url(api_url);
    if let Some(token) = token {
        client = client.with_api_key(format!("Token {token}"));
    }
    client
}

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
    let client = build_client(api_url.clone(), Some(&token));
    let me = client
        .users_me_retrieve(None)
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
    let who = me.username.as_deref().unwrap_or("(unknown user)");
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

async fn whoami(client: &HttpClient, format: OutputFormat) -> anyhow::Result<()> {
    let me = client.users_me_retrieve(None).await?;
    output::print_result(&me, WHOAMI_COLUMNS, format)
}

/// Prints in the same shape success output uses (plain text for `table`,
/// JSON for `format json`), so a script/agent parsing `--format json` output
/// doesn't also need a separate path for failures. Always goes to stderr,
/// regardless of format, so stdout stays clean on the success path only.
fn print_error(err: &anyhow::Error, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
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
            let client = build_client(config.api_url, config.token.as_deref());
            return whoami(&client, cli.format).await;
        }
        Commands::Group(cmd) => *cmd,
    };

    let config = config::Config::resolve(cli.api_url, cli.token, cli.project, &profile)?;
    let client = build_client(config.api_url.clone(), config.token.as_deref());
    cli::dispatch(
        &client,
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
