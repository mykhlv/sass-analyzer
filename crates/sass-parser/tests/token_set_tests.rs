use sass_parser::syntax_kind::SyntaxKind::*;
use sass_parser::token_set::TokenSet;

#[test]
fn empty_set_contains_nothing() {
    assert!(!TokenSet::EMPTY.contains(SEMICOLON));
    assert!(!TokenSet::EMPTY.contains(IDENT));
    assert!(!TokenSet::EMPTY.contains(SOURCE_FILE));
}

#[test]
fn set_contains_added_kinds() {
    const SET: TokenSet = TokenSet::new(&[SEMICOLON, IDENT, PLUS]);
    assert!(SET.contains(SEMICOLON));
    assert!(SET.contains(IDENT));
    assert!(SET.contains(PLUS));
    assert!(!SET.contains(COLON));
    assert!(!SET.contains(NUMBER));
}

#[test]
fn union_merges_two_sets() {
    const A: TokenSet = TokenSet::new(&[SEMICOLON, COLON]);
    const B: TokenSet = TokenSet::new(&[IDENT, NUMBER]);
    const MERGED: TokenSet = A.union(B);
    assert!(MERGED.contains(SEMICOLON));
    assert!(MERGED.contains(COLON));
    assert!(MERGED.contains(IDENT));
    assert!(MERGED.contains(NUMBER));
    assert!(!MERGED.contains(PLUS));
}

#[test]
fn node_kinds_not_contained() {
    const SET: TokenSet = TokenSet::new(&[SEMICOLON]);
    assert!(!SET.contains(SOURCE_FILE));
    assert!(!SET.contains(RULE_SET));
}
