use crate::parser::Parser;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;

/// `@media condition { }`
pub fn media_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // media
    p.continue_header(&super::lines::QUERY);

    // Media query list — opaque until `;` or `{`
    let errors = p.error_count();
    media_query_list(p);
    rule_body(p, errors);
    let _ = m.complete(p, MEDIA_RULE);
}

fn media_query_list(p: &mut Parser<'_>) {
    let m = p.start();
    super::eat_opaque_condition(p, query_stop(p), false);
    let _ = m.complete(p, MEDIA_QUERY);
}

/// The block of a braced `@media` or `@supports`, which Dart Sass requires: one error if
/// it is missing, unless the query reported one of its own since `errors`.
fn rule_body(p: &mut Parser<'_>, errors: usize) {
    if p.at(LBRACE) {
        super::block(p);
    } else if p.braced() && p.error_count() == errors {
        p.error_after_prev("expected `{`");
    }
}

/// Tokens that stop opaque condition consumption.
#[rustfmt::skip]
const BLOCK_STOP: crate::token_set::TokenSet = crate::token_set::TokenSet::new(&[
    LBRACE, RBRACE, SEMICOLON,
]);

/// Those, and tokens that start a selector but no media query or `@supports` condition:
/// braced, the query ends before them, as in Dart Sass, and leaves the rule they start to
/// itself, `@supports (a)` / `.z { … }`.
#[rustfmt::skip]
const QUERY_STOP: crate::token_set::TokenSet = crate::token_set::TokenSet::new(&[
    LBRACE, RBRACE, SEMICOLON,
    DOT, HASH, COLON, COLON_COLON, LBRACKET, AMP, PERCENT, STAR, PIPE, GT, PLUS, TILDE,
]);

fn query_stop(p: &Parser<'_>) -> crate::token_set::TokenSet {
    if p.braced() { QUERY_STOP } else { BLOCK_STOP }
}

/// `@supports condition { }`
pub fn supports_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // supports
    p.continue_header(&super::lines::QUERY);

    let errors = p.error_count();
    supports_condition(p);
    rule_body(p, errors);
    let _ = m.complete(p, SUPPORTS_RULE);
}

fn supports_condition(p: &mut Parser<'_>) {
    let m = p.start();
    super::eat_opaque_condition(p, query_stop(p), true);
    let _ = m.complete(p, SUPPORTS_CONDITION);
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
