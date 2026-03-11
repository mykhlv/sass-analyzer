use crate::parser::{CompletedMarker, Parser};
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;

use super::super::ParseContext;
use super::{expr, interpolation, sass_value};

// ── Atoms ──────────────────────────────────────────────────────────

pub(super) fn atom(p: &mut Parser<'_>, ctx: ParseContext) -> Option<CompletedMarker> {
    match p.current() {
        NUMBER => Some(number_or_dimension(p)),
        QUOTED_STRING => Some(quoted_string(p)),
        STRING_START => Some(interpolated_string(p, ctx)),
        HASH => Some(color_literal(p)),
        HASH_LBRACE => Some(interpolation_atom(p)),
        DOLLAR => Some(variable_ref(p)),
        IDENT => ident_or_call(p, ctx),
        LPAREN => Some(paren_or_map(p, ctx)),
        LBRACKET => Some(bracketed_list(p, ctx)),
        PERCENT => Some(standalone_percent(p)),
        BANG => Some(bang_dispatch(p)),
        AMP => {
            // Parent selector in expression context: `if(&, "&", "")`
            let m = p.start();
            p.bump();
            Some(m.complete(p, VALUE))
        }
        _ => {
            p.error("expected expression");
            None
        }
    }
}

/// `NUMBER` optionally followed by adjacent `IDENT` (unit) → `DIMENSION`.
pub(super) fn number_or_dimension(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();
    p.bump(); // NUMBER
    // Adjacent IDENT without whitespace = unit (e.g., `10px`, `2em`, `100%`)
    if p.at(IDENT) && !p.has_whitespace_before() {
        p.bump(); // unit
        m.complete(p, DIMENSION)
    } else if p.at(PERCENT) && !p.has_whitespace_before() {
        p.bump(); // %
        m.complete(p, DIMENSION)
    } else {
        m.complete(p, NUMBER_LITERAL)
    }
}

fn quoted_string(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();
    p.bump(); // QUOTED_STRING
    m.complete(p, STRING_LITERAL)
}

pub(crate) fn interpolated_string(p: &mut Parser<'_>, _ctx: ParseContext) -> CompletedMarker {
    let m = p.start();
    p.bump(); // STRING_START
    // Token sequence: STRING_START (HASH_LBRACE expr RBRACE (STRING_MID | STRING_END))*
    loop {
        match p.current() {
            HASH_LBRACE => {
                let _ = interpolation(p);
            }
            STRING_MID => p.bump(),
            STRING_END => {
                p.bump();
                break;
            }
            _ => {
                if p.at_end() {
                    p.error("unterminated interpolated string");
                } else {
                    p.error("expected string content or interpolation");
                }
                break;
            }
        }
    }
    m.complete(p, INTERPOLATED_STRING)
}

fn color_literal(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();
    p.bump(); // HASH
    // Hex color: `#` followed by NUMBER/IDENT tokens without whitespace.
    // e.g. #333 → HASH NUMBER, #fff → HASH IDENT,
    //      #3498db → HASH NUMBER IDENT, #00ff00 → HASH NUMBER IDENT
    while !p.has_whitespace_before() && (p.at(IDENT) || p.at(NUMBER)) {
        p.bump();
    }
    m.complete(p, COLOR_LITERAL)
}

/// `$var` in expression position → `VARIABLE_REF`.
pub(super) fn variable_ref(p: &mut Parser<'_>) -> CompletedMarker {
    assert!(p.at(DOLLAR));
    let m = p.start();
    p.bump(); // $
    p.expect(IDENT);
    m.complete(p, VARIABLE_REF)
}

/// Dispatch IDENT: namespace member, boolean/null literal, function call, or plain identifier.
fn ident_or_call(p: &mut Parser<'_>, ctx: ParseContext) -> Option<CompletedMarker> {
    let text = p.current_text();

    // Namespace member access: ns.$var or ns.func()
    if p.nth(1) == DOT && !p.nth_has_whitespace_before(1) && !p.nth_has_whitespace_before(2) {
        if p.nth(2) == DOLLAR && p.nth(3) == IDENT {
            let m = p.start();
            p.bump(); // IDENT (namespace)
            p.bump(); // DOT
            p.bump(); // DOLLAR
            p.bump(); // IDENT (variable name)
            return Some(m.complete(p, NAMESPACE_REF));
        }
        if p.nth(2) == IDENT && p.nth(3) == LPAREN {
            let m = p.start();
            p.bump(); // IDENT (namespace)
            p.bump(); // DOT
            let _ = super::functions::function_call(p, ctx);
            return Some(m.complete(p, NAMESPACE_REF));
        }
    }

    // Boolean and null literals
    if text == "true" || text == "false" {
        let m = p.start();
        p.bump();
        return Some(m.complete(p, BOOL_LITERAL));
    }
    if text == "null" {
        let m = p.start();
        p.bump();
        return Some(m.complete(p, NULL_LITERAL));
    }

    // In SassScript, `and`/`or` are infix operators — don't consume as atom
    if ctx == ParseContext::SassScript && (text == "and" || text == "or") {
        return None;
    }

    // Check for function call: IDENT immediately followed by LPAREN (no whitespace)
    if p.nth(1) == LPAREN && !p.nth_has_whitespace_before(1) {
        return Some(super::functions::function_dispatch(p, ctx));
    }

    // Plain identifier — possibly followed by interpolation: `--color-#{$name}`
    let m = p.start();
    p.bump();
    // Consume adjacent interpolation + fragments: `--color-#{$name}-suffix`
    if !p.at_end() && !p.has_whitespace_before() && p.at(HASH_LBRACE) {
        let _ = interpolation(p);
        while !p.at_end() && !p.has_whitespace_before() {
            if p.at(MINUS) || p.at(IDENT) || p.at(NUMBER) {
                p.bump();
            } else if p.at(HASH_LBRACE) {
                let _ = interpolation(p);
            } else {
                break;
            }
        }
    }
    Some(m.complete(p, VALUE))
}

