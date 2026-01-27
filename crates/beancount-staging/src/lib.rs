pub mod reconcile;
mod sorting;
mod utils;

pub type Directive = beancount_parser::Directive<Decimal>;
pub type Entry = beancount_parser::Entry<Decimal>;
pub type DirectiveContent = beancount_parser::DirectiveContent<Decimal>;
pub type Transaction = beancount_parser::Transaction<Decimal>;
pub type Decimal = rust_decimal::Decimal;

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
/// - Adding a balancing posting with the expense account (amount is inferred by beancount)
pub fn commit_transaction(
    directive: &Directive,
    expense_account: &str,
    payee: Option<&str>,
    narration: Option<&str>,
    journal_path: &Path,
) -> Result<()> {
    use anyhow::Context;
    use std::fs::OpenOptions;
    use std::io::Write;

    let original = directive;
    let mut directive = original.clone();

    if let DirectiveContent::Transaction(ref mut txn) = directive.content {
        // Change flag from ! to *
        txn.flag = Some('*');

        let meta = &mut txn
            .postings
            .first_mut()
            .expect("TODO: no first account")
            .metadata;

        // Update payee if provided, saving original as metadata on first posting
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

        // Update narration if provided, saving original as metadata on first posting
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

        // Add balancing posting with expense account (no amount - beancount infers it)
        let account: beancount_parser::Account = expense_account
            .parse()
            .with_context(|| format!("Failed to parse account name: '{}'", expense_account))?;
        txn.postings.push(beancount_parser::Posting::new(account));
    }

    let does_match = reconcile::matching::journal_matches_staging(&directive, original);
    assert!(
        does_match,
        "Internal error: commited transaction does not match original"
    );

    // Open journal file in append mode
    let mut file = BufWriter::new(OpenOptions::new().append(true).open(journal_path)?);

    writeln!(file, "\n{}", directive)?;

    Ok(())
}
