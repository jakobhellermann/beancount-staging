use crate::{Directive, DirectiveContent, Transaction};

/// Reasons why two transactions didn't match
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MismatchReason {
    DifferentDirectiveType,
    DifferentPayee { journal: String, staging: String },
    DifferentNarration { journal: String, staging: String },
    DifferentAccount { journal: String, staging: String },
    DifferentAmount,
    DifferentCost,
    DifferentPrice,
    NoPrimaryPosting,
}

fn journal_matches_staging_transaction(
    journal: &Transaction,
    staging: &Transaction,
    journal_directive: &Directive,
) -> Result<(), MismatchReason> {
    // flag can be anything
    // tags can be anything
    // links can be anything

    // The primary account (first posting) must match exactly
    let (Some(staging_primary), Some(journal_primary)) =
        (staging.postings.first(), journal.postings.first())
    else {
        return Err(MismatchReason::NoPrimaryPosting);
    };

    if staging_primary.account != journal_primary.account {
        return Err(MismatchReason::DifferentAccount {
            journal: journal_primary.account.to_string(),
            staging: staging_primary.account.to_string(),
        });
    }

    if staging_primary.amount != journal_primary.amount {
        return Err(MismatchReason::DifferentAmount);
    }

    if staging_primary.cost != journal_primary.cost {
        return Err(MismatchReason::DifferentCost);
    }

    if staging_primary.price != journal_primary.price {
        return Err(MismatchReason::DifferentPrice);
    }

    // Other postings in staging are allowed to differ or be absent in journal
    // (user might reorganize/edit expense accounts between staging and journal)

    // Non-primary postings shouldn't have cost or price
    for posting in staging.postings.iter().skip(1) {
        if posting.cost.is_some() || posting.price.is_some() {
            unimplemented!("Non-primary postings with cost or price are not yet supported");
        }
    }
    for posting in journal.postings.iter().skip(1) {
        if posting.cost.is_some() || posting.price.is_some() {
            unimplemented!("Non-primary postings with cost or price are not yet supported");
        }
    }

    let first_posting = journal.postings.first().expect("TODO: no accounts?");

    // Check directive metadata first (new location), then posting metadata (old location), then transaction field
    let journal_payee = journal_directive
        .metadata
        .get("source_payee")
        .and_then(|x| x.as_string())
        .or_else(|| {
            first_posting
                .metadata
                .get("source_payee")
                .and_then(|x| x.as_string())
        })
        .or(journal.payee.as_deref());

    let journal_narration = journal_directive
        .metadata
        .get("source_desc")
        .and_then(|x| x.as_string())
        .or_else(|| {
            first_posting
                .metadata
                .get("source_desc")
                .and_then(|x| x.as_string())
        })
        .or(journal.narration.as_deref());

    // Normalize empty strings to None for comparison
    let journal_payee = journal_payee.filter(|s| !s.is_empty());
    let staging_payee = staging.payee.as_deref().filter(|s| !s.is_empty());
    let journal_narration = journal_narration.filter(|s| !s.is_empty());
    let staging_narration = staging.narration.as_deref().filter(|s| !s.is_empty());

    // Normalize newlines for narration comparison
    let journal_narration = journal_narration.map(normalize_whitespace);
    let staging_narration = staging_narration.map(normalize_whitespace);

    if journal_payee != staging_payee {
        return Err(MismatchReason::DifferentPayee {
            journal: journal_payee.unwrap_or("").to_string(),
            staging: staging_payee.unwrap_or("").to_string(),
        });
    }

    if journal_narration != staging_narration {
        return Err(MismatchReason::DifferentNarration {
            journal: journal_narration.unwrap_or_default(),
            staging: staging_narration.unwrap_or_default(),
        });
    }

    Ok(())
}

