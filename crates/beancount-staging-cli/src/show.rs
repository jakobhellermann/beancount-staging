use std::path::PathBuf;

use anstyle::{AnsiColor, Color, Style};
use anyhow::Result;
use beancount_parser::DirectiveContent;
use beancount_staging::reconcile::{ReconcileConfig, ReconcileItem, StagingSource};

pub fn show_diff(journal: Vec<PathBuf>, staging_source: StagingSource) -> Result<()> {
    let config = ReconcileConfig::new(journal, staging_source);
    let state = config.read()?;
    let results = state.reconcile()?;

    let journal_style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Yellow)));
    let staging_style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green)));

    let mut journal_count = 0;
    let mut staging_count = 0;

    for item in &results {
        match item {
            ReconcileItem::OnlyInJournal(directive) => {
                if let DirectiveContent::Open(_)
                | DirectiveContent::Price(_)
                | DirectiveContent::Commodity(_)
                | DirectiveContent::Pad(_) = directive.content
                {
                    continue;
                }

                // println!("{journal_style}━━━ Only in Journal ━━━{journal_style:#}");
                // println!("{}", directive);
                // println!();
                journal_count += 1;
            }
            ReconcileItem::OnlyInStaging(directive) => {
                println!("{staging_style}━━━ Only in Staging (needs review) ━━━{staging_style:#}");
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
        println!("━━━ Summary ━━━");
        if journal_count > 0 {
            println!(
                "  {journal_style}{journal_count}{journal_style:#} transaction{s} only in journal",
                s = if journal_count == 1 { "" } else { "s" }
            );
        }
        if staging_count > 0 {
            println!(
                "  {staging_style}{staging_count}{staging_style:#} transaction{s} staging",
                s = if journal_count == 1 { "" } else { "s" }
            );
        }
    }

    Ok(())
}
