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

/// Read all directives from the given source.
pub fn read_directives(file: impl AsRef<Path>) -> Result<Vec<Directive>> {
    let mut directives = Vec::new();
    for entry in
        beancount_parser::read_files_iter::<Decimal>(std::iter::once(file.as_ref().to_owned()))
    {
        if let Entry::Directive(directive) = entry? {
            directives.push(directive);
        }
    }

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

        // Add balancing posting with expense account if provided (no amount - beancount infers it)
        if let Some(expense_account) = expense_account {
            let account: beancount_parser::Account = expense_account
                .parse()
                .with_context(|| format!("Failed to parse account name: '{}'", expense_account))?;
            txn.postings.push(beancount_parser::Posting::new(account));
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
          Assets:Checking	-50.00 USD
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
          Assets:Checking	-50.00 USD
          Assets:Savings	50.00 USD
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
          Assets:Checking	-50.00 USD
          Expenses:Food
          source_payee: "Original Payee"
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
          Assets:Checking	-50.00 USD
          Expenses:Food
          source_desc: "Original Narration"
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
          Assets:Checking	-50.00 USD
          Expenses:Food
          source_payee: "Original Payee"
          source_desc: "Original Narration"
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
          Assets:Checking	-50.00 USD
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
          Assets:Checking	-50.00 USD
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
}