fn standalone_percent(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();
    p.bump(); // %
    m.complete(p, STANDALONE_PERCENT)
}

fn bang_dispatch(p: &mut Parser<'_>) -> CompletedMarker {
    // !important, !default, !global, !optional
    let m = p.start();
    p.bump(); // !
    if p.at(IDENT) {
        let text = p.current_text();
        if text == "important" {
            p.bump();
            return m.complete(p, IMPORTANT);
        }
        if text == "default" || text == "global" || text == "optional" {
            p.bump();
            return m.complete(p, SASS_FLAG);
        }
    }
    p.error("expected `important`, `default`, `global`, or `optional` after `!`");
    m.complete(p, ERROR)
}

// ── Parenthesized expr / map ───────────────────────────────────────

fn paren_or_map(p: &mut Parser<'_>, ctx: ParseContext) -> CompletedMarker {
    assert!(p.at(LPAREN));
    let m = p.start();
    p.bump(); // (

    // Empty parens: `()` = empty list
    if p.eat(RPAREN) {
        return m.complete(p, LIST_EXPR);
    }

    let first = expr(p, ParseContext::SassScript);

    // After first expression: `:` → map, `,` → list, `)` → paren expr
    if p.at(COLON) {
        // It's a map: (key: value, ...)
        return finish_map(p, m, first, ctx);
    }

    // Space-separated values inside parens: `(28px 28px 0 0)`, `(small medium large)`
    if !p.at(COMMA) && !p.at(RPAREN) && !p.at_end() {
        while !p.at(COMMA) && !p.at(RPAREN) && !p.at_end() {
            if expr(p, ParseContext::SassScript).is_none() {
                break;
            }
        }
    }

    if p.at(COMMA) {
        // Comma-separated list inside parens (possibly of space-separated groups)
        while p.eat(COMMA) {
            if !p.at(RPAREN) && !p.at_end() {
                sass_value(p, ParseContext::SassScript);
            }
        }
        p.expect(RPAREN);
        return m.complete(p, LIST_EXPR);
    }

    p.expect(RPAREN);
    m.complete(p, PAREN_EXPR)
}

fn finish_map(
    p: &mut Parser<'_>,
    outer: crate::parser::Marker,
    first_key: Option<CompletedMarker>,
    _ctx: ParseContext,
) -> CompletedMarker {
    // Wrap already-parsed key expression in MAP_ENTRY
    let entry_m = if let Some(key) = first_key {
        key.precede(p)
    } else {
        p.start()
    };
    p.expect(COLON);
    sass_value(p, ParseContext::SassScript);
    let _ = entry_m.complete(p, MAP_ENTRY);

    // Parse remaining entries
    while p.eat(COMMA) {
        if p.at(RPAREN) || p.at_end() {
            break; // trailing comma
        }
        let em = p.start();
        expr(p, ParseContext::SassScript);
        p.expect(COLON);
        sass_value(p, ParseContext::SassScript);
        let _ = em.complete(p, MAP_ENTRY);
    }

    p.expect(RPAREN);
    outer.complete(p, MAP_EXPR)
}

// ── Bracketed list ─────────────────────────────────────────────────

fn bracketed_list(p: &mut Parser<'_>, ctx: ParseContext) -> CompletedMarker {
    assert!(p.at(LBRACKET));
    let m = p.start();
    p.bump(); // [
    if !p.at(RBRACKET) && !p.at_end() {
        sass_value(p, ctx);
        while p.eat(COMMA) {
            if !p.at(RBRACKET) && !p.at_end() {
                sass_value(p, ctx);
            }
        }
    }
    p.expect(RBRACKET);
    m.complete(p, BRACKETED_LIST)
}

/// Parse interpolation as an expression atom, consuming adjacent hyphenated fragments.
/// `#{$key}-font` → single VALUE node (not `#{$key} - font` subtraction).
fn interpolation_atom(p: &mut Parser<'_>) -> CompletedMarker {
    let cm = interpolation(p);
    // Consume adjacent fragments without whitespace: `-font`, `-#{...}`, `3`, etc.
    if !p.at_end() && !p.has_whitespace_before() && (p.at(MINUS) || p.at(IDENT) || p.at(NUMBER)) {
        let m = cm.precede(p);
        while !p.at_end() && !p.has_whitespace_before() {
            if p.at(MINUS) || p.at(IDENT) || p.at(NUMBER) {
                p.bump();
            } else if p.at(HASH_LBRACE) {
                let _ = interpolation(p);
            } else {
                break;
            }
        }
        m.complete(p, VALUE)
    } else {
        cm
    }
}
