mod review;
mod show;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Args as ClapArgs, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "beancount-staging")]
#[command(about = "Tools for reviewing and staging beancount transactions")]
struct Args {
    #[command(flatten)]
    files: FileArgs,

    #[command(subcommand)]
    command: Commands,
}

#[derive(ClapArgs)]
struct FileArgs {
    /// Journal file path
    #[arg(short, long, required = true)]
    journal: Vec<PathBuf>,

    /// Staging file path
    #[arg(short, long, required = true)]
    staging: Vec<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Show differences between journal and staging files
    Show,
    /// Interactively review and stage transactions (TUI)
    Review,
    /// Start web server for interactive review
    Web {
        /// Port to listen on
        #[arg(short, long, default_value = "8472")]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::Show => show::show_diff(args.files.journal, args.files.staging),
        Commands::Review => review::review_interactive(args.files.journal, args.files.staging),
        Commands::Web { port } => {
            beancount_staging_web::run(args.files.journal, args.files.staging, port).await
        }
    }
}
