//! Reconciling differences between existing journal entries and a full automatic import.

pub(crate) mod matching;

use crate::Result;
use crate::utils::sort_merge_diff::{JoinResult, SortMergeDiff};
use crate::{Decimal, Directive};
use beancount_parser::{Date, DirectiveContent, Entry};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::PathBuf;

#[derive(Debug)]
pub enum ReconcileItem<'a> {
    OnlyInJournal(&'a Directive),
    OnlyInStaging(&'a Directive),
}

pub type SourceSet = HashSet<PathBuf>;

#[derive(Debug, Clone)]
pub enum StagingSource {
    Files(Vec<PathBuf>),
    Command { command: Vec<String>, cwd: PathBuf },
}

pub struct ReconcileConfig {
    pub journal_paths: Vec<PathBuf>,
    pub staging_source: StagingSource,
}
impl ReconcileConfig {
    pub fn new(journal_paths: Vec<PathBuf>, staging_source: StagingSource) -> Self {
        ReconcileConfig {
            journal_paths,
            staging_source,
        }
    }

    pub fn read(&self) -> Result<ReconcileState> {
        let (journal, journal_sourceset) = read_directives_from_files(&self.journal_paths)?;
        let (staging, staging_sourceset) = match &self.staging_source {
            StagingSource::Files(paths) => read_directives_from_files(paths)?,
            StagingSource::Command { command, cwd } => read_directives_from_command(command, cwd)?,
        };
        Ok(ReconcileState {
            journal_sourceset,
            staging_sourceset,
            journal,
            staging,
        })
    }
}

#[derive(Default)]
pub struct ReconcileState {
    pub journal_sourceset: SourceSet,
    pub staging_sourceset: SourceSet,

    pub journal: Vec<Directive>,
    pub staging: Vec<Directive>,
}
impl ReconcileState {
    /// Try to associate all journal and staging items, returning a list of differences.
    pub fn reconcile(&self) -> Result<Vec<ReconcileItem<'_>>> {
        let journal = group_by_date(&self.journal);
        let staging = group_by_date(&self.staging);
        let results = reconcile(journal, staging);
        Ok(results)
    }

    pub fn accounts(&self) -> BTreeSet<String> {
        self.journal
            .iter()
            .filter_map(|directive| Some(directive.content.as_open()?.account.to_string()))
            .collect()
    }
}

fn read_directives_from_files(path: &[PathBuf]) -> Result<(Vec<Directive>, HashSet<PathBuf>)> {
    let mut directives = Vec::new();
    let files = path.iter().map(Clone::clone);
    let mut iter = beancount_parser::read_files_iter::<Decimal>(files);
    for entry in iter.by_ref() {
        if let Entry::Directive(directive) = entry? {
            directives.push(directive);
        }
    }
    crate::sorting::sort_dedup_directives(&mut directives);

    Ok((directives, iter.loaded()))
}

fn read_directives_from_command(
    command: &[String],
    cwd: &PathBuf,
) -> Result<(Vec<Directive>, HashSet<PathBuf>)> {
    use anyhow::Context;
    use std::process::{Command, Stdio};
    use std::time::Instant;

    if command.is_empty() {
        anyhow::bail!("Command cannot be empty");
    }

    let command_str = command.join(" ");

    let start = Instant::now();
    let output = Command::new(&command[0])
        .args(&command[1..])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("Failed to execute staging command: {}", command_str))?;
    let elapsed = start.elapsed();

    tracing::info!("Staging command executed in {:?}", elapsed);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr_msg = if stderr.is_empty() {
            String::new()
        } else {
            format!("\nStderr:\n{}", stderr.trim())
        };
        anyhow::bail!(
            "Staging command failed with exit code {}: {}{}",
            output.status.code().unwrap_or(-1),
            command_str,
            stderr_msg
        );
    }

    let stdout = String::from_utf8(output.stdout)
        .with_context(|| format!("Staging command output is not valid UTF-8: {}", command_str))?;

    let mut directives = Vec::new();
    for entry in beancount_parser::parse_iter::<Decimal>(&stdout) {
        if let Entry::Directive(directive) = entry
            .with_context(|| format!("Failed to parse staging command output: {}", command_str))?
        {
            directives.push(directive);
        }
    }
    crate::sorting::sort_dedup_directives(&mut directives);

    // For command-based staging, we don't have file paths, so return empty set
    Ok((directives, HashSet::new()))
}

