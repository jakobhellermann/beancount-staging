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
