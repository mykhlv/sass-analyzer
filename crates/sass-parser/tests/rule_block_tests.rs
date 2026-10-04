//! A braced rule whose selector is not followed by `{`: reported once, right after the
//! selector, with the statements after it still parsed. Indented Sass, where a selector
//! line may have no body, is unaffected.

mod common;

use common::check;
use expect_test::expect;

fn scss_errors(source: &str) -> Vec<(String, sass_parser::text_range::TextRange)> {
    let (green, errors) = sass_parser::parse_scss(source);
    let tree = sass_parser::syntax::SyntaxNode::new_root(green);
    assert_eq!(
        tree.text().to_string(),
        source,
        "lossless round-trip failed"
    );
    errors
}

// ── Typical ─────────────────────────────────────────────────────────────

#[test]
fn missing_block_at_top_level() {
    check(
        ".a { b: c }\n.d\n",
        expect![[r#"
            SOURCE_FILE@0..15
              RULE_SET@0..11
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..11
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  DECLARATION@4..9
                    PROPERTY@4..6
                      WHITESPACE@4..5 " "
                      IDENT@5..6 "b"
                    COLON@6..7 ":"
                    VALUE@7..9
                      VALUE@7..9
                        WHITESPACE@7..8 " "
                        IDENT@8..9 "c"
                  WHITESPACE@9..10 " "
                  RBRACE@10..11 "}"
              RULE_SET@11..14
                SELECTOR_LIST@11..14
                  SELECTOR@11..14
                    SIMPLE_SELECTOR@11..14
                      WHITESPACE@11..12 "\n"
                      DOT@12..13 "."
                      IDENT@13..14 "d"
              WHITESPACE@14..15 "\n"
            errors:
              14..14: expected `{`
        "#]],
    );
}

#[test]
fn missing_block_before_a_nested_declaration() {
    check(
        ".a {\n  .b\n  color: red;\n  margin: 0;\n}\n",
        expect![[r#"
            SOURCE_FILE@0..39
              RULE_SET@0..38
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..38
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  RULE_SET@4..9
                    SELECTOR_LIST@4..9
                      SELECTOR@4..9
                        SIMPLE_SELECTOR@4..9
                          WHITESPACE@4..7 "\n  "
                          DOT@7..8 "."
                          IDENT@8..9 "b"
                  DECLARATION@9..23
                    PROPERTY@9..17
                      WHITESPACE@9..12 "\n  "
                      IDENT@12..17 "color"
                    COLON@17..18 ":"
                    VALUE@18..22
                      VALUE@18..22
                        WHITESPACE@18..19 " "
                        IDENT@19..22 "red"
                    SEMICOLON@22..23 ";"
                  DECLARATION@23..36
                    PROPERTY@23..32
                      WHITESPACE@23..26 "\n  "
                      IDENT@26..32 "margin"
                    COLON@32..33 ":"
                    VALUE@33..35
                      NUMBER_LITERAL@33..35
                        WHITESPACE@33..34 " "
                        NUMBER@34..35 "0"
                    SEMICOLON@35..36 ";"
                  WHITESPACE@36..37 "\n"
                  RBRACE@37..38 "}"
              WHITESPACE@38..39 "\n"
            errors:
              9..9: expected `{`
        "#]],
    );
}

#[test]
fn missing_block_before_a_nested_property() {
    check(
        ".a { .b font: bold { family: x } }",
        expect![[r#"
            SOURCE_FILE@0..34
              RULE_SET@0..34
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..34
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  RULE_SET@4..7
                    SELECTOR_LIST@4..7
                      SELECTOR@4..7
                        SIMPLE_SELECTOR@4..7
                          WHITESPACE@4..5 " "
                          DOT@5..6 "."
                          IDENT@6..7 "b"
                  NESTED_PROPERTY@7..32
                    PROPERTY@7..12
                      WHITESPACE@7..8 " "
                      IDENT@8..12 "font"
                    COLON@12..13 ":"
                    VALUE@13..18
                      VALUE@13..18
                        WHITESPACE@13..14 " "
                        IDENT@14..18 "bold"
                    BLOCK@18..32
                      WHITESPACE@18..19 " "
                      LBRACE@19..20 "{"
                      DECLARATION@20..30
                        PROPERTY@20..27
                          WHITESPACE@20..21 " "
                          IDENT@21..27 "family"
                        COLON@27..28 ":"
                        VALUE@28..30
                          VALUE@28..30
                            WHITESPACE@28..29 " "
                            IDENT@29..30 "x"
                      WHITESPACE@30..31 " "
                      RBRACE@31..32 "}"
                  WHITESPACE@32..33 " "
                  RBRACE@33..34 "}"
            errors:
              7..7: expected `{`
        "#]],
    );
}

#[test]
fn missing_block_before_an_unspaced_declaration() {
    check(
        ".a { .b color:red; }",
        expect![[r#"
            SOURCE_FILE@0..20
              RULE_SET@0..20
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..20
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  RULE_SET@4..7
                    SELECTOR_LIST@4..7
                      SELECTOR@4..7
                        SIMPLE_SELECTOR@4..7
                          WHITESPACE@4..5 " "
                          DOT@5..6 "."
                          IDENT@6..7 "b"
                  DECLARATION@7..18
                    PROPERTY@7..13
                      WHITESPACE@7..8 " "
                      IDENT@8..13 "color"
                    COLON@13..14 ":"
                    VALUE@14..17
                      VALUE@14..17
                        IDENT@14..17 "red"
                    SEMICOLON@17..18 ";"
                  WHITESPACE@18..19 " "
                  RBRACE@19..20 "}"
            errors:
              7..7: expected `{`
        "#]],
    );
}

#[test]
fn missing_block_before_the_closing_brace() {
    check(
        ".a { .b } .c { d: e }",
        expect![[r#"
            SOURCE_FILE@0..21
              RULE_SET@0..9
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..9
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  RULE_SET@4..7
                    SELECTOR_LIST@4..7
                      SELECTOR@4..7
                        SIMPLE_SELECTOR@4..7
                          WHITESPACE@4..5 " "
                          DOT@5..6 "."
                          IDENT@6..7 "b"
                  WHITESPACE@7..8 " "
                  RBRACE@8..9 "}"
              RULE_SET@9..21
                SELECTOR_LIST@9..12
                  SELECTOR@9..12
                    SIMPLE_SELECTOR@9..12
                      WHITESPACE@9..10 " "
                      DOT@10..11 "."
                      IDENT@11..12 "c"
                BLOCK@12..21
                  WHITESPACE@12..13 " "
                  LBRACE@13..14 "{"
                  DECLARATION@14..19
                    PROPERTY@14..16
                      WHITESPACE@14..15 " "
                      IDENT@15..16 "d"
                    COLON@16..17 ":"
                    VALUE@17..19
                      VALUE@17..19
                        WHITESPACE@17..18 " "
                        IDENT@18..19 "e"
                  WHITESPACE@19..20 " "
                  RBRACE@20..21 "}"
            errors:
              7..7: expected `{`
        "#]],
    );
}

#[test]
fn missing_block_in_a_media_body() {
    check(
        "@media screen { .a }\n.b { c: d }\n",
        expect![[r#"
        SOURCE_FILE@0..33
          MEDIA_RULE@0..20
            AT@0..1 "@"
            IDENT@1..6 "media"
            MEDIA_QUERY@6..13
              WHITESPACE@6..7 " "
              IDENT@7..13 "screen"
            BLOCK@13..20
              WHITESPACE@13..14 " "
              LBRACE@14..15 "{"
              RULE_SET@15..18
                SELECTOR_LIST@15..18
                  SELECTOR@15..18
                    SIMPLE_SELECTOR@15..18
                      WHITESPACE@15..16 " "
                      DOT@16..17 "."
                      IDENT@17..18 "a"
              WHITESPACE@18..19 " "
              RBRACE@19..20 "}"
          RULE_SET@20..32
            SELECTOR_LIST@20..23
              SELECTOR@20..23
                SIMPLE_SELECTOR@20..23
                  WHITESPACE@20..21 "\n"
                  DOT@21..22 "."
                  IDENT@22..23 "b"
            BLOCK@23..32
              WHITESPACE@23..24 " "
              LBRACE@24..25 "{"
              DECLARATION@25..30
                PROPERTY@25..27
                  WHITESPACE@25..26 " "
                  IDENT@26..27 "c"
                COLON@27..28 ":"
                VALUE@28..30
                  VALUE@28..30
                    WHITESPACE@28..29 " "
                    IDENT@29..30 "d"
              WHITESPACE@30..31 " "
              RBRACE@31..32 "}"
          WHITESPACE@32..33 "\n"
        errors:
          18..18: expected `{`
    "#]],
    );
}

#[test]
fn missing_block_in_an_include_content_block() {
    check(
        ".x { @include m { &:hover } }\n",
        expect![[r#"
        SOURCE_FILE@0..30
          RULE_SET@0..29
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "x"
            BLOCK@2..29
              WHITESPACE@2..3 " "
              LBRACE@3..4 "{"
              INCLUDE_RULE@4..27
                WHITESPACE@4..5 " "
                AT@5..6 "@"
                IDENT@6..13 "include"
                WHITESPACE@13..14 " "
                IDENT@14..15 "m"
                BLOCK@15..27
                  WHITESPACE@15..16 " "
                  LBRACE@16..17 "{"
                  RULE_SET@17..25
                    SELECTOR_LIST@17..25
                      SELECTOR@17..25
                        SIMPLE_SELECTOR@17..19
                          WHITESPACE@17..18 " "
                          AMP@18..19 "&"
                        PSEUDO_SELECTOR@19..25
                          COLON@19..20 ":"
                          IDENT@20..25 "hover"
                  WHITESPACE@25..26 " "
                  RBRACE@26..27 "}"
              WHITESPACE@27..28 " "
              RBRACE@28..29 "}"
          WHITESPACE@29..30 "\n"
        errors:
          25..25: expected `{`
    "#]],
    );
}

#[test]
fn missing_block_after_a_placeholder_selector() {
    check(
        "%p\n",
        expect![[r#"
        SOURCE_FILE@0..3
          RULE_SET@0..2
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  PERCENT@0..1 "%"
                  IDENT@1..2 "p"
          WHITESPACE@2..3 "\n"
        errors:
          2..2: expected `{`
    "#]],
    );
}

#[test]
fn missing_block_after_a_keyframe_stop() {
    check(
        "@keyframes k { from to { a: b } 50% }\n",
        expect![[r#"
        SOURCE_FILE@0..38
          KEYFRAMES_RULE@0..37
            AT@0..1 "@"
            IDENT@1..10 "keyframes"
            WHITESPACE@10..11 " "
            IDENT@11..12 "k"
            WHITESPACE@12..13 " "
            LBRACE@13..14 "{"
            KEYFRAME_SELECTOR@14..19
              WHITESPACE@14..15 " "
              IDENT@15..19 "from"
            KEYFRAME_SELECTOR@19..31
              WHITESPACE@19..20 " "
              IDENT@20..22 "to"
              BLOCK@22..31
                WHITESPACE@22..23 " "
                LBRACE@23..24 "{"
                DECLARATION@24..29
                  PROPERTY@24..26
                    WHITESPACE@24..25 " "
                    IDENT@25..26 "a"
                  COLON@26..27 ":"
                  VALUE@27..29
                    VALUE@27..29
                      WHITESPACE@27..28 " "
                      IDENT@28..29 "b"
                WHITESPACE@29..30 " "
                RBRACE@30..31 "}"
            KEYFRAME_SELECTOR@31..35
              WHITESPACE@31..32 " "
              NUMBER@32..34 "50"
              PERCENT@34..35 "%"
            WHITESPACE@35..36 " "
            RBRACE@36..37 "}"
          WHITESPACE@37..38 "\n"
        errors:
          19..19: expected `{`
          35..35: expected `{`
    "#]],
    );
}

// ── Edge ────────────────────────────────────────────────────────────────

#[test]
fn selector_at_eof_without_newline() {
    check(
        ".a",
        expect![[r#"
            SOURCE_FILE@0..2
              RULE_SET@0..2
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
            errors:
              2..2: expected `{`
        "#]],
    );
}

#[test]
fn selector_list_ending_in_a_comma() {
    check(
        ".a,\n",
        expect![[r#"
            SOURCE_FILE@0..4
              RULE_SET@0..3
                SELECTOR_LIST@0..3
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                  COMMA@2..3 ","
              WHITESPACE@3..4 "\n"
            errors:
              3..3: expected `{`
        "#]],
    );
}

#[test]
fn comment_between_selector_and_eof() {
    check(
        ".a /* c */\n",
        expect![[r#"
            SOURCE_FILE@0..11
              RULE_SET@0..2
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
              WHITESPACE@2..3 " "
              MULTI_LINE_COMMENT@3..10 "/* c */"
              WHITESPACE@10..11 "\n"
            errors:
              2..2: expected `{`
        "#]],
    );
}

#[test]
fn selector_ended_by_a_semicolon() {
    check(
        ".a; .b { c: d }",
        expect![[r#"
            SOURCE_FILE@0..15
              RULE_SET@0..2
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
              SEMICOLON@2..3 ";"
              RULE_SET@3..15
                SELECTOR_LIST@3..6
                  SELECTOR@3..6
                    SIMPLE_SELECTOR@3..6
                      WHITESPACE@3..4 " "
                      DOT@4..5 "."
                      IDENT@5..6 "b"
                BLOCK@6..15
                  WHITESPACE@6..7 " "
                  LBRACE@7..8 "{"
                  DECLARATION@8..13
                    PROPERTY@8..10
                      WHITESPACE@8..9 " "
                      IDENT@9..10 "c"
                    COLON@10..11 ":"
                    VALUE@11..13
                      VALUE@11..13
                        WHITESPACE@11..12 " "
                        IDENT@12..13 "d"
                  WHITESPACE@13..14 " "
                  RBRACE@14..15 "}"
            errors:
              2..2: expected `{`
        "#]],
    );
}

#[test]
fn selector_followed_by_an_at_rule() {
    check(
        ".a {\n  .b\n  @include c;\n}\n",
        expect![[r#"
            SOURCE_FILE@0..26
              RULE_SET@0..25
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..25
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  RULE_SET@4..9
                    SELECTOR_LIST@4..9
                      SELECTOR@4..9
                        SIMPLE_SELECTOR@4..9
                          WHITESPACE@4..7 "\n  "
                          DOT@7..8 "."
                          IDENT@8..9 "b"
                  INCLUDE_RULE@9..23
                    WHITESPACE@9..12 "\n  "
                    AT@12..13 "@"
                    IDENT@13..20 "include"
                    WHITESPACE@20..21 " "
                    IDENT@21..22 "c"
                    SEMICOLON@22..23 ";"
                  WHITESPACE@23..24 "\n"
                  RBRACE@24..25 "}"
              WHITESPACE@25..26 "\n"
            errors:
              9..9: expected `{`
        "#]],
    );
}

// ── Failing, with recovery ──────────────────────────────────────────────

#[test]
fn stray_token_before_the_block_keeps_the_block() {
    check(
        ".a ) { b: c }\n.d { e: f }\n",
        expect![[r#"
            SOURCE_FILE@0..26
              RULE_SET@0..13
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                ERROR@2..4
                  WHITESPACE@2..3 " "
                  RPAREN@3..4 ")"
                BLOCK@4..13
                  WHITESPACE@4..5 " "
                  LBRACE@5..6 "{"
                  DECLARATION@6..11
                    PROPERTY@6..8
                      WHITESPACE@6..7 " "
                      IDENT@7..8 "b"
                    COLON@8..9 ":"
                    VALUE@9..11
                      VALUE@9..11
                        WHITESPACE@9..10 " "
                        IDENT@10..11 "c"
                  WHITESPACE@11..12 " "
                  RBRACE@12..13 "}"
              RULE_SET@13..25
                SELECTOR_LIST@13..16
                  SELECTOR@13..16
                    SIMPLE_SELECTOR@13..16
                      WHITESPACE@13..14 "\n"
                      DOT@14..15 "."
                      IDENT@15..16 "d"
                BLOCK@16..25
                  WHITESPACE@16..17 " "
                  LBRACE@17..18 "{"
                  DECLARATION@18..23
                    PROPERTY@18..20
                      WHITESPACE@18..19 " "
                      IDENT@19..20 "e"
                    COLON@20..21 ":"
                    VALUE@21..23
                      VALUE@21..23
                        WHITESPACE@21..22 " "
                        IDENT@22..23 "f"
                  WHITESPACE@23..24 " "
                  RBRACE@24..25 "}"
              WHITESPACE@25..26 "\n"
            errors:
              3..4: expected `{`
        "#]],
    );
}

#[test]
fn stray_token_in_a_nested_rule_ends_at_the_semicolon() {
    check(
        ".a { .b ) c: d; e: f }",
        expect![[r#"
            SOURCE_FILE@0..22
              RULE_SET@0..22
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..22
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  RULE_SET@4..14
                    SELECTOR_LIST@4..7
                      SELECTOR@4..7
                        SIMPLE_SELECTOR@4..7
                          WHITESPACE@4..5 " "
                          DOT@5..6 "."
                          IDENT@6..7 "b"
                    ERROR@7..14
                      WHITESPACE@7..8 " "
                      RPAREN@8..9 ")"
                      WHITESPACE@9..10 " "
                      IDENT@10..11 "c"
                      COLON@11..12 ":"
                      WHITESPACE@12..13 " "
                      IDENT@13..14 "d"
                  SEMICOLON@14..15 ";"
                  DECLARATION@15..20
                    PROPERTY@15..17
                      WHITESPACE@15..16 " "
                      IDENT@16..17 "e"
                    COLON@17..18 ":"
                    VALUE@18..20
                      VALUE@18..20
                        WHITESPACE@18..19 " "
                        IDENT@19..20 "f"
                  WHITESPACE@20..21 " "
                  RBRACE@21..22 "}"
            errors:
              8..9: expected `{`
        "#]],
    );
}

#[test]
fn unclosed_pseudo_function_at_eof() {
    check(
        ".a:f(1\n",
        expect![[r#"
            SOURCE_FILE@0..7
              RULE_SET@0..6
                SELECTOR_LIST@0..6
                  SELECTOR@0..6
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                    PSEUDO_SELECTOR@2..6
                      COLON@2..3 ":"
                      IDENT@3..4 "f"
                      LPAREN@4..5 "("
                      NUMBER@5..6 "1"
              WHITESPACE@6..7 "\n"
            errors:
              6..6: expected RPAREN
        "#]],
    );
}

#[test]
fn unclosed_pseudo_function_ends_at_the_next_block() {
    check(
        ".a:not(.b\n.c { d: e }\n.f { g: h }\n",
        expect![[r#"
            SOURCE_FILE@0..34
              RULE_SET@0..21
                SELECTOR_LIST@0..12
                  SELECTOR@0..12
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                    PSEUDO_SELECTOR@2..12
                      COLON@2..3 ":"
                      IDENT@3..6 "not"
                      LPAREN@6..7 "("
                      DOT@7..8 "."
                      IDENT@8..9 "b"
                      WHITESPACE@9..10 "\n"
                      DOT@10..11 "."
                      IDENT@11..12 "c"
                BLOCK@12..21
                  WHITESPACE@12..13 " "
                  LBRACE@13..14 "{"
                  DECLARATION@14..19
                    PROPERTY@14..16
                      WHITESPACE@14..15 " "
                      IDENT@15..16 "d"
                    COLON@16..17 ":"
                    VALUE@17..19
                      VALUE@17..19
                        WHITESPACE@17..18 " "
                        IDENT@18..19 "e"
                  WHITESPACE@19..20 " "
                  RBRACE@20..21 "}"
              RULE_SET@21..33
                SELECTOR_LIST@21..24
                  SELECTOR@21..24
                    SIMPLE_SELECTOR@21..24
                      WHITESPACE@21..22 "\n"
                      DOT@22..23 "."
                      IDENT@23..24 "f"
                BLOCK@24..33
                  WHITESPACE@24..25 " "
                  LBRACE@25..26 "{"
                  DECLARATION@26..31
                    PROPERTY@26..28
                      WHITESPACE@26..27 " "
                      IDENT@27..28 "g"
                    COLON@28..29 ":"
                    VALUE@29..31
                      VALUE@29..31
                        WHITESPACE@29..30 " "
                        IDENT@30..31 "h"
                  WHITESPACE@31..32 " "
                  RBRACE@32..33 "}"
              WHITESPACE@33..34 "\n"
            errors:
              13..14: expected RPAREN
        "#]],
    );
}

#[test]
fn unclosed_pseudo_function_ends_at_the_closing_brace() {
    check(
        ".a { .b:not(.c } .d { e: f }",
        expect![[r#"
            SOURCE_FILE@0..28
              RULE_SET@0..16
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..16
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  RULE_SET@4..14
                    SELECTOR_LIST@4..14
                      SELECTOR@4..14
                        SIMPLE_SELECTOR@4..7
                          WHITESPACE@4..5 " "
                          DOT@5..6 "."
                          IDENT@6..7 "b"
                        PSEUDO_SELECTOR@7..14
                          COLON@7..8 ":"
                          IDENT@8..11 "not"
                          LPAREN@11..12 "("
                          DOT@12..13 "."
                          IDENT@13..14 "c"
                  WHITESPACE@14..15 " "
                  RBRACE@15..16 "}"
              RULE_SET@16..28
                SELECTOR_LIST@16..19
                  SELECTOR@16..19
                    SIMPLE_SELECTOR@16..19
                      WHITESPACE@16..17 " "
                      DOT@17..18 "."
                      IDENT@18..19 "d"
                BLOCK@19..28
                  WHITESPACE@19..20 " "
                  LBRACE@20..21 "{"
                  DECLARATION@21..26
                    PROPERTY@21..23
                      WHITESPACE@21..22 " "
                      IDENT@22..23 "e"
                    COLON@23..24 ":"
                    VALUE@24..26
                      VALUE@24..26
                        WHITESPACE@24..25 " "
                        IDENT@25..26 "f"
                  WHITESPACE@26..27 " "
                  RBRACE@27..28 "}"
            errors:
              14..14: expected RPAREN
        "#]],
    );
}

#[test]
fn unclosed_attribute_selector_ends_at_the_next_block() {
    check(
        ".a[x\n.c { d: e }\n",
        expect![[r#"
            SOURCE_FILE@0..17
              RULE_SET@0..16
                SELECTOR_LIST@0..7
                  SELECTOR@0..7
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                    ATTR_SELECTOR@2..7
                      LBRACKET@2..3 "["
                      IDENT@3..4 "x"
                      WHITESPACE@4..5 "\n"
                      DOT@5..6 "."
                      IDENT@6..7 "c"
                BLOCK@7..16
                  WHITESPACE@7..8 " "
                  LBRACE@8..9 "{"
                  DECLARATION@9..14
                    PROPERTY@9..11
                      WHITESPACE@9..10 " "
                      IDENT@10..11 "d"
                    COLON@11..12 ":"
                    VALUE@12..14
                      VALUE@12..14
                        WHITESPACE@12..13 " "
                        IDENT@13..14 "e"
                  WHITESPACE@14..15 " "
                  RBRACE@15..16 "}"
              WHITESPACE@16..17 "\n"
            errors:
              8..9: expected RBRACKET
        "#]],
    );
}

#[test]
fn interpolation_in_an_unclosed_pseudo_function_does_not_end_it() {
    check(
        ".a:not(#{$b}\n.c { d: e }\n",
        expect![[r##"
            SOURCE_FILE@0..25
              RULE_SET@0..24
                SELECTOR_LIST@0..15
                  SELECTOR@0..15
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                    PSEUDO_SELECTOR@2..15
                      COLON@2..3 ":"
                      IDENT@3..6 "not"
                      LPAREN@6..7 "("
                      HASH_LBRACE@7..9 "#{"
                      DOLLAR@9..10 "$"
                      IDENT@10..11 "b"
                      RBRACE@11..12 "}"
                      WHITESPACE@12..13 "\n"
                      DOT@13..14 "."
                      IDENT@14..15 "c"
                BLOCK@15..24
                  WHITESPACE@15..16 " "
                  LBRACE@16..17 "{"
                  DECLARATION@17..22
                    PROPERTY@17..19
                      WHITESPACE@17..18 " "
                      IDENT@18..19 "d"
                    COLON@19..20 ":"
                    VALUE@20..22
                      VALUE@20..22
                        WHITESPACE@20..21 " "
                        IDENT@21..22 "e"
                  WHITESPACE@22..23 " "
                  RBRACE@23..24 "}"
              WHITESPACE@24..25 "\n"
            errors:
              16..17: expected RPAREN
        "##]],
    );
}

// ── Proportionality ─────────────────────────────────────────────────────

#[test]
fn each_missing_block_is_one_error() {
    let errors = scss_errors(".a\n.b { c: d }\n.e { .f; g: h }\n.i { j: k }\n.l\n");
    let messages: Vec<_> = errors.iter().map(|(m, _)| m.as_str()).collect();
    assert_eq!(messages, ["expected `{`", "expected `{`"], "{errors:?}");
}

#[test]
fn many_unclosed_parens_without_braces_stay_proportional() {
    let source = ".a:f(".repeat(20_000);
    let errors = scss_errors(&source);
    let messages: Vec<_> = errors.iter().map(|(m, _)| m.as_str()).collect();
    assert_eq!(messages, ["expected RPAREN"], "{errors:?}");
}

// ── Valid input stays error-free ────────────────────────────────────────

#[test]
fn valid_rules_without_a_block_of_their_own_are_not_flagged() {
    let long_list = (0..60)
        .map(|i| format!(".s{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sources = [
        "%p { a: b }\n.c { @extend %p; @extend .d !optional; }\n",
        "@at-root .a { b: c }\n.d { @at-root .e { f: g } }\n",
        "@keyframes k { from { a: b } 50% { a: c } to { a: d } }\n",
        "@keyframes k { from, 50% { a: b } #{$s} { a: c } }\n",
        "@mixin m { a: b }\n.c { @include m; }\n",
        ".a { --x: { b: c }; --y: d; }\n",
        ".a { font: { family: x; size: 1px } }\n",
        ".a { font: bold { family: x } }\n",
        ".a { li:hover { b: c } ul li:first-child { d: e } }\n",
        ".a { .b:not(#{$c}) { d: e } }\n",
        &format!(".a {{ .b li:hover, {long_list} {{ c: d }} }}\n"),
    ];
    for source in sources {
        let errors = scss_errors(source);
        assert!(errors.is_empty(), "{source:?}: {errors:?}");
    }
}

// ── Indented Sass ───────────────────────────────────────────────────────

#[test]
fn indented_selector_lines_without_a_body_are_legal() {
    for source in [
        ".a\n.b\n  c: d\n.e\n",
        ".a\n  .b\n  c: d\n",
        ".a:f(1\n.b\n  c: d\n",
        "@keyframes k\n  from\n  to\n    a: b\n",
    ] {
        let (green, errors) = sass_parser::parse_sass(source);
        let tree = sass_parser::syntax::SyntaxNode::new_root(green);
        assert_eq!(tree.text().to_string(), source);
        assert!(
            !errors.iter().any(|(m, _)| m == "expected `{`"),
            "{source:?}: {errors:?}"
        );
    }
}