/// Check if journal matches staging, returning a mismatch reason if they don't match
pub fn journal_matches_staging(
    journal: &Directive,
    staging: &Directive,
) -> Result<(), MismatchReason> {
    if std::mem::discriminant(&journal.content) != std::mem::discriminant(&staging.content) {
        return Err(MismatchReason::DifferentDirectiveType);
    }

    let matches = match (&journal.content, &staging.content) {
        (DirectiveContent::Balance(j), DirectiveContent::Balance(s)) => j == s,
        (DirectiveContent::Close(j), DirectiveContent::Close(s)) => j == s,
        (DirectiveContent::Commodity(j), DirectiveContent::Commodity(s)) => j == s,
        (DirectiveContent::Event(j), DirectiveContent::Event(s)) => j == s,
        (DirectiveContent::Open(j), DirectiveContent::Open(s)) => j == s,
        (DirectiveContent::Pad(j), DirectiveContent::Pad(s)) => j == s,
        (DirectiveContent::Price(j), DirectiveContent::Price(s)) => j == s,
        (DirectiveContent::Transaction(j), DirectiveContent::Transaction(s)) => {
            return journal_matches_staging_transaction(j, s, journal);
        }
        _ => {
            todo!("Journal: {}\nStaging: {}", journal, staging)
        }
    };

    if matches {
        Ok(())
    } else {
        Err(MismatchReason::DifferentDirectiveType)
    }
}

/// Normalize a string by replacing newlines with spaces and collapsing multiple spaces
fn normalize_whitespace(s: &str) -> String {
    let with_spaces = s.replace('\n', " ");
    // Collapse multiple spaces into single space
    let mut result = String::with_capacity(with_spaces.len());
    let mut prev_was_space = false;

    for c in with_spaces.chars() {
        if c.is_whitespace() {
            if !prev_was_space {
                result.push(' ');
                prev_was_space = true;
            }
        } else {
            result.push(c);
            prev_was_space = false;
        }
    }

    result.truncate(result.trim_end().len());
    result
}

