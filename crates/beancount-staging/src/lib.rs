pub mod reconcile;
mod sorting;
mod utils;

pub type Directive = beancount_parser::Directive<Decimal>;
pub type Entry = beancount_parser::Entry<Decimal>;
pub type DirectiveContent = beancount_parser::DirectiveContent<Decimal>;
pub type Transaction = beancount_parser::Transaction<Decimal>;
pub type Decimal = rust_decimal::Decimal;

pub use anyhow::Result;

use std::path::Path;

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
/// - Adding a balancing posting with the expense account (amount is inferred by beancount)
pub fn commit_transaction(
    directive: &Directive,
    expense_account: &str,
    journal_path: &Path,
) -> Result<()> {
    use anyhow::Context;
    use std::fs::OpenOptions;
    use std::io::Write;

    let mut directive = directive.clone();

    if let DirectiveContent::Transaction(ref mut txn) = directive.content {
        // Change flag from ! to *
        txn.flag = Some('*');

        // Add balancing posting with expense account (no amount - beancount infers it)
        let account: beancount_parser::Account = expense_account
            .parse()
            .with_context(|| format!("Failed to parse account name: '{}'", expense_account))?;
        txn.postings.push(beancount_parser::Posting::new(account));
    }

    // Open journal file in append mode
    let mut file = OpenOptions::new().append(true).open(journal_path)?;

    writeln!(file, "\n{}", directive)?;

    Ok(())
}
