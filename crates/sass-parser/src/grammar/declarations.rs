use crate::parser::Parser;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;

use super::selectors;

/// 2.7: Parse declaration or nested property.
///
/// Dispatches to custom property, nested property, or regular declaration.
pub fn declaration(p: &mut Parser<'_>) {
    // 2.9: Detect custom properties (--var)
    if p.at(IDENT) && p.current_text().starts_with("--") {
        custom_property_declaration(p);
        return;
    }

    let m = p.start();
    property(p);
    p.expect(COLON);

    // 2.10: Check for nested property patterns
    if p.at(LBRACE) {
        // IDENT COLON LBRACE → nested property with no value: `font: { weight: bold; }`
        super::block(p);
        let _ = m.complete(p, NESTED_PROPERTY);
        return;
    }

    if !p.at(SEMICOLON) && !p.at(RBRACE) && !p.at_end() {
        value(p);
    }

    // 2.10(c): Value-and-block: `margin: 10px { top: 20px; }`
    if p.at(LBRACE) {
        super::block(p);
        let _ = m.complete(p, NESTED_PROPERTY);
        return;
    }

    if !p.at(RBRACE) && !p.at_end() {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, DECLARATION);
}

/// 2.9: Parse CSS custom property declaration.
///
/// `--var: value;` where value is raw tokens (not Sass expressions).
/// Depth-aware: tracks `()`, `[]`, `{}` nesting.
fn custom_property_declaration(p: &mut Parser<'_>) {
    let m = p.start();
    property(p);
    p.expect(COLON);
    if !p.at(SEMICOLON) && !p.at(RBRACE) && !p.at_end() {
        custom_property_value(p);
    }
    if !p.at(RBRACE) && !p.at_end() {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, CUSTOM_PROPERTY_DECL);
}

/// Parse a property name, including interpolation.
///
/// Handles: `color`, `--custom`, `#{$prop}`, `border-#{$side}`,
/// `#{$a}-#{$b}`, `#{$ns}-#{$prop}-color`.
fn property(p: &mut Parser<'_>) {
    let m = p.start();
    if !p.at(IDENT) && !p.at(HASH_LBRACE) {
        p.error("expected property name");
        let _ = m.complete(p, PROPERTY);
        return;
    }
    // Consume interleaved IDENT/MINUS/HASH_LBRACE fragments (no whitespace between)
    loop {
        match p.current() {
            IDENT | MINUS => p.bump(),
            HASH_LBRACE => selectors::interpolation(p),
            _ => break,
        }
        if p.has_whitespace_before() {
            break;
        }
    }
    let _ = m.complete(p, PROPERTY);
}

/// 2.8: Parse declaration value — depth-aware flat token scan.
///
/// Tracks `()` and `[]` depth. Scans until `;`, `}`, or `{` at depth 0.
/// Stops before `{` at depth 0 (nested property check by caller).
fn value(p: &mut Parser<'_>) {
    let m = p.start();
    let mut paren_depth: u32 = 0;
    let mut bracket_depth: u32 = 0;
    loop {
        let at_depth_zero = paren_depth == 0 && bracket_depth == 0;
        match p.current() {
            EOF => break,
            SEMICOLON | RBRACE if at_depth_zero => break,
            LBRACE if at_depth_zero => break,
            LPAREN => {
                paren_depth += 1;
                p.bump();
            }
            RPAREN => {
                paren_depth = paren_depth.saturating_sub(1);
                p.bump();
            }
            LBRACKET => {
                bracket_depth += 1;
                p.bump();
            }
            RBRACKET => {
                bracket_depth = bracket_depth.saturating_sub(1);
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

/// 2.9: Parse custom property value — fully depth-aware raw token scan.
///
/// Tracks `()`, `[]`, `{}` nesting. Only terminates at `;` or `}` at depth 0.
/// Interpolation `#{...}` is still processed.
fn custom_property_value(p: &mut Parser<'_>) {
    let m = p.start();
    let mut paren_depth: u32 = 0;
    let mut bracket_depth: u32 = 0;
    let mut brace_depth: u32 = 0;
    loop {
        let at_depth_zero = paren_depth == 0 && bracket_depth == 0 && brace_depth == 0;
        match p.current() {
            EOF => break,
            SEMICOLON if at_depth_zero => break,
            RBRACE if at_depth_zero => break,
            LPAREN => {
                paren_depth += 1;
                p.bump();
            }
            RPAREN => {
                paren_depth = paren_depth.saturating_sub(1);
                p.bump();
            }
            LBRACKET => {
                bracket_depth += 1;
                p.bump();
            }
            RBRACKET => {
                bracket_depth = bracket_depth.saturating_sub(1);
                p.bump();
            }
            LBRACE => {
                brace_depth += 1;
                p.bump();
            }
            RBRACE => {
                brace_depth = brace_depth.saturating_sub(1);
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
