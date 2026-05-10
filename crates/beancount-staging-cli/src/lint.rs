//! `beancount-staging lint`: check auto_categorize rules against the journal.
//!
//! For each rule, walk the existing journal transactions that the rule would
//! match and compare the rule's `assign_target_account` to what the user
//! actually categorized those transactions as. Flag any disagreements, plus
//! rules that never matched (likely typos or stale entries).
//!
//! "User's historical category" is taken to be the second posting's account.
//! If the rule and the journal agree, the rule is a safe codification of past
//! behavior; if they disagree, committing this rule would have miscategorized
//! that transaction.

use std::path::PathBuf;

use anstyle::{AnsiColor, Color, Style};
use anyhow::Result;
use beancount_parser::DirectiveContent;
use beancount_staging::{AutoCategorizeRule, Directive};

pub fn run_lint(journal_paths: Vec<PathBuf>, rules: &[AutoCategorizeRule]) -> Result<()> {
    if rules.is_empty() {
        println!("No [[auto_categorize]] rules configured.");
        return Ok(());
    }

    let mut journal: Vec<Directive> = Vec::new();
    for path in &journal_paths {
        journal.extend(beancount_staging::read_directives(path)?);
    }

    let warn_style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Yellow)));
    let ok_style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green)));
    let info_style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::BrightBlack)));

    let mut total_disagreements = 0;
    let mut unused_rules = 0;

    for (idx, rule) in rules.iter().enumerate() {
        let mut agree = 0;
        // (date, payee, narration, historical_target)
        let mut disagreements: Vec<(String, String, String, String)> = Vec::new();

        for directive in &journal {
            if !rule.matches(directive) {
                continue;
            }
            let DirectiveContent::Transaction(txn) = &directive.content else {
                continue;
            };
            // Use the second posting's account as the historical category.
            // If the journal entry only has one posting, there's nothing to
            // compare against.
            let Some(historical) = txn.postings.get(1).map(|p| p.account.to_string()) else {
                continue;
            };

            if historical == rule.assign_target_account {
                agree += 1;
            } else {
                disagreements.push((
                    directive.date.to_string(),
                    txn.payee.clone().unwrap_or_default(),
                    txn.narration.clone().unwrap_or_default(),
                    historical,
                ));
            }
        }

        let total = agree + disagreements.len();
        let header = format!(
            "[{idx}] source={} payee={:?} narration={:?} -> {}",
            rule.match_source_account,
            rule.match_payee.as_ref().map(|r| r.as_str()),
            rule.match_narration.as_ref().map(|r| r.as_str()),
            rule.assign_target_account,
        );

        if total == 0 {
            unused_rules += 1;
            println!("{warn_style}⚠ unused rule{warn_style:#}  {header}");
            println!("{info_style}    no journal transactions matched this rule{info_style:#}");
        } else if disagreements.is_empty() {
            println!(
                "{ok_style}✓ ok{ok_style:#}         {header}  ({} match{})",
                agree,
                if agree == 1 { "" } else { "es" }
            );
        } else {
            total_disagreements += disagreements.len();
            println!("{warn_style}⚠ disagree{warn_style:#}   {header}",);
            println!(
                "{info_style}    {} agree, {} disagree{info_style:#}",
                agree,
                disagreements.len()
            );
            for (date, payee, narration, historical) in &disagreements {
                println!(
                    "{info_style}      {date} {payee:?} {narration:?}{info_style:#}  -> historical: {warn_style}{historical}{warn_style:#}"
                );
            }
        }
    }

    println!();
    if total_disagreements == 0 && unused_rules == 0 {
        println!("{ok_style}All rules consistent with journal history.{ok_style:#}");
    } else {
        if total_disagreements > 0 {
            println!(
                "{warn_style}{total_disagreements} disagreement{plural}{warn_style:#} between rules and journal history.",
                plural = if total_disagreements == 1 { "" } else { "s" },
            );
        }
        if unused_rules > 0 {
            println!(
                "{warn_style}{unused_rules} unused rule{plural}{warn_style:#} — possibly a typo or stale entry.",
                plural = if unused_rules == 1 { "" } else { "s" },
            );
        }
    }

    Ok(())
}
