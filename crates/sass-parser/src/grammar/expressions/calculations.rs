use crate::parser::{CompletedMarker, Parser};
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;

use super::super::ParseContext;
use super::atoms::{number_or_dimension, variable_ref};
use super::functions::CALC_NAMES;
use super::interpolation;

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
fn calc_sum(p: &mut Parser<'_>) {
    let m = p.start();
    calc_product(p);
    let mut has_op = false;
    while (p.at(PLUS) || p.at(MINUS)) && p.has_whitespace_before() {
        has_op = true;
        p.bump(); // + or -
        calc_product(p);
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
        _ => {
            g.error("expected value in calculation");
        }
    }
    let _ = m.complete(&mut g, CALC_VALUE);
}
