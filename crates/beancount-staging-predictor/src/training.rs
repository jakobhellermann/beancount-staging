use crate::{Directive, TrainingExample};
use beancount_parser::DirectiveContent;

pub fn extract_training_examples(directives: &[Directive]) -> Vec<TrainingExample> {
    let mut examples = Vec::new();

    for directive in directives {
        let DirectiveContent::Transaction(txn) = &directive.content else {
            continue;
        };

        if txn.postings.len() != 2 {
            continue;
        }

        let source = &txn.postings[0];
        let target = &txn.postings[1];

        if is_placeholder_account(&source.account) || is_placeholder_account(&target.account) {
            continue;
        }

        examples.push(TrainingExample {
            source_account: source.account.clone(),
            payee: txn.payee.clone(),
            narration: txn.narration.clone().unwrap_or_default(),
            target_account: target.account.clone(),
        });
    }

    examples
}

fn is_placeholder_account(account: &beancount_parser::Account) -> bool {
    let account_str = account.to_string().to_lowercase();
    account_str.contains("fixme") || account_str.contains("todo") || account_str.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_from_simple_transaction() {
        let source = r#"
2024-01-05 * "SuperMart" "Weekly groceries"
    Assets:Checking  -156.78 EUR
    Expenses:Groceries  156.78 EUR
"#;
        let directives = parse_test_source(source);
        let examples = extract_training_examples(&directives);

        assert_eq!(examples.len(), 1);
        assert_eq!(examples[0].source_account.to_string(), "Assets:Checking");
        assert_eq!(examples[0].target_account.to_string(), "Expenses:Groceries");
        assert_eq!(examples[0].payee.as_deref(), Some("SuperMart"));
        assert_eq!(examples[0].narration, "Weekly groceries");
    }

    #[test]
    fn test_skip_placeholder_accounts() {
        let source = r#"
2024-01-05 * "Unknown" "Transaction"
    Assets:Checking  -50.00 EUR
    Expenses:FIXME  50.00 EUR
"#;
        let directives = parse_test_source(source);
        let examples = extract_training_examples(&directives);

        assert_eq!(
            examples.len(),
            0,
            "Should skip transactions with placeholder accounts"
        );
    }

    #[test]
    fn test_skip_multi_posting_transactions() {
        let source = r#"
2024-01-05 * "Split transaction" "Multiple postings"
    Assets:Checking  -100.00 EUR
    Expenses:Food  60.00 EUR
    Expenses:Transport  40.00 EUR
"#;
        let directives = parse_test_source(source);
        let examples = extract_training_examples(&directives);

        assert_eq!(
            examples.len(),
            0,
            "Should skip transactions with more than 2 postings"
        );
    }

    fn parse_test_source(source: &str) -> Vec<Directive> {
        let parsed = beancount_parser::parse::<crate::Decimal>(source).unwrap();
        parsed.directives
    }
}
