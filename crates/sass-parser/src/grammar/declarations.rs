use crate::parser::Parser;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;

use super::selectors;

/// 2.7: Parse declaration — property + `:` + value + `;`.
pub fn declaration(p: &mut Parser<'_>) {
    let m = p.start();
    property(p);
    p.expect(COLON);
    if !p.at(SEMICOLON) && !p.at(RBRACE) && !p.at_end() {
        value(p);
    }
    if !p.at(RBRACE) && !p.at_end() {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, DECLARATION);
}

/// Parse a property name, including `--custom-prop` and interpolation.
fn property(p: &mut Parser<'_>) {
    let m = p.start();
    match p.current() {
        // Normal property name or custom property (--var)
        IDENT | MINUS => {
            p.bump();
            // Handle --custom-property: MINUS was consumed, now eat the rest
            // Actually, lexer emits `--custom` as a single IDENT token
            // So MINUS here would be `-webkit-foo` etc.
        }
        HASH_LBRACE => {
            selectors::interpolation(p);
        }
        _ => {
            p.error("expected property name");
        }
    }
    let _ = m.complete(p, PROPERTY);
}

/// 2.8: Parse declaration value — paren-depth-aware flat token scan.
///
/// Tracks `()` depth and scans until `;` or `}` at depth 0.
/// Handles `calc()`, `rgb()`, etc. without breaking.
fn value(p: &mut Parser<'_>) {
    let m = p.start();
    let mut depth: u32 = 0;
    loop {
        match p.current() {
            EOF => break,
            SEMICOLON | RBRACE if depth == 0 => break,
            LPAREN => {
                depth += 1;
                p.bump();
            }
            RPAREN => {
                depth = depth.saturating_sub(1);
                p.bump();
            }
            HASH_LBRACE => {
                selectors::interpolation(p);
            }
            _ => p.bump(),
        }
    }
    let _ = m.complete(p, VALUE);
}
