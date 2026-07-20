mod cli;
mod commands;
mod config;
mod output;

use clap::Parser;
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
    command: cli::GroupCommand,

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = config::Config::resolve(cli.api_url, cli.token)?;

    let mut client = HttpClient::new().with_base_url(config.api_url);
    if let Some(token) = config.token {
        client = client.with_api_key(token);
    }

    if let Err(err) = cli::dispatch(&client, cli.command, cli.format).await {
        eprintln!("Error: {err:#}");
        std::process::exit(1);
    }
    Ok(())
}
