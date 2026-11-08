use crate::grammar::{at_interpolated_identifier, interpolated_identifier};
use crate::parser::Parser;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;

/// `@media condition { }`
pub fn media_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // media

    // Media query list — opaque until `;` or `{`
    let errors = p.error_count();
    media_query_list(p);
    rule_body(p, errors);
    let _ = m.complete(p, MEDIA_RULE);
}

fn media_query_list(p: &mut Parser<'_>) {
    let m = p.start();
    if p.braced() {
        loop {
            media_query(p);
            if !p.eat(COMMA) {
                break;
            }
        }
    } else {
        let empty = p.at_ts(BLOCK_STOP) || p.at_end();
        super::eat_opaque_condition(p, BLOCK_STOP, false);
        super::lines::unfinished_condition(p, empty, &["and", "or", "not"], "media query");
    }
    let _ = m.complete(p, MEDIA_QUERY);
}

/// The block of a braced `@media` or `@supports`, which Dart Sass requires. If it is
/// missing, tokens left on the query's line are reported where the `{` belongs and
/// skipped up to a block that is then the rule's, as in `@media screen and (a) "x" {`.
/// Otherwise it is reported right after the query, unless the query reported an error
/// of its own since `errors`, and a statement on the next line is left to itself.
fn rule_body(p: &mut Parser<'_>, errors: usize) {
    if p.at(LBRACE) {
        super::block_after(p, p.error_count() > errors);
    } else if !p.braced() {
    } else if !p.at_end() && !p.at(SEMICOLON) && !p.at(RBRACE) && !p.at_line_break() {
        super::super::skip_to_rule_block(p, p.error_count() == errors);
    } else if p.error_count() == errors {
        p.error_after_prev("expected `{`");
    }
}

/// Tokens that stop opaque condition consumption.
#[rustfmt::skip]
const BLOCK_STOP: crate::token_set::TokenSet = crate::token_set::TokenSet::new(&[
    LBRACE, RBRACE, SEMICOLON,
]);

/// Whether the cursor is at the word `word`, in any case.
fn at_word(p: &Parser<'_>, word: &str) -> bool {
    p.at(IDENT) && p.current_text().eq_ignore_ascii_case(word)
}

/// `( … )` or an interpolation, where Dart Sass reads a media condition in parentheses.
fn media_in_parens(p: &mut Parser<'_>) -> bool {
    if p.at(LPAREN) {
        super::eat_condition_parens(p, false);
    } else if p.at(HASH_LBRACE) {
        let _ = super::interpolation(p);
    } else {
        return false;
    }
    true
}

/// Conditions in parentheses joined by `op`.
fn media_sequence(p: &mut Parser<'_>, op: &str) {
    while media_in_parens(p) && at_word(p, op) {
        p.bump();
    }
}

/// One braced media query, read as Dart Sass reads it, so that it ends where Dart's
/// does: `(…)` joined by `and` or `or`; `not (…)`; or a media type, after an optional
/// modifier, which may be any word, then `and` and conditions. As in Dart, the second
/// word may stand on the next line: `@media screen` / `div {` is the query `screen div`.
fn media_query(p: &mut Parser<'_>) {
    if p.at(LPAREN) {
        media_in_parens(p);
        if at_word(p, "and") {
            p.bump();
            media_sequence(p, "and");
        } else if at_word(p, "or") {
            p.bump();
            media_sequence(p, "or");
        }
        return;
    }
    if !at_interpolated_identifier(p) {
        return;
    }
    let not = at_word(p, "not");
    interpolated_identifier(p);
    if not && !at_interpolated_identifier(p) {
        media_in_parens(p);
        return;
    }
    if !at_interpolated_identifier(p) {
        return;
    }
    if at_word(p, "and") {
        p.bump();
    } else {
        interpolated_identifier(p);
        if !at_word(p, "and") {
            return;
        }
        p.bump();
    }
    if at_word(p, "not") {
        p.bump();
        media_in_parens(p);
        return;
    }
    media_sequence(p, "and");
}

/// `@supports condition { }`
pub fn supports_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // supports

    let errors = p.error_count();
    supports_condition(p);
    rule_body(p, errors);
    let _ = m.complete(p, SUPPORTS_RULE);
}

fn supports_condition(p: &mut Parser<'_>) {
    let m = p.start();
    if p.braced() {
        supports_logic(p);
    } else {
        let empty = p.at_ts(BLOCK_STOP) || p.at_end();
        super::eat_opaque_condition(p, BLOCK_STOP, true);
        super::lines::unfinished_condition(p, empty, &["and", "or", "not"], "condition");
    }
    let _ = m.complete(p, SUPPORTS_CONDITION);
}

