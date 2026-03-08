use crate::parser::Parser;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;
use crate::token_set::TokenSet;

/// Tokens that can start a selector (used by mod.rs too).
#[rustfmt::skip]
pub const SELECTOR_START: TokenSet = TokenSet::new(&[
    IDENT, DOT, HASH, COLON, COLON_COLON, LBRACKET, AMP, PERCENT, STAR,
    HASH_LBRACE,
]);

/// Combinator tokens (explicit combinators between compound selectors).
#[rustfmt::skip]
const COMBINATOR_TOKEN: TokenSet = TokenSet::new(&[
    GT, PLUS, TILDE,
]);

/// 2.6: Parse selector list — comma-separated compound selectors.
/// Stops before `{` or at EOF.
pub fn selector_list(p: &mut Parser<'_>) {
    let m = p.start();
    selector(p);
    while p.at(COMMA) {
        p.bump(); // ,
        if p.at_ts(SELECTOR_START) {
            selector(p);
        } else {
            p.error("expected selector after `,`");
        }
    }
    let _ = m.complete(p, SELECTOR_LIST);
}

/// Parse a single compound selector (sequence of simple selectors and combinators).
fn selector(p: &mut Parser<'_>) {
    let m = p.start();
    if !p.at_ts(SELECTOR_START) {
        p.error("expected selector");
        m.abandon(p);
        return;
    }
    compound_selector(p);
    loop {
        if p.at_ts(COMBINATOR_TOKEN) {
            let cm = p.start();
            p.bump();
            let _ = cm.complete(p, COMBINATOR);
            if p.at_ts(SELECTOR_START) {
                compound_selector(p);
            }
        } else if p.at_ts(SELECTOR_START) {
            // Descendant combinator (whitespace) — no explicit token
            compound_selector(p);
        } else {
            break;
        }
    }
    let _ = m.complete(p, SELECTOR);
}

/// Parse a compound selector: one or more adjacent simple selectors.
/// e.g., `div.class#id[attr]:hover::before`
fn compound_selector(p: &mut Parser<'_>) {
    simple_selector(p);
    while p.at_ts(SELECTOR_START) && !p.has_whitespace_before() {
        simple_selector(p);
    }
}

/// Parse a single simple selector.
fn simple_selector(p: &mut Parser<'_>) {
    match p.current() {
        IDENT | STAR => {
            let m = p.start();
            p.bump();
            let _ = m.complete(p, SIMPLE_SELECTOR);
        }
        DOT => {
            // .class
            let m = p.start();
            p.bump(); // .
            p.expect(IDENT);
            let _ = m.complete(p, SIMPLE_SELECTOR);
        }
        HASH => {
            // #id
            let m = p.start();
            p.bump(); // #
            if p.at(IDENT) || p.at(NUMBER) {
                p.bump();
            } else {
                p.error("expected identifier after `#`");
            }
            let _ = m.complete(p, SIMPLE_SELECTOR);
        }
        AMP => {
            // & parent selector (2.11)
            let m = p.start();
            p.bump(); // &
            // Optional suffix: &-suffix, &__suffix (no whitespace)
            if !p.has_whitespace_before() && (p.at(IDENT) || p.at(MINUS)) {
                p.bump();
            }
            let _ = m.complete(p, SIMPLE_SELECTOR);
        }
        PERCENT => {
            // %placeholder selector (2.11)
            let m = p.start();
            p.bump(); // %
            p.expect(IDENT);
            let _ = m.complete(p, SIMPLE_SELECTOR);
        }
        COLON_COLON => {
            // ::pseudo-element
            let m = p.start();
            p.bump(); // ::
            p.expect(IDENT);
            if p.at(LPAREN) {
                eat_balanced_parens(p);
            }
            let _ = m.complete(p, PSEUDO_SELECTOR);
        }
        COLON => {
            // :pseudo-class
            let m = p.start();
            p.bump(); // :
            p.expect(IDENT);
            if p.at(LPAREN) {
                eat_balanced_parens(p);
            }
            let _ = m.complete(p, PSEUDO_SELECTOR);
        }
        LBRACKET => {
            // [attr] selector — opaque content
            let m = p.start();
            eat_balanced_brackets(p);
            let _ = m.complete(p, ATTR_SELECTOR);
        }
        HASH_LBRACE => {
            // #{...} interpolation in selector (2.12)
            interpolation(p);
        }
        _ => {
            p.err_and_bump("expected selector");
        }
    }
}

/// Parse `#{...}` interpolation — opaque for Phase 2.
pub fn interpolation(p: &mut Parser<'_>) {
    assert!(p.at(HASH_LBRACE));
    let m = p.start();
    p.bump(); // #{
    let mut depth: u32 = 1;
    while !p.at_end() {
        match p.current() {
            LBRACE | HASH_LBRACE => {
                depth += 1;
                p.bump();
            }
            RBRACE => {
                depth -= 1;
                p.bump();
                if depth == 0 {
                    break;
                }
            }
            _ => p.bump(),
        }
    }
    let _ = m.complete(p, INTERPOLATION);
}

/// Consume `(...)` balanced parentheses, treating content as opaque.
fn eat_balanced_parens(p: &mut Parser<'_>) {
    assert!(p.at(LPAREN));
    p.bump(); // (
    let mut depth: u32 = 1;
    while !p.at_end() && depth > 0 {
        match p.current() {
            LPAREN => {
                depth += 1;
                p.bump();
            }
            RPAREN => {
                depth -= 1;
                p.bump();
            }
            _ => p.bump(),
        }
    }
}

/// Consume `[...]` balanced brackets, treating content as opaque.
fn eat_balanced_brackets(p: &mut Parser<'_>) {
    assert!(p.at(LBRACKET));
    p.bump(); // [
    let mut depth: u32 = 1;
    while !p.at_end() && depth > 0 {
        match p.current() {
            LBRACKET => {
                depth += 1;
                p.bump();
            }
            RBRACKET => {
                depth -= 1;
                p.bump();
            }
            _ => p.bump(),
        }
    }
}
