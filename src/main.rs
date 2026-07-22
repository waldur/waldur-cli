mod cli;
mod commands;
mod config;
mod output;

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
}

#[tokio::main]
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
        Commands::Group(cmd) => *cmd,
    };

    let config = config::Config::resolve(cli.api_url, cli.token)?;

    let mut client = HttpClient::new().with_base_url(config.api_url);
    if let Some(token) = config.token {
        // Waldur's DRF TokenAuthentication expects the literal "Token <key>"
        // format (not "Bearer <key>" -- that's only for the separate OIDC/PAT
        // auth schemes). rs-client's ApiKey auth mode sends this value
        // verbatim with no prefix, so we supply Waldur's own format here.
        client = client.with_api_key(format!("Token {token}"));
    }

    if let Err(err) = cli::dispatch(&client, command, cli.format).await {
        eprintln!("Error: {err:#}");
        std::process::exit(1);
    }
    Ok(())
}
