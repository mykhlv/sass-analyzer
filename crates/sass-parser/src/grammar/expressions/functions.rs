use crate::parser::{CompletedMarker, Parser};
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;

use super::super::ParseContext;
use super::sass_value;

/// Calculation function names recognized by the dispatcher.
#[rustfmt::skip]
pub(super) const CALC_NAMES: &[&str] = &[
    "calc", "min", "max", "clamp", "round", "mod", "rem",
    "sin", "cos", "tan", "asin", "acos", "atan", "atan2",
    "pow", "sqrt", "hypot", "log", "exp", "abs", "sign",
];

/// Scan tokens inside parentheses to detect SassScript-only features.
/// Used to disambiguate `min()`/`max()`: CSS calculation vs Sass function call.
fn has_sass_signals(p: &Parser<'_>) -> bool {
    let mut offset = 2; // skip function name + LPAREN
    let mut depth = 1u32;
    let mut brace_depth = 0u32;
    // Limit lookahead to avoid O(n^2) on pathological inputs with many nested calc-name calls.
    // If we can't decide within 256 tokens, fall back to Sass function (more permissive).
    let max_offset = offset + 256;
    loop {
        if offset > max_offset {
            return true;
        }
        let kind = p.nth(offset);
        // Track interpolation brace depth — skip signal checks inside #{...}
        if kind == HASH_LBRACE || (kind == LBRACE && brace_depth > 0) {
            brace_depth += 1;
            offset += 1;
            continue;
        }
        if kind == RBRACE && brace_depth > 0 {
            brace_depth -= 1;
            offset += 1;
            continue;
        }
        if brace_depth > 0 {
            offset += 1;
            continue;
        }
        match kind {
            LPAREN => depth += 1,
            RPAREN => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            EOF => break,
            // Comparison operators — never valid in CSS calc
            EQ_EQ | BANG_EQ | LT | GT | LT_EQ | GT_EQ => return true,
            // Brackets — not valid CSS calc atoms (interpolation IS valid in calc)
            LBRACKET => return true,
            // Strings — not valid CSS calc atoms
            STRING_START | QUOTED_STRING => return true,
            // Namespace member access: ns.func(), ns.$var — not valid in CSS calc
            IDENT if p.nth(offset + 1) == DOT && !p.nth_has_whitespace_before(offset + 1) => {
                let after_dot = p.nth(offset + 2);
                if after_dot == IDENT || after_dot == DOLLAR {
                    return true;
                }
            }
            // `%` with whitespace before = modulo operator, not dimension unit
            PERCENT if p.nth_has_whitespace_before(offset) => return true,
            // `and`/`or`/`not` keywords as operators
            IDENT => {
                let text = p.nth_text(offset);
                if text == "and" || text == "or" || text == "not" {
                    return true;
                }
            }
            _ => {}
        }
        offset += 1;
    }
    false
}

/// Dispatch function call by name.
pub(super) fn function_dispatch(p: &mut Parser<'_>, ctx: ParseContext) -> CompletedMarker {
    let name = p.current_text();

    // Special functions: url(), element(), progid:...()
    if name.eq_ignore_ascii_case("url") {
        return special_function_call(p);
    }
    if name.eq_ignore_ascii_case("element") || name.starts_with("progid:") {
        return special_function_call(p);
    }

    // CSS if() function: `if(condition: value; else: alternate)`.
    // Distinguished from Sass if() by `:` (not after $ident) before any `,` at depth 1.
    if name == "if" && is_css_if(p) {
        return special_function_call(p);
    }

    // Legacy MS filter syntax: `alpha(opacity=50)` — contains `=` at depth 1.
    if name.eq_ignore_ascii_case("alpha") && has_eq_in_args(p) {
        return special_function_call(p);
    }

    // Calculation functions
    if CALC_NAMES.iter().any(|n| name.eq_ignore_ascii_case(n)) {
        // Calc function with SassScript-only features → Sass function call
        if has_sass_signals(p) {
            return function_call(p, ctx);
        }
        return super::calculations::calculation(p);
    }

    function_call(p, ctx)
}

