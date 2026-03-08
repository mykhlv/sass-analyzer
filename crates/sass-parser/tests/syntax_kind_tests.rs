use sass_parser::syntax_kind::*;

#[test]
fn syntax_kind_round_trips_through_u16() {
    for raw in 0..=INTERPOLATION as u16 {
        let kind = SyntaxKind::from(raw);
        let back: u16 = kind.into();
        assert_eq!(raw, back, "round-trip failed for {kind:?}");
    }
}

#[test]
fn is_token_vs_is_node() {
    assert!(SEMICOLON.is_token());
    assert!(IDENT.is_token());
    assert!(WHITESPACE.is_token());
    assert!(ERROR.is_token());
    assert!(EOF.is_token());
    assert!(HASH_LBRACE.is_token());
    assert!(COLON_COLON.is_token());
    assert!(TILDE_EQ.is_token());
    assert!(STRING_START.is_token());
    assert!(URL_CONTENTS.is_token());
    assert!(UNICODE_RANGE.is_token());
    assert!(!SEMICOLON.is_node());

    assert!(!__LAST_TOKEN.is_token());
    assert!(!__LAST_TOKEN.is_node());

    assert!(SOURCE_FILE.is_node());
    assert!(RULE_SET.is_node());
    assert!(DECLARATION.is_node());
    assert!(!SOURCE_FILE.is_token());
}

#[test]
fn is_trivia() {
    assert!(WHITESPACE.is_trivia());
    assert!(SINGLE_LINE_COMMENT.is_trivia());
    assert!(MULTI_LINE_COMMENT.is_trivia());
    assert!(!IDENT.is_trivia());
    assert!(!SEMICOLON.is_trivia());
}
