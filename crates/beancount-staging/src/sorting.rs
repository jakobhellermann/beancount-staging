use crate::Directive;
use beancount_parser::DirectiveContent;

pub fn sort_dedup_directives(directives: &mut Vec<Directive>) {
    directives.sort_by_key(directive_order);
    directives.dedup_by(|a, b| is_identical(a, b));
}

fn directive_order(directive: &Directive) -> u8 {
    match directive.content {
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
