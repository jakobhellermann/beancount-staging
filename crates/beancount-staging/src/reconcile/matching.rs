use crate::{Directive, DirectiveContent, Transaction};

fn journal_matches_staging_transaction(journal: &Transaction, staging: &Transaction) -> bool {
    // flag can be anything
    // tags can be anything
    // links can be anything

    let postings_match = match (journal.postings.as_slice(), staging.postings.as_slice()) {
        ([j0, ..], [s]) => {
            // flag can be anything
            // todo: metadata
            s.account == j0.account
                && s.amount == j0.amount
                && s.cost == j0.cost
                && s.price == j0.price
        }
        _ => todo!(
            "j {} postings, s {} postings\n{}",
            journal.postings.len(),
            staging.postings.len(),
            journal
        ),
    };
    if !postings_match {
        return false;
    }

    journal.payee == staging.payee && journal.narration == staging.narration
}

pub fn journal_matches_staging(journal: &Directive, staging: &Directive) -> bool {
    if std::mem::discriminant(&journal.content) != std::mem::discriminant(&staging.content) {
        return false;
    }

    match (&journal.content, &staging.content) {
        (DirectiveContent::Balance(j), DirectiveContent::Balance(s)) => j == s,
        (DirectiveContent::Close(j), DirectiveContent::Close(s)) => j == s,
        (DirectiveContent::Commodity(j), DirectiveContent::Commodity(s)) => j == s,
        (DirectiveContent::Event(j), DirectiveContent::Event(s)) => j == s,
        (DirectiveContent::Open(j), DirectiveContent::Open(s)) => j == s,
        (DirectiveContent::Pad(j), DirectiveContent::Pad(s)) => j == s,
        (DirectiveContent::Price(j), DirectiveContent::Price(s)) => j == s,
        (DirectiveContent::Transaction(j), DirectiveContent::Transaction(s)) => {
            journal_matches_staging_transaction(j, s)
        }
        _ => {
            todo!("Journal: {}\nStaging: {}", journal, staging)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Directive, Entry, Result, reconcile::matching::journal_matches_staging};

    fn parse_single_entry(source: &str) -> Entry {
        let mut entries = beancount_parser::parse_iter(source)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
            .unwrap();
        assert_eq!(entries.len(), 1);
        entries.pop().unwrap()
    }
    fn parse_single_directive(source: &str) -> Directive {
        match parse_single_entry(source) {
            Entry::Directive(directive) => directive,
            _ => panic!(),
        }
    }

    #[test]
    fn match_simple() {
        let journal = r#"
2025-12-01 * "payee" "narration"
    Assets:Account  -99.00 EUR
        date: 2025-12-01
        source_desc: "narration"
    Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 * "payee" "narration"
    Assets:Account  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging));
    }

    #[test]
    fn match_allows_new_metadata() {
        let journal = r#"
2025-12-01 * "payee" "narration"
    Assets:Account  -99.00 EUR
        date: 2025-12-01
        meta: "foo"
    Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 * "payee" "narration"
    Assets:Account  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging));
    }

    #[test]
    fn match_allows_multiline_narration() {
        let journal = r#"
2025-12-01 * "payee" "narration
continued here"
    Assets:Account  -99.00 EUR
    Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 * "payee" "narration
continued here"
    Assets:Account  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging));
    }

    #[test]
    fn dont_match_different_payee() {
        let journal = r#"
2025-12-01 * "payee" "narration"
    Assets:Account  -99.00 EUR
    Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 * "anotherpayee" "narration"
    Assets:Account  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(!journal_matches_staging(&directive, &staging));
    }

    #[test]
    fn dont_match_different_narration() {
        let journal = r#"
2025-12-01 * "payee" "narration A"
    Assets:Account  -99.00 EUR
    Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 * "payee" "narration B"
    Assets:Account  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(!journal_matches_staging(&directive, &staging));
    }

    #[test]
    fn dont_match_different_account() {
        let journal = r#"
2025-12-01 * "payee" "narration"
    Assets:Checking  -99.00 EUR
    Expenses:Food    99.00 EUR
"#;
        let staging = r#"
2025-12-01 * "payee" "narration"
    Assets:Savings  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(!journal_matches_staging(&directive, &staging));
    }

    #[test]
    fn dont_match_different_amount() {
        let journal = r#"
2025-12-01 * "payee" "narration"
    Assets:Account  -99.00 EUR
    Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 * "payee" "narration"
    Assets:Account  -50.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(!journal_matches_staging(&directive, &staging));
    }

    #[test]
    fn dont_match_different_cost() {
        let journal = r#"
2025-12-01 * "payee" "narration"
    Assets:Account  -99.00 EUR {1.10 USD}
    Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 * "payee" "narration"
    Assets:Account  -99.00 EUR {1.20 USD}
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(!journal_matches_staging(&directive, &staging));
    }

    #[test]
    fn dont_match_different_price() {
        let journal = r#"
2025-12-01 * "payee" "narration"
    Assets:Account  -99.00 EUR @ 1.10 USD
    Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 * "payee" "narration"
    Assets:Account  -99.00 EUR @ 1.20 USD
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(!journal_matches_staging(&directive, &staging));
    }

    #[test]
    fn match_ignores_different_flags() {
        let journal = r#"
2025-12-01 * "payee" "narration"
    Assets:Account  -99.00 EUR
    Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 ! "payee" "narration"
    Assets:Account  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging));
    }

    #[test]
    fn match_ignores_tags() {
        let journal = r#"
2025-12-01 * "payee" "narration" #tag1 #tag2
    Assets:Account  -99.00 EUR
    Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 * "payee" "narration"
    Assets:Account  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging));
    }

    #[test]
    fn match_ignores_links() {
        let journal = r#"
2025-12-01 * "payee" "narration" ^link1
    Assets:Account  -99.00 EUR
    Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 * "payee" "narration"
    Assets:Account  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging));
    }

    #[test]
    fn match_balance_directives() {
        let journal = r#"
2025-12-01 balance Assets:Checking  100.00 EUR
"#;
        let staging = r#"
2025-12-01 balance Assets:Checking  100.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging));
    }

    #[test]
    fn dont_match_different_balance_directives() {
        let journal = r#"
2025-12-01 balance Assets:Checking  100.00 EUR
"#;
        let staging = r#"
2025-12-01 balance Assets:Checking  200.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(!journal_matches_staging(&directive, &staging));
    }

    #[test]
    fn dont_match_empty_payee() {
        let journal = r#"
2025-12-01 * "" "narration"
    Assets:Account  -99.00 EUR
    Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 * "payee" "narration"
    Assets:Account  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(!journal_matches_staging(&directive, &staging));
    }

    #[test]
    fn dont_match_different_directive_types() {
        let journal = r#"
2025-12-01 * "payee" "narration"
    Assets:Account  -99.00 EUR
    Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 balance Assets:Checking  100.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(!journal_matches_staging(&directive, &staging));
    }
}
