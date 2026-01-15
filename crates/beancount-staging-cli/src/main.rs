use std::path::PathBuf;

use anstyle::{AnsiColor, Color, Style};
use anyhow::Result;
use beancount_parser::DirectiveContent;
use beancount_staging::reconcile::{ReconcileConfig, ReconcileItem};
use clap::{Args as ClapArgs, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "diff")]
#[command(about = "Compare journal and staging beancount files")]
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
    /// Interactively review and stage transactions
    Review,
}

fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::Show => show_diff(args.files.journal, args.files.staging),
        Commands::Review => review_interactive(args.files.journal, args.files.staging),
    }
}

fn show_diff(journal: Vec<PathBuf>, staging: Vec<PathBuf>) -> Result<()> {
    let results = ReconcileConfig::new(journal, staging).reconcile()?;

    let journal_style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Yellow)));
    let staging_style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green)));
    let reset = Style::new();

    let mut journal_count = 0;
    let mut staging_count = 0;

    for item in &results {
        match item {
            ReconcileItem::OnlyInJournal(directive) => {
                if let DirectiveContent::Open(_) = directive.content {
                    continue;
                }

                println!("{journal_style}━━━ Only in Journal ━━━{reset}");
                println!("{}", directive);
                println!();
                journal_count += 1;
            }
            ReconcileItem::OnlyInStaging(directive) => {
                println!("{staging_style}━━━ Only in Staging (needs review) ━━━{reset}");
                println!("{}", directive);
                println!();
                staging_count += 1;
            }
        }
    }

    // Summary
    if results.is_empty() {
        println!("✓ All transactions match!");
    } else {
        println!("{}━━━ Summary ━━━{}", Style::new().bold(), reset);
        if journal_count > 0 {
            println!("  {journal_style}{journal_count}{reset} transaction(s) only in journal");
        }
        if staging_count > 0 {
            println!(
                "  {staging_style}{staging_count}{reset} transaction(s) only in staging (need review)"
            );
        }
    }

    Ok(())
}

fn review_interactive(journal: Vec<PathBuf>, staging: Vec<PathBuf>) -> Result<()> {
    println!("Interactive review mode - coming soon!");
    println!("Journal: {:?}", journal);
    println!("Staging: {:?}", staging);
    Ok(())
}
