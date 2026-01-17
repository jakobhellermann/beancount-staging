use crate::{Directive, DirectiveContent};

pub fn sort_dedup_directives(directives: &mut Vec<Directive>) {
    directives.sort_by(|a, b| {
        a.date
            .cmp(&b.date)
            .then_with(|| directive_order(&a.content).cmp(&directive_order(&b.content)))
            .then_with(|| directive_content_key(&a.content).cmp(directive_content_key(&b.content)))
    });
    directives.dedup_by(|a, b| is_identical(a, b));
}

fn directive_content_key(content: &DirectiveContent) -> &str {
    match content {
        DirectiveContent::Balance(b) => b.account.as_str(),
        DirectiveContent::Transaction(t) => t.payee.as_deref().unwrap_or_default(),
        DirectiveContent::Open(o) => o.account.as_str(),
        DirectiveContent::Close(c) => c.account.as_str(),
        DirectiveContent::Pad(p) => p.account.as_str(),
        DirectiveContent::Commodity(c) => c.as_str(),
        DirectiveContent::Event(e) => e.name.as_str(),
        _ => "",
    }
}

fn directive_order(directive: &DirectiveContent) -> u8 {
    match directive {
        DirectiveContent::Open(_) => 0,
        DirectiveContent::Pad(_) => 1,
        DirectiveContent::Commodity(_) => 2,
        DirectiveContent::Transaction(_) => 3,
        DirectiveContent::Balance(_) => 4,
        DirectiveContent::Price(_) => 5,
        DirectiveContent::Close(_) => 6,
        DirectiveContent::Event(_) => 7,
        _ => u8::MAX,
    }
}

// The two directives are the exact same and can be deduplicated
fn is_identical(a: &Directive, b: &Directive) -> bool {
    match (&a.content, &b.content) {
        (DirectiveContent::Balance(ca), DirectiveContent::Balance(cb)) => {
            a.date == b.date && a.metadata == b.metadata && ca == cb
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Directive, Entry, Result};

    fn parse_entries(source: &str) -> Vec<Entry> {
        beancount_parser::parse_iter(source)
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    fn build_directives(source: &str) -> Vec<Directive> {
        let mut all_directives = Vec::new();
        for entry in parse_entries(source) {
            if let Entry::Directive(directive) = entry {
                all_directives.push(directive);
            }
        }
        sort_dedup_directives(&mut all_directives);
        all_directives
    }

    #[test]
    fn dedup_non_consecutive_balance_directives() {
        // This test reproduces the real-world scenario where the same balance
        // appears multiple times on the same date but from different source files,
        // creating an "a b a" pattern where duplicates are not consecutive
        let source = r#"
2023-09-01 balance Assets:BIBEssen:Checking 40273.05 EUR
2023-09-01 balance Assets:ScalableCapital:CashBaader 101.01 EUR
2023-09-01 balance Assets:ScalableCapital:CashBaader 101.01 EUR
2023-09-01 balance Assets:BIBEssen:Checking 40273.05 EUR
2023-09-01 balance Assets:ScalableCapital:CashBaader 101.01 EUR
"#;
        let directives = build_directives(source);

        // Should deduplicate to just 2 unique balances
        assert_eq!(directives.len(), 2);

        // Verify we kept one of each
        let accounts: Vec<_> = directives
            .iter()
            .map(|d| {
                if let DirectiveContent::Balance(b) = &d.content {
                    b.account.as_str()
                } else {
                    panic!("Expected balance directive")
                }
            })
            .collect();

        assert!(accounts.contains(&"Assets:BIBEssen:Checking"));
        assert!(accounts.contains(&"Assets:ScalableCapital:CashBaader"));
    }
}
