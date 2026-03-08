pub(crate) mod at_rules;
mod declarations;
pub mod expressions;
pub(crate) mod selectors;

use crate::parser::Parser;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;
use crate::token_set::TokenSet;

/// Parsing context — drives disambiguation for `/`, `%`, `min()`/`max()`, etc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseContext {
    /// Inside variable declarations, function bodies, `@if` conditions, mixin args.
    /// `/` is division, `%` is modulo.
    SassScript,
    /// Inside CSS property values.
    /// `/` is a separator (unless heuristics say otherwise), `%` can be a unit.
    CssValue,
    /// Inside `calc()`, `min()`, `max()`, `clamp()`, etc.
    /// `/` is always division, Sass variables allowed, `%` modulo NOT allowed.
    Calculation,
    /// Inside `url()`, `element()`, `progid:...()`.
    /// Content is mostly opaque.
    SpecialFunction,
}

/// Tokens that can start a top-level statement (rule, declaration, at-rule).
#[rustfmt::skip]
pub const STMT_RECOVERY: TokenSet = TokenSet::new(&[
    IDENT, DOT, HASH, COLON, COLON_COLON, LBRACKET, AMP, PERCENT, STAR,
    AT, DOLLAR, RBRACE, SEMICOLON,
]);

/// Tokens that terminate a block — used to stop error recovery inside blocks.
#[rustfmt::skip]
pub const BLOCK_RECOVERY: TokenSet = TokenSet::new(&[
    RBRACE, SEMICOLON,
]);

/// Parse `SourceFile` — top-level sequence of items with error recovery.
pub fn source_file(p: &mut Parser<'_>) {
    let m = p.start();
    while !p.at_end() {
        if p.at(AT) {
            at_rules::at_rule(p);
        } else if p.at(DOLLAR) {
            expressions::variable_declaration(p);
        } else if p.at_ts(selectors::SELECTOR_START) {
            rule_set(p);
        } else if p.at(SEMICOLON) {
            p.bump();
        } else {
            let m = p.start();
            p.error("expected rule");
            p.bump();
            while !p.at_end()
                && !p.at(AT)
                && !p.at_ts(selectors::SELECTOR_START)
                && !p.at(SEMICOLON)
                && !p.at(DOLLAR)
            {
                p.bump();
            }
            let _ = m.complete(p, SyntaxKind::ERROR);
        }
    }
    let _ = m.complete(p, SOURCE_FILE);
}

/// Parse a single item inside a block.
/// Disambiguates between rule sets and declarations.
fn block_item(p: &mut Parser<'_>) {
    if p.at(AT) {
        at_rules::at_rule(p);
    } else if p.at(DOLLAR) {
        expressions::variable_declaration(p);
    } else if looks_like_declaration(p) {
        declarations::declaration(p);
    } else if p.at_ts(selectors::SELECTOR_START) {
        rule_set(p);
    } else {
        p.err_and_bump("expected declaration or nested rule");
    }
}

/// Lookahead scan to decide if current position starts a declaration.
///
/// Recognizes:
/// - `IDENT COLON ...` — plain property, scan for `{`/`;`/`}`
/// - `IDENT COLON LBRACE` — nested property
/// - `HASH_LBRACE ... RBRACE [fragments] COLON` — interpolated property name
fn looks_like_declaration(p: &Parser<'_>) -> bool {
    if p.at(HASH_LBRACE) {
        return looks_like_interpolated_declaration(p);
    }
    if !p.at(IDENT) || p.nth(1) != COLON {
        return false;
    }
    // 2.10: IDENT COLON LBRACE → nested property
    if p.nth(2) == LBRACE {
        return true;
    }
    scan_for_declaration_end(p, 2)
}