fn group_by_date(all: &[Directive]) -> BTreeMap<Date, Vec<&Directive>> {
    let mut directives: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for directive in all {
        directives
            .entry(directive.date)
            .or_default()
            .push(directive);
    }

    directives
}

fn reconcile<'a>(
    journal: BTreeMap<Date, Vec<&'a Directive>>,
    staging: BTreeMap<Date, Vec<&'a Directive>>,
) -> Vec<ReconcileItem<'a>> {
    let mut results = Vec::new();

    for bucket in SortMergeDiff::new(
        journal.into_iter(),
        staging.into_iter(),
        |(date_a, _), (date_b, _)| date_a.cmp(date_b),
    ) {
        match bucket {
            JoinResult::OnlyInFirst((_, items)) => {
                results.extend(items.into_iter().map(ReconcileItem::OnlyInJournal));
            }
            JoinResult::OnlyInSecond((_, items)) => {
                results.extend(items.into_iter().map(ReconcileItem::OnlyInStaging));
            }
            JoinResult::InBoth((_, bucket_journal), (_, bucket_staging)) => {
                reconcile_bucket(&mut results, bucket_journal, bucket_staging);
            }
        }
    }

    results
}

// PERF: O(journal*staging) per bucket
fn reconcile_bucket<'a>(
    results: &mut Vec<ReconcileItem<'a>>,
    mut journal: Vec<&'a Directive>,
    mut staging: Vec<&'a Directive>,
) {
    while let Some(staging_item) = staging.pop() {
        if let DirectiveContent::Transaction(staging_item) = &staging_item.content {
            // not supported yet
            if staging_item.postings.is_empty() {
                tracing::warn!("Staging transaction contains zero postings");
                continue;
            }
        }

        let match_at = journal.iter().position(|journal_item| {
            matching::journal_matches_staging(journal_item, staging_item).is_ok()
        });
        if let Some(match_at) = match_at {
            journal.remove(match_at);
        } else {
            results.push(ReconcileItem::OnlyInStaging(staging_item));
        }
    }
    results.extend(journal.into_iter().map(ReconcileItem::OnlyInJournal));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_entries(source: &str) -> Vec<Entry<Decimal>> {
        beancount_parser::parse_iter(source)
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap()
    }

    fn build_directives(source: &str) -> Vec<Directive> {
        let mut all_directives = Vec::new();
        for entry in parse_entries(source) {
            if let Entry::Directive(directive) = entry {
                all_directives.push(directive);
            }
        }
        crate::sorting::sort_dedup_directives(&mut all_directives);
        all_directives
    }

    fn build_date_map<'a>(directives: &'a [Directive]) -> BTreeMap<Date, Vec<&'a Directive>> {
        let mut map: BTreeMap<Date, Vec<&'a Directive>> = BTreeMap::new();
        for directive in directives {
            map.entry(directive.date).or_default().push(directive);
        }
        map
    }

    fn count_results(results: &[ReconcileItem]) -> (usize, usize) {
        let journal_count = results
            .iter()
            .filter(|item| matches!(item, ReconcileItem::OnlyInJournal(_)))
            .count();
        let staging_count = results
            .iter()
            .filter(|item| matches!(item, ReconcileItem::OnlyInStaging(_)))
            .count();
        (journal_count, staging_count)
    }

    fn format_results(results: &[ReconcileItem]) -> String {
        let mut output = String::new();
        for item in results {
            match item {
                ReconcileItem::OnlyInJournal(directive) => {
                    output.push_str("; OnlyInJournal\n");
                    output.push_str(&directive.to_string());
                }
                ReconcileItem::OnlyInStaging(directive) => {
                    output.push_str("; OnlyInStaging\n");
                    output.push_str(&directive.to_string());
                }
            }
            output.push('\n');
        }
        output
    }

    // Core reconciliation logic tests

    #[test]
    fn reconcile_all_match() {
        let journal = r#"
2025-01-01 * "Payee1" "Transaction 1"
    Assets:Checking  -100.00 EUR
    Expenses:Food    100.00 EUR

2025-01-02 * "Payee2" "Transaction 2"
    Assets:Checking  -50.00 EUR
    Expenses:Transport  50.00 EUR

2025-01-03 * "Payee3" "Transaction 3"
    Assets:Checking  -75.00 EUR
    Expenses:Shopping  75.00 EUR
"#;
        let staging = r#"
2025-01-01 * "Payee1" "Transaction 1"
    Assets:Checking  -100.00 EUR

2025-01-02 * "Payee2" "Transaction 2"
    Assets:Checking  -50.00 EUR

2025-01-03 * "Payee3" "Transaction 3"
    Assets:Checking  -75.00 EUR
"#;
        let journal_directives = build_directives(journal);
        let staging_directives = build_directives(staging);
        let journal_map = build_date_map(&journal_directives);
        let staging_map = build_date_map(&staging_directives);
        let results = reconcile(journal_map, staging_map);

        assert_eq!(count_results(&results), (0, 0));
        assert!(results.is_empty());
    }

    #[test]
    fn reconcile_all_only_journal() {
        let journal = r#"
2025-01-01 * "Payee1" "Transaction 1"
    Assets:Checking  -100.00 EUR
    Expenses:Food    100.00 EUR

2025-01-02 * "Payee2" "Transaction 2"
    Assets:Checking  -50.00 EUR
    Expenses:Transport  50.00 EUR

2025-01-03 * "Payee3" "Transaction 3"
    Assets:Checking  -75.00 EUR
    Expenses:Shopping  75.00 EUR
"#;
        let journal_directives = build_directives(journal);
        let journal_map = build_date_map(&journal_directives);
        let staging_map = BTreeMap::new();
        let results = reconcile(journal_map, staging_map);

        assert_eq!(count_results(&results), (3, 0));
        insta::assert_snapshot!(format_results(&results), @r#"
        ; OnlyInJournal
        2025-01-01 * "Payee1" "Transaction 1"
          Assets:Checking	-100.00 EUR
          Expenses:Food	100.00 EUR
        ; OnlyInJournal
        2025-01-02 * "Payee2" "Transaction 2"
          Assets:Checking	-50.00 EUR
          Expenses:Transport	50.00 EUR
        ; OnlyInJournal
        2025-01-03 * "Payee3" "Transaction 3"
          Assets:Checking	-75.00 EUR
          Expenses:Shopping	75.00 EUR
        "#);
    }

    #[test]
    fn reconcile_all_only_staging() {
        let staging = r#"
2025-01-01 * "Payee1" "Transaction 1"
    Assets:Checking  -100.00 EUR

2025-01-02 * "Payee2" "Transaction 2"
    Assets:Checking  -50.00 EUR

2025-01-03 * "Payee3" "Transaction 3"
    Assets:Checking  -75.00 EUR
"#;
        let staging_directives = build_directives(staging);
        let journal_map = BTreeMap::new();
        let staging_map = build_date_map(&staging_directives);
        let results = reconcile(journal_map, staging_map);

        assert_eq!(count_results(&results), (0, 3));
        insta::assert_snapshot!(format_results(&results), @r#"
        ; OnlyInStaging
        2025-01-01 * "Payee1" "Transaction 1"
          Assets:Checking	-100.00 EUR
        ; OnlyInStaging
        2025-01-02 * "Payee2" "Transaction 2"
          Assets:Checking	-50.00 EUR
        ; OnlyInStaging
        2025-01-03 * "Payee3" "Transaction 3"
          Assets:Checking	-75.00 EUR
        "#);
    }

    #[test]
    fn reconcile_mixed_scenario() {
        let journal = r#"
2025-01-01 * "Payee1" "Transaction A"
    Assets:Checking  -100.00 EUR
    Expenses:Food    100.00 EUR

2025-01-02 * "Payee2" "Transaction B"
    Assets:Checking  -50.00 EUR
    Expenses:Transport  50.00 EUR
"#;
        let staging = r#"
2025-01-01 * "Payee1" "Transaction A"
    Assets:Checking  -100.00 EUR

2025-01-03 * "Payee3" "Transaction C"
    Assets:Checking  -75.00 EUR
"#;
        let journal_directives = build_directives(journal);
        let staging_directives = build_directives(staging);
        let journal_map = build_date_map(&journal_directives);
        let staging_map = build_date_map(&staging_directives);
        let results = reconcile(journal_map, staging_map);

        assert_eq!(count_results(&results), (1, 1));
        insta::assert_snapshot!(format_results(&results), @r#"
        ; OnlyInJournal
        2025-01-02 * "Payee2" "Transaction B"
          Assets:Checking	-50.00 EUR
          Expenses:Transport	50.00 EUR
        ; OnlyInStaging
        2025-01-03 * "Payee3" "Transaction C"
          Assets:Checking	-75.00 EUR
        "#);
    }

    #[test]
    fn reconcile_partial_match_same_date() {
        let journal = r#"
2025-01-01 * "Payee1" "Transaction 1"
    Assets:Checking  -100.00 EUR
    Expenses:Food    100.00 EUR

2025-01-01 * "Payee2" "Transaction 2"
    Assets:Checking  -50.00 EUR
    Expenses:Transport  50.00 EUR

2025-01-01 * "Payee3" "Transaction 3"
    Assets:Checking  -75.00 EUR
    Expenses:Shopping  75.00 EUR
"#;
        let staging = r#"
2025-01-01 * "Payee1" "Transaction 1"
    Assets:Checking  -100.00 EUR

2025-01-01 * "Payee2" "Transaction 2"
    Assets:Checking  -50.00 EUR
"#;
        let journal_directives = build_directives(journal);
        let staging_directives = build_directives(staging);
        let journal_map = build_date_map(&journal_directives);
        let staging_map = build_date_map(&staging_directives);
        let results = reconcile(journal_map, staging_map);

        assert_eq!(count_results(&results), (1, 0));
        insta::assert_snapshot!(format_results(&results), @r#"
        ; OnlyInJournal
        2025-01-01 * "Payee3" "Transaction 3"
          Assets:Checking	-75.00 EUR
          Expenses:Shopping	75.00 EUR
        "#);
    }

    // Date bucket handling tests

    #[test]
    fn reconcile_date_only_in_journal() {
        let journal = r#"
2025-01-01 * "Payee1" "Transaction on Jan 1"
    Assets:Checking  -100.00 EUR
    Expenses:Food    100.00 EUR
"#;
        let staging = r#"
2025-01-02 * "Payee2" "Transaction on Jan 2"
    Assets:Checking  -50.00 EUR
"#;
        let journal_directives = build_directives(journal);
        let staging_directives = build_directives(staging);
        let journal_map = build_date_map(&journal_directives);
        let staging_map = build_date_map(&staging_directives);
        let results = reconcile(journal_map, staging_map);

        assert_eq!(count_results(&results), (1, 1));
        insta::assert_snapshot!(format_results(&results), @r#"
        ; OnlyInJournal
        2025-01-01 * "Payee1" "Transaction on Jan 1"
          Assets:Checking	-100.00 EUR
          Expenses:Food	100.00 EUR
        ; OnlyInStaging
        2025-01-02 * "Payee2" "Transaction on Jan 2"
          Assets:Checking	-50.00 EUR
        "#);
    }

    #[test]
    fn reconcile_multiple_same_date_all_match() {
        let journal = r#"
2025-01-01 * "Payee1" "Transaction 1"
    Assets:Checking  -100.00 EUR
    Expenses:Food    100.00 EUR

2025-01-01 * "Payee2" "Transaction 2"
    Assets:Checking  -50.00 EUR
    Expenses:Transport  50.00 EUR

2025-01-01 * "Payee3" "Transaction 3"
    Assets:Checking  -75.00 EUR
    Expenses:Shopping  75.00 EUR
"#;
        let staging = r#"
2025-01-01 * "Payee1" "Transaction 1"
    Assets:Checking  -100.00 EUR

2025-01-01 * "Payee2" "Transaction 2"
    Assets:Checking  -50.00 EUR

2025-01-01 * "Payee3" "Transaction 3"
    Assets:Checking  -75.00 EUR
"#;
        let journal_directives = build_directives(journal);
        let staging_directives = build_directives(staging);
        let journal_map = build_date_map(&journal_directives);
        let staging_map = build_date_map(&staging_directives);
        let results = reconcile(journal_map, staging_map);

        assert_eq!(count_results(&results), (0, 0));
        assert!(results.is_empty());
    }

    #[test]
    fn reconcile_multiple_same_date_none_match() {
        let journal = r#"
2025-01-01 * "Payee1" "Transaction 1"
    Assets:Checking  -100.00 EUR
    Expenses:Food    100.00 EUR

2025-01-01 * "Payee2" "Transaction 2"
    Assets:Checking  -50.00 EUR
    Expenses:Transport  50.00 EUR

2025-01-01 * "Payee3" "Transaction 3"
    Assets:Checking  -75.00 EUR
    Expenses:Shopping  75.00 EUR
"#;
        let staging = r#"
2025-01-01 * "PayeeA" "Transaction A"
    Assets:Savings  -200.00 EUR

2025-01-01 * "PayeeB" "Transaction B"
    Assets:Savings  -150.00 EUR

2025-01-01 * "PayeeC" "Transaction C"
    Assets:Savings  -125.00 EUR
"#;
        let journal_directives = build_directives(journal);
        let staging_directives = build_directives(staging);
        let journal_map = build_date_map(&journal_directives);
        let staging_map = build_date_map(&staging_directives);
        let results = reconcile(journal_map, staging_map);

        assert_eq!(count_results(&results), (3, 3));
        insta::assert_snapshot!(format_results(&results), @r#"
        ; OnlyInStaging
        2025-01-01 * "PayeeC" "Transaction C"
          Assets:Savings	-125.00 EUR
        ; OnlyInStaging
        2025-01-01 * "PayeeB" "Transaction B"
          Assets:Savings	-150.00 EUR
        ; OnlyInStaging
        2025-01-01 * "PayeeA" "Transaction A"
          Assets:Savings	-200.00 EUR
        ; OnlyInJournal
        2025-01-01 * "Payee1" "Transaction 1"
          Assets:Checking	-100.00 EUR
          Expenses:Food	100.00 EUR
        ; OnlyInJournal
        2025-01-01 * "Payee2" "Transaction 2"
          Assets:Checking	-50.00 EUR
          Expenses:Transport	50.00 EUR
        ; OnlyInJournal
        2025-01-01 * "Payee3" "Transaction 3"
          Assets:Checking	-75.00 EUR
          Expenses:Shopping	75.00 EUR
        "#);
    }

    // Bucket-level matching tests

    #[test]
    fn reconcile_bucket_staging_exceeds_journal() {
        let journal = r#"
2025-01-01 * "Payee1" "Transaction 1"
    Assets:Checking  -100.00 EUR
    Expenses:Food    100.00 EUR
"#;
        let staging = r#"
2025-01-01 * "Payee1" "Transaction 1"
    Assets:Checking  -100.00 EUR

2025-01-01 * "Payee2" "Transaction 2"
    Assets:Checking  -50.00 EUR

2025-01-01 * "Payee3" "Transaction 3"
    Assets:Checking  -75.00 EUR
"#;
        let journal_directives = build_directives(journal);
        let staging_directives = build_directives(staging);
        let journal_map = build_date_map(&journal_directives);
        let staging_map = build_date_map(&staging_directives);
        let results = reconcile(journal_map, staging_map);

        assert_eq!(count_results(&results), (0, 2));
        insta::assert_snapshot!(format_results(&results), @r#"
        ; OnlyInStaging
        2025-01-01 * "Payee3" "Transaction 3"
          Assets:Checking	-75.00 EUR
        ; OnlyInStaging
        2025-01-01 * "Payee2" "Transaction 2"
          Assets:Checking	-50.00 EUR
        "#);
    }

    #[test]
    fn reconcile_bucket_journal_exceeds_staging() {
        let journal = r#"
2025-01-01 * "Payee1" "Transaction 1"
    Assets:Checking  -100.00 EUR
    Expenses:Food    100.00 EUR

2025-01-01 * "Payee2" "Transaction 2"
    Assets:Checking  -50.00 EUR
    Expenses:Transport  50.00 EUR

2025-01-01 * "Payee3" "Transaction 3"
    Assets:Checking  -75.00 EUR
    Expenses:Shopping  75.00 EUR
"#;
        let staging = r#"
2025-01-01 * "Payee1" "Transaction 1"
    Assets:Checking  -100.00 EUR
"#;
        let journal_directives = build_directives(journal);
        let staging_directives = build_directives(staging);
        let journal_map = build_date_map(&journal_directives);
        let staging_map = build_date_map(&staging_directives);
        let results = reconcile(journal_map, staging_map);

        assert_eq!(count_results(&results), (2, 0));
        insta::assert_snapshot!(format_results(&results), @r#"
        ; OnlyInJournal
        2025-01-01 * "Payee2" "Transaction 2"
          Assets:Checking	-50.00 EUR
          Expenses:Transport	50.00 EUR
        ; OnlyInJournal
        2025-01-01 * "Payee3" "Transaction 3"
          Assets:Checking	-75.00 EUR
          Expenses:Shopping	75.00 EUR
        "#);
    }

    // Edge case tests

    #[test]
    fn reconcile_empty_both() {
        let journal_map = BTreeMap::new();
        let staging_map = BTreeMap::new();
        let results = reconcile(journal_map, staging_map);

        assert_eq!(count_results(&results), (0, 0));
        assert!(results.is_empty());
    }

    #[test]
    fn reconcile_balance_directives() {
        let journal = r#"
2025-01-01 balance Assets:Checking  1000.00 EUR
"#;
        let staging = r#"
2025-01-01 balance Assets:Checking  1500.00 EUR
"#;
        let journal_directives = build_directives(journal);
        let staging_directives = build_directives(staging);
        let journal_map = build_date_map(&journal_directives);
        let staging_map = build_date_map(&staging_directives);
        let results = reconcile(journal_map, staging_map);

        assert_eq!(count_results(&results), (1, 1));
        insta::assert_snapshot!(format_results(&results), @"
        ; OnlyInStaging
        2025-01-01 balance Assets:Checking 1500.00 EUR
        ; OnlyInJournal
        2025-01-01 balance Assets:Checking 1000.00 EUR
        ");
    }

    #[test]
    fn reconcile_four_identical_transactions_one_committed() {
        // Scenario: User has 4 identical transactions on the same day in staging.
        // They commit the first one to the journal. Now journal has 1 and staging has 4.
        // The reconciler should match the 1 journal transaction with 1 of the 4 staging
        // transactions, leaving 3 staging transactions unmatched.
        let journal = r#"
2025-01-01 * "Coffee Shop" "Morning coffee"
    Assets:Checking  -5.00 EUR
    Expenses:Food    5.00 EUR
"#;
        let staging = r#"
2025-01-01 ! "Coffee Shop" "Morning coffee"
    Assets:Checking  -5.00 EUR

2025-01-01 ! "Coffee Shop" "Morning coffee"
    Assets:Checking  -5.00 EUR

2025-01-01 ! "Coffee Shop" "Morning coffee"
    Assets:Checking  -5.00 EUR

2025-01-01 ! "Coffee Shop" "Morning coffee"
    Assets:Checking  -5.00 EUR
"#;
        let journal_directives = build_directives(journal);
        let staging_directives = build_directives(staging);
        let journal_map = build_date_map(&journal_directives);
        let staging_map = build_date_map(&staging_directives);
        let results = reconcile(journal_map, staging_map);

        // Expected: 0 in journal only, 3 in staging only
        // (1 staging matched with 1 journal, leaving 3 staging unmatched)
        assert_eq!(count_results(&results), (0, 3));
    }
}
