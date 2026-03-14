use crate::parser::Parser;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;

use super::ParseContext;
use super::expressions;
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

    if !p.at(SEMICOLON) && !p.at(RBRACE) && !p.at_end() && !p.at(LBRACE) {
        value(p);
    }

    // Check for !important after the value
    if p.at(BANG) {
        let flag_text = if p.nth(1) == IDENT { p.nth_text(1) } else { "" };
        if flag_text == "important" {
            let fm = p.start();
            p.bump(); // !
            p.bump(); // important
            let _ = fm.complete(p, IMPORTANT);
        }
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
    if !p.at(IDENT) && !p.at(HASH_LBRACE) && !p.at(MINUS) {
        p.error("expected property name");
        let _ = m.complete(p, PROPERTY);
        return;
    }
    // Consume interleaved IDENT/MINUS/HASH_LBRACE fragments (no whitespace between)
    loop {
        match p.current() {
            IDENT | MINUS => p.bump(),
            HASH_LBRACE => {
                let _ = expressions::interpolation(p);
            }
            _ => break,
        }
        if p.has_whitespace_before() {
            break;
        }
    }
    let _ = m.complete(p, PROPERTY);
}

/// Parse declaration value using expression-based parsing.
///
/// Parses space-separated expressions as direct children of the DECLARATION.
/// Wraps in a VALUE node (acts as the expression-group container).
/// Comma-separated values and `/` separators are consumed within.
fn value(p: &mut Parser<'_>) {
    let m = p.start();
    let ctx = ParseContext::CssValue;

    let mut has_content = false;
    loop {
        if p.at(SEMICOLON) || p.at(RBRACE) || p.at(LBRACE) || p.at_end() || p.at(BANG) {
            break;
        }
        // In CssValue context, `/` is a separator — just consume it
        if p.at(SLASH) {
            p.bump();
            has_content = true;
            continue;
        }
        if p.at(COMMA) {
            p.bump();
            has_content = true;
            continue;
        }
        if !p.at_ts(expressions::EXPR_START) {
            // Unknown token in value position — bump for progress
            p.bump();
            has_content = true;
            continue;
        }
        if expressions::expr(p, ctx).is_some() {
            has_content = true;
        } else {
            break;
        }
    }

    if !has_content && !p.at(SEMICOLON) && !p.at(RBRACE) && !p.at(LBRACE) && !p.at_end() {
        p.err_and_bump("expected value");
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
                let _ = selectors::interpolation(p);
            }
            _ => p.bump(),
        }
    }
    let _ = m.complete(p, VALUE);
}