/// Check if `#{...}[fragments]COLON` looks like a declaration with interpolated property.
fn looks_like_interpolated_declaration(p: &Parser<'_>) -> bool {
    // Skip past the interpolation and any trailing property-name fragments
    let mut offset = 1; // past HASH_LBRACE
    let mut depth: u32 = 1;
    // Skip interpolation body
    loop {
        match p.nth(offset) {
            EOF => return false,
            LBRACE | HASH_LBRACE => depth += 1,
            RBRACE => {
                depth -= 1;
                if depth == 0 {
                    offset += 1;
                    break;
                }
            }
            _ => {}
        }
        offset += 1;
    }
    // Skip trailing property fragments (IDENT, MINUS, more interpolations)
    loop {
        match p.nth(offset) {
            IDENT | MINUS => offset += 1,
            HASH_LBRACE => {
                // Skip another interpolation
                offset += 1;
                let mut d: u32 = 1;
                loop {
                    match p.nth(offset) {
                        EOF => return false,
                        LBRACE | HASH_LBRACE => d += 1,
                        RBRACE => {
                            d -= 1;
                            if d == 0 {
                                offset += 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    offset += 1;
                }
            }
            _ => break,
        }
    }
    // After property name fragments, expect COLON
    p.nth(offset) == COLON
}

/// Scan tokens from `offset` looking for `{`, `;`, or `}` at depth 0.
/// Returns `true` if `;`/`}`/EOF found first (declaration), `false` if `{` found (selector).
fn scan_for_declaration_end(p: &Parser<'_>, start: usize) -> bool {
    let mut depth: u32 = 0;
    let mut offset = start;
    loop {
        match p.nth(offset) {
            EOF => return true,
            LBRACE if depth == 0 => return false,
            RBRACE if depth == 0 => return true,
            SEMICOLON if depth == 0 => return true,
            LPAREN | LBRACKET => depth += 1,
            RPAREN | RBRACKET => depth = depth.saturating_sub(1),
            _ => {}
        }
        offset += 1;
    }
}

/// 2.5: Parse rule set — selector list + `{` block `}`.
fn rule_set(p: &mut Parser<'_>) {
    let Ok(mut g) = p.depth_guard() else {
        // Skip past this construct to prevent infinite loops at depth limit
        skip_until_block_end(p);
        return;
    };
    let m = g.start();
    selectors::selector_list(&mut g);
    if g.at(LBRACE) {
        block(&mut g);
    } else {
        g.error("expected `{`");
    }
    let _ = m.complete(&mut g, RULE_SET);
}

/// Skip tokens until `}` or EOF, consuming a balanced `{ ... }` block if present.
/// Wraps all skipped tokens in an ERROR node.
fn skip_until_block_end(p: &mut Parser<'_>) {
    let m = p.start();
    let mut depth: u32 = 0;
    let mut consumed = false;
    while !p.at_end() {
        match p.current() {
            LBRACE => {
                depth += 1;
                p.bump();
                consumed = true;
            }
            RBRACE => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                p.bump();
            }
            _ => {
                p.bump();
                consumed = true;
            }
        }
    }
    if consumed {
        let _ = m.complete(p, SyntaxKind::ERROR);
    } else {
        m.abandon(p);
    }
}

/// Parse a `{ ... }` block containing declarations and/or nested rules.
pub(crate) fn block(p: &mut Parser<'_>) {
    let Ok(mut g) = p.depth_guard() else {
        // Skip past the block to prevent infinite loops at depth limit
        skip_until_block_end(p);
        return;
    };
    assert!(g.at(LBRACE));
    let m = g.start();
    g.bump(); // {
    while !g.at(RBRACE) && !g.at_end() {
        if g.at(SEMICOLON) {
            g.bump();
        } else if g.at(AT) || g.at(DOLLAR) || g.at_ts(selectors::SELECTOR_START) {
            block_item(&mut g);
        } else {
            g.err_and_bump("expected declaration or nested rule");
        }
    }
    g.expect(RBRACE);
    let _ = m.complete(&mut g, BLOCK);
}
