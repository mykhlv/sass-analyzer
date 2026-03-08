use crate::parser::Parser;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;

/// `@media query { }`
pub fn media_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // media
    media_query_list(p);
    if p.at(LBRACE) {
        super::block(p);
    } else {
        p.error("expected `{`");
    }
    let _ = m.complete(p, MEDIA_RULE);
}

/// Parse comma-separated media queries.
fn media_query_list(p: &mut Parser<'_>) {
    media_query(p);
    while p.eat(COMMA) {
        media_query(p);
    }
}

/// Parse a single media query.
/// Handles: `screen`, `not print`, `(width >= 768px)`, `screen and (color)`, etc.
/// Also handles interpolation `#{$var}`.
fn media_query(p: &mut Parser<'_>) {
    let m = p.start();
    // Consume tokens until `{`, `,`, `;`, `}` or EOF at depth 0
    let mut depth: u32 = 0;
    while !p.at_end() {
        match p.current() {
            LBRACE | RBRACE | SEMICOLON if depth == 0 => break,
            COMMA if depth == 0 => break,
            LPAREN => {
                depth += 1;
                p.bump();
            }
            RPAREN => {
                depth = depth.saturating_sub(1);
                p.bump();
            }
            HASH_LBRACE => {
                let _ = super::interpolation(p);
            }
            _ => p.bump(),
        }
    }
    let _ = m.complete(p, MEDIA_QUERY);
}

/// `@supports condition { }`
pub fn supports_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // supports
    supports_condition(p);
    if p.at(LBRACE) {
        super::block(p);
    } else {
        p.error("expected `{`");
    }
    let _ = m.complete(p, SUPPORTS_RULE);
}

/// Parse a @supports condition: `not`/`and`/`or` combinators with `(prop: value)`.
fn supports_condition(p: &mut Parser<'_>) {
    let m = p.start();
    // Consume tokens until `{`, `}`, or `;` at depth 0
    let mut depth: u32 = 0;
    while !p.at_end() {
        match p.current() {
            LBRACE | RBRACE | SEMICOLON if depth == 0 => break,
            LPAREN => {
                depth += 1;
                p.bump();
            }
            RPAREN => {
                depth = depth.saturating_sub(1);
                p.bump();
            }
            HASH_LBRACE => {
                let _ = super::interpolation(p);
            }
            _ => p.bump(),
        }
    }
    let _ = m.complete(p, SUPPORTS_CONDITION);
}

/// `@keyframes name { from { } to { } 50% { } }`
pub fn keyframes_rule(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // @
    p.bump(); // keyframes / -webkit-keyframes / etc.

    // Name (can be interpolated)
    if p.at(HASH_LBRACE) {
        let _ = super::interpolation(p);
    } else if p.at(IDENT) {
        p.bump();
    } else {
        p.error("expected keyframes name");
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
        } else {
            keyframe_block(p);
        }
    }
    p.expect(RBRACE);
    let _ = m.complete(p, KEYFRAMES_RULE);
}

/// Parse a keyframe block: `from { }`, `to { }`, `50% { }`, `10%, 20% { }`
fn keyframe_block(p: &mut Parser<'_>) {
    let m = p.start();

    // Parse keyframe selector(s)
    keyframe_selector(p);
    while p.eat(COMMA) {
        keyframe_selector(p);
    }

    if p.at(LBRACE) {
        super::block(p);
    } else {
        p.error("expected `{`");
    }
    // The outer KEYFRAME_SELECTOR wraps selector + block
    let _ = m.complete(p, KEYFRAME_SELECTOR);
}

/// Parse a single keyframe stop: `from`, `to`, or `50%`
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
        _ => {
            p.error("expected keyframe selector");
        }
    }
}