impl std::fmt::Display for MismatchReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MismatchReason::DifferentDirectiveType => write!(f, "Different directive"),
            MismatchReason::DifferentPayee { journal, staging } => {
                write!(
                    f,
                    "Different payee: journal=\"{}\" staging=\"{}\"",
                    journal, staging
                )
            }
            MismatchReason::DifferentNarration { journal, staging } => {
                write!(
                    f,
                    "Different narration: journal=\"{}\" staging=\"{}\"",
                    journal, staging
                )
            }
            MismatchReason::DifferentAccount { journal, staging } => {
                write!(
                    f,
                    "Different account: journal={} staging={}",
                    journal, staging
                )
            }
            MismatchReason::DifferentAmount => write!(f, "Different amount"),
            MismatchReason::DifferentCost => write!(f, "Different cost"),
            MismatchReason::DifferentPrice => write!(f, "Different price"),
            MismatchReason::NoPrimaryPosting => write!(f, "No primary posting"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Directive, Entry, Result, reconcile::matching::journal_matches_staging};

    #[track_caller]
    fn parse_single_entry(source: &str) -> Entry {
        let mut entries = beancount_parser::parse_iter(source)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
            .unwrap();
        assert_eq!(entries.len(), 1);
        entries.pop().unwrap()
    }
    #[track_caller]
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

        assert!(journal_matches_staging(&directive, &staging).is_ok());
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

        assert!(journal_matches_staging(&directive, &staging).is_ok());
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

        assert!(journal_matches_staging(&directive, &staging).is_ok());
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

        assert!(journal_matches_staging(&directive, &staging).is_err());
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

        assert!(journal_matches_staging(&directive, &staging).is_err());
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

        assert!(journal_matches_staging(&directive, &staging).is_err());
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

        assert!(journal_matches_staging(&directive, &staging).is_err());
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

        assert!(journal_matches_staging(&directive, &staging).is_err());
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

        assert!(journal_matches_staging(&directive, &staging).is_err());
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

        assert!(journal_matches_staging(&directive, &staging).is_ok());
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

        assert!(journal_matches_staging(&directive, &staging).is_ok());
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

        assert!(journal_matches_staging(&directive, &staging).is_ok());
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

        assert!(journal_matches_staging(&directive, &staging).is_ok());
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

        assert!(journal_matches_staging(&directive, &staging).is_err());
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

        assert!(journal_matches_staging(&directive, &staging).is_err());
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

        assert!(journal_matches_staging(&directive, &staging).is_err());
    }

    #[test]
    fn match_with_source_payee_metadata() {
        let journal = r#"
2025-12-01 * "Updated Payee" "narration"
  source_payee: "Original Payee"
  Assets:Account  -99.00 EUR
  Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 ! "Original Payee" "narration"
    Assets:Account  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        // Should match because source_payee matches the staging payee
        assert!(journal_matches_staging(&directive, &staging).is_ok());
    }

    #[test]
    fn match_with_source_desc_metadata() {
        let journal = r#"
2025-12-01 * "payee" "Updated Description"
  source_desc: "Original Description"
  Assets:Account  -99.00 EUR
  Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 ! "payee" "Original Description"
    Assets:Account  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        // Should match because source_desc matches the staging narration
        assert!(journal_matches_staging(&directive, &staging).is_ok());
    }

    #[test]
    fn match_with_both_source_metadata_fields() {
        let journal = r#"
2025-12-01 * "Updated Payee" "Updated Description"
  source_payee: "Original Payee"
  source_desc: "Original Description"
  Assets:Account  -99.00 EUR
  Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 ! "Original Payee" "Original Description"
    Assets:Account  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        // Should match because both source_payee and source_desc match the staging values
        assert!(journal_matches_staging(&directive, &staging).is_ok());
    }

    #[test]
    fn dont_match_with_edited_payee_against_new_staging() {
        // When payee is edited, staging with the NEW payee should NOT match (only original matches)
        let journal = r#"
2025-12-01 * "Edited Payee" "narration"
  source_payee: "Original Payee"
  Assets:Account  -99.00 EUR
  Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 ! "Edited Payee" "narration"
    Assets:Account  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        // Should NOT match because staging has edited payee, but journal looks for original
        assert!(journal_matches_staging(&directive, &staging).is_err());
    }

    #[test]
    fn dont_match_with_edited_narration_against_new_staging() {
        // When narration is edited, staging with the NEW narration should NOT match (only original matches)
        let journal = r#"
2025-12-01 * "payee" "Edited Narration"
  source_desc: "Original Narration"
  Assets:Account  -99.00 EUR
  Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 ! "payee" "Edited Narration"
    Assets:Account  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        // Should NOT match because staging has edited narration, but journal looks for original
        assert!(journal_matches_staging(&directive, &staging).is_err());
    }

    #[test]
    fn match_without_metadata_uses_current_values() {
        // When there's no metadata, should match against current payee/narration
        let journal = r#"
2025-12-01 * "Current Payee" "Current Narration"
    Assets:Account  -99.00 EUR
    Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 ! "Current Payee" "Current Narration"
    Assets:Account  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        // Should match because there's no metadata, so it uses current values
        assert!(journal_matches_staging(&directive, &staging).is_ok());
    }

    #[test]
    fn match_with_posting_level_metadata_for_backward_compatibility() {
        // Old format: metadata on posting level should still work
        let journal = r#"
2025-12-01 * "Updated Payee" "Updated Description"
    Assets:Account  -99.00 EUR
        source_payee: "Original Payee"
        source_desc: "Original Description"
    Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 ! "Original Payee" "Original Description"
    Assets:Account  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        // Should match because backward compatibility fallback to posting metadata
        assert!(journal_matches_staging(&directive, &staging).is_ok());
    }

    #[test]
    fn transaction_metadata_takes_priority_over_posting_metadata() {
        // Transaction metadata should take priority when both exist
        let journal = r#"
2025-12-01 * "Wrong Payee" "narration"
  source_payee: "Correct Payee"
  Assets:Account  -99.00 EUR
    source_payee: "Incorrect Payee"
  Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 ! "Correct Payee" "narration"
    Assets:Account  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        // Should match because transaction-level metadata takes priority
        assert!(journal_matches_staging(&directive, &staging).is_ok());
    }

    #[test]
    fn match_with_narration_only() {
        // When both have only narration (no payee), should match
        let journal = r#"
2025-12-01 * "description"
    Assets:Account  -99.00 EUR
    Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 ! "description"
    Assets:Account  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging).is_ok());
    }

    #[test]
    fn match_narration_only_with_empty_payee() {
        // When journal has narration only and staging has empty payee + narration, should match
        let journal = r#"
2025-12-01 * "description"
    Assets:Account  -99.00 EUR
    Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 ! "" "description"
    Assets:Account  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging).is_ok());
    }

    #[test]
    fn dont_match_narration_vs_payee() {
        // When journal has narration "foo" and staging has payee "foo" with empty narration, should not match
        let journal = r#"
2025-12-01 * "foo"
    Assets:Account  -99.00 EUR
    Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 ! "foo" ""
    Assets:Account  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging).is_err());
    }

    #[test]
    fn match_commodity() {
        let journal = r#"
2025-12-10 * "" "ETF Sparplan: iShares Core MSCI Emerging Markets IMI (Acc)"
    Assets:ScalableCapital:MsciWorldEM	3.94 IE00BKM4GZ66 {150.00 EUR}
    Assets:ScalableCapital:Cash	-150.00 EUR
"#;
        let staging = r#"
2025-12-10 * "" "ETF Sparplan: iShares Core MSCI Emerging Markets IMI (Acc)"
    Assets:ScalableCapital:MsciWorldEM	3.94 IE00BKM4GZ66 {150.00 EUR}
    Assets:ScalableCapital:Cash	-150.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging).is_ok());
    }

    #[test]
    fn match_multiple() {
        let journal = r#"
2025-12-10 * "desc"
    Assets:A -101 EUR
    Expenses:Fees 1 EUR
    Expenses:Food
"#;
        let staging = r#"
2025-12-10 * "desc"
    Assets:A -101 EUR
    Expenses:Fees 1 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging).is_ok());
    }

    // TODO: relax this
    #[test]
    fn dont_match_different_first_posting() {
        let journal = r#"
2025-12-10 * "desc"
    Assets:A -101 EUR
    Expenses:Fees 1 EUR
    Expenses:Food 100 EUR
"#;
        let staging = r#"
2025-12-10 * "desc"
    Expenses:Fees 1 EUR
    Assets:A -101 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging).is_err());
    }

    #[test]
    fn match_second_posting_different_account() {
        // First posting matches, second posting has different account - still matches
        let journal = r#"
2025-12-10 * "desc"
    Assets:A -101 EUR
    Expenses:Fees 1 EUR
    Expenses:Food
"#;
        let staging = r#"
2025-12-10 * "desc"
    Assets:A -101 EUR
    Expenses:Tax 1 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging).is_ok());
    }

    #[test]
    fn match_three_postings() {
        // Both have three postings, all match
        let journal = r#"
2025-12-10 * "desc"
    Assets:A -101 EUR
    Expenses:Fees 1 EUR
    Expenses:Food 100 EUR
"#;
        let staging = r#"
2025-12-10 * "desc"
    Assets:A -101 EUR
    Expenses:Fees 1 EUR
    Expenses:Food 100 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging).is_ok());
    }

    #[test]
    fn match_staging_fewer_postings() {
        // Staging has fewer postings than journal - matches if first posting and totals match
        let journal = r#"
2025-12-10 * "desc"
    Assets:A -100 EUR
    Expenses:Food 50 EUR
    Expenses:Fees 50 EUR
"#;
        let staging = r#"
2025-12-10 * "desc"
    Assets:A -100 EUR
    Expenses:Combined 100 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging).is_ok());
    }

    #[test]
    fn match_multiple_with_missing_amounts() {
        // Journal has postings with and without explicit amounts
        let journal = r#"
2025-12-10 * "desc"
    Assets:A -101 EUR
    Expenses:Fees 1 EUR
    Expenses:Food
"#;
        let staging = r#"
2025-12-10 * "desc"
    Assets:A -101 EUR
    Expenses:Fees 1 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging).is_ok());
    }

    #[test]
    fn dont_match_first_posting_different_cost() {
        // First posting has different cost - should not match
        let journal = r#"
2025-12-10 * "desc"
    Assets:A 10 AAPL {100.00 EUR}
    Assets:Cash -1000 EUR
"#;
        let staging = r#"
2025-12-10 * "desc"
    Assets:A 10 AAPL {99.00 EUR}
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging).is_err());
    }

    #[test]
    fn dont_match_first_posting_different_price() {
        // First posting has different price - should not match
        let journal = r#"
2025-12-10 * "desc"
    Assets:A 10 AAPL @ 100.00 EUR
    Assets:Cash -1000 EUR
"#;
        let staging = r#"
2025-12-10 * "desc"
    Assets:A 10 AAPL @ 99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging).is_err());
    }

    #[test]
    fn dont_match_first_posting_different_commodity() {
        // First posting has different commodity - should not match
        let journal = r#"
2025-12-10 * "desc"
    Assets:Stocks 10 AAPL
    Assets:Cash -1000 EUR
"#;
        let staging = r#"
2025-12-10 * "desc"
    Assets:Stocks 10 GOOGL
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging).is_err());
    }

    #[test]
    fn match_same_total_different_accounts() {
        // First posting matches, total amount matches, but accounts differ - should match
        let journal = r#"
2025-12-10 * "desc"
    Assets:A -101 EUR
    Expenses:Fees 1 EUR
    Expenses:Food
"#;
        let staging = r#"
2025-12-10 * "desc"
    Assets:A -101 EUR
    Expenses:Tax 1 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging).is_ok());
    }

    #[test]
    fn match_multi_currency_same_totals() {
        // Multiple currencies, totals match per currency
        let journal = r#"
2025-12-10 * "desc"
    Assets:A -100 EUR
    Expenses:Fees 10 USD
    Expenses:Food
"#;
        let staging = r#"
2025-12-10 * "desc"
    Assets:A -100 EUR
    Expenses:Combined 10 USD
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging).is_ok());
    }

    // TODO: relax this
    #[test]
    #[should_panic(expected = "Non-primary postings with cost or price are not yet supported")]
    fn panic_on_staging_non_primary_with_cost() {
        // Non-primary posting in staging with cost should panic
        let journal = r#"
2025-12-10 * "desc"
    Assets:A -100 EUR
    Expenses:Food 100 EUR
"#;
        let staging = r#"
2025-12-10 * "desc"
    Assets:A -100 EUR
    Assets:Stocks 10 AAPL {10.00 EUR}
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        let _ = journal_matches_staging(&directive, &staging);
    }

    // TODO: relax this
    #[test]
    #[should_panic(expected = "Non-primary postings with cost or price are not yet supported")]
    fn panic_on_journal_non_primary_with_cost() {
        // Non-primary posting in journal with cost should panic
        let journal = r#"
2025-12-10 * "desc"
    Assets:A -100 EUR
    Assets:Stocks 10 AAPL {10.00 EUR}
"#;
        let staging = r#"
2025-12-10 * "desc"
    Assets:A -100 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        let _ = journal_matches_staging(&directive, &staging);
    }

    // TODO: relax this
    #[test]
    #[should_panic(expected = "Non-primary postings with cost or price are not yet supported")]
    fn panic_on_staging_non_primary_with_price() {
        // Non-primary posting in staging with price should panic
        let journal = r#"
2025-12-10 * "desc"
    Assets:A -100 EUR
    Expenses:Food 100 EUR
"#;
        let staging = r#"
2025-12-10 * "desc"
    Assets:A -100 EUR
    Assets:Stocks 10 AAPL @ 10.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        let _ = journal_matches_staging(&directive, &staging);
    }

    // TODO: relax this
    #[test]
    #[should_panic(expected = "Non-primary postings with cost or price are not yet supported")]
    fn panic_on_journal_non_primary_with_price() {
        // Non-primary posting in journal with price should panic
        let journal = r#"
2025-12-10 * "desc"
    Assets:A -100 EUR
    Assets:Stocks 10 AAPL @ 10.00 EUR
"#;
        let staging = r#"
2025-12-10 * "desc"
    Assets:A -100 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        let _ = journal_matches_staging(&directive, &staging);
    }

    #[test]
    fn match_ignores_extra_whitespace_in_narration() {
        // Journal has lots of extra spaces, staging has single spaces
        let journal = r#"
2025-12-01 * "payee" "narration with     many       extra    spaces"
    Assets:Account  -99.00 EUR
    Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 * "payee" "narration with many extra spaces"
    Assets:Account  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging).is_ok());
    }

    #[test]
    fn match_ignores_newlines_and_extra_whitespace() {
        // Journal has newlines with extra spaces, staging has compact version
        let journal = r#"
2025-12-01 * "payee" "narration with     many
extra    spaces    everywhere"
    Assets:Account  -99.00 EUR
    Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 * "payee" "narration with many extra spaces everywhere"
    Assets:Account  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging).is_ok());
    }

    #[test]
    fn match_ignores_trailing_whitespace_in_narration() {
        // Journal has trailing whitespace after narration
        let journal = r#"
2025-12-01 * "payee" "narration with trailing spaces   "
    Assets:Account  -99.00 EUR
    Expenses:Food   99.00 EUR
"#;
        let staging = r#"
2025-12-01 * "payee" "narration with trailing spaces"
    Assets:Account  -99.00 EUR
"#;
        let directive = parse_single_directive(journal);
        let staging = parse_single_directive(staging);

        assert!(journal_matches_staging(&directive, &staging).is_ok());
    }
}
