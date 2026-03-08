use crate::parser::{CompletedMarker, Parser};
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;
use crate::token_set::TokenSet;

use super::ParseContext;

/// Tokens that can start an expression atom.
#[rustfmt::skip]
pub const EXPR_START: TokenSet = TokenSet::new(&[
    NUMBER, QUOTED_STRING, STRING_START, HASH, HASH_LBRACE, IDENT, DOLLAR, LPAREN, LBRACKET,
    MINUS, PLUS, PERCENT, BANG,
]);

// ── Pratt binding power table ──────────────────────────────────────
//
// Priority  Operator     Assoc   Left BP  Right BP
// 1         or           left    1        2
// 2         and          left    3        4
// 3         == !=        left    5        6
// 4         < > <= >=    left    7        8
// 5         + -          left    9        10
// 6         * / %        left    11       12
// 7 (pfx)   - + not      prefix  -        13

fn prefix_bp(kind: SyntaxKind, text: &str, ctx: ParseContext) -> Option<u8> {
    match kind {
        MINUS | PLUS => Some(13),
        // `not` is only a prefix operator in SassScript; in CssValue it's a plain ident
        IDENT if text == "not" && ctx == ParseContext::SassScript => Some(13),
        _ => None,
    }
}

fn infix_bp(kind: SyntaxKind, text: &str, ctx: ParseContext) -> Option<(u8, u8)> {
    match kind {
        // `or`/`and` are infix operators only in SassScript; plain idents in CssValue
        IDENT if text == "or" && ctx == ParseContext::SassScript => Some((1, 2)),
        IDENT if text == "and" && ctx == ParseContext::SassScript => Some((3, 4)),
        EQ_EQ | BANG_EQ => Some((5, 6)),
        LT | GT | LT_EQ | GT_EQ => Some((7, 8)),
        PLUS | MINUS => Some((9, 10)),
        STAR => Some((11, 12)),
        SLASH => match ctx {
            ParseContext::SassScript | ParseContext::Calculation => Some((11, 12)),
            ParseContext::CssValue | ParseContext::SpecialFunction => None,
        },
        PERCENT => match ctx {
            ParseContext::SassScript => Some((11, 12)),
            _ => None,
        },
        _ => None,
    }
}

// ── Public entry point ─────────────────────────────────────────────

/// Parse an expression in the given context.
pub fn expr(p: &mut Parser<'_>, ctx: ParseContext) -> Option<CompletedMarker> {
    expr_bp(p, 0, ctx)
}

/// Parse a comma-separated list of expressions (for property values).
/// Returns `Some` if at least one expression was parsed.
pub fn expr_list(p: &mut Parser<'_>, ctx: ParseContext) -> Option<CompletedMarker> {
    let first = expr(p, ctx)?;
    if !p.at(COMMA) {
        return Some(first);
    }
    let m = first.precede(p);
    while p.eat(COMMA) {
        if p.at_ts(EXPR_START) {
            expr(p, ctx);
        }
    }
    Some(m.complete(p, LIST_EXPR))
}

// ── Pratt parser core ──────────────────────────────────────────────

fn expr_bp(p: &mut Parser<'_>, min_bp: u8, ctx: ParseContext) -> Option<CompletedMarker> {
    let Ok(mut g) = p.depth_guard() else {
        return None;
    };

    let mut lhs = lhs(&mut g, ctx)?;

    loop {
        let kind = g.current();
        let text = g.current_text();

        // Whitespace disambiguation for unary -/+ (task 3.7):
        // `$a -$b` (space before, no space after) → not infix, stop
        if (kind == MINUS || kind == PLUS) && g.has_whitespace_before() {
            // Check if there's NO whitespace after: `$a -$b`
            // In that case, `-` is unary prefix of the next token, not infix.
            // We peek: if next-next token has no whitespace before it relative to `-`,
            // then it's a unary operator binding to the next token.
            let next_has_ws = g.nth_has_whitespace_before(1);
            if !next_has_ws {
                break;
            }
        }

        let Some((l_bp, r_bp)) = infix_bp(kind, text, ctx) else {
            break;
        };
        if l_bp < min_bp {
            break;
        }

        let m = lhs.precede(&mut g);
        g.bump(); // operator
        expr_bp(&mut g, r_bp, ctx);
        lhs = m.complete(&mut g, BINARY_EXPR);
    }

    Some(lhs)
}

// ── LHS: prefix operators + atoms ──────────────────────────────────

fn lhs(p: &mut Parser<'_>, ctx: ParseContext) -> Option<CompletedMarker> {
    let kind = p.current();
    let text = p.current_text();

    if let Some(r_bp) = prefix_bp(kind, text, ctx) {
        let m = p.start();
        p.bump(); // operator
        expr_bp(p, r_bp, ctx);
        return Some(m.complete(p, UNARY_EXPR));
    }

    atom(p, ctx)
}

// ── Atoms ──────────────────────────────────────────────────────────

fn atom(p: &mut Parser<'_>, ctx: ParseContext) -> Option<CompletedMarker> {
    match p.current() {
        NUMBER => Some(number_or_dimension(p)),
        QUOTED_STRING => Some(quoted_string(p)),
        STRING_START => Some(interpolated_string(p, ctx)),
        HASH => Some(color_literal(p)),
        HASH_LBRACE => Some(interpolation(p)),
        DOLLAR => Some(variable_ref(p)),
        IDENT => ident_or_call(p, ctx),
        LPAREN => Some(paren_or_map(p, ctx)),
        LBRACKET => Some(bracketed_list(p, ctx)),
        PERCENT => Some(standalone_percent(p)),
        BANG => Some(bang_dispatch(p)),
        _ => {
            p.error("expected expression");
            None
        }
    }
}

