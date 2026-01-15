//! Reconciling differences between existing journal entries and a full automatic import.

mod matching;

use crate::Result;
use crate::utils::sort_merge_diff::{JoinResult, SortMergeDiff};
use crate::{Decimal, Directive};
use beancount_parser::{Date, Entry};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug)]
pub enum ReconcileItem {
    OnlyInJournal(Directive),
    OnlyInStaging(Directive),
}

pub struct ReconcileConfig<'a> {
    journal_paths: &'a [&'a str],
    staging_paths: &'a [&'a str],
}
impl<'a> ReconcileConfig<'a> {
    pub fn new(journal_paths: &'a [&'a str], staging_paths: &'a [&'a str]) -> Self {
        ReconcileConfig {
            journal_paths,
            staging_paths,
        }
    }
    /// Try to associate all journal and staging items, returning a list of differences.
    pub fn reconcile(&self) -> Result<Vec<ReconcileItem>> {
        let journal = read_directives_by_date(self.journal_paths)?;
        let staging = read_directives_by_date(self.staging_paths)?;

        let results = reconcile(journal, staging);
        Ok(results)
    }
}

fn read_directives_by_date(path: &[&str]) -> Result<BTreeMap<Date, Vec<Directive>>> {
    let mut directives: BTreeMap<_, Vec<_>> = BTreeMap::new();
    let files = path.iter().map(PathBuf::from);
    for entry in beancount_parser::read_files_iter::<Decimal>(files) {
        if let Entry::Directive(directive) = entry? {
            directives
                .entry(directive.date)
                .or_default()
                .push(directive);
        }
    }
    for bucket in directives.values_mut() {
        crate::sorting::sort_dedup_directives(bucket);
    }

    Ok(directives)
}

fn reconcile(
    journal: BTreeMap<Date, Vec<Directive>>,
    staging: BTreeMap<Date, Vec<Directive>>,
) -> Vec<ReconcileItem> {
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
fn reconcile_bucket(
    results: &mut Vec<ReconcileItem>,
    mut journal: Vec<Directive>,
    mut staging: Vec<Directive>,
) {
    while let Some(staging_item) = staging.pop() {
        let match_at = journal.iter().position(|journal_item| {
            matching::journal_matches_staging(journal_item, &staging_item)
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

    fn build_date_map(source: &str) -> BTreeMap<Date, Vec<Directive>> {
        let mut map: BTreeMap<Date, Vec<Directive>> = BTreeMap::new();
        for entry in parse_entries(source) {
            if let Entry::Directive(directive) = entry {
                map.entry(directive.date).or_default().push(directive);
            }
        }
        for bucket in map.values_mut() {
            crate::sorting::sort_dedup_directives(bucket);
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
        let journal_map = build_date_map(journal);
        let staging_map = build_date_map(staging);
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
        let journal_map = build_date_map(journal);
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
        let journal_map = BTreeMap::new();
        let staging_map = build_date_map(staging);
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
        let journal_map = build_date_map(journal);
        let staging_map = build_date_map(staging);
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
        let journal_map = build_date_map(journal);
        let staging_map = build_date_map(staging);
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
        let journal_map = build_date_map(journal);
        let staging_map = build_date_map(staging);
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
        let journal_map = build_date_map(journal);
        let staging_map = build_date_map(staging);
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
        let journal_map = build_date_map(journal);
        let staging_map = build_date_map(staging);
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
        let journal_map = build_date_map(journal);
        let staging_map = build_date_map(staging);
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
        let journal_map = build_date_map(journal);
        let staging_map = build_date_map(staging);
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
        let journal_map = build_date_map(journal);
        let staging_map = build_date_map(staging);
        let results = reconcile(journal_map, staging_map);

        assert_eq!(count_results(&results), (1, 1));
        insta::assert_snapshot!(format_results(&results), @"
        ; OnlyInStaging
        2025-01-01 balance Assets:Checking 1500.00 EUR
        ; OnlyInJournal
        2025-01-01 balance Assets:Checking 1000.00 EUR
        ");
    }
}
