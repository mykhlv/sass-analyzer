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
/// When at `IDENT COLON`, scans forward (tracking paren/bracket depth)
/// for the first `{`, `;`, or `}` at depth 0:
/// - `;` or `}` found first → declaration
/// - `{` found first → rule set (selector with pseudo-class)
fn looks_like_declaration(p: &Parser<'_>) -> bool {
    if !p.at(IDENT) || p.nth(1) != COLON {
        return false;
    }
    // IDENT COLON — scan from position 2 onward
    let mut depth: u32 = 0;
    let mut offset = 2;
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
fn block(p: &mut Parser<'_>) {
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