/// Check if `if(...)` uses CSS conditional syntax (colon-separated) rather than
/// Sass syntax (comma-separated). Scans ahead from current position (at IDENT "if").
fn is_css_if(p: &Parser<'_>) -> bool {
    const MAX_SCAN: usize = 100;
    // p.nth(0) = IDENT "if", p.nth(1) = LPAREN
    if p.nth(1) != LPAREN {
        return false;
    }
    let mut offset: usize = 2;
    let mut depth: u32 = 1;
    let limit = offset + MAX_SCAN;
    loop {
        if offset >= limit {
            return false;
        }
        let kind = p.nth(offset);
        match kind {
            LPAREN => depth += 1,
            RPAREN => {
                depth -= 1;
                if depth == 0 {
                    return false;
                }
            }
            EOF => return false,
            COLON if depth == 1 => {
                // Keyword arg `$name:` — not CSS if()
                if offset >= 2 && p.nth(offset - 1) == IDENT && p.nth(offset - 2) == DOLLAR {
                    offset += 1;
                    continue;
                }
                return true;
            }
            COMMA if depth == 1 => return false,
            _ => {}
        }
        offset += 1;
    }
}

/// Check if function args contain bare `=` at depth 1 (MS filter syntax: `alpha(opacity=50)`).
fn has_eq_in_args(p: &Parser<'_>) -> bool {
    const MAX_SCAN: usize = 100;
    if p.nth(1) != LPAREN {
        return false;
    }
    let mut offset: usize = 2;
    let mut depth: u32 = 1;
    let limit = offset + MAX_SCAN;
    loop {
        if offset >= limit {
            return false;
        }
        let kind = p.nth(offset);
        match kind {
            LPAREN => depth += 1,
            RPAREN => {
                depth -= 1;
                if depth == 0 {
                    return false;
                }
            }
            EOF => return false,
            EQ if depth == 1 => return true,
            _ => {}
        }
        offset += 1;
    }
}

/// Parse `name(args)` normal Sass/CSS function call.
pub(super) fn function_call(p: &mut Parser<'_>, ctx: ParseContext) -> CompletedMarker {
    let m = p.start();
    p.bump(); // IDENT (function name)
    arg_list(p, ctx);
    m.complete(p, FUNCTION_CALL)
}

/// Parse argument list `(...)` for function calls and `@include`.
pub(crate) fn arg_list(p: &mut Parser<'_>, ctx: ParseContext) {
    assert!(p.at(LPAREN));
    let m = p.start();
    p.bump(); // (

    if !p.at(RPAREN) && !p.at_end() {
        arg(p, ctx);
        while p.eat(COMMA) {
            if !p.at(RPAREN) && !p.at_end() {
                arg(p, ctx);
            }
        }
    }

    p.expect(RPAREN);
    let _ = m.complete(p, ARG_LIST);
}

/// Parse a single argument: positional, keyword (`$name: value`), or splat (`$list...`).
fn arg(p: &mut Parser<'_>, ctx: ParseContext) {
    let m = p.start();

    // Check for keyword argument: `$name: value`
    if p.at(DOLLAR) && p.nth(1) == IDENT && p.nth(2) == COLON {
        p.bump(); // $
        p.bump(); // name
        p.bump(); // :
        sass_value(p, ctx);
        let _ = m.complete(p, ARG);
        return;
    }

    // Positional argument (may be space-separated: `func(1px 2px, 3px)`)
    sass_value(p, ctx);

    // Check for splat: `$list...`
    if p.at(DOT_DOT_DOT) {
        p.bump();
    }

    let _ = m.complete(p, ARG);
}

// ── Special functions ──────────────────────────────────────────────

/// Parse `url(...)`, `element(...)`, `progid:...(...)`.
/// For `url("quoted")`, falls through to normal function call.
fn special_function_call(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();
    p.bump(); // function name

    if !p.at(LPAREN) {
        // Not a call — just an ident
        return m.complete(p, VALUE);
    }

    p.bump(); // (

    // Consume everything inside as opaque tokens until matching `)`
    let mut depth: u32 = 1;
    while !p.at_end() && depth > 0 {
        match p.current() {
            LPAREN => {
                depth += 1;
                p.bump();
            }
            RPAREN => {
                depth -= 1;
                if depth > 0 {
                    p.bump();
                }
            }
            HASH_LBRACE => {
                let _ = super::super::selectors::interpolation(p);
            }
            _ => p.bump(),
        }
    }
    if depth == 0 {
        p.bump(); // final )
    }
    m.complete(p, SPECIAL_FUNCTION_CALL)
}
