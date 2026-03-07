// Top-level CLI definition (clap) and command dispatch.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::commands;

/// FSN – FreeSynergy.Node management tool
#[derive(Parser)]
#[command(
    name = "fsn",
    version,
    author,
    about = "FreeSynergy.Node – deploy and manage your self-hosted platform",
    long_about = None,
)]
pub struct Cli {
    /// Path to the FSN root directory (default: auto-detected)
    #[arg(long, global = true, env = "FSN_ROOT")]
    pub root: Option<PathBuf>,

    /// Path to the project config file
    #[arg(long, global = true, env = "FSN_PROJECT")]
    pub project: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Deploy all services (or a single service) to reach desired state
    Deploy {
        /// Deploy only this service instance (e.g. "forgejo")
        #[arg(long)]
        service: Option<String>,
    },

    /// Stop services without removing data
    Undeploy {
        /// Undeploy only this service instance
        #[arg(long)]
        service: Option<String>,
    },

    /// Pull new images and redeploy modules where version changed
    Update {
        /// Update only this service instance
        #[arg(long)]
        service: Option<String>,
    },

    /// Restart services
    Restart {
        /// Restart only this service instance
        #[arg(long)]
        service: Option<String>,
    },

    /// Remove services and all their data permanently
    Remove {
        /// Remove only this service instance
        #[arg(long)]
        service: Option<String>,

        /// Skip the confirmation prompt
        #[arg(long)]
        confirm: bool,
    },

    /// Remove orphaned containers and volumes not in any project
    Clean,

    /// Show what would change without applying any changes (dry-run)
    Sync,

    /// Show running services and their health status
    Status,

    /// Show live logs for a service
    Logs {
        /// Service instance name (e.g. "forgejo")
        service: String,

        /// Follow log output (like tail -f)
        #[arg(short, long)]
        follow: bool,
    },

    /// Config file management
    Config {
        #[command(subcommand)]
        cmd: ConfigCommand,
    },

    /// Start the web management UI
    Serve {
        /// Port to listen on
        #[arg(long, default_value = "8080")]
        port: u16,

        /// Bind address
        #[arg(long, default_value = "127.0.0.1")]
        bind: String,
    },

    /// Interactive first-time setup wizard (replaces fsn-install.sh for ongoing use)
    Init,

    /// Server-level administration (run as root)
    Server {
        #[command(subcommand)]
        cmd: ServerCommand,
    },
}

#[derive(Subcommand)]
pub enum ServerCommand {
    /// Prepare a server for FreeSynergy.Node (Podman, linger, unprivileged ports).
    /// Must be run as root or via sudo.
    Setup,
}

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Show the merged resolved config (module defaults + project.yml)
    Show,

    /// Open project.yml in $EDITOR
    Edit,

    /// Validate config files and check constraints
    Validate,
}

/// Parse args and dispatch to the right command handler.
pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    // Resolve FSN root: --root flag > env var > auto-detect
    let root = cli
        .root
        .or_else(|| std::env::var("FSN_ROOT").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));

    match cli.command {
        Command::Deploy { service }        => commands::deploy::run(&root, cli.project.as_deref(), service.as_deref()).await,
        Command::Undeploy { service }      => commands::undeploy::run(&root, cli.project.as_deref(), service.as_deref()).await,
        Command::Update { service }        => commands::update::run(&root, cli.project.as_deref(), service.as_deref()).await,
        Command::Restart { service }       => commands::restart::run(&root, cli.project.as_deref(), service.as_deref()).await,
        Command::Remove { service, confirm } => commands::remove::run(&root, cli.project.as_deref(), service.as_deref(), confirm).await,
        Command::Clean                     => commands::clean::run(&root, cli.project.as_deref()).await,
        Command::Sync                      => commands::sync::run(&root, cli.project.as_deref()).await,
        Command::Status                    => commands::status::run(&root, cli.project.as_deref()).await,
        Command::Logs { service, follow }  => commands::logs::run(&service, follow).await,
        Command::Config { cmd }            => commands::config::run(&root, cli.project.as_deref(), cmd).await,
        Command::Serve { port, bind }      => commands::serve::run(&root, cli.project.as_deref(), &bind, port).await,
        Command::Init                      => commands::init::run(&root).await,
        Command::Server { cmd }            => match cmd {
            ServerCommand::Setup           => commands::server_setup::run(&root).await,
        },
    }
}