/// A braced `@supports` condition, read as Dart Sass reads it: `not` and a condition in
/// parentheses, or conditions in parentheses joined by one of `and` and `or`. It ends
/// before anything else, as Dart's does.
fn supports_logic(p: &mut Parser<'_>) {
    if at_word(p, "not") {
        p.bump();
        supports_in_parens(p);
        return;
    }
    if !supports_in_parens(p) {
        return;
    }
    let mut op: Option<&str> = None;
    while p.at(IDENT) {
        match op {
            Some(op) if at_word(p, op) => {}
            None if at_word(p, "and") => op = Some("and"),
            None if at_word(p, "or") => op = Some("or"),
            _ => return,
        }
        p.bump();
        if !supports_in_parens(p) {
            return;
        }
    }
}

/// `( … )`, an interpolation, or a function such as `selector(…)` or `#{"a"}(b)`.
fn supports_in_parens(p: &mut Parser<'_>) -> bool {
    if at_interpolated_identifier(p) {
        interpolated_identifier(p);
        if p.at(LPAREN) && !p.has_whitespace_before() {
            super::eat_condition_parens(p, true);
        }
    } else if p.at(LPAREN) {
        super::eat_condition_parens(p, true);
    } else {
        return false;
    }
    true
}

/// `@keyframes name { from { } to { } 50% { } }`
pub fn keyframes_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // keyframes / -webkit-keyframes / etc.

    // Name (can be interpolated, a variable, or omitted for anonymous)
    if p.at(HASH_LBRACE) {
        let _ = super::interpolation(p);
    } else if p.at(IDENT) {
        p.bump();
    } else if p.at(DOLLAR) {
        // `$variable` used literally as name
        p.bump();
        if p.at(IDENT) {
            p.bump();
        }
    }
    // Empty name (anonymous keyframes) — no error, proceed to block

    // Tolerate extra tokens between name and block (e.g., `@keyframes name line 429 { }`)
    while !p.at(LBRACE) && !p.at(SEMICOLON) && !p.at(RBRACE) && !p.at_end() {
        super::lines::bump_in_run(p);
    }

    if !p.at(LBRACE) {
        p.error("expected `{`");
        let _ = m.complete(p, KEYFRAMES_RULE);
        return;
    }

    // Parse keyframes block manually (not a normal block — contains keyframe selectors)
    p.bump(); // {
    while !p.at(RBRACE) && !p.at_end() {
        if p.at(SEMICOLON) {
            p.bump();
        } else if p.at(DOLLAR) {
            // Variable declaration inside keyframes: `$b: 10%;`
            super::expressions::variable_declaration(p);
        } else if p.at(IDENT) && p.nth(1) == COLON {
            // Declaration inside keyframes: `blah: blee;`
            super::declarations::declaration(p);
        } else if p.at(IDENT) || p.at(NUMBER) || p.at(HASH_LBRACE) {
            keyframe_block(p);
        } else if p.at(AT) {
            // Allow nested at-rules (e.g., @at-root inside keyframes)
            super::at_rule(p);
        } else {
            super::lines::err_and_bump(p, "expected keyframe selector");
        }
    }
    p.expect(RBRACE);
    let _ = m.complete(p, KEYFRAMES_RULE);
}

/// Parse a keyframe block: `from { }`, `to { }`, `50% { }`, `10%, 20% { }`
fn keyframe_block(p: &mut Parser<'_>) {
    let m = p.start();
    let errors = p.error_count();

    // Parse keyframe selector(s)
    keyframe_selector(p);
    while p.eat(COMMA) {
        p.continue_line_across_blank();
        keyframe_selector(p);
    }

    if p.at(LBRACE) {
        super::block(p);
    } else if p.braced() && p.error_count() == errors {
        p.error_after_prev("expected `{`");
    }
    // The outer KEYFRAME_SELECTOR wraps selector + block
    let _ = m.complete(p, KEYFRAME_SELECTOR);
}

/// Parse a single keyframe stop: `from`, `to`, `50%`, or `#{interpolation}`
fn keyframe_selector(p: &mut Parser<'_>) {
    match p.current() {
        IDENT => {
            let text = p.current_text();
            if text == "from" || text == "to" {
                p.bump();
            } else {
                p.error("expected `from`, `to`, or percentage");
                p.bump();
            }
        }
        NUMBER => {
            p.bump();
            if p.at(PERCENT) && !p.has_whitespace_before() {
                p.bump();
            }
        }
        HASH_LBRACE => {
            let _ = super::interpolation(p);
        }
        _ => {
            p.error("expected keyframe selector");
        }
    }
}
