use crate::parser::{CompletedMarker, Parser};
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;
use crate::token_set::TokenSet;

/// Tokens that can start a selector (used by mod.rs too).
#[rustfmt::skip]
pub const SELECTOR_START: TokenSet = TokenSet::new(&[
    IDENT, DOT, HASH, COLON, COLON_COLON, LBRACKET, AMP, PERCENT, STAR,
    HASH_LBRACE, PIPE,
]);

/// Combinator tokens (explicit combinators between compound selectors).
#[rustfmt::skip]
pub const COMBINATOR_TOKEN: TokenSet = TokenSet::new(&[
    GT, PLUS, TILDE,
]);

/// 2.6: Parse selector list — comma-separated compound selectors.
/// Stops before `{` or at EOF.
pub fn selector_list(p: &mut Parser<'_>) {
    let m = p.start();
    selector(p);
    while p.at(COMMA) {
        p.bump(); // ,
        p.continue_line_across_blank();
        if p.at_ts(SELECTOR_START) || p.at_ts(COMBINATOR_TOKEN) {
            selector(p);
        }
        // Empty position after comma (trailing/extra commas) — silently skip
    }
    let _ = m.complete(p, SELECTOR_LIST);
}

/// Parse a single compound selector (sequence of simple selectors and combinators).
fn selector(p: &mut Parser<'_>) {
    let m = p.start();
    if !p.at_ts(SELECTOR_START) && !p.at_ts(COMBINATOR_TOKEN) {
        p.error("expected selector");
        m.abandon(p);
        return;
    }
    // Leading combinator in nested context: `> .child`, `+ .sibling`
    if p.at_ts(COMBINATOR_TOKEN) {
        // A combinator alone on its line is the whole selector, not an unfinished one.
        let alone = p.at_statement_start();
        let cm = p.start();
        p.bump();
        let _ = cm.complete(p, COMBINATOR);
        if !alone {
            p.continue_line();
        }
    }
    if p.at_ts(SELECTOR_START) {
        compound_selector(p);
    }
    loop {
        if p.at_ts(COMBINATOR_TOKEN) {
            let cm = p.start();
            p.bump();
            let _ = cm.complete(p, COMBINATOR);
            p.continue_line();
            if p.at_ts(SELECTOR_START) {
                compound_selector(p);
            }
        } else if p.at_ts(SELECTOR_START) {
            if p.braced() && super::declaration_interrupts_selector(p) {
                break;
            }
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
    while !p.at_end() && !p.has_whitespace_before() {
        if p.at_ts(SELECTOR_START) {
            simple_selector(p);
        } else if p.at(MINUS) || p.at(NUMBER) || p.at(SyntaxKind::ERROR) {
            // Hyphen/number/escape connecting selector parts: `.col#{$infix}-1`, `\.5`
            super::lines::bump_in_run(p);
        } else {
            break;
        }
    }
}

/// Parse a single simple selector.
fn simple_selector(p: &mut Parser<'_>) {
    match p.current() {
        IDENT | STAR => {
            let m = p.start();
            p.bump();
            // Namespace selector: `ns|element`, `*|*`, `ns|*`
            if p.at(PIPE) && !p.has_whitespace_before() {
                p.bump(); // |
                if p.at(IDENT) || p.at(STAR) {
                    p.bump();
                }
            }
            // Functional selector with escaped colon: `\:nth-child(...)`
            if p.at(LPAREN) && !p.has_whitespace_before() {
                eat_balanced_parens(p);
            }
            let _ = m.complete(p, SIMPLE_SELECTOR);
        }
        PIPE => {
            // `|element` — no namespace
            let m = p.start();
            p.bump(); // |
            if p.at(IDENT) || p.at(STAR) {
                p.bump();
            }
            let _ = m.complete(p, SIMPLE_SELECTOR);
        }
        DOT => {
            // .class, .#{$var}, or .-#{$name}
            let m = p.start();
            p.bump(); // .
            if p.at(HASH_LBRACE) {
                let _ = interpolation(p);
            } else if p.at(MINUS) && !p.has_whitespace_before() {
                // `.-#{$name}` — class name starting with hyphen
                super::lines::bump_in_run(p); // -
                if p.at(HASH_LBRACE) && !p.has_whitespace_before() {
                    let _ = interpolation(p);
                }
            } else {
                p.expect(IDENT);
            }
            let _ = m.complete(p, SIMPLE_SELECTOR);
        }
        HASH => {
            // #id or ##{$var}
            let m = p.start();
            p.bump(); // #
            if p.at(HASH_LBRACE) {
                let _ = interpolation(p);
            } else if p.at(IDENT) || p.at(NUMBER) {
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
                super::lines::bump_in_run(p);
            }
            let _ = m.complete(p, SIMPLE_SELECTOR);
        }
        PERCENT => {
            // %placeholder selector (2.11)
            let m = p.start();
            super::lines::bump_in_run(p); // %
            if p.at(HASH_LBRACE) {
                let _ = interpolation(p);
            } else {
                p.expect(IDENT);
            }
            let _ = m.complete(p, SIMPLE_SELECTOR);
        }
        COLON_COLON => {
            // ::pseudo-element or ::#{$var}
            let m = p.start();
            p.bump(); // ::
            if p.at(HASH_LBRACE) {
                let _ = interpolation(p);
            } else {
                p.expect(IDENT);
            }
            if p.at(LPAREN) {
                eat_balanced_parens(p);
            }
            let _ = m.complete(p, PSEUDO_SELECTOR);
        }
        COLON => {
            // :pseudo-class or :#{$var}
            let m = p.start();
            p.bump(); // :
            if p.at(HASH_LBRACE) {
                let _ = interpolation(p);
            } else {
                p.expect(IDENT);
            }
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
            let _ = interpolation(p);
        }
        _ => {
            super::lines::err_and_bump(p, "expected selector");
        }
    }
}

/// Parse `#{...}` interpolation with fully-parsed inner expression.
pub fn interpolation(p: &mut Parser<'_>) -> CompletedMarker {
    super::expressions::interpolation(p)
}

/// Consume `(...)` balanced parentheses, treating content as opaque.
fn eat_balanced_parens(p: &mut Parser<'_>) {
    eat_balanced(p, LPAREN, RPAREN);
}

/// Consume `[...]` balanced brackets, treating content as opaque.
fn eat_balanced_brackets(p: &mut Parser<'_>) {
    eat_balanced(p, LBRACKET, RBRACKET);
}

/// Consume `open ... close`, treating content as opaque. Braced, a `{` or `}` outside
/// interpolation ends a bracket left unclosed: no selector argument holds one, and the
/// rules after it stay intact. The indented layout recovers at a line start instead.
fn eat_balanced(p: &mut Parser<'_>, open: SyntaxKind, close: SyntaxKind) {
    assert!(p.at(open));
    let p = &mut *p.transparent_guard();
    let braced = p.braced();
    p.bump();
    let mut depth: u32 = 1;
    let mut interpolation: u32 = 0;
    // A string left unclosed took the rest of its line, the closing bracket and all.
    while !p.at_end() && depth > 0 && !p.at_unclosed_string_break() {
        let kind = p.current();
        if braced {
            match kind {
                HASH_LBRACE => interpolation += 1,
                LBRACE | RBRACE if interpolation == 0 => break,
                LBRACE => interpolation += 1,
                RBRACE => interpolation -= 1,
                _ => {}
            }
        }
        if kind == open {
            depth += 1;
        } else if kind == close {
            depth -= 1;
        }
        p.bump();
    }
    if braced && depth > 0 && !p.at_unclosed_string_break() {
        // At a `{` the rule's block starts: that is where the bracket is found unclosed.
        let msg = format!("expected {close:?}");
        if p.at(LBRACE) {
            p.error(msg);
        } else {
            p.error_after_prev(msg);
        }
    }
}