/// `NUMBER` optionally followed by adjacent `IDENT` (unit) → `DIMENSION`.
fn number_or_dimension(p: &mut Parser<'_>) -> CompletedMarker {
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
fn variable_ref(p: &mut Parser<'_>) -> CompletedMarker {
    assert!(p.at(DOLLAR));
    let m = p.start();
    p.bump(); // $
    p.expect(IDENT);
    m.complete(p, VARIABLE_REF)
}

/// Dispatch IDENT: boolean/null literal, function call, or plain identifier.
fn ident_or_call(p: &mut Parser<'_>, ctx: ParseContext) -> Option<CompletedMarker> {
    let text = p.current_text();

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
        return Some(function_dispatch(p, ctx));
    }

    // Plain identifier (CSS value keyword, color name, etc.)
    let m = p.start();
    p.bump();
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

    if p.at(COMMA) {
        // Comma-separated list inside parens
        while p.eat(COMMA) {
            if !p.at(RPAREN) && !p.at_end() {
                expr(p, ParseContext::SassScript);
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
    expr(p, ParseContext::SassScript);
    let _ = entry_m.complete(p, MAP_ENTRY);

    // Parse remaining entries
    while p.eat(COMMA) {
        if p.at(RPAREN) || p.at_end() {
            break; // trailing comma
        }
        let em = p.start();
        expr(p, ParseContext::SassScript);
        p.expect(COLON);
        expr(p, ParseContext::SassScript);
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
        expr(p, ctx);
        while p.eat(COMMA) {
            if !p.at(RBRACKET) && !p.at_end() {
                expr(p, ctx);
            }
        }
    }
    p.expect(RBRACKET);
    m.complete(p, BRACKETED_LIST)
}

// ── Function calls ─────────────────────────────────────────────────

/// Calculation function names recognized by the dispatcher.
#[rustfmt::skip]
const CALC_NAMES: &[&str] = &[
    "calc", "min", "max", "clamp", "round", "mod", "rem",
    "sin", "cos", "tan", "asin", "acos", "atan", "atan2",
    "pow", "sqrt", "hypot", "log", "exp", "abs", "sign",
];

/// Dispatch function call by name.
fn function_dispatch(p: &mut Parser<'_>, ctx: ParseContext) -> CompletedMarker {
    let name = p.current_text();

    // Special functions: url(), element(), progid:...()
    if name.eq_ignore_ascii_case("url") {
        return special_function_call(p);
    }
    if name.eq_ignore_ascii_case("element") || name.starts_with("progid:") {
        return special_function_call(p);
    }

    // Calculation functions
    if CALC_NAMES.iter().any(|n| name.eq_ignore_ascii_case(n)) {
        // min()/max() with SassScript-only features → fall back to normal call
        // For now, always parse as calculation; Phase 3.10 refinement handles fallback
        return calculation(p);
    }

    function_call(p, ctx)
}

/// Parse `name(args)` normal Sass/CSS function call.
fn function_call(p: &mut Parser<'_>, ctx: ParseContext) -> CompletedMarker {
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
        expr(p, ctx);
        let _ = m.complete(p, ARG);
        return;
    }

    // Positional argument
    expr(p, ctx);

    // Check for splat: `$list...`
    if p.at(DOT_DOT_DOT) {
        p.bump();
    }

    let _ = m.complete(p, ARG);
}

// ── Calculation functions ──────────────────────────────────────────

/// Parse a calculation function: `calc(...)`, `min(...)`, `max(...)`, etc.
fn calculation(p: &mut Parser<'_>) -> CompletedMarker {
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
            // Nested function call inside calc (e.g., `calc(min(10px, 5vw) + 1rem)`)
            if g.nth(1) == LPAREN && !g.nth_has_whitespace_before(1) {
                let name = g.current_text();
                if CALC_NAMES.iter().any(|n| name.eq_ignore_ascii_case(n)) {
                    let _ = calculation(&mut g);
                    m.abandon(&mut g);
                    return;
                }
            }
            // Plain ident (e.g., env(safe-area-inset-top))
            g.bump();
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
                let _ = super::selectors::interpolation(p);
            }
            _ => p.bump(),
        }
    }
    if depth == 0 {
        p.bump(); // final )
    }
    m.complete(p, SPECIAL_FUNCTION_CALL)
}

// ── Variable declarations ──────────────────────────────────────────

/// Parse `$var: expr !default? !global?;`
pub fn variable_declaration(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // $
    p.expect(IDENT);
    p.expect(COLON);

    expr(p, ParseContext::SassScript);

    // Parse flags: !default, !global (can co-occur)
    while p.at(BANG) {
        let flag_text = if p.nth(1) == IDENT { p.nth_text(1) } else { "" };
        if flag_text == "default" || flag_text == "global" {
            let fm = p.start();
            p.bump(); // !
            p.bump(); // default/global
            let _ = fm.complete(p, SASS_FLAG);
        } else {
            break;
        }
    }

    if !p.at(RBRACE) && !p.at_end() {
        p.expect(SEMICOLON);
    }
    let _ = m.complete(p, VARIABLE_DECL);
}

// ── Interpolation (Phase 3 upgrade) ────────────────────────────────

/// Parse `#{expr}` with fully-parsed inner expression.
/// Replaces Phase 2's opaque `interpolation()`.
pub fn interpolation(p: &mut Parser<'_>) -> CompletedMarker {
    assert!(p.at(HASH_LBRACE));
    let m = p.start();
    p.bump(); // #{
    if !p.at(RBRACE) && !p.at_end() {
        expr(p, ParseContext::SassScript);
    }
    p.expect(RBRACE);
    m.complete(p, INTERPOLATION)
}
