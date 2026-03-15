use crate::parser::{CompletedMarker, Parser};
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;
use crate::token_set::TokenSet;

use super::super::ParseContext;
use super::atoms::{number_or_dimension, variable_ref};
use super::functions::CALC_NAMES;
use super::interpolation;

/// Tokens that can start a calc value.
#[rustfmt::skip]
const CALC_VALUE_START: TokenSet = TokenSet::new(&[
    NUMBER, IDENT, DOLLAR, HASH_LBRACE, LPAREN, MINUS, PLUS,
]);

/// Parse a calculation function: `calc(...)`, `min(...)`, `max(...)`, etc.
pub(super) fn calculation(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();
    p.bump(); // function name (calc, min, max, etc.)
    p.expect(LPAREN);

    if !p.at(RPAREN) && !p.at_end() {
        calc_sum(p);
        while p.eat(COMMA) {
            if !p.at(RPAREN) && !p.at_end() {
                calc_sum(p);
            }
        }
    }

    p.expect(RPAREN);
    m.complete(p, CALCULATION)
}

/// Parse calc sum: `a + b` or `a - b` (lowest precedence inside calc).
/// Also allows space-separated values without operators when interpolation,
/// `var()`, or Sass variables produce opaque content.
fn calc_sum(p: &mut Parser<'_>) {
    let m = p.start();
    calc_product(p);
    let mut has_op = false;
    loop {
        if (p.at(PLUS) || p.at(MINUS)) && p.has_whitespace_before() {
            // Explicit + or - operator with whitespace
            has_op = true;
            p.bump();
            calc_product(p);
        } else if p.at_ts(CALC_VALUE_START) && !p.at(PLUS) && !p.at(MINUS) {
            // Space-separated value without operator (opaque content from
            // interpolation, var(), or variables): `calc(#{$x} 2)`, `calc(var(--c) 1)`
            has_op = true;
            calc_product(p);
        } else {
            break;
        }
    }
    if has_op {
        let _ = m.complete(p, CALC_SUM);
    } else {
        m.abandon(p);
    }
}

/// Parse calc product: `a * b` or `a / b` (higher precedence inside calc).
fn calc_product(p: &mut Parser<'_>) {
    let m = p.start();
    calc_value(p);
    let mut has_op = false;
    while p.at(STAR) || p.at(SLASH) {
        has_op = true;
        p.bump(); // * or /
        calc_value(p);
    }
    if has_op {
        let _ = m.complete(p, CALC_PRODUCT);
    } else {
        m.abandon(p);
    }
}

/// Parse a single calc value: number, dimension, variable, nested calc, or parenthesized.
fn calc_value(p: &mut Parser<'_>) {
    let Ok(mut g) = p.depth_guard() else {
        return;
    };
    let m = g.start();
    match g.current() {
        NUMBER => {
            let _ = number_or_dimension(&mut g);
        }
        DOLLAR => {
            let _ = variable_ref(&mut g);
        }
        IDENT => {
            if g.nth(1) == LPAREN && !g.nth_has_whitespace_before(1) {
                let name = g.current_text();
                if CALC_NAMES.iter().any(|n| name.eq_ignore_ascii_case(n)) {
                    // Nested calculation: `calc(min(10px, 5vw) + 1rem)`
                    let _ = calculation(&mut g);
                    m.abandon(&mut g);
                    return;
                }
                // Non-calc function inside calc: `var(--x)`, `env(safe-area-inset-top)`
                let _ = super::functions::function_call(&mut g, ParseContext::CssValue);
                m.abandon(&mut g);
                return;
            }
            // Plain ident
            g.bump();
        }
        HASH_LBRACE => {
            // Interpolation inside calc: `calc(#{$width} + 1rem)`
            let _ = interpolation(&mut g);
        }
        LPAREN => {
            g.bump(); // (
            calc_sum(&mut g);
            g.expect(RPAREN);
        }
        MINUS | PLUS => {
            // Unary inside calc
            g.bump();
            calc_value(&mut g);
        }
        DOT_DOT_DOT => {
            // Splat: `min(1 2 3...)`
            g.bump();
        }
        _ => {
            g.error("expected value in calculation");
        }
    }
    let _ = m.complete(&mut g, CALC_VALUE);
}
