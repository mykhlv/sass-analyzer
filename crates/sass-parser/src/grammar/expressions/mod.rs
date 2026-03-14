mod atoms;
mod calculations;
pub(super) mod functions;

pub(crate) use atoms::interpolated_string;
pub(crate) use functions::arg_list;

use crate::parser::{CompletedMarker, Parser};
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;
use crate::token_set::TokenSet;

use super::ParseContext;

/// Tokens that can start an expression atom.
#[rustfmt::skip]
pub const EXPR_START: TokenSet = TokenSet::new(&[
    NUMBER, QUOTED_STRING, STRING_START, HASH, HASH_LBRACE, IDENT, DOLLAR, LPAREN, LBRACKET,
    MINUS, PLUS, PERCENT, BANG, AMP,
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

/// Parse a space-separated group of expressions in `SassScript` context.
/// Stops at `;`, `}`, `!`, `,`, `)`, or EOF.
/// Extra values become siblings (no wrapper node).
pub(crate) fn sass_value(p: &mut Parser<'_>, ctx: ParseContext) -> Option<CompletedMarker> {
    let cm = expr(p, ctx)?;
    while !at_value_end(p) && !p.at(COMMA) && !p.at(RPAREN) && !p.at(RBRACKET) {
        if p.at(SLASH) {
            // CSS separator: `11px/1.5`, `font: 12px/1.4 sans-serif`
            p.bump();
        } else if p.at_ts(EXPR_START) {
            expr(p, ctx);
        } else {
            break;
        }
    }
    Some(cm)
}

/// Parse a comma-separated list of space-separated groups.
/// `$x: 1px 2px, 3px 4px;` → `LIST_EXPR` containing both groups separated by COMMA.
/// Single values or space-only groups return without wrapper.
pub(crate) fn sass_value_list(p: &mut Parser<'_>, ctx: ParseContext) -> Option<CompletedMarker> {
    let first = sass_value(p, ctx)?;
    if !p.at(COMMA) {
        return Some(first);
    }
    let m = first.precede(p);
    while p.eat(COMMA) {
        if !at_value_end(p) {
            sass_value(p, ctx);
        }
    }
    Some(m.complete(p, LIST_EXPR))
}

fn at_value_end(p: &Parser<'_>) -> bool {
    p.at(SEMICOLON) || p.at(RBRACE) || p.at(BANG) || p.at_end()
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

    atoms::atom(p, ctx)
}

// ── Variable declarations ──────────────────────────────────────────

/// Parse `$var: expr !default? !global?;`
pub fn variable_declaration(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(); // $
    p.expect(IDENT);
    p.expect(COLON);

    sass_value_list(p, ParseContext::SassScript);

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
/// Supports space/comma-separated lists: `#{transform $dur ease-in-out}`.
pub fn interpolation(p: &mut Parser<'_>) -> CompletedMarker {
    assert!(p.at(HASH_LBRACE));
    let m = p.start();
    p.bump(); // #{
    if p.at(RBRACE) {
        p.error("expected expression");
    } else if !p.at_end() {
        sass_value_list(p, ParseContext::SassScript);
    }
    p.expect(RBRACE);
    m.complete(p, INTERPOLATION)
}
