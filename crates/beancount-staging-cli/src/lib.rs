#[allow(dead_code)]
mod review;
mod show;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Args as ClapArgs, CommandFactory as _, Parser, Subcommand};

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
    #[arg(short, long, required = true)]
    journal_file: Vec<PathBuf>,

    /// Staging file path
    #[arg(short, long, required = true)]
    staging_file: Vec<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start web server for interactive review (default)
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "8472")]
        port: u16,
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
    let command = args.command.unwrap_or(Commands::Serve {
        port: beancount_staging_web::DEFAULT_PORT,
    });
    match command {
        Commands::Diff => show::show_diff(args.files.journal_file, args.files.staging_file),
        Commands::Serve { port } => {
            beancount_staging_web::run(args.files.journal_file, args.files.staging_file, port).await
        } /*Commands::Cli => {
                review::review_interactive(args.files.journal_file, args.files.staging_file)
          }*/
    }
}
