use crate::parser::Parser;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;

/// `@at-root { }` / `@at-root selector { }` / `@at-root (with: ...) { }`
pub fn at_root_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // at-root

    // Check for query: `(with: ...)` or `(without: ...)`
    if p.at(LPAREN) {
        at_root_query(p);
    }

    // Either a block directly or selector + block
    if p.at(LBRACE) {
        super::block(p);
    } else if !p.at(SEMICOLON) && !p.at(RBRACE) && !p.at_end() {
        // Selector(s) before block
        super::selectors::selector_list(p);
        if p.at(LBRACE) {
            super::block(p);
        } else {
            p.error("expected `{`");
        }
    }

    let _ = m.complete(p, AT_ROOT_RULE);
}

/// Parse `(with: media supports)` or `(without: rule all)`
fn at_root_query(p: &mut Parser<'_>) {
    assert!(p.at(LPAREN));
    let m = p.start();
    p.bump(); // (

    // Expect `with` or `without`
    if p.at(IDENT) && (p.current_text() == "with" || p.current_text() == "without") {
        p.bump();
    } else {
        p.error("expected `with` or `without`");
    }
    p.expect(COLON);

    // Parse values: `rule`, `all`, or at-rule names (may be quoted strings)
    while p.at(IDENT) || p.at(QUOTED_STRING) {
        p.bump();
    }

    p.expect(RPAREN);
    let _ = m.complete(p, AT_ROOT_QUERY);
}
