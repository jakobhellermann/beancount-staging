pub mod reconcile;
mod sorting;
mod utils;

pub type Directive = beancount_parser::Directive<Decimal>;
pub type Entry = beancount_parser::Entry<Decimal>;
pub type DirectiveContent = beancount_parser::DirectiveContent<Decimal>;
pub type Transaction = beancount_parser::Transaction<Decimal>;
pub type Decimal = rust_decimal::Decimal;

/// Specifies where to store source metadata (source_desc, source_payee)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceMetaTarget {
    /// Store in transaction-level metadata (directive.metadata)
    Transaction,
    /// Store in first posting's metadata (for backward compatibility)
    Posting,
}

pub use anyhow::Result;
use beancount_parser::metadata::Value;

use std::{io::BufWriter, path::Path};

/// A rule for auto-categorizing staging transactions.
///
/// When a staging transaction's payee matches the `payee` regex (as a
/// substring, not anchored) and its first posting's account equals
/// `source_account`, the transaction is committed to the journal with
/// `target_account` as the balancing posting — without UI review.
#[derive(Debug, Clone)]
pub struct AutoCategorizeRule {
    pub payee: regex::Regex,
    pub source_account: String,
    pub target_account: String,
}

impl AutoCategorizeRule {
    pub fn matches(&self, directive: &Directive) -> bool {
        let DirectiveContent::Transaction(txn) = &directive.content else {
            return false;
        };
        let Some(payee) = &txn.payee else {
            return false;
        };
        if !self.payee.is_match(payee) {
            return false;
        }
        let Some(first_posting) = txn.postings.first() else {
            return false;
        };
        first_posting.account.to_string() == self.source_account
    }
}

/// Find the first rule that matches the given directive, if any.
pub fn find_matching_rule<'a>(
    directive: &Directive,
    rules: &'a [AutoCategorizeRule],
) -> Option<&'a AutoCategorizeRule> {
    rules.iter().find(|rule| rule.matches(directive))
}

/// Read all directives from the given source.
pub fn read_directives(file: impl AsRef<Path>) -> Result<Vec<Directive>> {
    let mut directives = Vec::new();

    beancount_parser::read_files_v2::<Decimal, _>(
        std::iter::once(file.as_ref().to_owned()),
        |entry| {
            if let Entry::Directive(directive) = entry {
                directives.push(directive);
            }
        },
    )?;

    sorting::sort_dedup_directives(&mut directives);
    Ok(directives)
}

/// Commit a transaction to the journal file with the specified expense account.
///
/// This modifies the transaction by:
/// - Changing the flag from `!` to `*`
/// - Optionally updating payee and narration if provided
/// - Adding a balancing posting with the expense account if provided (amount is inferred by beancount)
pub fn commit_transaction(
    directive: &Directive,
    expense_account: Option<&str>,
    payee: Option<&str>,
    narration: Option<&str>,
    source_meta_target: SourceMetaTarget,
    journal_path: &Path,
) -> Result<()> {
    use std::fs::OpenOptions;

    // Open journal file in append mode
    let file = BufWriter::new(OpenOptions::new().append(true).open(journal_path)?);

    commit_transaction_to_writer(
        directive,
        expense_account,
        payee,
        narration,
        source_meta_target,
        file,
    )
}

