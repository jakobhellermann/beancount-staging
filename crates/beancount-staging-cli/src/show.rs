use std::path::PathBuf;

use anstyle::{AnsiColor, Color, Style};
use anyhow::Result;
use beancount_parser::DirectiveContent;
use beancount_staging::reconcile::{
    MismatchReason, ReconcileConfig, ReconcileItemKind, StagingSource,
};

pub fn show_diff(
    journal: Vec<PathBuf>,
    staging_source: StagingSource,
    debug: bool,
    include_only_journal: bool,
) -> Result<()> {
    let config = ReconcileConfig::new(journal, staging_source);
    let state = config.read()?;
    let results = state.reconcile()?;

    let journal_style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Yellow)));
    let staging_style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green)));
    let debug_style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::BrightBlack)));

    let mut journal_count = 0;
    let mut staging_count = 0;

    for item in &results {
        match &item.item {
            ReconcileItemKind::OnlyInJournal(directive) => {
                if let DirectiveContent::Open(_)
                | DirectiveContent::Price(_)
                | DirectiveContent::Commodity(_)
                | DirectiveContent::Pad(_) = directive.content
                {
                    continue;
                }
                journal_count += 1;

                if include_only_journal {
                    println!("{journal_style}━━━ Only in Journal ━━━{journal_style:#}");
                    println!("{}", directive);
                    println!();
                }
            }
            ReconcileItemKind::OnlyInStaging(directive) => {
                println!("{staging_style}━━━ Only in Staging (needs review) ━━━{staging_style:#}");
                println!("{}", directive);

                if debug && !item.mismatch_reasons.is_empty() {
                    println!();
                    println!(
                        "{debug_style}Debug: Checked against {} journal transaction(s) on same date:{debug_style:#}",
                        item.mismatch_reasons.len()
                    );
                    for (idx, journal_item, reason) in &item.mismatch_reasons {
                        if let MismatchReason::DifferentDirectiveType = reason {
                            continue;
                        }
                        println!(
                            "{debug_style}  [{}] {} - {}{debug_style:#}",
                            idx, reason, journal_item.date
                        );
                        // Show a preview of the journal transaction
                        if let DirectiveContent::Transaction(txn) = &journal_item.content {
                            let payee = txn.payee.as_deref().unwrap_or("");
                            let narration = txn.narration.as_deref().unwrap_or("");
                            if !payee.is_empty() {
                                println!(
                                    "{debug_style}      Journal: \"{}\" \"{}\"{debug_style:#}",
                                    payee, narration
                                );
                            } else {
                                println!(
                                    "{debug_style}      Journal: \"{}\"{debug_style:#}",
                                    narration
                                );
                            }
                        }
                    }
                }

                println!();
                staging_count += 1;
            }
        }
    }

    // Summary
    if journal_count == 0 && staging_count == 0 {
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
