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

pub fn source_file(p: &mut Parser<'_>) {
    let m = p.start();
    while !p.at_end() {
        p.bump();
    }
    let _ = m.complete(p, SOURCE_FILE);
}
