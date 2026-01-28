mod config;
#[allow(dead_code)]
mod review;
mod show;

pub use beancount_staging;

use std::path::PathBuf;

use anyhow::Result;
use beancount_staging::reconcile::StagingSource;
use clap::{Args as ClapArgs, CommandFactory as _, Parser, Subcommand, error::ErrorKind};

#[derive(Parser)]
#[command(
    name = "beancount-staging",
    about = "Tools for reviewing and staging beancount transactions"
)]
#[command(disable_help_subcommand = true)]
struct Args {
    #[command(flatten)]
    files: FileArgs,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(ClapArgs)]
struct FileArgs {
    /// Journal file path. Staged transactions will be written into the first file.
    #[arg(short, long)]
    journal_file: Vec<PathBuf>,

    /// Staging file path.
    #[arg(short, long)]
    staging_file: Vec<PathBuf>,

    /// Config file or directory path. If a directory is provided, will look for beancount-staging.toml or .beancount-staging.toml in that directory. If not provided, will look in current directory.
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start web server for interactive review (default)
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "8472", conflicts_with = "socket")]
        port: Option<u16>,

        /// Unix socket path to listen on (alternative to --port)
        #[arg(long, conflicts_with = "port")]
        socket: Option<PathBuf>,
    },
    /// Show differences between journal and staging files and exit
    Diff,
    // /// Interactively review and stage transactions in the terminal
    // Cli,
}

pub async fn run(args: impl IntoIterator<Item = String>) -> Result<()> {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "beancount_staging=info".into());
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    clap_complete::CompleteEnv::with_factory(Args::command).complete();

    let args = Args::parse_from(args);
    let mut cmd = Args::command();

    // load config
    let mut config = if let Some(config_path) = args.files.config {
        Some(config::Config::load_from_file(&config_path)?)
    } else {
        config::Config::find_and_load()?
    };

    let mut journal_paths = config
        .as_mut()
        .map(|(base_dir, c)| {
            c.journal
                .files
                .iter()
                .map(|path| base_dir.join(path))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Extract staging source from config (either files or command)
    let mut staging_source = config.as_mut().map(|(base_dir, c)| match &c.staging.0 {
        StagingSource::Files(files) => StagingSource::Files(
            files
                .iter()
                .map(|path| base_dir.join(path))
                .collect::<Vec<_>>(),
        ),
        StagingSource::Command { command, cwd: _ } => StagingSource::Command {
            command: command.clone(),
            cwd: base_dir.clone(),
        },
    });

    // override from cli
    if !args.files.journal_file.is_empty() {
        journal_paths = args.files.journal_file;
    }
    if !args.files.staging_file.is_empty() {
        staging_source = Some(StagingSource::Files(args.files.staging_file));
    }

    // Validate that we have both journal and staging source
    if journal_paths.is_empty() {
        cmd.error(
            ErrorKind::MissingRequiredArgument,
            "Journal file path required:\n    Pass via --journal-file <JOURNAL_FILE> or beancount-staging.toml",
        )
        .exit();
    }
    if staging_source.is_none() {
        cmd.error(
            ErrorKind::MissingRequiredArgument,
            "Staging file path or command required:\n    Pass via --staging-file <STAGING_FILE> or beancount-staging.toml",
        )
        .exit();
    }
    let staging_source = staging_source.unwrap();

    let command = args.command.unwrap_or(Commands::Serve {
        port: Some(beancount_staging_web::DEFAULT_PORT),
        socket: None,
    });
    match command {
        Commands::Diff => show::show_diff(journal_paths, staging_source),
        Commands::Serve { port, socket } => {
            let listener = if let Some(socket_path) = socket {
                beancount_staging_web::ListenerType::UnixSocket(socket_path)
            } else {
                beancount_staging_web::ListenerType::Tcp(
                    port.unwrap_or(beancount_staging_web::DEFAULT_PORT),
                )
            };
            beancount_staging_web::run(journal_paths, staging_source, listener).await
        } /*Commands::Cli => {
                review::review_interactive(journal_paths, staging_source)
          }*/
    }
}
