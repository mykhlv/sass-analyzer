mod declarations;
pub(crate) mod selectors;

use crate::parser::Parser;
#[allow(clippy::wildcard_imports)]
use crate::syntax_kind::*;
use crate::token_set::TokenSet;

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

/// 2.4: Parse `SourceFile` — top-level sequence of items with error recovery.
pub fn source_file(p: &mut Parser<'_>) {
    let m = p.start();
    while !p.at_end() {
        if p.at_ts(selectors::SELECTOR_START) {
            rule_set(p);
        } else if p.at(SEMICOLON) {
            p.bump();
        } else {
            // Skip unknown tokens (e.g. `@import`, `$var`) until next statement start
            p.err_recover("expected rule", STMT_RECOVERY);
        }
    }
    let _ = m.complete(p, SOURCE_FILE);
}

/// Parse a single item inside a block.
/// Disambiguates between rule sets and declarations.
fn block_item(p: &mut Parser<'_>) {
    if looks_like_declaration(p) {
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
    let Ok(mut g) = p.depth_guard() else { return };
    let m = g.start();
    selectors::selector_list(&mut g);
    if g.at(LBRACE) {
        block(&mut g);
    } else {
        g.error("expected `{`");
    }
    let _ = m.complete(&mut g, RULE_SET);
}

/// Parse a `{ ... }` block containing declarations and/or nested rules.
pub(super) fn block(p: &mut Parser<'_>) {
    let Ok(mut p) = p.depth_guard() else { return };
    assert!(p.at(LBRACE));
    let m = p.start();
    p.bump(); // {
    while !p.at(RBRACE) && !p.at_end() {
        if p.at(SEMICOLON) {
            p.bump();
        } else if p.at_ts(selectors::SELECTOR_START) {
            block_item(&mut p);
        } else {
            p.err_and_bump("expected declaration or nested rule");
        }
    }
    p.expect(RBRACE);
    let _ = m.complete(&mut p, BLOCK);
}
