mod cli;
mod commands;
mod config;
mod output;

use anyhow::Context;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use output::OutputFormat;
use waldur_client::HttpClient;

/// Scriptable CLI for Waldur MasterMind, covering OpenStack resource
/// management and team/organization management. Generated command surface
/// (see waldur/waldur-cli-generator); this file and config.rs/output.rs are
/// hand-written and not touched by generation.
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
    /// later commands don't need --token/WALDUR_ACCESS_TOKEN set
    Login,
    /// Remove the config file written by `login`
    Logout,
}

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

async fn login(api_url_flag: Option<String>, token_flag: Option<String>) -> anyhow::Result<()> {
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

    config::save_stored(&config::StoredCredentials {
        api_url: api_url.clone(),
        token,
    })?;
    let who = me.username.as_deref().unwrap_or("(unknown user)");
    println!(
        "Logged in to {api_url} as {who}. Credentials saved to {}.",
        config::config_path()?.display()
    );
    Ok(())
}

fn logout() -> anyhow::Result<()> {
    if config::delete_stored()? {
        println!("Logged out; removed {}", config::config_path()?.display());
    } else {
        println!("Not logged in (no stored credentials found).");
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

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
        Commands::Login => return login(cli.api_url, cli.token).await,
        Commands::Logout => return logout(),
        Commands::Group(cmd) => *cmd,
    };

    let config = config::Config::resolve(cli.api_url, cli.token)?;
    let client = build_client(config.api_url, config.token.as_deref());

    if let Err(err) = cli::dispatch(&client, command, cli.format).await {
        eprintln!("Error: {err:#}");
        std::process::exit(1);
    }
    Ok(())
}