/// Internal function that commits to a writer. Used by both the public API and tests.
fn commit_transaction_to_writer(
    directive: &Directive,
    expense_account: Option<&str>,
    payee: Option<&str>,
    narration: Option<&str>,
    source_meta_target: SourceMetaTarget,
    mut writer: impl std::io::Write,
) -> Result<()> {
    use anyhow::Context;

    let original = directive;
    let mut directive = original.clone();

    if let DirectiveContent::Transaction(ref mut txn) = directive.content {
        // Change flag from ! to *
        txn.flag = Some('*');

        // Select metadata target based on configuration
        let meta = match source_meta_target {
            SourceMetaTarget::Transaction => &mut directive.metadata,
            SourceMetaTarget::Posting => {
                &mut txn
                    .postings
                    .first_mut()
                    .expect("TODO: no first account")
                    .metadata
            }
        };

        // Update payee if provided, saving original as metadata
        if let Some(new_payee) = payee {
            if let Some(original_payee) = &txn.payee
                && original_payee != new_payee
            {
                meta.insert(
                    "source_payee".parse().unwrap(),
                    Value::String(original_payee.clone()),
                );
            }
            txn.payee = Some(new_payee.to_string());
        }

        // Update narration if provided, saving original as metadata
        if let Some(new_narration) = narration {
            if let Some(original_narration) = &txn.narration
                && original_narration != new_narration
            {
                meta.insert(
                    "source_desc".parse().unwrap(),
                    Value::String(original_narration.clone()),
                );
            }
            txn.narration = Some(new_narration.to_string());
        }

        // Add or update balancing posting with expense account if provided (no amount - beancount infers it)
        if let Some(expense_account) = expense_account {
            let account: beancount_parser::Account = expense_account
                .parse()
                .with_context(|| format!("Failed to parse account name: '{}'", expense_account))?;

            // If there's already an unbalanced posting (no amount), update its account
            // Otherwise, add a new balancing posting
            if let Some(unbalanced_posting) = txn.postings.iter_mut().find(|p| p.amount.is_none()) {
                unbalanced_posting.account = account;
            } else {
                txn.postings
                    .push(beancount_parser::Posting::from_account(account));
            }
        }
    }

    let does_match = reconcile::matching::journal_matches_staging(&directive, original);
    assert!(
        does_match.is_ok(),
        "Internal error: commited transaction does not match original"
    );

    writeln!(writer, "\n{}", directive)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_directive(content: &str) -> Directive {
        let mut directives = Vec::new();
        for entry in beancount_parser::parse_iter::<Decimal>(content) {
            if let Entry::Directive(directive) = entry.unwrap() {
                directives.push(directive);
            }
        }
        assert_eq!(directives.len(), 1, "Expected exactly one directive");
        directives.into_iter().next().unwrap()
    }

    fn create_test_transaction(flag: char, payee: &str, narration: &str) -> Directive {
        let content = format!(
            r#"2024-01-15 {} "{}" "{}"
    Assets:Checking  -50.00 USD
"#,
            flag, payee, narration
        );
        parse_directive(&content)
    }

    fn create_balanced_transaction(flag: char, payee: &str, narration: &str) -> Directive {
        let content = format!(
            r#"2024-01-15 {} "{}" "{}"
    Assets:Checking  -50.00 USD
    Assets:Savings    50.00 USD
"#,
            flag, payee, narration
        );
        parse_directive(&content)
    }

    #[test]
    fn test_commit_transaction_basic() {
        let directive = create_test_transaction('!', "Test Payee", "Test Narration");
        let mut output = Vec::new();

        commit_transaction_to_writer(
            &directive,
            Some("Expenses:Groceries"),
            None,
            None,
            SourceMetaTarget::Transaction,
            &mut output,
        )
        .unwrap();

        let content = String::from_utf8(output).unwrap();
        insta::assert_snapshot!(content, @r#"

        2024-01-15 * "Test Payee" "Test Narration"
          Assets:Checking -50.00 USD
          Expenses:Groceries
        "#);
    }

    #[test]
    fn test_commit_transaction_balanced() {
        let directive = create_balanced_transaction('!', "Transfer", "Internal transfer");
        let mut output = Vec::new();

        commit_transaction_to_writer(
            &directive,
            None,
            None,
            None,
            SourceMetaTarget::Transaction,
            &mut output,
        )
        .unwrap();

        let content = String::from_utf8(output).unwrap();
        insta::assert_snapshot!(content, @r#"

        2024-01-15 * "Transfer" "Internal transfer"
          Assets:Checking -50.00 USD
          Assets:Savings 50.00 USD
        "#);
    }

    #[test]
    fn test_commit_transaction_with_payee_override() {
        let directive = create_test_transaction('!', "Original Payee", "Test Narration");
        let mut output = Vec::new();

        commit_transaction_to_writer(
            &directive,
            Some("Expenses:Food"),
            Some("New Payee"),
            None,
            SourceMetaTarget::Transaction,
            &mut output,
        )
        .unwrap();

        let content = String::from_utf8(output).unwrap();
        insta::assert_snapshot!(content, @r#"

        2024-01-15 * "New Payee" "Test Narration"
          source_payee: "Original Payee"
          Assets:Checking -50.00 USD
          Expenses:Food
        "#);
    }

    #[test]
    fn test_commit_transaction_with_narration_override() {
        let directive = create_test_transaction('!', "Test Payee", "Original Narration");
        let mut output = Vec::new();

        commit_transaction_to_writer(
            &directive,
            Some("Expenses:Food"),
            None,
            Some("New Narration"),
            SourceMetaTarget::Transaction,
            &mut output,
        )
        .unwrap();

        let content = String::from_utf8(output).unwrap();
        insta::assert_snapshot!(content, @r#"

        2024-01-15 * "Test Payee" "New Narration"
          source_desc: "Original Narration"
          Assets:Checking -50.00 USD
          Expenses:Food
        "#);
    }

    #[test]
    fn test_commit_transaction_with_both_overrides() {
        let directive = create_test_transaction('!', "Original Payee", "Original Narration");
        let mut output = Vec::new();

        commit_transaction_to_writer(
            &directive,
            Some("Expenses:Food"),
            Some("New Payee"),
            Some("New Narration"),
            SourceMetaTarget::Transaction,
            &mut output,
        )
        .unwrap();

        let content = String::from_utf8(output).unwrap();
        insta::assert_snapshot!(content, @r#"

        2024-01-15 * "New Payee" "New Narration"
          source_payee: "Original Payee"
          source_desc: "Original Narration"
          Assets:Checking -50.00 USD
          Expenses:Food
        "#);
    }

    #[test]
    fn test_commit_transaction_no_override_no_metadata() {
        let directive = create_test_transaction('!', "Test Payee", "Test Narration");
        let mut output = Vec::new();

        commit_transaction_to_writer(
            &directive,
            Some("Expenses:Food"),
            None,
            None,
            SourceMetaTarget::Transaction,
            &mut output,
        )
        .unwrap();

        let content = String::from_utf8(output).unwrap();
        insta::assert_snapshot!(content, @r#"

        2024-01-15 * "Test Payee" "Test Narration"
          Assets:Checking -50.00 USD
          Expenses:Food
        "#);
    }

    #[test]
    fn test_commit_transaction_same_payee_no_metadata() {
        let directive = create_test_transaction('!', "Same Payee", "Test Narration");
        let mut output = Vec::new();

        commit_transaction_to_writer(
            &directive,
            Some("Expenses:Food"),
            Some("Same Payee"),
            None,
            SourceMetaTarget::Transaction,
            &mut output,
        )
        .unwrap();

        let content = String::from_utf8(output).unwrap();
        insta::assert_snapshot!(content, @r#"

        2024-01-15 * "Same Payee" "Test Narration"
          Assets:Checking -50.00 USD
          Expenses:Food
        "#);
    }

    #[test]
    fn test_commit_transaction_invalid_account() {
        let directive = create_test_transaction('!', "Test Payee", "Test Narration");
        let mut output = Vec::new();

        let result = commit_transaction_to_writer(
            &directive,
            Some("Invalid Account Name!"),
            None,
            None,
            SourceMetaTarget::Transaction,
            &mut output,
        );

        assert!(result.is_err());
        insta::assert_snapshot!(result.unwrap_err().to_string(), @"Failed to parse account name: 'Invalid Account Name!'");
    }

    /// Test for bug: transaction with existing unbalanced posting should not get duplicate posting
    /// When a staging transaction already has a second posting without an amount (e.g. Assets:ZeroSum:Transfers),
    /// committing with an expense_account should NOT add another posting.
    #[test]
    fn test_commit_transaction_with_existing_unbalanced_posting() {
        let content = r#"2024-01-15 ! "Lastschrift"
    Assets:ScalableCapital:Cash 500.00 EUR
    Assets:ZeroSum:Transfers
"#;
        let directive = parse_directive(content);
        let mut output = Vec::new();

        // User selects "Assets:ZeroSum:Transfers" in the UI (same account as the existing posting)
        commit_transaction_to_writer(
            &directive,
            Some("Assets:ZeroSum:Transfers"),
            None,
            None,
            SourceMetaTarget::Transaction,
            &mut output,
        )
        .unwrap();

        let content = String::from_utf8(output).unwrap();
        // Should NOT have duplicate Assets:ZeroSum:Transfers posting
        insta::assert_snapshot!(content, @r#"

        2024-01-15 * "Lastschrift"
          Assets:ScalableCapital:Cash 500.00 EUR
          Assets:ZeroSum:Transfers
        "#);
    }

    /// Test that when user changes the account of an existing unbalanced posting,
    /// the account is updated rather than adding a new posting.
    #[test]
    fn test_commit_transaction_updates_existing_unbalanced_posting() {
        let content = r#"2024-01-15 ! "Lastschrift"
    Assets:ScalableCapital:Cash 500.00 EUR
    Assets:ZeroSum:Transfers
"#;
        let directive = parse_directive(content);
        let mut output = Vec::new();

        // User changes the account to something different
        commit_transaction_to_writer(
            &directive,
            Some("Expenses:Groceries"),
            None,
            None,
            SourceMetaTarget::Transaction,
            &mut output,
        )
        .unwrap();

        let content = String::from_utf8(output).unwrap();
        // Should update the existing unbalanced posting, not add a new one
        insta::assert_snapshot!(content, @r#"

        2024-01-15 * "Lastschrift"
          Assets:ScalableCapital:Cash 500.00 EUR
          Expenses:Groceries
        "#);
    }

    #[test]
    fn test_commit_transaction_flag_always_changes() {
        let directive_exclaim = create_test_transaction('!', "Test", "Test");
        let directive_asterisk = create_test_transaction('*', "Test", "Test");
        let directive_txn = create_test_transaction('T', "Test", "Test");

        let mut output1 = Vec::new();
        let mut output2 = Vec::new();
        let mut output3 = Vec::new();

        commit_transaction_to_writer(
            &directive_exclaim,
            Some("Expenses:Food"),
            None,
            None,
            SourceMetaTarget::Transaction,
            &mut output1,
        )
        .unwrap();
        commit_transaction_to_writer(
            &directive_asterisk,
            Some("Expenses:Food"),
            None,
            None,
            SourceMetaTarget::Transaction,
            &mut output2,
        )
        .unwrap();
        commit_transaction_to_writer(
            &directive_txn,
            Some("Expenses:Food"),
            None,
            None,
            SourceMetaTarget::Transaction,
            &mut output3,
        )
        .unwrap();

        let content1 = String::from_utf8(output1).unwrap();
        let content2 = String::from_utf8(output2).unwrap();
        let content3 = String::from_utf8(output3).unwrap();

        // All should have * flag
        assert!(content1.contains("2024-01-15 *"));
        assert!(content2.contains("2024-01-15 *"));
        assert!(content3.contains("2024-01-15 *"));
    }

    fn make_rule(payee_pattern: &str, source: &str, target: &str) -> AutoCategorizeRule {
        AutoCategorizeRule {
            payee: regex::Regex::new(payee_pattern).unwrap(),
            source_account: source.to_string(),
            target_account: target.to_string(),
        }
    }

    #[test]
    fn auto_rule_contains_match() {
        let directive = parse_directive(
            r#"2024-01-15 ! "PayPal Europe S.a.r.l. et Cie S.C.A" "Spotify"
    Assets:BIBEssen:Checking  -12.99 EUR
    Assets:ZeroSum:Transfers
"#,
        );
        let rule = make_rule(
            "PayPal Europe",
            "Assets:BIBEssen:Checking",
            "Assets:ZeroSum:Transfers",
        );
        assert!(rule.matches(&directive));
    }

    #[test]
    fn auto_rule_anchored_match() {
        let directive = parse_directive(
            r#"2024-01-15 ! "PayPal Europe S.a.r.l. et Cie S.C.A" "Spotify"
    Assets:BIBEssen:Checking  -12.99 EUR
    Assets:ZeroSum:Transfers
"#,
        );
        let exact = make_rule(
            "^PayPal Europe S.a.r.l. et Cie S.C.A$",
            "Assets:BIBEssen:Checking",
            "X",
        );
        assert!(exact.matches(&directive));
        let too_short = make_rule("^PayPal$", "Assets:BIBEssen:Checking", "X");
        assert!(!too_short.matches(&directive));
    }

    #[test]
    fn auto_rule_wrong_source_account() {
        let directive = parse_directive(
            r#"2024-01-15 ! "PayPal" "X"
    Assets:OtherBank:Checking  -12.99 EUR
    Assets:ZeroSum:Transfers
"#,
        );
        let rule = make_rule(
            "PayPal",
            "Assets:BIBEssen:Checking",
            "Assets:ZeroSum:Transfers",
        );
        assert!(!rule.matches(&directive));
    }

    #[test]
    fn auto_rule_payee_missing() {
        let directive = parse_directive(
            r#"2024-01-15 ! "narration only"
    Assets:BIBEssen:Checking  -12.99 EUR
    Assets:ZeroSum:Transfers
"#,
        );
        let rule = make_rule(
            "PayPal",
            "Assets:BIBEssen:Checking",
            "Assets:ZeroSum:Transfers",
        );
        assert!(!rule.matches(&directive));
    }

    #[test]
    fn find_matching_rule_returns_first() {
        let directive = parse_directive(
            r#"2024-01-15 ! "PayPal Spotify" "x"
    Assets:BIBEssen:Checking  -12.99 EUR
"#,
        );
        let rules = vec![
            make_rule("Netflix", "Assets:BIBEssen:Checking", "X"),
            make_rule(
                "PayPal",
                "Assets:BIBEssen:Checking",
                "Assets:ZeroSum:Transfers",
            ),
            make_rule("PayPal Spotify", "Assets:BIBEssen:Checking", "Z"),
        ];
        let matched = find_matching_rule(&directive, &rules).unwrap();
        assert_eq!(matched.target_account, "Assets:ZeroSum:Transfers");
    }
}
