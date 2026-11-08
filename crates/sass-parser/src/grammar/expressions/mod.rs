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
    MINUS, PLUS, PERCENT, BANG, AMP, SLASH,
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
    while !at_value_end(p)
        && !p.at(COMMA)
        && !p.at(RPAREN)
        && !p.at(RBRACKET)
        && !p.at_unclosed_string_break()
    {
        if p.at(SLASH) {
            // CSS separator: `11px/1.5`, `font: 12px/1.4 sans-serif`
            p.bump();
            p.continue_line_to(EXPR_START);
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
    // A comma that ends its line ends the list there, as a trailing comma: the line
    // break goes on only inside brackets, where it is plain whitespace.
    while p.eat(COMMA) {
        if !at_value_end(p) && !p.at_lines_beneath() {
            sass_value(p, ctx);
        }
    }
    Some(m.complete(p, LIST_EXPR))
}

/// The comma between the items of a parenthesized or bracketed list, a map or an argument
/// list, or the line break after a string left unclosed there, which swallowed it.
pub(super) fn eat_comma(p: &mut Parser<'_>) -> bool {
    p.eat(COMMA) || p.eat_unclosed_string_break()
}

fn at_value_end(p: &Parser<'_>) -> bool {
    p.at(SEMICOLON) || p.at(RBRACE) || p.at(BANG) || p.at_end()
}

// ── Pratt parser core ──────────────────────────────────────────────

fn expr_bp(p: &mut Parser<'_>, min_bp: u8, ctx: ParseContext) -> Option<CompletedMarker> {
    let Ok(mut g) = p.depth_guard() else {
        skip_too_deep(p);
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
            // A line break after the operator counts, where the line goes on.
            let joined = g.joined(super::lines::LINE_JOINS);
            let next_has_ws = g.nth_has_whitespace_before_joined(joined, 1);
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
        super::lines::bump_operator(&mut g);
        expr_bp(&mut g, r_bp, ctx);
        lhs = m.complete(&mut g, BINARY_EXPR);
    }

    Some(lhs)
}

/// At the nesting limit, which has just been reported: the rest of the operand that went
/// too deep, with the brackets it opens, as one error, up to what ends it at the depth it
/// started at — a `,`, a closer of a bracket opened above, or the end of the statement.
/// The brackets left open above it then end there with no error of their own.
pub(crate) fn skip_too_deep(p: &mut Parser<'_>) {
    let m = p.start();
    let mut open: Vec<SyntaxKind> = Vec::new();
    while !p.at_end() && !p.at_virtual() && !p.at_unclosed_string_break() {
        let closer = match p.current() {
            LPAREN => Some(RPAREN),
            LBRACKET => Some(RBRACKET),
            HASH_LBRACE => Some(RBRACE),
            _ => None,
        };
        if let Some(closer) = closer {
            open.push(closer);
        } else {
            match p.current() {
                kind @ (RPAREN | RBRACKET | RBRACE) if open.last() == Some(&kind) => {
                    open.pop();
                }
                RPAREN | RBRACKET | RBRACE | LBRACE | SEMICOLON => break,
                COMMA if open.is_empty() => break,
                _ => {}
            }
        }
        p.bump();
    }
    let _ = m.complete(p, ERROR);
    p.quiet_here();
}

// ── LHS: prefix operators + atoms ──────────────────────────────────

fn lhs(p: &mut Parser<'_>, ctx: ParseContext) -> Option<CompletedMarker> {
    let kind = p.current();
    let text = p.current_text();

    if let Some(r_bp) = prefix_bp(kind, text, ctx) {
        let m = p.start();
        p.bump(); // operator
        p.continue_line_to(EXPR_START);
        expr_bp(p, r_bp, ctx);
        return Some(m.complete(p, UNARY_EXPR));
    }

    atoms::atom(p, ctx)
}

// ── Variable declarations ──────────────────────────────────────────

/// Parse `$var: expr !default? !global?;`
pub fn variable_declaration(p: &mut Parser<'_>) {
    let m = p.start();
    let errors = p.error_count();
    // Only a declaration that starts a statement goes on across lines.
    let statement = p.at_statement_start();
    p.bump(); // $
    if p.expect(IDENT) && statement {
        p.continue_line_across_blank();
    }
    if p.expect(COLON) && statement {
        p.continue_line_across_blank();
    }

    let value_errors = p.error_count();
    sass_value_list(p, ParseContext::SassScript);
    if p.error_count() > value_errors && skip_rest_of_line(p) {
        let _ = m.complete(p, VARIABLE_DECL);
        return;
    }

    // Parse flags: !default, !global (can co-occur)
    while p.at(BANG) {
        let joined = p.joined(super::lines::LINE_JOINS);
        let flag_text = if p.nth_joined(joined, 1) == IDENT {
            p.nth_text_joined(joined, 1)
        } else {
            ""
        };
        if flag_text == "default" || flag_text == "global" {
            let fm = p.start();
            p.bump(); // !
            p.continue_line();
            p.bump(); // default/global
            let _ = fm.complete(p, SASS_FLAG);
        } else {
            break;
        }
    }

    super::lines::end_without_block(p, errors, "a variable declaration");
    let _ = m.complete(p, VARIABLE_DECL);
}

/// After a value that reported an error where it could not be read, such as a bracket
/// left unclosed, braced: the rest of the statement on its line, as one error, up to the
/// `;` or `}` that ends it there, or else to the line break, unless a `{`, a flag or a
/// bracket, which may go on below, comes first. The next line most likely starts the next
/// statement, `$m: (` / `a: 1` / `b: 2` / `.c {`; the `;` missing before it is that same
/// mistake. Returns whether it skipped to such a line. The indented layout ends the
/// statement at the line itself.
fn skip_rest_of_line(p: &mut Parser<'_>) -> bool {
    if !p.braced() || p.at_line_break() {
        return false;
    }
    let mut count = 0;
    let to_line = loop {
        match p.nth(count) {
            SEMICOLON | RBRACE => break false,
            LBRACE | BANG | EOF | LPAREN | LBRACKET | HASH_LBRACE | STRING_START => {
                return false;
            }
            _ => {}
        }
        count += 1;
        if p.nth_at_line_break(count) {
            break true;
        }
    };
    if count > 0 {
        let m = p.start();
        for _ in 0..count {
            p.bump();
        }
        let _ = m.complete(p, ERROR);
    }
    to_line
}

// ── Interpolation (Phase 3 upgrade) ────────────────────────────────

/// Parse `#{expr}` with fully-parsed inner expression.
/// Supports space/comma-separated lists: `#{transform $dur ease-in-out}`.
pub fn interpolation(p: &mut Parser<'_>) -> CompletedMarker {
    assert!(p.at(HASH_LBRACE));
    let m = p.start();
    let unclosed_strings = p.unclosed_strings();
    {
        let mut g = p.transparent_guard();
        let errors = g.error_count();
        g.bump(); // #{
        if g.at(RBRACE) {
            g.error("expected expression");
        } else if !g.at_end() {
            sass_value_list(&mut g, ParseContext::SassScript);
        }
        // A string left unclosed inside ended the interpolation too, already reported.
        if g.unclosed_strings() == unclosed_strings {
            crate::grammar::close_bracket(&mut g, RBRACE, errors);
        }
    }
    m.complete(p, INTERPOLATION)
}
