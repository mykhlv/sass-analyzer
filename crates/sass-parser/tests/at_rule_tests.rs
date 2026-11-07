mod common;

use common::check;
use expect_test::expect;
use sass_parser::syntax::SyntaxNode;

// ── @mixin ──────────────────────────────────────────────────────────────

#[test]
fn mixin_simple() {
    check(
        "@mixin center { display: flex; }",
        expect![[r#"
            SOURCE_FILE@0..32
              MIXIN_RULE@0..32
                AT@0..1 "@"
                IDENT@1..6 "mixin"
                WHITESPACE@6..7 " "
                IDENT@7..13 "center"
                BLOCK@13..32
                  WHITESPACE@13..14 " "
                  LBRACE@14..15 "{"
                  DECLARATION@15..30
                    PROPERTY@15..23
                      WHITESPACE@15..16 " "
                      IDENT@16..23 "display"
                    COLON@23..24 ":"
                    VALUE@24..29
                      VALUE@24..29
                        WHITESPACE@24..25 " "
                        IDENT@25..29 "flex"
                    SEMICOLON@29..30 ";"
                  WHITESPACE@30..31 " "
                  RBRACE@31..32 "}"
        "#]],
    );
}

#[test]
fn mixin_with_params() {
    check(
        "@mixin size($w, $h: 100px) { width: $w; }",
        expect![[r#"
            SOURCE_FILE@0..41
              MIXIN_RULE@0..41
                AT@0..1 "@"
                IDENT@1..6 "mixin"
                WHITESPACE@6..7 " "
                IDENT@7..11 "size"
                PARAM_LIST@11..26
                  LPAREN@11..12 "("
                  PARAM@12..14
                    DOLLAR@12..13 "$"
                    IDENT@13..14 "w"
                  COMMA@14..15 ","
                  PARAM@15..25
                    WHITESPACE@15..16 " "
                    DOLLAR@16..17 "$"
                    IDENT@17..18 "h"
                    COLON@18..19 ":"
                    DIMENSION@19..25
                      WHITESPACE@19..20 " "
                      NUMBER@20..23 "100"
                      IDENT@23..25 "px"
                  RPAREN@25..26 ")"
                BLOCK@26..41
                  WHITESPACE@26..27 " "
                  LBRACE@27..28 "{"
                  DECLARATION@28..39
                    PROPERTY@28..34
                      WHITESPACE@28..29 " "
                      IDENT@29..34 "width"
                    COLON@34..35 ":"
                    VALUE@35..38
                      VARIABLE_REF@35..38
                        WHITESPACE@35..36 " "
                        DOLLAR@36..37 "$"
                        IDENT@37..38 "w"
                    SEMICOLON@38..39 ";"
                  WHITESPACE@39..40 " "
                  RBRACE@40..41 "}"
        "#]],
    );
}

#[test]
fn mixin_rest_param() {
    check(
        "@mixin args($a, $rest...) { }",
        expect![[r#"
            SOURCE_FILE@0..29
              MIXIN_RULE@0..29
                AT@0..1 "@"
                IDENT@1..6 "mixin"
                WHITESPACE@6..7 " "
                IDENT@7..11 "args"
                PARAM_LIST@11..25
                  LPAREN@11..12 "("
                  PARAM@12..14
                    DOLLAR@12..13 "$"
                    IDENT@13..14 "a"
                  COMMA@14..15 ","
                  PARAM@15..24
                    WHITESPACE@15..16 " "
                    DOLLAR@16..17 "$"
                    IDENT@17..21 "rest"
                    DOT_DOT_DOT@21..24 "..."
                  RPAREN@24..25 ")"
                BLOCK@25..29
                  WHITESPACE@25..26 " "
                  LBRACE@26..27 "{"
                  WHITESPACE@27..28 " "
                  RBRACE@28..29 "}"
        "#]],
    );
}

// ── @include ────────────────────────────────────────────────────────────

#[test]
fn include_simple() {
    check(
        ".box { @include center; }",
        expect![[r#"
            SOURCE_FILE@0..25
              RULE_SET@0..25
                SELECTOR_LIST@0..4
                  SELECTOR@0..4
                    SIMPLE_SELECTOR@0..4
                      DOT@0..1 "."
                      IDENT@1..4 "box"
                BLOCK@4..25
                  WHITESPACE@4..5 " "
                  LBRACE@5..6 "{"
                  INCLUDE_RULE@6..23
                    WHITESPACE@6..7 " "
                    AT@7..8 "@"
                    IDENT@8..15 "include"
                    WHITESPACE@15..16 " "
                    IDENT@16..22 "center"
                    SEMICOLON@22..23 ";"
                  WHITESPACE@23..24 " "
                  RBRACE@24..25 "}"
        "#]],
    );
}

#[test]
fn include_with_args() {
    check(
        "@include size(10px, 20px);",
        expect![[r#"
            SOURCE_FILE@0..26
              INCLUDE_RULE@0..26
                AT@0..1 "@"
                IDENT@1..8 "include"
                WHITESPACE@8..9 " "
                IDENT@9..13 "size"
                ARG_LIST@13..25
                  LPAREN@13..14 "("
                  ARG@14..18
                    DIMENSION@14..18
                      NUMBER@14..16 "10"
                      IDENT@16..18 "px"
                  COMMA@18..19 ","
                  ARG@19..24
                    DIMENSION@19..24
                      WHITESPACE@19..20 " "
                      NUMBER@20..22 "20"
                      IDENT@22..24 "px"
                  RPAREN@24..25 ")"
                SEMICOLON@25..26 ";"
        "#]],
    );
}

#[test]
fn include_with_content_block() {
    check(
        "@include respond-to(md) { color: red; }",
        expect![[r#"
            SOURCE_FILE@0..39
              INCLUDE_RULE@0..39
                AT@0..1 "@"
                IDENT@1..8 "include"
                WHITESPACE@8..9 " "
                IDENT@9..19 "respond-to"
                ARG_LIST@19..23
                  LPAREN@19..20 "("
                  ARG@20..22
                    VALUE@20..22
                      IDENT@20..22 "md"
                  RPAREN@22..23 ")"
                BLOCK@23..39
                  WHITESPACE@23..24 " "
                  LBRACE@24..25 "{"
                  DECLARATION@25..37
                    PROPERTY@25..31
                      WHITESPACE@25..26 " "
                      IDENT@26..31 "color"
                    COLON@31..32 ":"
                    VALUE@32..36
                      VALUE@32..36
                        WHITESPACE@32..33 " "
                        IDENT@33..36 "red"
                    SEMICOLON@36..37 ";"
                  WHITESPACE@37..38 " "
                  RBRACE@38..39 "}"
        "#]],
    );
}

#[test]
fn include_using() {
    check(
        "@include mixin using ($a) { }",
        expect![[r#"
            SOURCE_FILE@0..29
              INCLUDE_RULE@0..29
                AT@0..1 "@"
                IDENT@1..8 "include"
                WHITESPACE@8..9 " "
                IDENT@9..14 "mixin"
                WHITESPACE@14..15 " "
                IDENT@15..20 "using"
                PARAM_LIST@20..25
                  WHITESPACE@20..21 " "
                  LPAREN@21..22 "("
                  PARAM@22..24
                    DOLLAR@22..23 "$"
                    IDENT@23..24 "a"
                  RPAREN@24..25 ")"
                BLOCK@25..29
                  WHITESPACE@25..26 " "
                  LBRACE@26..27 "{"
                  WHITESPACE@27..28 " "
                  RBRACE@28..29 "}"
        "#]],
    );
}

// ── @content ────────────────────────────────────────────────────────────

#[test]
fn content_simple() {
    check(
        "@mixin wrap { @content; }",
        expect![[r#"
            SOURCE_FILE@0..25
              MIXIN_RULE@0..25
                AT@0..1 "@"
                IDENT@1..6 "mixin"
                WHITESPACE@6..7 " "
                IDENT@7..11 "wrap"
                BLOCK@11..25
                  WHITESPACE@11..12 " "
                  LBRACE@12..13 "{"
                  CONTENT_RULE@13..23
                    WHITESPACE@13..14 " "
                    AT@14..15 "@"
                    IDENT@15..22 "content"
                    SEMICOLON@22..23 ";"
                  WHITESPACE@23..24 " "
                  RBRACE@24..25 "}"
        "#]],
    );
}

// ── @function / @return ─────────────────────────────────────────────────

#[test]
fn function_rule() {
    check(
        "@function double($n) { @return $n * 2; }",
        expect![[r#"
            SOURCE_FILE@0..40
              FUNCTION_RULE@0..40
                AT@0..1 "@"
                IDENT@1..9 "function"
                WHITESPACE@9..10 " "
                IDENT@10..16 "double"
                PARAM_LIST@16..20
                  LPAREN@16..17 "("
                  PARAM@17..19
                    DOLLAR@17..18 "$"
                    IDENT@18..19 "n"
                  RPAREN@19..20 ")"
                BLOCK@20..40
                  WHITESPACE@20..21 " "
                  LBRACE@21..22 "{"
                  RETURN_RULE@22..38
                    WHITESPACE@22..23 " "
                    AT@23..24 "@"
                    IDENT@24..30 "return"
                    BINARY_EXPR@30..37
                      VARIABLE_REF@30..33
                        WHITESPACE@30..31 " "
                        DOLLAR@31..32 "$"
                        IDENT@32..33 "n"
                      WHITESPACE@33..34 " "
                      STAR@34..35 "*"
                      NUMBER_LITERAL@35..37
                        WHITESPACE@35..36 " "
                        NUMBER@36..37 "2"
                    SEMICOLON@37..38 ";"
                  WHITESPACE@38..39 " "
                  RBRACE@39..40 "}"
        "#]],
    );
}

// ── @if / @else ─────────────────────────────────────────────────────────

#[test]
fn if_simple() {
    check(
        "@if $x { color: red; }",
        expect![[r#"
            SOURCE_FILE@0..22
              IF_RULE@0..22
                AT@0..1 "@"
                IDENT@1..3 "if"
                VARIABLE_REF@3..6
                  WHITESPACE@3..4 " "
                  DOLLAR@4..5 "$"
                  IDENT@5..6 "x"
                BLOCK@6..22
                  WHITESPACE@6..7 " "
                  LBRACE@7..8 "{"
                  DECLARATION@8..20
                    PROPERTY@8..14
                      WHITESPACE@8..9 " "
                      IDENT@9..14 "color"
                    COLON@14..15 ":"
                    VALUE@15..19
                      VALUE@15..19
                        WHITESPACE@15..16 " "
                        IDENT@16..19 "red"
                    SEMICOLON@19..20 ";"
                  WHITESPACE@20..21 " "
                  RBRACE@21..22 "}"
        "#]],
    );
}

#[test]
fn if_else_chain() {
    check(
        "@if $a { } @else if $b { } @else { }",
        expect![[r#"
            SOURCE_FILE@0..36
              IF_RULE@0..36
                AT@0..1 "@"
                IDENT@1..3 "if"
                VARIABLE_REF@3..6
                  WHITESPACE@3..4 " "
                  DOLLAR@4..5 "$"
                  IDENT@5..6 "a"
                BLOCK@6..10
                  WHITESPACE@6..7 " "
                  LBRACE@7..8 "{"
                  WHITESPACE@8..9 " "
                  RBRACE@9..10 "}"
                ELSE_CLAUSE@10..26
                  WHITESPACE@10..11 " "
                  AT@11..12 "@"
                  IDENT@12..16 "else"
                  WHITESPACE@16..17 " "
                  IDENT@17..19 "if"
                  VARIABLE_REF@19..22
                    WHITESPACE@19..20 " "
                    DOLLAR@20..21 "$"
                    IDENT@21..22 "b"
                  BLOCK@22..26
                    WHITESPACE@22..23 " "
                    LBRACE@23..24 "{"
                    WHITESPACE@24..25 " "
                    RBRACE@25..26 "}"
                ELSE_CLAUSE@26..36
                  WHITESPACE@26..27 " "
                  AT@27..28 "@"
                  IDENT@28..32 "else"
                  BLOCK@32..36
                    WHITESPACE@32..33 " "
                    LBRACE@33..34 "{"
                    WHITESPACE@34..35 " "
                    RBRACE@35..36 "}"
        "#]],
    );
}

// ── @for ────────────────────────────────────────────────────────────────

#[test]
fn for_through() {
    check(
        "@for $i from 1 through 3 { }",
        expect![[r#"
            SOURCE_FILE@0..28
              FOR_RULE@0..28
                AT@0..1 "@"
                IDENT@1..4 "for"
                WHITESPACE@4..5 " "
                DOLLAR@5..6 "$"
                IDENT@6..7 "i"
                WHITESPACE@7..8 " "
                IDENT@8..12 "from"
                NUMBER_LITERAL@12..14
                  WHITESPACE@12..13 " "
                  NUMBER@13..14 "1"
                WHITESPACE@14..15 " "
                IDENT@15..22 "through"
                NUMBER_LITERAL@22..24
                  WHITESPACE@22..23 " "
                  NUMBER@23..24 "3"
                BLOCK@24..28
                  WHITESPACE@24..25 " "
                  LBRACE@25..26 "{"
                  WHITESPACE@26..27 " "
                  RBRACE@27..28 "}"
        "#]],
    );
}

#[test]
fn for_to() {
    check(
        "@for $i from 1 to 5 { }",
        expect![[r#"
            SOURCE_FILE@0..23
              FOR_RULE@0..23
                AT@0..1 "@"
                IDENT@1..4 "for"
                WHITESPACE@4..5 " "
                DOLLAR@5..6 "$"
                IDENT@6..7 "i"
                WHITESPACE@7..8 " "
                IDENT@8..12 "from"
                NUMBER_LITERAL@12..14
                  WHITESPACE@12..13 " "
                  NUMBER@13..14 "1"
                WHITESPACE@14..15 " "
                IDENT@15..17 "to"
                NUMBER_LITERAL@17..19
                  WHITESPACE@17..18 " "
                  NUMBER@18..19 "5"
                BLOCK@19..23
                  WHITESPACE@19..20 " "
                  LBRACE@20..21 "{"
                  WHITESPACE@21..22 " "
                  RBRACE@22..23 "}"
        "#]],
    );
}

// ── @each ───────────────────────────────────────────────────────────────

#[test]
fn each_simple() {
    check(
        "@each $x in a, b, c { }",
        expect![[r#"
            SOURCE_FILE@0..23
              EACH_RULE@0..23
                AT@0..1 "@"
                IDENT@1..5 "each"
                WHITESPACE@5..6 " "
                DOLLAR@6..7 "$"
                IDENT@7..8 "x"
                WHITESPACE@8..9 " "
                IDENT@9..11 "in"
                VALUE@11..13
                  WHITESPACE@11..12 " "
                  IDENT@12..13 "a"
                COMMA@13..14 ","
                VALUE@14..16
                  WHITESPACE@14..15 " "
                  IDENT@15..16 "b"
                COMMA@16..17 ","
                VALUE@17..19
                  WHITESPACE@17..18 " "
                  IDENT@18..19 "c"
                BLOCK@19..23
                  WHITESPACE@19..20 " "
                  LBRACE@20..21 "{"
                  WHITESPACE@21..22 " "
                  RBRACE@22..23 "}"
        "#]],
    );
}

#[test]
fn each_destructuring() {
    check(
        "@each $k, $v in $map { }",
        expect![[r#"
            SOURCE_FILE@0..24
              EACH_RULE@0..24
                AT@0..1 "@"
                IDENT@1..5 "each"
                WHITESPACE@5..6 " "
                DOLLAR@6..7 "$"
                IDENT@7..8 "k"
                COMMA@8..9 ","
                WHITESPACE@9..10 " "
                DOLLAR@10..11 "$"
                IDENT@11..12 "v"
                WHITESPACE@12..13 " "
                IDENT@13..15 "in"
                VARIABLE_REF@15..20
                  WHITESPACE@15..16 " "
                  DOLLAR@16..17 "$"
                  IDENT@17..20 "map"
                BLOCK@20..24
                  WHITESPACE@20..21 " "
                  LBRACE@21..22 "{"
                  WHITESPACE@22..23 " "
                  RBRACE@23..24 "}"
        "#]],
    );
}

// ── @while ──────────────────────────────────────────────────────────────

#[test]
fn while_simple() {
    check(
        "@while $i > 0 { }",
        expect![[r#"
            SOURCE_FILE@0..17
              WHILE_RULE@0..17
                AT@0..1 "@"
                IDENT@1..6 "while"
                BINARY_EXPR@6..13
                  VARIABLE_REF@6..9
                    WHITESPACE@6..7 " "
                    DOLLAR@7..8 "$"
                    IDENT@8..9 "i"
                  WHITESPACE@9..10 " "
                  GT@10..11 ">"
                  NUMBER_LITERAL@11..13
                    WHITESPACE@11..12 " "
                    NUMBER@12..13 "0"
                BLOCK@13..17
                  WHITESPACE@13..14 " "
                  LBRACE@14..15 "{"
                  WHITESPACE@15..16 " "
                  RBRACE@16..17 "}"
        "#]],
    );
}

// ── @extend ─────────────────────────────────────────────────────────────

#[test]
fn extend_simple() {
    check(
        ".btn { @extend .base; }",
        expect![[r#"
            SOURCE_FILE@0..23
              RULE_SET@0..23
                SELECTOR_LIST@0..4
                  SELECTOR@0..4
                    SIMPLE_SELECTOR@0..4
                      DOT@0..1 "."
                      IDENT@1..4 "btn"
                BLOCK@4..23
                  WHITESPACE@4..5 " "
                  LBRACE@5..6 "{"
                  EXTEND_RULE@6..21
                    WHITESPACE@6..7 " "
                    AT@7..8 "@"
                    IDENT@8..14 "extend"
                    WHITESPACE@14..15 " "
                    DOT@15..16 "."
                    IDENT@16..20 "base"
                    SEMICOLON@20..21 ";"
                  WHITESPACE@21..22 " "
                  RBRACE@22..23 "}"
        "#]],
    );
}

#[test]
fn extend_optional() {
    check(
        ".btn { @extend .base !optional; }",
        expect![[r#"
            SOURCE_FILE@0..33
              RULE_SET@0..33
                SELECTOR_LIST@0..4
                  SELECTOR@0..4
                    SIMPLE_SELECTOR@0..4
                      DOT@0..1 "."
                      IDENT@1..4 "btn"
                BLOCK@4..33
                  WHITESPACE@4..5 " "
                  LBRACE@5..6 "{"
                  EXTEND_RULE@6..31
                    WHITESPACE@6..7 " "
                    AT@7..8 "@"
                    IDENT@8..14 "extend"
                    WHITESPACE@14..15 " "
                    DOT@15..16 "."
                    IDENT@16..20 "base"
                    SASS_FLAG@20..30
                      WHITESPACE@20..21 " "
                      BANG@21..22 "!"
                      IDENT@22..30 "optional"
                    SEMICOLON@30..31 ";"
                  WHITESPACE@31..32 " "
                  RBRACE@32..33 "}"
        "#]],
    );
}

// ── @error / @warn / @debug ─────────────────────────────────────────────

#[test]
fn error_rule() {
    check(
        "@error \"not found\";",
        expect![[r#"
            SOURCE_FILE@0..19
              ERROR_RULE@0..19
                AT@0..1 "@"
                IDENT@1..6 "error"
                STRING_LITERAL@6..18
                  WHITESPACE@6..7 " "
                  QUOTED_STRING@7..18 "\"not found\""
                SEMICOLON@18..19 ";"
        "#]],
    );
}

#[test]
fn warn_rule() {
    check(
        "@warn \"deprecated\";",
        expect![[r#"
            SOURCE_FILE@0..19
              WARN_RULE@0..19
                AT@0..1 "@"
                IDENT@1..5 "warn"
                STRING_LITERAL@5..18
                  WHITESPACE@5..6 " "
                  QUOTED_STRING@6..18 "\"deprecated\""
                SEMICOLON@18..19 ";"
        "#]],
    );
}

#[test]
fn debug_rule() {
    check(
        "@debug $value;",
        expect![[r#"
            SOURCE_FILE@0..14
              DEBUG_RULE@0..14
                AT@0..1 "@"
                IDENT@1..6 "debug"
                VARIABLE_REF@6..13
                  WHITESPACE@6..7 " "
                  DOLLAR@7..8 "$"
                  IDENT@8..13 "value"
                SEMICOLON@13..14 ";"
        "#]],
    );
}

// ── @at-root ────────────────────────────────────────────────────────────

#[test]
fn at_root_block() {
    check(
        "@at-root { .child { } }",
        expect![[r#"
            SOURCE_FILE@0..23
              AT_ROOT_RULE@0..23
                AT@0..1 "@"
                IDENT@1..8 "at-root"
                BLOCK@8..23
                  WHITESPACE@8..9 " "
                  LBRACE@9..10 "{"
                  RULE_SET@10..21
                    SELECTOR_LIST@10..17
                      SELECTOR@10..17
                        SIMPLE_SELECTOR@10..17
                          WHITESPACE@10..11 " "
                          DOT@11..12 "."
                          IDENT@12..17 "child"
                    BLOCK@17..21
                      WHITESPACE@17..18 " "
                      LBRACE@18..19 "{"
                      WHITESPACE@19..20 " "
                      RBRACE@20..21 "}"
                  WHITESPACE@21..22 " "
                  RBRACE@22..23 "}"
        "#]],
    );
}

#[test]
fn at_root_query() {
    check(
        "@at-root (without: media) { }",
        expect![[r#"
            SOURCE_FILE@0..29
              AT_ROOT_RULE@0..29
                AT@0..1 "@"
                IDENT@1..8 "at-root"
                AT_ROOT_QUERY@8..25
                  WHITESPACE@8..9 " "
                  LPAREN@9..10 "("
                  IDENT@10..17 "without"
                  COLON@17..18 ":"
                  WHITESPACE@18..19 " "
                  IDENT@19..24 "media"
                  RPAREN@24..25 ")"
                BLOCK@25..29
                  WHITESPACE@25..26 " "
                  LBRACE@26..27 "{"
                  WHITESPACE@27..28 " "
                  RBRACE@28..29 "}"
        "#]],
    );
}

// ── @media ──────────────────────────────────────────────────────────────

#[test]
fn media_simple() {
    check(
        "@media screen { body { } }",
        expect![[r#"
            SOURCE_FILE@0..26
              MEDIA_RULE@0..26
                AT@0..1 "@"
                IDENT@1..6 "media"
                MEDIA_QUERY@6..13
                  WHITESPACE@6..7 " "
                  IDENT@7..13 "screen"
                BLOCK@13..26
                  WHITESPACE@13..14 " "
                  LBRACE@14..15 "{"
                  RULE_SET@15..24
                    SELECTOR_LIST@15..20
                      SELECTOR@15..20
                        SIMPLE_SELECTOR@15..20
                          WHITESPACE@15..16 " "
                          IDENT@16..20 "body"
                    BLOCK@20..24
                      WHITESPACE@20..21 " "
                      LBRACE@21..22 "{"
                      WHITESPACE@22..23 " "
                      RBRACE@23..24 "}"
                  WHITESPACE@24..25 " "
                  RBRACE@25..26 "}"
        "#]],
    );
}

#[test]
fn media_with_condition() {
    check(
        "@media (min-width: 768px) { }",
        expect![[r#"
            SOURCE_FILE@0..29
              MEDIA_RULE@0..29
                AT@0..1 "@"
                IDENT@1..6 "media"
                MEDIA_QUERY@6..25
                  WHITESPACE@6..7 " "
                  LPAREN@7..8 "("
                  IDENT@8..17 "min-width"
                  COLON@17..18 ":"
                  WHITESPACE@18..19 " "
                  NUMBER@19..22 "768"
                  IDENT@22..24 "px"
                  RPAREN@24..25 ")"
                BLOCK@25..29
                  WHITESPACE@25..26 " "
                  LBRACE@26..27 "{"
                  WHITESPACE@27..28 " "
                  RBRACE@28..29 "}"
        "#]],
    );
}

#[test]
fn media_unclosed_paren_keeps_later_rules() {
    check(
        "@media (a\n.x { b: c; }\n.y { d: e; }\n",
        expect![[r#"
        SOURCE_FILE@0..36
          MEDIA_RULE@0..22
            AT@0..1 "@"
            IDENT@1..6 "media"
            MEDIA_QUERY@6..12
              WHITESPACE@6..7 " "
              LPAREN@7..8 "("
              IDENT@8..9 "a"
              WHITESPACE@9..10 "\n"
              DOT@10..11 "."
              IDENT@11..12 "x"
            BLOCK@12..22
              WHITESPACE@12..13 " "
              LBRACE@13..14 "{"
              DECLARATION@14..20
                PROPERTY@14..16
                  WHITESPACE@14..15 " "
                  IDENT@15..16 "b"
                COLON@16..17 ":"
                VALUE@17..19
                  VALUE@17..19
                    WHITESPACE@17..18 " "
                    IDENT@18..19 "c"
                SEMICOLON@19..20 ";"
              WHITESPACE@20..21 " "
              RBRACE@21..22 "}"
          RULE_SET@22..35
            SELECTOR_LIST@22..25
              SELECTOR@22..25
                SIMPLE_SELECTOR@22..25
                  WHITESPACE@22..23 "\n"
                  DOT@23..24 "."
                  IDENT@24..25 "y"
            BLOCK@25..35
              WHITESPACE@25..26 " "
              LBRACE@26..27 "{"
              DECLARATION@27..33
                PROPERTY@27..29
                  WHITESPACE@27..28 " "
                  IDENT@28..29 "d"
                COLON@29..30 ":"
                VALUE@30..32
                  VALUE@30..32
                    WHITESPACE@30..31 " "
                    IDENT@31..32 "e"
                SEMICOLON@32..33 ";"
              WHITESPACE@33..34 " "
              RBRACE@34..35 "}"
          WHITESPACE@35..36 "\n"
        errors:
          13..14: expected RPAREN
    "#]],
    );
}

#[test]
fn media_unclosed_paren_at_eof() {
    check(
        "@media (a",
        expect![[r#"
        SOURCE_FILE@0..9
          MEDIA_RULE@0..9
            AT@0..1 "@"
            IDENT@1..6 "media"
            MEDIA_QUERY@6..9
              WHITESPACE@6..7 " "
              LPAREN@7..8 "("
              IDENT@8..9 "a"
        errors:
          9..9: expected RPAREN
    "#]],
    );
}

#[test]
fn media_unclosed_paren_ends_at_the_closing_brace() {
    check(
        ".x { @media (a }\n.y { b: c }\n",
        expect![[r#"
        SOURCE_FILE@0..29
          RULE_SET@0..16
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "x"
            BLOCK@2..16
              WHITESPACE@2..3 " "
              LBRACE@3..4 "{"
              MEDIA_RULE@4..14
                WHITESPACE@4..5 " "
                AT@5..6 "@"
                IDENT@6..11 "media"
                MEDIA_QUERY@11..14
                  WHITESPACE@11..12 " "
                  LPAREN@12..13 "("
                  IDENT@13..14 "a"
              WHITESPACE@14..15 " "
              RBRACE@15..16 "}"
          RULE_SET@16..28
            SELECTOR_LIST@16..19
              SELECTOR@16..19
                SIMPLE_SELECTOR@16..19
                  WHITESPACE@16..17 "\n"
                  DOT@17..18 "."
                  IDENT@18..19 "y"
            BLOCK@19..28
              WHITESPACE@19..20 " "
              LBRACE@20..21 "{"
              DECLARATION@21..26
                PROPERTY@21..23
                  WHITESPACE@21..22 " "
                  IDENT@22..23 "b"
                COLON@23..24 ":"
                VALUE@24..26
                  VALUE@24..26
                    WHITESPACE@24..25 " "
                    IDENT@25..26 "c"
              WHITESPACE@26..27 " "
              RBRACE@27..28 "}"
          WHITESPACE@28..29 "\n"
        errors:
          14..14: expected RPAREN
    "#]],
    );
}

#[test]
fn media_unclosed_nested_paren_ends_at_the_block() {
    check(
        "@media ((a) and (b { c: d }\n",
        expect![[r#"
        SOURCE_FILE@0..28
          MEDIA_RULE@0..27
            AT@0..1 "@"
            IDENT@1..6 "media"
            MEDIA_QUERY@6..18
              WHITESPACE@6..7 " "
              LPAREN@7..8 "("
              LPAREN@8..9 "("
              IDENT@9..10 "a"
              RPAREN@10..11 ")"
              WHITESPACE@11..12 " "
              IDENT@12..15 "and"
              WHITESPACE@15..16 " "
              LPAREN@16..17 "("
              IDENT@17..18 "b"
            BLOCK@18..27
              WHITESPACE@18..19 " "
              LBRACE@19..20 "{"
              DECLARATION@20..25
                PROPERTY@20..22
                  WHITESPACE@20..21 " "
                  IDENT@21..22 "c"
                COLON@22..23 ":"
                VALUE@23..25
                  VALUE@23..25
                    WHITESPACE@23..24 " "
                    IDENT@24..25 "d"
              WHITESPACE@25..26 " "
              RBRACE@26..27 "}"
          WHITESPACE@27..28 "\n"
        errors:
          19..20: expected RPAREN
    "#]],
    );
}

#[test]
fn media_interpolated_brace_and_semicolon_in_parens() {
    check(
        r#"@media (a: #{"{"};b) { c: d }"#,
        expect![[r##"
        SOURCE_FILE@0..29
          MEDIA_RULE@0..29
            AT@0..1 "@"
            IDENT@1..6 "media"
            MEDIA_QUERY@6..20
              WHITESPACE@6..7 " "
              LPAREN@7..8 "("
              IDENT@8..9 "a"
              COLON@9..10 ":"
              INTERPOLATION@10..17
                WHITESPACE@10..11 " "
                HASH_LBRACE@11..13 "#{"
                STRING_LITERAL@13..16
                  QUOTED_STRING@13..16 "\"{\""
                RBRACE@16..17 "}"
              SEMICOLON@17..18 ";"
              IDENT@18..19 "b"
              RPAREN@19..20 ")"
            BLOCK@20..29
              WHITESPACE@20..21 " "
              LBRACE@21..22 "{"
              DECLARATION@22..27
                PROPERTY@22..24
                  WHITESPACE@22..23 " "
                  IDENT@23..24 "c"
                COLON@24..25 ":"
                VALUE@25..27
                  VALUE@25..27
                    WHITESPACE@25..26 " "
                    IDENT@26..27 "d"
              WHITESPACE@27..28 " "
              RBRACE@28..29 "}"
    "##]],
    );
}

#[test]
fn condition_unclosed_paren_is_one_error_and_spares_the_next_rule() {
    for source in [
        "@supports (a: b\n.x { c: d; }\n",
        "@container card (width > 1px\n.x { c: d; }\n",
        "@scope (.a) to (.b\n.x { c: d; }\n",
        "@media screen and (a\n.x { c: d; }\n",
    ] {
        let (green, errors) = sass_parser::parse_scss(source);
        let tree = SyntaxNode::new_root(green);
        assert_eq!(
            tree.text().to_string(),
            source,
            "lossless round-trip failed"
        );
        let messages: Vec<_> = errors.iter().map(|(m, _)| m.as_str()).collect();
        assert_eq!(messages, ["expected RPAREN"], "{source:?}");
        let brace = u32::try_from(source.find('{').unwrap()).unwrap();
        assert_eq!(u32::from(errors[0].1.start()), brace, "{source:?}");
        let declarations = tree
            .descendants()
            .filter(|n| n.kind() == sass_parser::syntax_kind::SyntaxKind::DECLARATION)
            .count();
        assert_eq!(declarations, 1, "{source:?}");
    }
}

#[test]
fn media_unclosed_paren_nested_in_a_rule_ends_at_the_first_block() {
    check(
        ".x {\n  @media (a\n  .b { c: d }\n  .e { f: g }\n}\n",
        expect![[r#"
        SOURCE_FILE@0..47
          RULE_SET@0..46
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "x"
            BLOCK@2..46
              WHITESPACE@2..3 " "
              LBRACE@3..4 "{"
              MEDIA_RULE@4..30
                WHITESPACE@4..7 "\n  "
                AT@7..8 "@"
                IDENT@8..13 "media"
                MEDIA_QUERY@13..21
                  WHITESPACE@13..14 " "
                  LPAREN@14..15 "("
                  IDENT@15..16 "a"
                  WHITESPACE@16..19 "\n  "
                  DOT@19..20 "."
                  IDENT@20..21 "b"
                BLOCK@21..30
                  WHITESPACE@21..22 " "
                  LBRACE@22..23 "{"
                  DECLARATION@23..28
                    PROPERTY@23..25
                      WHITESPACE@23..24 " "
                      IDENT@24..25 "c"
                    COLON@25..26 ":"
                    VALUE@26..28
                      VALUE@26..28
                        WHITESPACE@26..27 " "
                        IDENT@27..28 "d"
                  WHITESPACE@28..29 " "
                  RBRACE@29..30 "}"
              RULE_SET@30..44
                SELECTOR_LIST@30..35
                  SELECTOR@30..35
                    SIMPLE_SELECTOR@30..35
                      WHITESPACE@30..33 "\n  "
                      DOT@33..34 "."
                      IDENT@34..35 "e"
                BLOCK@35..44
                  WHITESPACE@35..36 " "
                  LBRACE@36..37 "{"
                  DECLARATION@37..42
                    PROPERTY@37..39
                      WHITESPACE@37..38 " "
                      IDENT@38..39 "f"
                    COLON@39..40 ":"
                    VALUE@40..42
                      VALUE@40..42
                        WHITESPACE@40..41 " "
                        IDENT@41..42 "g"
                  WHITESPACE@42..43 " "
                  RBRACE@43..44 "}"
              WHITESPACE@44..45 "\n"
              RBRACE@45..46 "}"
          WHITESPACE@46..47 "\n"
        errors:
          22..23: expected RPAREN
    "#]],
    );
}

#[test]
fn media_unclosed_paren_is_not_closed_by_a_call_below() {
    check(
        "@media (a { .b { c: d } }\n.x { y: z }\n.w { v: calc(1px)) }\n",
        expect![[r#"
            SOURCE_FILE@0..59
              MEDIA_RULE@0..25
                AT@0..1 "@"
                IDENT@1..6 "media"
                MEDIA_QUERY@6..9
                  WHITESPACE@6..7 " "
                  LPAREN@7..8 "("
                  IDENT@8..9 "a"
                BLOCK@9..25
                  WHITESPACE@9..10 " "
                  LBRACE@10..11 "{"
                  RULE_SET@11..23
                    SELECTOR_LIST@11..14
                      SELECTOR@11..14
                        SIMPLE_SELECTOR@11..14
                          WHITESPACE@11..12 " "
                          DOT@12..13 "."
                          IDENT@13..14 "b"
                    BLOCK@14..23
                      WHITESPACE@14..15 " "
                      LBRACE@15..16 "{"
                      DECLARATION@16..21
                        PROPERTY@16..18
                          WHITESPACE@16..17 " "
                          IDENT@17..18 "c"
                        COLON@18..19 ":"
                        VALUE@19..21
                          VALUE@19..21
                            WHITESPACE@19..20 " "
                            IDENT@20..21 "d"
                      WHITESPACE@21..22 " "
                      RBRACE@22..23 "}"
                  WHITESPACE@23..24 " "
                  RBRACE@24..25 "}"
              RULE_SET@25..37
                SELECTOR_LIST@25..28
                  SELECTOR@25..28
                    SIMPLE_SELECTOR@25..28
                      WHITESPACE@25..26 "\n"
                      DOT@26..27 "."
                      IDENT@27..28 "x"
                BLOCK@28..37
                  WHITESPACE@28..29 " "
                  LBRACE@29..30 "{"
                  DECLARATION@30..35
                    PROPERTY@30..32
                      WHITESPACE@30..31 " "
                      IDENT@31..32 "y"
                    COLON@32..33 ":"
                    VALUE@33..35
                      VALUE@33..35
                        WHITESPACE@33..34 " "
                        IDENT@34..35 "z"
                  WHITESPACE@35..36 " "
                  RBRACE@36..37 "}"
              RULE_SET@37..58
                SELECTOR_LIST@37..40
                  SELECTOR@37..40
                    SIMPLE_SELECTOR@37..40
                      WHITESPACE@37..38 "\n"
                      DOT@38..39 "."
                      IDENT@39..40 "w"
                BLOCK@40..58
                  WHITESPACE@40..41 " "
                  LBRACE@41..42 "{"
                  DECLARATION@42..56
                    PROPERTY@42..44
                      WHITESPACE@42..43 " "
                      IDENT@43..44 "v"
                    COLON@44..45 ":"
                    VALUE@45..56
                      CALCULATION@45..55
                        WHITESPACE@45..46 " "
                        IDENT@46..50 "calc"
                        LPAREN@50..51 "("
                        CALC_VALUE@51..54
                          DIMENSION@51..54
                            NUMBER@51..52 "1"
                            IDENT@52..54 "px"
                        RPAREN@54..55 ")"
                      RPAREN@55..56 ")"
                  WHITESPACE@56..57 " "
                  RBRACE@57..58 "}"
              WHITESPACE@58..59 "\n"
            errors:
              10..11: expected RPAREN
        "#]],
    );
}

#[test]
fn media_paren_closed_inside_a_block_is_unclosed_at_the_block() {
    check(
        "@media (a {\n .b { c: d) }\n}\n",
        expect![[r#"
        SOURCE_FILE@0..28
          MEDIA_RULE@0..27
            AT@0..1 "@"
            IDENT@1..6 "media"
            MEDIA_QUERY@6..9
              WHITESPACE@6..7 " "
              LPAREN@7..8 "("
              IDENT@8..9 "a"
            BLOCK@9..27
              WHITESPACE@9..10 " "
              LBRACE@10..11 "{"
              RULE_SET@11..25
                SELECTOR_LIST@11..15
                  SELECTOR@11..15
                    SIMPLE_SELECTOR@11..15
                      WHITESPACE@11..13 "\n "
                      DOT@13..14 "."
                      IDENT@14..15 "b"
                BLOCK@15..25
                  WHITESPACE@15..16 " "
                  LBRACE@16..17 "{"
                  DECLARATION@17..23
                    PROPERTY@17..19
                      WHITESPACE@17..18 " "
                      IDENT@18..19 "c"
                    COLON@19..20 ":"
                    VALUE@20..23
                      VALUE@20..22
                        WHITESPACE@20..21 " "
                        IDENT@21..22 "d"
                      RPAREN@22..23 ")"
                  WHITESPACE@23..24 " "
                  RBRACE@24..25 "}"
              WHITESPACE@25..26 "\n"
              RBRACE@26..27 "}"
          WHITESPACE@27..28 "\n"
        errors:
          10..11: expected RPAREN
    "#]],
    );
}

#[test]
fn nested_condition_unclosed_paren_spares_the_last_rule_of_the_block() {
    check(
        ".x {\n  @supports (a: b\n  .b { c: d }\n}\n",
        expect![[r#"
        SOURCE_FILE@0..39
          RULE_SET@0..38
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "x"
            BLOCK@2..38
              WHITESPACE@2..3 " "
              LBRACE@3..4 "{"
              SUPPORTS_RULE@4..36
                WHITESPACE@4..7 "\n  "
                AT@7..8 "@"
                IDENT@8..16 "supports"
                SUPPORTS_CONDITION@16..27
                  WHITESPACE@16..17 " "
                  LPAREN@17..18 "("
                  IDENT@18..19 "a"
                  COLON@19..20 ":"
                  WHITESPACE@20..21 " "
                  IDENT@21..22 "b"
                  WHITESPACE@22..25 "\n  "
                  DOT@25..26 "."
                  IDENT@26..27 "b"
                BLOCK@27..36
                  WHITESPACE@27..28 " "
                  LBRACE@28..29 "{"
                  DECLARATION@29..34
                    PROPERTY@29..31
                      WHITESPACE@29..30 " "
                      IDENT@30..31 "c"
                    COLON@31..32 ":"
                    VALUE@32..34
                      VALUE@32..34
                        WHITESPACE@32..33 " "
                        IDENT@33..34 "d"
                  WHITESPACE@34..35 " "
                  RBRACE@35..36 "}"
              WHITESPACE@36..37 "\n"
              RBRACE@37..38 "}"
          WHITESPACE@38..39 "\n"
        errors:
          28..29: expected RPAREN
    "#]],
    );
}

#[test]
fn nested_condition_unclosed_paren_spares_the_rule_below() {
    use sass_parser::syntax_kind::SyntaxKind::{DECLARATION, RULE_SET};
    for at_rule in [
        "@media (a",
        "@supports (a: b",
        "@container (width > 1px",
        "@scope (.a",
    ] {
        let source = format!(".x {{\n  {at_rule}\n  .b {{ c: d }}\n  .e {{ f: g }}\n}}\n");
        let (green, errors) = sass_parser::parse_scss(&source);
        let tree = SyntaxNode::new_root(green);
        assert_eq!(
            tree.text().to_string(),
            source,
            "lossless round-trip failed"
        );
        let messages: Vec<_> = errors.iter().map(|(m, _)| m.as_str()).collect();
        assert_eq!(messages, ["expected RPAREN"], "{source:?}");
        let brace = u32::try_from(source.find(".b {").unwrap() + 3).unwrap();
        assert_eq!(u32::from(errors[0].1.start()), brace, "{source:?}");
        let count = |kind| tree.descendants().filter(|n| n.kind() == kind).count();
        // `.x` and `.e`; `.b`'s block is the at-rule's.
        assert_eq!(count(RULE_SET), 2, "{source:?}");
        assert_eq!(count(DECLARATION), 2, "{source:?}");
    }
}

#[test]
fn supports_balanced_braces_in_parens_stay_part_of_them() {
    check(
        "@supports (a {b} c) { d: e }",
        expect![[r#"
        SOURCE_FILE@0..28
          SUPPORTS_RULE@0..28
            AT@0..1 "@"
            IDENT@1..9 "supports"
            SUPPORTS_CONDITION@9..19
              WHITESPACE@9..10 " "
              LPAREN@10..11 "("
              IDENT@11..12 "a"
              WHITESPACE@12..13 " "
              LBRACE@13..14 "{"
              IDENT@14..15 "b"
              RBRACE@15..16 "}"
              WHITESPACE@16..17 " "
              IDENT@17..18 "c"
              RPAREN@18..19 ")"
            BLOCK@19..28
              WHITESPACE@19..20 " "
              LBRACE@20..21 "{"
              DECLARATION@21..26
                PROPERTY@21..23
                  WHITESPACE@21..22 " "
                  IDENT@22..23 "d"
                COLON@23..24 ":"
                VALUE@24..26
                  VALUE@24..26
                    WHITESPACE@24..25 " "
                    IDENT@25..26 "e"
              WHITESPACE@26..27 " "
              RBRACE@27..28 "}"
    "#]],
    );
}

#[test]
fn supports_unclosed_paren_before_a_brace_group_ends_at_the_group() {
    check(
        ".x { @supports (a {b} }\n.y { c: d }\n",
        expect![[r#"
            SOURCE_FILE@0..36
              RULE_SET@0..23
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "x"
                BLOCK@2..23
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  SUPPORTS_RULE@4..21
                    WHITESPACE@4..5 " "
                    AT@5..6 "@"
                    IDENT@6..14 "supports"
                    SUPPORTS_CONDITION@14..17
                      WHITESPACE@14..15 " "
                      LPAREN@15..16 "("
                      IDENT@16..17 "a"
                    BLOCK@17..21
                      WHITESPACE@17..18 " "
                      LBRACE@18..19 "{"
                      RULE_SET@19..20
                        SELECTOR_LIST@19..20
                          SELECTOR@19..20
                            SIMPLE_SELECTOR@19..20
                              IDENT@19..20 "b"
                      RBRACE@20..21 "}"
                  WHITESPACE@21..22 " "
                  RBRACE@22..23 "}"
              RULE_SET@23..35
                SELECTOR_LIST@23..26
                  SELECTOR@23..26
                    SIMPLE_SELECTOR@23..26
                      WHITESPACE@23..24 "\n"
                      DOT@24..25 "."
                      IDENT@25..26 "y"
                BLOCK@26..35
                  WHITESPACE@26..27 " "
                  LBRACE@27..28 "{"
                  DECLARATION@28..33
                    PROPERTY@28..30
                      WHITESPACE@28..29 " "
                      IDENT@29..30 "c"
                    COLON@30..31 ":"
                    VALUE@31..33
                      VALUE@31..33
                        WHITESPACE@31..32 " "
                        IDENT@32..33 "d"
                  WHITESPACE@33..34 " "
                  RBRACE@34..35 "}"
              WHITESPACE@35..36 "\n"
            errors:
              18..19: expected RPAREN
              20..20: expected `{`
        "#]],
    );
}

// ── @supports ───────────────────────────────────────────────────────────

#[test]
fn supports_simple() {
    check(
        "@supports (display: grid) { }",
        expect![[r#"
            SOURCE_FILE@0..29
              SUPPORTS_RULE@0..29
                AT@0..1 "@"
                IDENT@1..9 "supports"
                SUPPORTS_CONDITION@9..25
                  WHITESPACE@9..10 " "
                  LPAREN@10..11 "("
                  IDENT@11..18 "display"
                  COLON@18..19 ":"
                  WHITESPACE@19..20 " "
                  IDENT@20..24 "grid"
                  RPAREN@24..25 ")"
                BLOCK@25..29
                  WHITESPACE@25..26 " "
                  LBRACE@26..27 "{"
                  WHITESPACE@27..28 " "
                  RBRACE@28..29 "}"
        "#]],
    );
}

// ── @keyframes ──────────────────────────────────────────────────────────

#[test]
fn keyframes_simple() {
    check(
        "@keyframes fade { from { opacity: 0; } to { opacity: 1; } }",
        expect![[r#"
            SOURCE_FILE@0..59
              KEYFRAMES_RULE@0..59
                AT@0..1 "@"
                IDENT@1..10 "keyframes"
                WHITESPACE@10..11 " "
                IDENT@11..15 "fade"
                WHITESPACE@15..16 " "
                LBRACE@16..17 "{"
                KEYFRAME_SELECTOR@17..38
                  WHITESPACE@17..18 " "
                  IDENT@18..22 "from"
                  BLOCK@22..38
                    WHITESPACE@22..23 " "
                    LBRACE@23..24 "{"
                    DECLARATION@24..36
                      PROPERTY@24..32
                        WHITESPACE@24..25 " "
                        IDENT@25..32 "opacity"
                      COLON@32..33 ":"
                      VALUE@33..35
                        NUMBER_LITERAL@33..35
                          WHITESPACE@33..34 " "
                          NUMBER@34..35 "0"
                      SEMICOLON@35..36 ";"
                    WHITESPACE@36..37 " "
                    RBRACE@37..38 "}"
                KEYFRAME_SELECTOR@38..57
                  WHITESPACE@38..39 " "
                  IDENT@39..41 "to"
                  BLOCK@41..57
                    WHITESPACE@41..42 " "
                    LBRACE@42..43 "{"
                    DECLARATION@43..55
                      PROPERTY@43..51
                        WHITESPACE@43..44 " "
                        IDENT@44..51 "opacity"
                      COLON@51..52 ":"
                      VALUE@52..54
                        NUMBER_LITERAL@52..54
                          WHITESPACE@52..53 " "
                          NUMBER@53..54 "1"
                      SEMICOLON@54..55 ";"
                    WHITESPACE@55..56 " "
                    RBRACE@56..57 "}"
                WHITESPACE@57..58 " "
                RBRACE@58..59 "}"
        "#]],
    );
}

// ── @layer ──────────────────────────────────────────────────────────────

#[test]
fn layer_block() {
    check(
        "@layer base { body { } }",
        expect![[r#"
            SOURCE_FILE@0..24
              LAYER_RULE@0..24
                AT@0..1 "@"
                IDENT@1..6 "layer"
                WHITESPACE@6..7 " "
                IDENT@7..11 "base"
                BLOCK@11..24
                  WHITESPACE@11..12 " "
                  LBRACE@12..13 "{"
                  RULE_SET@13..22
                    SELECTOR_LIST@13..18
                      SELECTOR@13..18
                        SIMPLE_SELECTOR@13..18
                          WHITESPACE@13..14 " "
                          IDENT@14..18 "body"
                    BLOCK@18..22
                      WHITESPACE@18..19 " "
                      LBRACE@19..20 "{"
                      WHITESPACE@20..21 " "
                      RBRACE@21..22 "}"
                  WHITESPACE@22..23 " "
                  RBRACE@23..24 "}"
        "#]],
    );
}

#[test]
fn layer_statement() {
    check(
        "@layer base, theme;",
        expect![[r#"
            SOURCE_FILE@0..19
              LAYER_RULE@0..19
                AT@0..1 "@"
                IDENT@1..6 "layer"
                WHITESPACE@6..7 " "
                IDENT@7..11 "base"
                COMMA@11..12 ","
                WHITESPACE@12..13 " "
                IDENT@13..18 "theme"
                SEMICOLON@18..19 ";"
        "#]],
    );
}

// ── @container ──────────────────────────────────────────────────────────

#[test]
fn container_simple() {
    check(
        "@container (min-width: 700px) { }",
        expect![[r#"
            SOURCE_FILE@0..33
              CONTAINER_RULE@0..33
                AT@0..1 "@"
                IDENT@1..10 "container"
                WHITESPACE@10..11 " "
                LPAREN@11..12 "("
                IDENT@12..21 "min-width"
                COLON@21..22 ":"
                WHITESPACE@22..23 " "
                NUMBER@23..26 "700"
                IDENT@26..28 "px"
                RPAREN@28..29 ")"
                BLOCK@29..33
                  WHITESPACE@29..30 " "
                  LBRACE@30..31 "{"
                  WHITESPACE@31..32 " "
                  RBRACE@32..33 "}"
        "#]],
    );
}

// ── @scope ──────────────────────────────────────────────────────────────

#[test]
fn scope_simple() {
    check(
        "@scope (.card) to (.content) { }",
        expect![[r#"
            SOURCE_FILE@0..32
              SCOPE_RULE@0..32
                AT@0..1 "@"
                IDENT@1..6 "scope"
                WHITESPACE@6..7 " "
                LPAREN@7..8 "("
                DOT@8..9 "."
                IDENT@9..13 "card"
                RPAREN@13..14 ")"
                WHITESPACE@14..15 " "
                IDENT@15..17 "to"
                WHITESPACE@17..18 " "
                LPAREN@18..19 "("
                DOT@19..20 "."
                IDENT@20..27 "content"
                RPAREN@27..28 ")"
                BLOCK@28..32
                  WHITESPACE@28..29 " "
                  LBRACE@29..30 "{"
                  WHITESPACE@30..31 " "
                  RBRACE@31..32 "}"
        "#]],
    );
}

// ── @property ───────────────────────────────────────────────────────────

#[test]
fn property_rule() {
    check(
        "@property --color { }",
        expect![[r#"
            SOURCE_FILE@0..21
              PROPERTY_RULE@0..21
                AT@0..1 "@"
                IDENT@1..9 "property"
                WHITESPACE@9..10 " "
                IDENT@10..17 "--color"
                BLOCK@17..21
                  WHITESPACE@17..18 " "
                  LBRACE@18..19 "{"
                  WHITESPACE@19..20 " "
                  RBRACE@20..21 "}"
        "#]],
    );
}

// ── @charset ────────────────────────────────────────────────────────────

#[test]
fn charset_rule() {
    check(
        "@charset \"UTF-8\";",
        expect![[r#"
            SOURCE_FILE@0..17
              CHARSET_RULE@0..17
                AT@0..1 "@"
                IDENT@1..8 "charset"
                WHITESPACE@8..9 " "
                QUOTED_STRING@9..16 "\"UTF-8\""
                SEMICOLON@16..17 ";"
        "#]],
    );
}

// ── @font-face ──────────────────────────────────────────────────────────

#[test]
fn font_face_rule() {
    check(
        "@font-face { font-family: \"Noto\"; }",
        expect![[r#"
            SOURCE_FILE@0..35
              FONT_FACE_RULE@0..35
                AT@0..1 "@"
                IDENT@1..10 "font-face"
                BLOCK@10..35
                  WHITESPACE@10..11 " "
                  LBRACE@11..12 "{"
                  DECLARATION@12..33
                    PROPERTY@12..24
                      WHITESPACE@12..13 " "
                      IDENT@13..24 "font-family"
                    COLON@24..25 ":"
                    VALUE@25..32
                      STRING_LITERAL@25..32
                        WHITESPACE@25..26 " "
                        QUOTED_STRING@26..32 "\"Noto\""
                    SEMICOLON@32..33 ";"
                  WHITESPACE@33..34 " "
                  RBRACE@34..35 "}"
        "#]],
    );
}

// ── @page ───────────────────────────────────────────────────────────────

#[test]
fn page_rule() {
    check(
        "@page :first { margin: 2cm; }",
        expect![[r#"
            SOURCE_FILE@0..29
              PAGE_RULE@0..29
                AT@0..1 "@"
                IDENT@1..5 "page"
                WHITESPACE@5..6 " "
                COLON@6..7 ":"
                IDENT@7..12 "first"
                BLOCK@12..29
                  WHITESPACE@12..13 " "
                  LBRACE@13..14 "{"
                  DECLARATION@14..27
                    PROPERTY@14..21
                      WHITESPACE@14..15 " "
                      IDENT@15..21 "margin"
                    COLON@21..22 ":"
                    VALUE@22..26
                      DIMENSION@22..26
                        WHITESPACE@22..23 " "
                        NUMBER@23..24 "2"
                        IDENT@24..26 "cm"
                    SEMICOLON@26..27 ";"
                  WHITESPACE@27..28 " "
                  RBRACE@28..29 "}"
        "#]],
    );
}

// ── @namespace ──────────────────────────────────────────────────────────

#[test]
fn namespace_rule() {
    check(
        "@namespace svg \"http://www.w3.org/2000/svg\";",
        expect![[r#"
            SOURCE_FILE@0..44
              NAMESPACE_RULE@0..44
                AT@0..1 "@"
                IDENT@1..10 "namespace"
                WHITESPACE@10..11 " "
                IDENT@11..14 "svg"
                WHITESPACE@14..15 " "
                QUOTED_STRING@15..43 "\"http://www.w3.org/2000/svg\""
                SEMICOLON@43..44 ";"
        "#]],
    );
}

// ── @use ────────────────────────────────────────────────────────────────

#[test]
fn use_simple() {
    check(
        "@use \"colors\";",
        expect![[r#"
            SOURCE_FILE@0..14
              USE_RULE@0..14
                AT@0..1 "@"
                IDENT@1..4 "use"
                WHITESPACE@4..5 " "
                QUOTED_STRING@5..13 "\"colors\""
                SEMICOLON@13..14 ";"
        "#]],
    );
}

#[test]
fn use_as_namespace() {
    check(
        "@use \"colors\" as c;",
        expect![[r#"
            SOURCE_FILE@0..19
              USE_RULE@0..19
                AT@0..1 "@"
                IDENT@1..4 "use"
                WHITESPACE@4..5 " "
                QUOTED_STRING@5..13 "\"colors\""
                WHITESPACE@13..14 " "
                IDENT@14..16 "as"
                WHITESPACE@16..17 " "
                IDENT@17..18 "c"
                SEMICOLON@18..19 ";"
        "#]],
    );
}

#[test]
fn use_as_star() {
    check(
        "@use \"colors\" as *;",
        expect![[r#"
            SOURCE_FILE@0..19
              USE_RULE@0..19
                AT@0..1 "@"
                IDENT@1..4 "use"
                WHITESPACE@4..5 " "
                QUOTED_STRING@5..13 "\"colors\""
                WHITESPACE@13..14 " "
                IDENT@14..16 "as"
                WHITESPACE@16..17 " "
                STAR@17..18 "*"
                SEMICOLON@18..19 ";"
        "#]],
    );
}

// ── @forward ────────────────────────────────────────────────────────────

#[test]
fn forward_simple() {
    check(
        "@forward \"mixins\";",
        expect![[r#"
            SOURCE_FILE@0..18
              FORWARD_RULE@0..18
                AT@0..1 "@"
                IDENT@1..8 "forward"
                WHITESPACE@8..9 " "
                QUOTED_STRING@9..17 "\"mixins\""
                SEMICOLON@17..18 ";"
        "#]],
    );
}

#[test]
fn forward_hide() {
    check(
        "@forward \"src\" hide $secret, internal;",
        expect![[r#"
            SOURCE_FILE@0..38
              FORWARD_RULE@0..38
                AT@0..1 "@"
                IDENT@1..8 "forward"
                WHITESPACE@8..9 " "
                QUOTED_STRING@9..14 "\"src\""
                WHITESPACE@14..15 " "
                IDENT@15..19 "hide"
                WHITESPACE@19..20 " "
                DOLLAR@20..21 "$"
                IDENT@21..27 "secret"
                COMMA@27..28 ","
                WHITESPACE@28..29 " "
                IDENT@29..37 "internal"
                SEMICOLON@37..38 ";"
        "#]],
    );
}

// ── @import ─────────────────────────────────────────────────────────────

#[test]
fn import_simple() {
    check(
        "@import \"base\";",
        expect![[r#"
            SOURCE_FILE@0..15
              IMPORT_RULE@0..15
                AT@0..1 "@"
                IDENT@1..7 "import"
                WHITESPACE@7..8 " "
                QUOTED_STRING@8..14 "\"base\""
                SEMICOLON@14..15 ";"
        "#]],
    );
}

// ── Generic (unknown) at-rule ───────────────────────────────────────────

#[test]
fn generic_at_rule_statement() {
    check(
        "@unknown foo;",
        expect![[r#"
            SOURCE_FILE@0..13
              GENERIC_AT_RULE@0..13
                AT@0..1 "@"
                IDENT@1..8 "unknown"
                WHITESPACE@8..9 " "
                IDENT@9..12 "foo"
                SEMICOLON@12..13 ";"
        "#]],
    );
}

#[test]
fn generic_at_rule_block() {
    check(
        "@unknown { }",
        expect![[r#"
            SOURCE_FILE@0..12
              GENERIC_AT_RULE@0..12
                AT@0..1 "@"
                IDENT@1..8 "unknown"
                BLOCK@8..12
                  WHITESPACE@8..9 " "
                  LBRACE@9..10 "{"
                  WHITESPACE@10..11 " "
                  RBRACE@11..12 "}"
        "#]],
    );
}

#[test]
fn generic_at_rule_unclosed_paren_keeps_later_rules() {
    check(
        "@foo (a\n.x { y: z; }\n.w { v: u; }\n",
        expect![[r#"
            SOURCE_FILE@0..34
              GENERIC_AT_RULE@0..20
                AT@0..1 "@"
                IDENT@1..4 "foo"
                WHITESPACE@4..5 " "
                LPAREN@5..6 "("
                IDENT@6..7 "a"
                WHITESPACE@7..8 "\n"
                DOT@8..9 "."
                IDENT@9..10 "x"
                BLOCK@10..20
                  WHITESPACE@10..11 " "
                  LBRACE@11..12 "{"
                  DECLARATION@12..18
                    PROPERTY@12..14
                      WHITESPACE@12..13 " "
                      IDENT@13..14 "y"
                    COLON@14..15 ":"
                    VALUE@15..17
                      VALUE@15..17
                        WHITESPACE@15..16 " "
                        IDENT@16..17 "z"
                    SEMICOLON@17..18 ";"
                  WHITESPACE@18..19 " "
                  RBRACE@19..20 "}"
              RULE_SET@20..33
                SELECTOR_LIST@20..23
                  SELECTOR@20..23
                    SIMPLE_SELECTOR@20..23
                      WHITESPACE@20..21 "\n"
                      DOT@21..22 "."
                      IDENT@22..23 "w"
                BLOCK@23..33
                  WHITESPACE@23..24 " "
                  LBRACE@24..25 "{"
                  DECLARATION@25..31
                    PROPERTY@25..27
                      WHITESPACE@25..26 " "
                      IDENT@26..27 "v"
                    COLON@27..28 ":"
                    VALUE@28..30
                      VALUE@28..30
                        WHITESPACE@28..29 " "
                        IDENT@29..30 "u"
                    SEMICOLON@30..31 ";"
                  WHITESPACE@31..32 " "
                  RBRACE@32..33 "}"
              WHITESPACE@33..34 "\n"
            errors:
              11..12: expected RPAREN
        "#]],
    );
}

#[test]
fn generic_at_rule_unclosed_paren_at_eof() {
    check(
        "@foo (a",
        expect![[r#"
        SOURCE_FILE@0..7
          GENERIC_AT_RULE@0..7
            AT@0..1 "@"
            IDENT@1..4 "foo"
            WHITESPACE@4..5 " "
            LPAREN@5..6 "("
            IDENT@6..7 "a"
        errors:
          7..7: expected RPAREN
    "#]],
    );
}

#[test]
fn generic_at_rule_brace_in_parens() {
    check(
        "@foo (a {b});",
        expect![[r#"
        SOURCE_FILE@0..13
          GENERIC_AT_RULE@0..11
            AT@0..1 "@"
            IDENT@1..4 "foo"
            WHITESPACE@4..5 " "
            LPAREN@5..6 "("
            IDENT@6..7 "a"
            BLOCK@7..11
              WHITESPACE@7..8 " "
              LBRACE@8..9 "{"
              RULE_SET@9..10
                SELECTOR_LIST@9..10
                  SELECTOR@9..10
                    SIMPLE_SELECTOR@9..10
                      IDENT@9..10 "b"
              RBRACE@10..11 "}"
          ERROR@11..12
            RPAREN@11..12 ")"
          SEMICOLON@12..13 ";"
        errors:
          8..9: expected RPAREN
          10..10: expected `{`
          11..12: expected rule
    "#]],
    );
}

#[test]
fn generic_at_rule_semicolon_in_parens() {
    check(
        "@foo (a;b);",
        expect![[r#"
        SOURCE_FILE@0..11
          GENERIC_AT_RULE@0..11
            AT@0..1 "@"
            IDENT@1..4 "foo"
            WHITESPACE@4..5 " "
            LPAREN@5..6 "("
            IDENT@6..7 "a"
            SEMICOLON@7..8 ";"
            IDENT@8..9 "b"
            RPAREN@9..10 ")"
            SEMICOLON@10..11 ";"
    "#]],
    );
}

#[test]
fn generic_at_rule_interpolated_brace_in_parens() {
    check(
        r#"@foo (#{"{"});"#,
        expect![[r##"
        SOURCE_FILE@0..14
          GENERIC_AT_RULE@0..14
            AT@0..1 "@"
            IDENT@1..4 "foo"
            WHITESPACE@4..5 " "
            LPAREN@5..6 "("
            HASH_LBRACE@6..8 "#{"
            QUOTED_STRING@8..11 "\"{\""
            RBRACE@11..12 "}"
            RPAREN@12..13 ")"
            SEMICOLON@13..14 ";"
    "##]],
    );
}

// ── Document rules ──────────────────────────────────────────────────────

#[test]
fn moz_document_url_prefix() {
    check(
        "@-moz-document url-prefix(http://x.org/a/) { a { b: c } }",
        expect![[r#"
            SOURCE_FILE@0..57
              GENERIC_AT_RULE@0..57
                AT@0..1 "@"
                IDENT@1..14 "-moz-document"
                WHITESPACE@14..15 " "
                IDENT@15..25 "url-prefix"
                LPAREN@25..26 "("
                URL_CONTENTS@26..41 "http://x.org/a/"
                RPAREN@41..42 ")"
                BLOCK@42..57
                  WHITESPACE@42..43 " "
                  LBRACE@43..44 "{"
                  RULE_SET@44..55
                    SELECTOR_LIST@44..46
                      SELECTOR@44..46
                        SIMPLE_SELECTOR@44..46
                          WHITESPACE@44..45 " "
                          IDENT@45..46 "a"
                    BLOCK@46..55
                      WHITESPACE@46..47 " "
                      LBRACE@47..48 "{"
                      DECLARATION@48..53
                        PROPERTY@48..50
                          WHITESPACE@48..49 " "
                          IDENT@49..50 "b"
                        COLON@50..51 ":"
                        VALUE@51..53
                          VALUE@51..53
                            WHITESPACE@51..52 " "
                            IDENT@52..53 "c"
                      WHITESPACE@53..54 " "
                      RBRACE@54..55 "}"
                  WHITESPACE@55..56 " "
                  RBRACE@56..57 "}"
        "#]],
    );
}

#[test]
fn moz_document_function_list() {
    check(
        "@-moz-document domain(a.org), url-prefix(), url(http://a/b) {}",
        expect![[r#"
            SOURCE_FILE@0..62
              GENERIC_AT_RULE@0..62
                AT@0..1 "@"
                IDENT@1..14 "-moz-document"
                WHITESPACE@14..15 " "
                IDENT@15..21 "domain"
                LPAREN@21..22 "("
                URL_CONTENTS@22..27 "a.org"
                RPAREN@27..28 ")"
                COMMA@28..29 ","
                WHITESPACE@29..30 " "
                IDENT@30..40 "url-prefix"
                LPAREN@40..41 "("
                RPAREN@41..42 ")"
                COMMA@42..43 ","
                WHITESPACE@43..44 " "
                IDENT@44..47 "url"
                LPAREN@47..48 "("
                URL_CONTENTS@48..58 "http://a/b"
                RPAREN@58..59 ")"
                BLOCK@59..62
                  WHITESPACE@59..60 " "
                  LBRACE@60..61 "{"
                  RBRACE@61..62 "}"
        "#]],
    );
}

#[test]
fn document_url_prefix() {
    check(
        "@document url-prefix(//x.org/) {}",
        expect![[r#"
        SOURCE_FILE@0..33
          GENERIC_AT_RULE@0..33
            AT@0..1 "@"
            IDENT@1..9 "document"
            WHITESPACE@9..10 " "
            IDENT@10..20 "url-prefix"
            LPAREN@20..21 "("
            URL_CONTENTS@21..29 "//x.org/"
            RPAREN@29..30 ")"
            BLOCK@30..33
              WHITESPACE@30..31 " "
              LBRACE@31..32 "{"
              RBRACE@32..33 "}"
    "#]],
    );
}

#[test]
fn document_domain_is_not_a_url() {
    // Only `@-moz-document` reads `domain(` as a URL, as in Dart Sass.
    check(
        "@document domain(a//b);",
        expect![[r#"
        SOURCE_FILE@0..23
          GENERIC_AT_RULE@0..18
            AT@0..1 "@"
            IDENT@1..9 "document"
            WHITESPACE@9..10 " "
            IDENT@10..16 "domain"
            LPAREN@16..17 "("
            IDENT@17..18 "a"
          SINGLE_LINE_COMMENT@18..23 "//b);"
        errors:
          18..18: expected RPAREN
    "#]],
    );
}

#[test]
fn moz_document_unclosed_url_prefix() {
    check(
        "@-moz-document url-prefix(http://x",
        expect![[r#"
        SOURCE_FILE@0..34
          GENERIC_AT_RULE@0..34
            AT@0..1 "@"
            IDENT@1..14 "-moz-document"
            WHITESPACE@14..15 " "
            IDENT@15..25 "url-prefix"
            LPAREN@25..26 "("
            URL_CONTENTS@26..34 "http://x"
        errors:
          34..34: expected RPAREN
    "#]],
    );
}

#[test]
fn domain_function_outside_document_prelude() {
    check(
        "@-moz-document domain(a) { a { b: domain($x) } }\nc { d: domain($y) }",
        expect![[r#"
            SOURCE_FILE@0..68
              GENERIC_AT_RULE@0..48
                AT@0..1 "@"
                IDENT@1..14 "-moz-document"
                WHITESPACE@14..15 " "
                IDENT@15..21 "domain"
                LPAREN@21..22 "("
                URL_CONTENTS@22..23 "a"
                RPAREN@23..24 ")"
                BLOCK@24..48
                  WHITESPACE@24..25 " "
                  LBRACE@25..26 "{"
                  RULE_SET@26..46
                    SELECTOR_LIST@26..28
                      SELECTOR@26..28
                        SIMPLE_SELECTOR@26..28
                          WHITESPACE@26..27 " "
                          IDENT@27..28 "a"
                    BLOCK@28..46
                      WHITESPACE@28..29 " "
                      LBRACE@29..30 "{"
                      DECLARATION@30..44
                        PROPERTY@30..32
                          WHITESPACE@30..31 " "
                          IDENT@31..32 "b"
                        COLON@32..33 ":"
                        VALUE@33..44
                          FUNCTION_CALL@33..44
                            WHITESPACE@33..34 " "
                            IDENT@34..40 "domain"
                            ARG_LIST@40..44
                              LPAREN@40..41 "("
                              ARG@41..43
                                VARIABLE_REF@41..43
                                  DOLLAR@41..42 "$"
                                  IDENT@42..43 "x"
                              RPAREN@43..44 ")"
                      WHITESPACE@44..45 " "
                      RBRACE@45..46 "}"
                  WHITESPACE@46..47 " "
                  RBRACE@47..48 "}"
              RULE_SET@48..68
                SELECTOR_LIST@48..50
                  SELECTOR@48..50
                    SIMPLE_SELECTOR@48..50
                      WHITESPACE@48..49 "\n"
                      IDENT@49..50 "c"
                BLOCK@50..68
                  WHITESPACE@50..51 " "
                  LBRACE@51..52 "{"
                  DECLARATION@52..66
                    PROPERTY@52..54
                      WHITESPACE@52..53 " "
                      IDENT@53..54 "d"
                    COLON@54..55 ":"
                    VALUE@55..66
                      FUNCTION_CALL@55..66
                        WHITESPACE@55..56 " "
                        IDENT@56..62 "domain"
                        ARG_LIST@62..66
                          LPAREN@62..63 "("
                          ARG@63..65
                            VARIABLE_REF@63..65
                              DOLLAR@63..64 "$"
                              IDENT@64..65 "y"
                          RPAREN@65..66 ")"
                  WHITESPACE@66..67 " "
                  RBRACE@67..68 "}"
        "#]],
    );
}

#[test]
fn css_function_unclosed_params() {
    check(
        "@function --f(a\n{ result: 1; }\n.x { y: z; }",
        expect![[r#"
            SOURCE_FILE@0..43
              GENERIC_AT_RULE@0..30
                AT@0..1 "@"
                IDENT@1..9 "function"
                WHITESPACE@9..10 " "
                IDENT@10..13 "--f"
                LPAREN@13..14 "("
                IDENT@14..15 "a"
                BLOCK@15..30
                  WHITESPACE@15..16 "\n"
                  LBRACE@16..17 "{"
                  DECLARATION@17..28
                    PROPERTY@17..24
                      WHITESPACE@17..18 " "
                      IDENT@18..24 "result"
                    COLON@24..25 ":"
                    VALUE@25..27
                      WHITESPACE@25..26 " "
                      NUMBER@26..27 "1"
                    SEMICOLON@27..28 ";"
                  WHITESPACE@28..29 " "
                  RBRACE@29..30 "}"
              RULE_SET@30..43
                SELECTOR_LIST@30..33
                  SELECTOR@30..33
                    SIMPLE_SELECTOR@30..33
                      WHITESPACE@30..31 "\n"
                      DOT@31..32 "."
                      IDENT@32..33 "x"
                BLOCK@33..43
                  WHITESPACE@33..34 " "
                  LBRACE@34..35 "{"
                  DECLARATION@35..41
                    PROPERTY@35..37
                      WHITESPACE@35..36 " "
                      IDENT@36..37 "y"
                    COLON@37..38 ":"
                    VALUE@38..40
                      VALUE@38..40
                        WHITESPACE@38..39 " "
                        IDENT@39..40 "z"
                    SEMICOLON@40..41 ";"
                  WHITESPACE@41..42 " "
                  RBRACE@42..43 "}"
            errors:
              16..17: expected RPAREN
        "#]],
    );
}

#[test]
fn import_url_unclosed() {
    // The `{` inside `url(` is URL text, so the rest of the file is the URL's.
    check(
        "@import url(a\n.x { y: z; }",
        expect![[r#"
        SOURCE_FILE@0..26
          IMPORT_RULE@0..26
            AT@0..1 "@"
            IDENT@1..7 "import"
            WHITESPACE@7..8 " "
            IDENT@8..11 "url"
            LPAREN@11..12 "("
            URL_CONTENTS@12..13 "a"
            WHITESPACE@13..14 "\n"
            URL_CONTENTS@14..16 ".x"
            WHITESPACE@16..17 " "
            URL_CONTENTS@17..18 "{"
            WHITESPACE@18..19 " "
            URL_CONTENTS@19..21 "y:"
            WHITESPACE@21..22 " "
            URL_CONTENTS@22..24 "z;"
            WHITESPACE@24..25 " "
            URL_CONTENTS@25..26 "}"
        errors:
          26..26: expected RPAREN
    "#]],
    );
}

// ── Error recovery ──────────────────────────────────────────────────────

#[test]
fn error_orphan_else() {
    check(
        "@else { }",
        expect![[r#"
            SOURCE_FILE@0..9
              GENERIC_AT_RULE@0..9
                AT@0..1 "@"
                IDENT@1..5 "else"
                BLOCK@5..9
                  WHITESPACE@5..6 " "
                  LBRACE@6..7 "{"
                  WHITESPACE@7..8 " "
                  RBRACE@8..9 "}"
            errors:
              0..1: `@else` without preceding `@if`
        "#]],
    );
}

#[test]
fn error_mixin_missing_name() {
    check(
        "@mixin { }",
        expect![[r#"
            SOURCE_FILE@0..10
              MIXIN_RULE@0..10
                AT@0..1 "@"
                IDENT@1..6 "mixin"
                BLOCK@6..10
                  WHITESPACE@6..7 " "
                  LBRACE@7..8 "{"
                  WHITESPACE@8..9 " "
                  RBRACE@9..10 "}"
            errors:
              7..8: expected IDENT
        "#]],
    );
}

#[test]
fn error_if_missing_condition() {
    check(
        "@if { }",
        expect![[r#"
            SOURCE_FILE@0..7
              IF_RULE@0..7
                AT@0..1 "@"
                IDENT@1..3 "if"
                BLOCK@3..7
                  WHITESPACE@3..4 " "
                  LBRACE@4..5 "{"
                  WHITESPACE@5..6 " "
                  RBRACE@6..7 "}"
            errors:
              4..5: expected expression
        "#]],
    );
}

// ── Integration tests ───────────────────────────────────────────────────

#[test]
fn integration_mixin_in_rule() {
    check(
        ".card { @include flex; color: red; }",
        expect![[r#"
            SOURCE_FILE@0..36
              RULE_SET@0..36
                SELECTOR_LIST@0..5
                  SELECTOR@0..5
                    SIMPLE_SELECTOR@0..5
                      DOT@0..1 "."
                      IDENT@1..5 "card"
                BLOCK@5..36
                  WHITESPACE@5..6 " "
                  LBRACE@6..7 "{"
                  INCLUDE_RULE@7..22
                    WHITESPACE@7..8 " "
                    AT@8..9 "@"
                    IDENT@9..16 "include"
                    WHITESPACE@16..17 " "
                    IDENT@17..21 "flex"
                    SEMICOLON@21..22 ";"
                  DECLARATION@22..34
                    PROPERTY@22..28
                      WHITESPACE@22..23 " "
                      IDENT@23..28 "color"
                    COLON@28..29 ":"
                    VALUE@29..33
                      VALUE@29..33
                        WHITESPACE@29..30 " "
                        IDENT@30..33 "red"
                    SEMICOLON@33..34 ";"
                  WHITESPACE@34..35 " "
                  RBRACE@35..36 "}"
        "#]],
    );
}

#[test]
fn integration_if_inside_mixin() {
    check(
        "@mixin theme($dark) { @if $dark { color: white; } @else { color: black; } }",
        expect![[r#"
            SOURCE_FILE@0..75
              MIXIN_RULE@0..75
                AT@0..1 "@"
                IDENT@1..6 "mixin"
                WHITESPACE@6..7 " "
                IDENT@7..12 "theme"
                PARAM_LIST@12..19
                  LPAREN@12..13 "("
                  PARAM@13..18
                    DOLLAR@13..14 "$"
                    IDENT@14..18 "dark"
                  RPAREN@18..19 ")"
                BLOCK@19..75
                  WHITESPACE@19..20 " "
                  LBRACE@20..21 "{"
                  IF_RULE@21..73
                    WHITESPACE@21..22 " "
                    AT@22..23 "@"
                    IDENT@23..25 "if"
                    VARIABLE_REF@25..31
                      WHITESPACE@25..26 " "
                      DOLLAR@26..27 "$"
                      IDENT@27..31 "dark"
                    BLOCK@31..49
                      WHITESPACE@31..32 " "
                      LBRACE@32..33 "{"
                      DECLARATION@33..47
                        PROPERTY@33..39
                          WHITESPACE@33..34 " "
                          IDENT@34..39 "color"
                        COLON@39..40 ":"
                        VALUE@40..46
                          VALUE@40..46
                            WHITESPACE@40..41 " "
                            IDENT@41..46 "white"
                        SEMICOLON@46..47 ";"
                      WHITESPACE@47..48 " "
                      RBRACE@48..49 "}"
                    ELSE_CLAUSE@49..73
                      WHITESPACE@49..50 " "
                      AT@50..51 "@"
                      IDENT@51..55 "else"
                      BLOCK@55..73
                        WHITESPACE@55..56 " "
                        LBRACE@56..57 "{"
                        DECLARATION@57..71
                          PROPERTY@57..63
                            WHITESPACE@57..58 " "
                            IDENT@58..63 "color"
                          COLON@63..64 ":"
                          VALUE@64..70
                            VALUE@64..70
                              WHITESPACE@64..65 " "
                              IDENT@65..70 "black"
                          SEMICOLON@70..71 ";"
                        WHITESPACE@71..72 " "
                        RBRACE@72..73 "}"
                  WHITESPACE@73..74 " "
                  RBRACE@74..75 "}"
        "#]],
    );
}

#[test]
fn integration_media_nested_in_rule() {
    check(
        ".box { @media (max-width: 600px) { display: none; } }",
        expect![[r#"
            SOURCE_FILE@0..53
              RULE_SET@0..53
                SELECTOR_LIST@0..4
                  SELECTOR@0..4
                    SIMPLE_SELECTOR@0..4
                      DOT@0..1 "."
                      IDENT@1..4 "box"
                BLOCK@4..53
                  WHITESPACE@4..5 " "
                  LBRACE@5..6 "{"
                  MEDIA_RULE@6..51
                    WHITESPACE@6..7 " "
                    AT@7..8 "@"
                    IDENT@8..13 "media"
                    MEDIA_QUERY@13..32
                      WHITESPACE@13..14 " "
                      LPAREN@14..15 "("
                      IDENT@15..24 "max-width"
                      COLON@24..25 ":"
                      WHITESPACE@25..26 " "
                      NUMBER@26..29 "600"
                      IDENT@29..31 "px"
                      RPAREN@31..32 ")"
                    BLOCK@32..51
                      WHITESPACE@32..33 " "
                      LBRACE@33..34 "{"
                      DECLARATION@34..49
                        PROPERTY@34..42
                          WHITESPACE@34..35 " "
                          IDENT@35..42 "display"
                        COLON@42..43 ":"
                        VALUE@43..48
                          VALUE@43..48
                            WHITESPACE@43..44 " "
                            IDENT@44..48 "none"
                        SEMICOLON@48..49 ";"
                      WHITESPACE@49..50 " "
                      RBRACE@50..51 "}"
                  WHITESPACE@51..52 " "
                  RBRACE@52..53 "}"
        "#]],
    );
}

// ── Round-trip tests ─────────────────────────────────────────────────────

#[test]
fn round_trip_all_at_rules() {
    let inputs = [
        "@mixin foo { }",
        "@mixin bar($a, $b: 10) { }",
        "@include foo;",
        "@include bar(1, $b: 2);",
        "@function f($x) { @return $x; }",
        "@if true { } @else { }",
        "@for $i from 1 through 10 { }",
        "@each $x in a, b { }",
        "@while true { }",
        ".a { @extend .b; }",
        "@error \"msg\";",
        "@warn \"msg\";",
        "@debug 42;",
        "@at-root { }",
        "@at-root (with: media) { }",
        "@media screen { }",
        "@supports (display: flex) { }",
        "@keyframes fade { from { } to { } }",
        "@layer base { }",
        "@layer a, b;",
        "@container (min-width: 700px) { }",
        "@scope (.a) to (.b) { }",
        "@property --x { }",
        "@namespace svg \"http://w3.org\";",
        "@charset \"UTF-8\";",
        "@page :first { }",
        "@font-face { }",
        "@use \"colors\";",
        "@use \"colors\" as c;",
        "@use \"colors\" as *;",
        "@forward \"src\";",
        "@forward \"src\" hide $x;",
        "@import \"base\";",
        "@unknown foo;",
        "@unknown { }",
    ];

    for input in inputs {
        let (green, _errors) = sass_parser::parse_scss(input);
        let tree = SyntaxNode::new_root(green);
        assert_eq!(
            tree.text().to_string(),
            input,
            "round-trip failed for: {input}"
        );
    }
}

// ── Regression: P0 — supports_condition recovery ────────────────────────

#[test]
fn regression_supports_missing_brace_recovery() {
    // P0-3: @supports without `{` must not consume the enclosing `}`
    check(
        ".a { @supports (display: flex) color: red; }",
        expect![[r#"
            SOURCE_FILE@0..44
              RULE_SET@0..44
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..44
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  SUPPORTS_RULE@4..41
                    WHITESPACE@4..5 " "
                    AT@5..6 "@"
                    IDENT@6..14 "supports"
                    SUPPORTS_CONDITION@14..41
                      WHITESPACE@14..15 " "
                      LPAREN@15..16 "("
                      IDENT@16..23 "display"
                      COLON@23..24 ":"
                      WHITESPACE@24..25 " "
                      IDENT@25..29 "flex"
                      RPAREN@29..30 ")"
                      WHITESPACE@30..31 " "
                      IDENT@31..36 "color"
                      COLON@36..37 ":"
                      WHITESPACE@37..38 " "
                      IDENT@38..41 "red"
                  SEMICOLON@41..42 ";"
                  WHITESPACE@42..43 " "
                  RBRACE@43..44 "}"
        "#]],
    );
}

// ── Regression: P0 — container/scope recovery ───────────────────────────

#[test]
fn regression_container_missing_brace_recovery() {
    // P0-4: @container without `{` must not eat the enclosing `}`
    check(
        ".a { @container (width > 400px) color: red; }",
        expect![[r#"
            SOURCE_FILE@0..45
              RULE_SET@0..45
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..45
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  CONTAINER_RULE@4..42
                    WHITESPACE@4..5 " "
                    AT@5..6 "@"
                    IDENT@6..15 "container"
                    WHITESPACE@15..16 " "
                    LPAREN@16..17 "("
                    IDENT@17..22 "width"
                    WHITESPACE@22..23 " "
                    GT@23..24 ">"
                    WHITESPACE@24..25 " "
                    NUMBER@25..28 "400"
                    IDENT@28..30 "px"
                    RPAREN@30..31 ")"
                    WHITESPACE@31..32 " "
                    IDENT@32..37 "color"
                    COLON@37..38 ":"
                    WHITESPACE@38..39 " "
                    IDENT@39..42 "red"
                  SEMICOLON@42..43 ";"
                  WHITESPACE@43..44 " "
                  RBRACE@44..45 "}"
        "#]],
    );
}

// ── Regression: P1 — @extend descendant combinator ──────────────────────

#[test]
fn regression_extend_descendant_combinator_error() {
    // P1-2: @extend .foo .bar should produce an error
    check(
        ".a { @extend .foo .bar; }",
        expect![[r#"
            SOURCE_FILE@0..25
              RULE_SET@0..25
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..25
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  EXTEND_RULE@4..23
                    WHITESPACE@4..5 " "
                    AT@5..6 "@"
                    IDENT@6..12 "extend"
                    WHITESPACE@12..13 " "
                    DOT@13..14 "."
                    IDENT@14..17 "foo"
                    WHITESPACE@17..18 " "
                    DOT@18..19 "."
                    IDENT@19..22 "bar"
                    SEMICOLON@22..23 ";"
                  WHITESPACE@23..24 " "
                  RBRACE@24..25 "}"
            errors:
              22..23: `@extend` does not support descendant combinators
        "#]],
    );
}

#[test]
fn regression_extend_compound_no_error() {
    // Compound selector (no space) should NOT produce an error
    check(
        ".a { @extend .foo.bar; }",
        expect![[r#"
            SOURCE_FILE@0..24
              RULE_SET@0..24
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..24
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  EXTEND_RULE@4..22
                    WHITESPACE@4..5 " "
                    AT@5..6 "@"
                    IDENT@6..12 "extend"
                    WHITESPACE@12..13 " "
                    DOT@13..14 "."
                    IDENT@14..17 "foo"
                    DOT@17..18 "."
                    IDENT@18..21 "bar"
                    SEMICOLON@21..22 ";"
                  WHITESPACE@22..23 " "
                  RBRACE@23..24 "}"
        "#]],
    );
}

// ── Regression: P1 — @property -- prefix validation ─────────────────────

#[test]
fn regression_property_no_dashes_error() {
    // P1-3: @property without `--` prefix should produce an error
    check(
        "@property invalid { }",
        expect![[r#"
            SOURCE_FILE@0..21
              PROPERTY_RULE@0..21
                AT@0..1 "@"
                IDENT@1..9 "property"
                WHITESPACE@9..10 " "
                IDENT@10..17 "invalid"
                BLOCK@17..21
                  WHITESPACE@17..18 " "
                  LBRACE@18..19 "{"
                  WHITESPACE@19..20 " "
                  RBRACE@20..21 "}"
            errors:
              10..17: @property name must start with `--`
        "#]],
    );
}

// ── Regression: P1 — media_query RBRACE recovery ────────────────────────

#[test]
fn regression_media_malformed_rbrace_stops() {
    // P1-1: malformed @media must not eat enclosing RBRACE
    check(
        ".a { @media screen } .b { }",
        expect![[r#"
            SOURCE_FILE@0..27
              RULE_SET@0..20
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..20
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  MEDIA_RULE@4..18
                    WHITESPACE@4..5 " "
                    AT@5..6 "@"
                    IDENT@6..11 "media"
                    MEDIA_QUERY@11..18
                      WHITESPACE@11..12 " "
                      IDENT@12..18 "screen"
                  WHITESPACE@18..19 " "
                  RBRACE@19..20 "}"
              RULE_SET@20..27
                SELECTOR_LIST@20..23
                  SELECTOR@20..23
                    SIMPLE_SELECTOR@20..23
                      WHITESPACE@20..21 " "
                      DOT@21..22 "."
                      IDENT@22..23 "b"
                BLOCK@23..27
                  WHITESPACE@23..24 " "
                  LBRACE@24..25 "{"
                  WHITESPACE@25..26 " "
                  RBRACE@26..27 "}"
        "#]],
    );
}

// ── Additional coverage tests ───────────────────────────────────────────

#[test]
fn use_with_config() {
    check(
        "@use \"colors\" with ($primary: red);",
        expect![[r#"
            SOURCE_FILE@0..35
              USE_RULE@0..35
                AT@0..1 "@"
                IDENT@1..4 "use"
                WHITESPACE@4..5 " "
                QUOTED_STRING@5..13 "\"colors\""
                WHITESPACE@13..14 " "
                IDENT@14..18 "with"
                WHITESPACE@18..19 " "
                LPAREN@19..20 "("
                DOLLAR@20..21 "$"
                IDENT@21..28 "primary"
                COLON@28..29 ":"
                VALUE@29..33
                  WHITESPACE@29..30 " "
                  IDENT@30..33 "red"
                RPAREN@33..34 ")"
                SEMICOLON@34..35 ";"
        "#]],
    );
}

#[test]
fn forward_as_prefix_star() {
    check(
        "@forward \"src\" as my-*;",
        expect![[r#"
            SOURCE_FILE@0..23
              FORWARD_RULE@0..23
                AT@0..1 "@"
                IDENT@1..8 "forward"
                WHITESPACE@8..9 " "
                QUOTED_STRING@9..14 "\"src\""
                WHITESPACE@14..15 " "
                IDENT@15..17 "as"
                WHITESPACE@17..18 " "
                IDENT@18..21 "my-"
                STAR@21..22 "*"
                SEMICOLON@22..23 ";"
        "#]],
    );
}

#[test]
fn forward_with_default() {
    check(
        "@forward \"src\" with ($x: 1 !default);",
        expect![[r#"
            SOURCE_FILE@0..37
              FORWARD_RULE@0..37
                AT@0..1 "@"
                IDENT@1..8 "forward"
                WHITESPACE@8..9 " "
                QUOTED_STRING@9..14 "\"src\""
                WHITESPACE@14..15 " "
                IDENT@15..19 "with"
                WHITESPACE@19..20 " "
                LPAREN@20..21 "("
                DOLLAR@21..22 "$"
                IDENT@22..23 "x"
                COLON@23..24 ":"
                NUMBER_LITERAL@24..26
                  WHITESPACE@24..25 " "
                  NUMBER@25..26 "1"
                SASS_FLAG@26..35
                  WHITESPACE@26..27 " "
                  BANG@27..28 "!"
                  IDENT@28..35 "default"
                RPAREN@35..36 ")"
                SEMICOLON@36..37 ";"
        "#]],
    );
}

#[test]
fn content_with_args() {
    check(
        "@mixin m { @content($x); }",
        expect![[r#"
            SOURCE_FILE@0..26
              MIXIN_RULE@0..26
                AT@0..1 "@"
                IDENT@1..6 "mixin"
                WHITESPACE@6..7 " "
                IDENT@7..8 "m"
                BLOCK@8..26
                  WHITESPACE@8..9 " "
                  LBRACE@9..10 "{"
                  CONTENT_RULE@10..24
                    WHITESPACE@10..11 " "
                    AT@11..12 "@"
                    IDENT@12..19 "content"
                    ARG_LIST@19..23
                      LPAREN@19..20 "("
                      ARG@20..22
                        VARIABLE_REF@20..22
                          DOLLAR@20..21 "$"
                          IDENT@21..22 "x"
                      RPAREN@22..23 ")"
                    SEMICOLON@23..24 ";"
                  WHITESPACE@24..25 " "
                  RBRACE@25..26 "}"
        "#]],
    );
}

#[test]
fn keyframes_percentage() {
    check(
        "@keyframes slide { 0% { left: 0; } 100% { left: 100px; } }",
        expect![[r#"
            SOURCE_FILE@0..58
              KEYFRAMES_RULE@0..58
                AT@0..1 "@"
                IDENT@1..10 "keyframes"
                WHITESPACE@10..11 " "
                IDENT@11..16 "slide"
                WHITESPACE@16..17 " "
                LBRACE@17..18 "{"
                KEYFRAME_SELECTOR@18..34
                  WHITESPACE@18..19 " "
                  NUMBER@19..20 "0"
                  PERCENT@20..21 "%"
                  BLOCK@21..34
                    WHITESPACE@21..22 " "
                    LBRACE@22..23 "{"
                    DECLARATION@23..32
                      PROPERTY@23..28
                        WHITESPACE@23..24 " "
                        IDENT@24..28 "left"
                      COLON@28..29 ":"
                      VALUE@29..31
                        NUMBER_LITERAL@29..31
                          WHITESPACE@29..30 " "
                          NUMBER@30..31 "0"
                      SEMICOLON@31..32 ";"
                    WHITESPACE@32..33 " "
                    RBRACE@33..34 "}"
                KEYFRAME_SELECTOR@34..56
                  WHITESPACE@34..35 " "
                  NUMBER@35..38 "100"
                  PERCENT@38..39 "%"
                  BLOCK@39..56
                    WHITESPACE@39..40 " "
                    LBRACE@40..41 "{"
                    DECLARATION@41..54
                      PROPERTY@41..46
                        WHITESPACE@41..42 " "
                        IDENT@42..46 "left"
                      COLON@46..47 ":"
                      VALUE@47..53
                        DIMENSION@47..53
                          WHITESPACE@47..48 " "
                          NUMBER@48..51 "100"
                          IDENT@51..53 "px"
                      SEMICOLON@53..54 ";"
                    WHITESPACE@54..55 " "
                    RBRACE@55..56 "}"
                WHITESPACE@56..57 " "
                RBRACE@57..58 "}"
        "#]],
    );
}

#[test]
fn layer_dot_separated() {
    check(
        "@layer base.reset { }",
        expect![[r#"
            SOURCE_FILE@0..21
              LAYER_RULE@0..21
                AT@0..1 "@"
                IDENT@1..6 "layer"
                WHITESPACE@6..7 " "
                IDENT@7..11 "base"
                DOT@11..12 "."
                IDENT@12..17 "reset"
                BLOCK@17..21
                  WHITESPACE@17..18 " "
                  LBRACE@18..19 "{"
                  WHITESPACE@19..20 " "
                  RBRACE@20..21 "}"
        "#]],
    );
}

#[test]
fn layer_name_with_interpolation() {
    // `@layer` only accepted a bare IDENT, so an interpolated name errored with
    // "expected SEMICOLON" at the `#{`. Vuetify's `_layer.scss` uses this form.
    check(
        "@layer vuetify-#{$name} { }",
        expect![[r##"
        SOURCE_FILE@0..27
          LAYER_RULE@0..27
            AT@0..1 "@"
            IDENT@1..6 "layer"
            WHITESPACE@6..7 " "
            IDENT@7..15 "vuetify-"
            INTERPOLATION@15..23
              HASH_LBRACE@15..17 "#{"
              VARIABLE_REF@17..22
                DOLLAR@17..18 "$"
                IDENT@18..22 "name"
              RBRACE@22..23 "}"
            BLOCK@23..27
              WHITESPACE@23..24 " "
              LBRACE@24..25 "{"
              WHITESPACE@25..26 " "
              RBRACE@26..27 "}"
    "##]],
    );
}

#[test]
fn layer_name_is_only_interpolation() {
    check(
        "@layer #{$n} { }",
        expect![[r##"
        SOURCE_FILE@0..16
          LAYER_RULE@0..16
            AT@0..1 "@"
            IDENT@1..6 "layer"
            INTERPOLATION@6..12
              WHITESPACE@6..7 " "
              HASH_LBRACE@7..9 "#{"
              VARIABLE_REF@9..11
                DOLLAR@9..10 "$"
                IDENT@10..11 "n"
              RBRACE@11..12 "}"
            BLOCK@12..16
              WHITESPACE@12..13 " "
              LBRACE@13..14 "{"
              WHITESPACE@14..15 " "
              RBRACE@15..16 "}"
    "##]],
    );
}

#[test]
fn layer_interpolated_names_dotted_and_listed() {
    // Interpolation composes with both the dot-separated and comma-separated forms.
    check(
        "@layer a-#{$n}.b, #{$m}-x { }",
        expect![[r##"
        SOURCE_FILE@0..29
          LAYER_RULE@0..29
            AT@0..1 "@"
            IDENT@1..6 "layer"
            WHITESPACE@6..7 " "
            IDENT@7..9 "a-"
            INTERPOLATION@9..14
              HASH_LBRACE@9..11 "#{"
              VARIABLE_REF@11..13
                DOLLAR@11..12 "$"
                IDENT@12..13 "n"
              RBRACE@13..14 "}"
            DOT@14..15 "."
            IDENT@15..16 "b"
            COMMA@16..17 ","
            INTERPOLATION@17..23
              WHITESPACE@17..18 " "
              HASH_LBRACE@18..20 "#{"
              VARIABLE_REF@20..22
                DOLLAR@20..21 "$"
                IDENT@21..22 "m"
              RBRACE@22..23 "}"
            IDENT@23..25 "-x"
            BLOCK@25..29
              WHITESPACE@25..26 " "
              LBRACE@26..27 "{"
              WHITESPACE@27..28 " "
              RBRACE@28..29 "}"
    "##]],
    );
}

#[test]
fn layer_trailing_dot_is_still_an_error() {
    // Allowing interpolation must not lose the diagnostic for a name that just stops.
    check(
        "@layer a. { }",
        expect![[r#"
        SOURCE_FILE@0..13
          LAYER_RULE@0..13
            AT@0..1 "@"
            IDENT@1..6 "layer"
            WHITESPACE@6..7 " "
            IDENT@7..8 "a"
            DOT@8..9 "."
            BLOCK@9..13
              WHITESPACE@9..10 " "
              LBRACE@10..11 "{"
              WHITESPACE@11..12 " "
              RBRACE@12..13 "}"
        errors:
          10..11: expected IDENT
    "#]],
    );
}

#[test]
fn container_with_name() {
    check(
        "@container sidebar (min-width: 700px) { }",
        expect![[r#"
            SOURCE_FILE@0..41
              CONTAINER_RULE@0..41
                AT@0..1 "@"
                IDENT@1..10 "container"
                WHITESPACE@10..11 " "
                IDENT@11..18 "sidebar"
                WHITESPACE@18..19 " "
                LPAREN@19..20 "("
                IDENT@20..29 "min-width"
                COLON@29..30 ":"
                WHITESPACE@30..31 " "
                NUMBER@31..34 "700"
                IDENT@34..36 "px"
                RPAREN@36..37 ")"
                BLOCK@37..41
                  WHITESPACE@37..38 " "
                  LBRACE@38..39 "{"
                  WHITESPACE@39..40 " "
                  RBRACE@40..41 "}"
        "#]],
    );
}

// ── Error recovery: @for ─────────────────────────────────────────────────

#[test]
fn error_for_missing_from() {
    check(
        "@for $i 1 through 10 { }",
        expect![[r#"
            SOURCE_FILE@0..24
              FOR_RULE@0..24
                AT@0..1 "@"
                IDENT@1..4 "for"
                WHITESPACE@4..5 " "
                DOLLAR@5..6 "$"
                IDENT@6..7 "i"
                NUMBER_LITERAL@7..9
                  WHITESPACE@7..8 " "
                  NUMBER@8..9 "1"
                WHITESPACE@9..10 " "
                IDENT@10..17 "through"
                NUMBER_LITERAL@17..20
                  WHITESPACE@17..18 " "
                  NUMBER@18..20 "10"
                BLOCK@20..24
                  WHITESPACE@20..21 " "
                  LBRACE@21..22 "{"
                  WHITESPACE@22..23 " "
                  RBRACE@23..24 "}"
            errors:
              8..9: expected `from`
        "#]],
    );
}

#[test]
fn error_for_missing_through() {
    check(
        "@for $i from 1 10 { }",
        expect![[r#"
            SOURCE_FILE@0..21
              FOR_RULE@0..21
                AT@0..1 "@"
                IDENT@1..4 "for"
                WHITESPACE@4..5 " "
                DOLLAR@5..6 "$"
                IDENT@6..7 "i"
                WHITESPACE@7..8 " "
                IDENT@8..12 "from"
                NUMBER_LITERAL@12..14
                  WHITESPACE@12..13 " "
                  NUMBER@13..14 "1"
                NUMBER_LITERAL@14..17
                  WHITESPACE@14..15 " "
                  NUMBER@15..17 "10"
                BLOCK@17..21
                  WHITESPACE@17..18 " "
                  LBRACE@18..19 "{"
                  WHITESPACE@19..20 " "
                  RBRACE@20..21 "}"
            errors:
              15..17: expected `through` or `to`
        "#]],
    );
}

// ── Error recovery: @each ────────────────────────────────────────────────

#[test]
fn error_each_missing_in() {
    check(
        "@each $x 1, 2, 3 { }",
        expect![[r#"
            SOURCE_FILE@0..20
              EACH_RULE@0..20
                AT@0..1 "@"
                IDENT@1..5 "each"
                WHITESPACE@5..6 " "
                DOLLAR@6..7 "$"
                IDENT@7..8 "x"
                NUMBER_LITERAL@8..10
                  WHITESPACE@8..9 " "
                  NUMBER@9..10 "1"
                COMMA@10..11 ","
                NUMBER_LITERAL@11..13
                  WHITESPACE@11..12 " "
                  NUMBER@12..13 "2"
                COMMA@13..14 ","
                NUMBER_LITERAL@14..16
                  WHITESPACE@14..15 " "
                  NUMBER@15..16 "3"
                BLOCK@16..20
                  WHITESPACE@16..17 " "
                  LBRACE@17..18 "{"
                  WHITESPACE@18..19 " "
                  RBRACE@19..20 "}"
            errors:
              9..10: expected `in`
        "#]],
    );
}

// ── Error recovery: @function ────────────────────────────────────────────

#[test]
fn error_function_missing_name() {
    check(
        "@function ($x) { @return $x; }",
        expect![[r#"
            SOURCE_FILE@0..30
              FUNCTION_RULE@0..30
                AT@0..1 "@"
                IDENT@1..9 "function"
                PARAM_LIST@9..14
                  WHITESPACE@9..10 " "
                  LPAREN@10..11 "("
                  PARAM@11..13
                    DOLLAR@11..12 "$"
                    IDENT@12..13 "x"
                  RPAREN@13..14 ")"
                BLOCK@14..30
                  WHITESPACE@14..15 " "
                  LBRACE@15..16 "{"
                  RETURN_RULE@16..28
                    WHITESPACE@16..17 " "
                    AT@17..18 "@"
                    IDENT@18..24 "return"
                    VARIABLE_REF@24..27
                      WHITESPACE@24..25 " "
                      DOLLAR@25..26 "$"
                      IDENT@26..27 "x"
                    SEMICOLON@27..28 ";"
                  WHITESPACE@28..29 " "
                  RBRACE@29..30 "}"
            errors:
              10..11: expected IDENT
        "#]],
    );
}

// ── Error recovery: @use ─────────────────────────────────────────────────

#[test]
fn error_use_missing_path() {
    check(
        "@use;",
        expect![[r#"
            SOURCE_FILE@0..5
              USE_RULE@0..5
                AT@0..1 "@"
                IDENT@1..4 "use"
                SEMICOLON@4..5 ";"
            errors:
              4..5: expected QUOTED_STRING
        "#]],
    );
}

// ── @include with whitespace before parens ───────────────────────────────

#[test]
fn include_with_whitespace_before_parens() {
    check(
        "@include size (100px);",
        expect![[r#"
            SOURCE_FILE@0..22
              INCLUDE_RULE@0..22
                AT@0..1 "@"
                IDENT@1..8 "include"
                WHITESPACE@8..9 " "
                IDENT@9..13 "size"
                ARG_LIST@13..21
                  WHITESPACE@13..14 " "
                  LPAREN@14..15 "("
                  ARG@15..20
                    DIMENSION@15..20
                      NUMBER@15..18 "100"
                      IDENT@18..20 "px"
                  RPAREN@20..21 ")"
                SEMICOLON@21..22 ";"
        "#]],
    );
}

// ── @content with whitespace before parens ───────────────────────────────

#[test]
fn content_with_whitespace_before_parens() {
    check(
        "@mixin m { @content ($x); }",
        expect![[r#"
            SOURCE_FILE@0..27
              MIXIN_RULE@0..27
                AT@0..1 "@"
                IDENT@1..6 "mixin"
                WHITESPACE@6..7 " "
                IDENT@7..8 "m"
                BLOCK@8..27
                  WHITESPACE@8..9 " "
                  LBRACE@9..10 "{"
                  CONTENT_RULE@10..25
                    WHITESPACE@10..11 " "
                    AT@11..12 "@"
                    IDENT@12..19 "content"
                    ARG_LIST@19..24
                      WHITESPACE@19..20 " "
                      LPAREN@20..21 "("
                      ARG@21..23
                        VARIABLE_REF@21..23
                          DOLLAR@21..22 "$"
                          IDENT@22..23 "x"
                      RPAREN@23..24 ")"
                    SEMICOLON@24..25 ";"
                  WHITESPACE@25..26 " "
                  RBRACE@26..27 "}"
        "#]],
    );
}

// ── Stress: semantic AST accuracy ──────────────────────────────────────

#[test]
fn use_as_with_combined() {
    check(
        r#"@use "theme" as t with ($primary: red, $size: 16px);"#,
        expect![[r#"
            SOURCE_FILE@0..52
              USE_RULE@0..52
                AT@0..1 "@"
                IDENT@1..4 "use"
                WHITESPACE@4..5 " "
                QUOTED_STRING@5..12 "\"theme\""
                WHITESPACE@12..13 " "
                IDENT@13..15 "as"
                WHITESPACE@15..16 " "
                IDENT@16..17 "t"
                WHITESPACE@17..18 " "
                IDENT@18..22 "with"
                WHITESPACE@22..23 " "
                LPAREN@23..24 "("
                DOLLAR@24..25 "$"
                IDENT@25..32 "primary"
                COLON@32..33 ":"
                VALUE@33..37
                  WHITESPACE@33..34 " "
                  IDENT@34..37 "red"
                COMMA@37..38 ","
                WHITESPACE@38..39 " "
                DOLLAR@39..40 "$"
                IDENT@40..44 "size"
                COLON@44..45 ":"
                DIMENSION@45..50
                  WHITESPACE@45..46 " "
                  NUMBER@46..48 "16"
                  IDENT@48..50 "px"
                RPAREN@50..51 ")"
                SEMICOLON@51..52 ";"
        "#]],
    );
}

#[test]
fn forward_show_members() {
    check(
        r#"@forward "utils" show flex-center, $breakpoints;"#,
        expect![[r#"
            SOURCE_FILE@0..48
              FORWARD_RULE@0..48
                AT@0..1 "@"
                IDENT@1..8 "forward"
                WHITESPACE@8..9 " "
                QUOTED_STRING@9..16 "\"utils\""
                WHITESPACE@16..17 " "
                IDENT@17..21 "show"
                WHITESPACE@21..22 " "
                IDENT@22..33 "flex-center"
                COMMA@33..34 ","
                WHITESPACE@34..35 " "
                DOLLAR@35..36 "$"
                IDENT@36..47 "breakpoints"
                SEMICOLON@47..48 ";"
        "#]],
    );
}

#[test]
fn forward_as_hide_combined() {
    check(
        r#"@forward "src" as btn-* hide _private;"#,
        expect![[r#"
            SOURCE_FILE@0..38
              FORWARD_RULE@0..38
                AT@0..1 "@"
                IDENT@1..8 "forward"
                WHITESPACE@8..9 " "
                QUOTED_STRING@9..14 "\"src\""
                WHITESPACE@14..15 " "
                IDENT@15..17 "as"
                WHITESPACE@17..18 " "
                IDENT@18..22 "btn-"
                STAR@22..23 "*"
                WHITESPACE@23..24 " "
                IDENT@24..28 "hide"
                WHITESPACE@28..29 " "
                IDENT@29..37 "_private"
                SEMICOLON@37..38 ";"
        "#]],
    );
}

// ── sass-spec false negative regression tests ────────────────────────────

#[test]
fn elseif_deprecated_no_space() {
    check(
        "@if true { a: b } @elseif false { c: d }",
        expect![[r#"
            SOURCE_FILE@0..40
              IF_RULE@0..40
                AT@0..1 "@"
                IDENT@1..3 "if"
                BOOL_LITERAL@3..8
                  WHITESPACE@3..4 " "
                  IDENT@4..8 "true"
                BLOCK@8..17
                  WHITESPACE@8..9 " "
                  LBRACE@9..10 "{"
                  DECLARATION@10..15
                    PROPERTY@10..12
                      WHITESPACE@10..11 " "
                      IDENT@11..12 "a"
                    COLON@12..13 ":"
                    VALUE@13..15
                      VALUE@13..15
                        WHITESPACE@13..14 " "
                        IDENT@14..15 "b"
                  WHITESPACE@15..16 " "
                  RBRACE@16..17 "}"
                ELSE_CLAUSE@17..40
                  WHITESPACE@17..18 " "
                  AT@18..19 "@"
                  IDENT@19..25 "elseif"
                  BOOL_LITERAL@25..31
                    WHITESPACE@25..26 " "
                    IDENT@26..31 "false"
                  BLOCK@31..40
                    WHITESPACE@31..32 " "
                    LBRACE@32..33 "{"
                    DECLARATION@33..38
                      PROPERTY@33..35
                        WHITESPACE@33..34 " "
                        IDENT@34..35 "c"
                      COLON@35..36 ":"
                      VALUE@36..38
                        VALUE@36..38
                          WHITESPACE@36..37 " "
                          IDENT@37..38 "d"
                    WHITESPACE@38..39 " "
                    RBRACE@39..40 "}"
        "#]],
    );
}

#[test]
fn keyframes_interpolated_selector() {
    check(
        "@keyframes a { #{$b} { c: d } }",
        expect![[r##"
            SOURCE_FILE@0..31
              KEYFRAMES_RULE@0..31
                AT@0..1 "@"
                IDENT@1..10 "keyframes"
                WHITESPACE@10..11 " "
                IDENT@11..12 "a"
                WHITESPACE@12..13 " "
                LBRACE@13..14 "{"
                KEYFRAME_SELECTOR@14..29
                  INTERPOLATION@14..20
                    WHITESPACE@14..15 " "
                    HASH_LBRACE@15..17 "#{"
                    VARIABLE_REF@17..19
                      DOLLAR@17..18 "$"
                      IDENT@18..19 "b"
                    RBRACE@19..20 "}"
                  BLOCK@20..29
                    WHITESPACE@20..21 " "
                    LBRACE@21..22 "{"
                    DECLARATION@22..27
                      PROPERTY@22..24
                        WHITESPACE@22..23 " "
                        IDENT@23..24 "c"
                      COLON@24..25 ":"
                      VALUE@25..27
                        VALUE@25..27
                          WHITESPACE@25..26 " "
                          IDENT@26..27 "d"
                    WHITESPACE@27..28 " "
                    RBRACE@28..29 "}"
                WHITESPACE@29..30 " "
                RBRACE@30..31 "}"
        "##]],
    );
}

#[test]
fn keyframes_anonymous() {
    check(
        "@keyframes { from { a: b } }",
        expect![[r#"
            SOURCE_FILE@0..28
              KEYFRAMES_RULE@0..28
                AT@0..1 "@"
                IDENT@1..10 "keyframes"
                WHITESPACE@10..11 " "
                LBRACE@11..12 "{"
                KEYFRAME_SELECTOR@12..26
                  WHITESPACE@12..13 " "
                  IDENT@13..17 "from"
                  BLOCK@17..26
                    WHITESPACE@17..18 " "
                    LBRACE@18..19 "{"
                    DECLARATION@19..24
                      PROPERTY@19..21
                        WHITESPACE@19..20 " "
                        IDENT@20..21 "a"
                      COLON@21..22 ":"
                      VALUE@22..24
                        VALUE@22..24
                          WHITESPACE@22..23 " "
                          IDENT@23..24 "b"
                    WHITESPACE@24..25 " "
                    RBRACE@25..26 "}"
                WHITESPACE@26..27 " "
                RBRACE@27..28 "}"
        "#]],
    );
}

#[test]
fn keyframes_variable_name() {
    check(
        "@keyframes $name { to { a: b } }",
        expect![[r#"
            SOURCE_FILE@0..32
              KEYFRAMES_RULE@0..32
                AT@0..1 "@"
                IDENT@1..10 "keyframes"
                WHITESPACE@10..11 " "
                DOLLAR@11..12 "$"
                IDENT@12..16 "name"
                WHITESPACE@16..17 " "
                LBRACE@17..18 "{"
                KEYFRAME_SELECTOR@18..30
                  WHITESPACE@18..19 " "
                  IDENT@19..21 "to"
                  BLOCK@21..30
                    WHITESPACE@21..22 " "
                    LBRACE@22..23 "{"
                    DECLARATION@23..28
                      PROPERTY@23..25
                        WHITESPACE@23..24 " "
                        IDENT@24..25 "a"
                      COLON@25..26 ":"
                      VALUE@26..28
                        VALUE@26..28
                          WHITESPACE@26..27 " "
                          IDENT@27..28 "b"
                    WHITESPACE@28..29 " "
                    RBRACE@29..30 "}"
                WHITESPACE@30..31 " "
                RBRACE@31..32 "}"
        "#]],
    );
}

#[test]
fn keyframes_variable_declaration() {
    check(
        "@keyframes a { $x: 10%; #{$x} { c: d } }",
        expect![[r##"
            SOURCE_FILE@0..40
              KEYFRAMES_RULE@0..40
                AT@0..1 "@"
                IDENT@1..10 "keyframes"
                WHITESPACE@10..11 " "
                IDENT@11..12 "a"
                WHITESPACE@12..13 " "
                LBRACE@13..14 "{"
                VARIABLE_DECL@14..23
                  WHITESPACE@14..15 " "
                  DOLLAR@15..16 "$"
                  IDENT@16..17 "x"
                  COLON@17..18 ":"
                  DIMENSION@18..22
                    WHITESPACE@18..19 " "
                    NUMBER@19..21 "10"
                    PERCENT@21..22 "%"
                  SEMICOLON@22..23 ";"
                KEYFRAME_SELECTOR@23..38
                  INTERPOLATION@23..29
                    WHITESPACE@23..24 " "
                    HASH_LBRACE@24..26 "#{"
                    VARIABLE_REF@26..28
                      DOLLAR@26..27 "$"
                      IDENT@27..28 "x"
                    RBRACE@28..29 "}"
                  BLOCK@29..38
                    WHITESPACE@29..30 " "
                    LBRACE@30..31 "{"
                    DECLARATION@31..36
                      PROPERTY@31..33
                        WHITESPACE@31..32 " "
                        IDENT@32..33 "c"
                      COLON@33..34 ":"
                      VALUE@34..36
                        VALUE@34..36
                          WHITESPACE@34..35 " "
                          IDENT@35..36 "d"
                    WHITESPACE@36..37 " "
                    RBRACE@37..38 "}"
                WHITESPACE@38..39 " "
                RBRACE@39..40 "}"
        "##]],
    );
}

#[test]
fn extend_comma_separated() {
    check(
        ".a { @extend .b, .c; }",
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
                  EXTEND_RULE@4..20
                    WHITESPACE@4..5 " "
                    AT@5..6 "@"
                    IDENT@6..12 "extend"
                    WHITESPACE@12..13 " "
                    DOT@13..14 "."
                    IDENT@14..15 "b"
                    COMMA@15..16 ","
                    WHITESPACE@16..17 " "
                    DOT@17..18 "."
                    IDENT@18..19 "c"
                    SEMICOLON@19..20 ";"
                  WHITESPACE@20..21 " "
                  RBRACE@21..22 "}"
        "#]],
    );
}

#[test]
fn each_space_separated() {
    check(
        "@each $n in 1px 2px 3px { a: $n }",
        expect![[r#"
            SOURCE_FILE@0..33
              EACH_RULE@0..33
                AT@0..1 "@"
                IDENT@1..5 "each"
                WHITESPACE@5..6 " "
                DOLLAR@6..7 "$"
                IDENT@7..8 "n"
                WHITESPACE@8..9 " "
                IDENT@9..11 "in"
                DIMENSION@11..15
                  WHITESPACE@11..12 " "
                  NUMBER@12..13 "1"
                  IDENT@13..15 "px"
                DIMENSION@15..19
                  WHITESPACE@15..16 " "
                  NUMBER@16..17 "2"
                  IDENT@17..19 "px"
                DIMENSION@19..23
                  WHITESPACE@19..20 " "
                  NUMBER@20..21 "3"
                  IDENT@21..23 "px"
                BLOCK@23..33
                  WHITESPACE@23..24 " "
                  LBRACE@24..25 "{"
                  DECLARATION@25..31
                    PROPERTY@25..27
                      WHITESPACE@25..26 " "
                      IDENT@26..27 "a"
                    COLON@27..28 ":"
                    VALUE@28..31
                      VARIABLE_REF@28..31
                        WHITESPACE@28..29 " "
                        DOLLAR@29..30 "$"
                        IDENT@30..31 "n"
                  WHITESPACE@31..32 " "
                  RBRACE@32..33 "}"
        "#]],
    );
}

#[test]
fn generic_at_rule_interpolated_name() {
    check(
        "@#{\"foo\"} bar { a: b }",
        expect![[r##"
            SOURCE_FILE@0..22
              GENERIC_AT_RULE@0..22
                AT@0..1 "@"
                INTERPOLATION@1..9
                  HASH_LBRACE@1..3 "#{"
                  STRING_LITERAL@3..8
                    QUOTED_STRING@3..8 "\"foo\""
                  RBRACE@8..9 "}"
                WHITESPACE@9..10 " "
                IDENT@10..13 "bar"
                BLOCK@13..22
                  WHITESPACE@13..14 " "
                  LBRACE@14..15 "{"
                  DECLARATION@15..20
                    PROPERTY@15..17
                      WHITESPACE@15..16 " "
                      IDENT@16..17 "a"
                    COLON@17..18 ":"
                    VALUE@18..20
                      VALUE@18..20
                        WHITESPACE@18..19 " "
                        IDENT@19..20 "b"
                  WHITESPACE@20..21 " "
                  RBRACE@21..22 "}"
        "##]],
    );
}

#[test]
fn generic_at_rule_interpolated_keyframes_block() {
    check(
        "@#{\"keyframes\"} name { 10% { x: y } }",
        expect![[r##"
            SOURCE_FILE@0..37
              GENERIC_AT_RULE@0..37
                AT@0..1 "@"
                INTERPOLATION@1..15
                  HASH_LBRACE@1..3 "#{"
                  STRING_LITERAL@3..14
                    QUOTED_STRING@3..14 "\"keyframes\""
                  RBRACE@14..15 "}"
                WHITESPACE@15..16 " "
                IDENT@16..20 "name"
                BLOCK@20..37
                  WHITESPACE@20..21 " "
                  LBRACE@21..22 "{"
                  KEYFRAME_SELECTOR@22..35
                    WHITESPACE@22..23 " "
                    NUMBER@23..25 "10"
                    PERCENT@25..26 "%"
                    BLOCK@26..35
                      WHITESPACE@26..27 " "
                      LBRACE@27..28 "{"
                      DECLARATION@28..33
                        PROPERTY@28..30
                          WHITESPACE@28..29 " "
                          IDENT@29..30 "x"
                        COLON@30..31 ":"
                        VALUE@31..33
                          VALUE@31..33
                            WHITESPACE@31..32 " "
                            IDENT@32..33 "y"
                      WHITESPACE@33..34 " "
                      RBRACE@34..35 "}"
                  WHITESPACE@35..36 " "
                  RBRACE@36..37 "}"
        "##]],
    );
}

#[test]
fn generic_at_rule_interpolated_name_semicolon() {
    check(
        "@#{\"error\"} not really an error;",
        expect![[r##"
            SOURCE_FILE@0..32
              GENERIC_AT_RULE@0..32
                AT@0..1 "@"
                INTERPOLATION@1..11
                  HASH_LBRACE@1..3 "#{"
                  STRING_LITERAL@3..10
                    QUOTED_STRING@3..10 "\"error\""
                  RBRACE@10..11 "}"
                WHITESPACE@11..12 " "
                IDENT@12..15 "not"
                WHITESPACE@15..16 " "
                IDENT@16..22 "really"
                WHITESPACE@22..23 " "
                IDENT@23..25 "an"
                WHITESPACE@25..26 " "
                IDENT@26..31 "error"
                SEMICOLON@31..32 ";"
        "##]],
    );
}

#[test]
fn keyframes_vendor_prefix_extra_tokens() {
    check(
        "@-moz-keyframes anim /* Firefox */ line 429 { from { width: 0% } }",
        expect![[r#"
            SOURCE_FILE@0..66
              KEYFRAMES_RULE@0..66
                AT@0..1 "@"
                IDENT@1..15 "-moz-keyframes"
                WHITESPACE@15..16 " "
                IDENT@16..20 "anim"
                WHITESPACE@20..21 " "
                MULTI_LINE_COMMENT@21..34 "/* Firefox */"
                WHITESPACE@34..35 " "
                IDENT@35..39 "line"
                WHITESPACE@39..40 " "
                NUMBER@40..43 "429"
                WHITESPACE@43..44 " "
                LBRACE@44..45 "{"
                KEYFRAME_SELECTOR@45..64
                  WHITESPACE@45..46 " "
                  IDENT@46..50 "from"
                  BLOCK@50..64
                    WHITESPACE@50..51 " "
                    LBRACE@51..52 "{"
                    DECLARATION@52..62
                      PROPERTY@52..58
                        WHITESPACE@52..53 " "
                        IDENT@53..58 "width"
                      COLON@58..59 ":"
                      VALUE@59..62
                        DIMENSION@59..62
                          WHITESPACE@59..60 " "
                          NUMBER@60..61 "0"
                          PERCENT@61..62 "%"
                    WHITESPACE@62..63 " "
                    RBRACE@63..64 "}"
                WHITESPACE@64..65 " "
                RBRACE@65..66 "}"
        "#]],
    );
}

#[test]
fn generic_at_rule_interpolated_value() {
    check(
        "@foo bar#{$baz} qux { a: b }",
        expect![[r##"
            SOURCE_FILE@0..28
              GENERIC_AT_RULE@0..28
                AT@0..1 "@"
                IDENT@1..4 "foo"
                WHITESPACE@4..5 " "
                IDENT@5..8 "bar"
                INTERPOLATION@8..15
                  HASH_LBRACE@8..10 "#{"
                  VARIABLE_REF@10..14
                    DOLLAR@10..11 "$"
                    IDENT@11..14 "baz"
                  RBRACE@14..15 "}"
                WHITESPACE@15..16 " "
                IDENT@16..19 "qux"
                BLOCK@19..28
                  WHITESPACE@19..20 " "
                  LBRACE@20..21 "{"
                  DECLARATION@21..26
                    PROPERTY@21..23
                      WHITESPACE@21..22 " "
                      IDENT@22..23 "a"
                    COLON@23..24 ":"
                    VALUE@24..26
                      VALUE@24..26
                        WHITESPACE@24..25 " "
                        IDENT@25..26 "b"
                  WHITESPACE@26..27 " "
                  RBRACE@27..28 "}"
        "##]],
    );
}

#[test]
fn css_function_declaration() {
    check(
        "@function --my-fn() { result: b }",
        expect![[r#"
            SOURCE_FILE@0..33
              GENERIC_AT_RULE@0..33
                AT@0..1 "@"
                IDENT@1..9 "function"
                WHITESPACE@9..10 " "
                IDENT@10..17 "--my-fn"
                LPAREN@17..18 "("
                RPAREN@18..19 ")"
                BLOCK@19..33
                  WHITESPACE@19..20 " "
                  LBRACE@20..21 "{"
                  DECLARATION@21..31
                    PROPERTY@21..28
                      WHITESPACE@21..22 " "
                      IDENT@22..28 "result"
                    COLON@28..29 ":"
                    VALUE@29..31
                      WHITESPACE@29..30 " "
                      IDENT@30..31 "b"
                  WHITESPACE@31..32 " "
                  RBRACE@32..33 "}"
        "#]],
    );
}

#[test]
fn css_function_raw_characters() {
    check(
        "@function --a() {\n  result: {}#&%^*;\n}",
        expect![[r##"
            SOURCE_FILE@0..38
              GENERIC_AT_RULE@0..38
                AT@0..1 "@"
                IDENT@1..9 "function"
                WHITESPACE@9..10 " "
                IDENT@10..13 "--a"
                LPAREN@13..14 "("
                RPAREN@14..15 ")"
                BLOCK@15..38
                  WHITESPACE@15..16 " "
                  LBRACE@16..17 "{"
                  DECLARATION@17..36
                    PROPERTY@17..26
                      WHITESPACE@17..20 "\n  "
                      IDENT@20..26 "result"
                    COLON@26..27 ":"
                    VALUE@27..35
                      WHITESPACE@27..28 " "
                      LBRACE@28..29 "{"
                      RBRACE@29..30 "}"
                      HASH@30..31 "#"
                      AMP@31..32 "&"
                      PERCENT@32..33 "%"
                      ERROR@33..34 "^"
                      STAR@34..35 "*"
                    SEMICOLON@35..36 ";"
                  WHITESPACE@36..37 "\n"
                  RBRACE@37..38 "}"
        "##]],
    );
}

#[test]
fn css_function_uppercase_raw_characters() {
    check(
        "@FUNCTION --a() {\n  result: {}#&%^*;\n}",
        expect![[r##"
            SOURCE_FILE@0..38
              GENERIC_AT_RULE@0..38
                AT@0..1 "@"
                IDENT@1..9 "FUNCTION"
                WHITESPACE@9..10 " "
                IDENT@10..13 "--a"
                LPAREN@13..14 "("
                RPAREN@14..15 ")"
                BLOCK@15..38
                  WHITESPACE@15..16 " "
                  LBRACE@16..17 "{"
                  DECLARATION@17..36
                    PROPERTY@17..26
                      WHITESPACE@17..20 "\n  "
                      IDENT@20..26 "result"
                    COLON@26..27 ":"
                    VALUE@27..35
                      WHITESPACE@27..28 " "
                      LBRACE@28..29 "{"
                      RBRACE@29..30 "}"
                      HASH@30..31 "#"
                      AMP@31..32 "&"
                      PERCENT@32..33 "%"
                      ERROR@33..34 "^"
                      STAR@34..35 "*"
                    SEMICOLON@35..36 ";"
                  WHITESPACE@36..37 "\n"
                  RBRACE@37..38 "}"
        "##]],
    );
}

#[test]
fn css_function_with_returns_clause() {
    check(
        "@function --a() returns <ident> { result: b }",
        expect![[r#"
            SOURCE_FILE@0..45
              GENERIC_AT_RULE@0..45
                AT@0..1 "@"
                IDENT@1..9 "function"
                WHITESPACE@9..10 " "
                IDENT@10..13 "--a"
                LPAREN@13..14 "("
                RPAREN@14..15 ")"
                WHITESPACE@15..16 " "
                IDENT@16..23 "returns"
                WHITESPACE@23..24 " "
                LT@24..25 "<"
                IDENT@25..30 "ident"
                GT@30..31 ">"
                BLOCK@31..45
                  WHITESPACE@31..32 " "
                  LBRACE@32..33 "{"
                  DECLARATION@33..43
                    PROPERTY@33..40
                      WHITESPACE@33..34 " "
                      IDENT@34..40 "result"
                    COLON@40..41 ":"
                    VALUE@41..43
                      WHITESPACE@41..42 " "
                      IDENT@42..43 "b"
                  WHITESPACE@43..44 " "
                  RBRACE@44..45 "}"
        "#]],
    );
}

#[test]
fn css_function_normal_sass_function_still_works() {
    check(
        "@function double($n) { @return $n * 2; }",
        expect![[r#"
            SOURCE_FILE@0..40
              FUNCTION_RULE@0..40
                AT@0..1 "@"
                IDENT@1..9 "function"
                WHITESPACE@9..10 " "
                IDENT@10..16 "double"
                PARAM_LIST@16..20
                  LPAREN@16..17 "("
                  PARAM@17..19
                    DOLLAR@17..18 "$"
                    IDENT@18..19 "n"
                  RPAREN@19..20 ")"
                BLOCK@20..40
                  WHITESPACE@20..21 " "
                  LBRACE@21..22 "{"
                  RETURN_RULE@22..38
                    WHITESPACE@22..23 " "
                    AT@23..24 "@"
                    IDENT@24..30 "return"
                    BINARY_EXPR@30..37
                      VARIABLE_REF@30..33
                        WHITESPACE@30..31 " "
                        DOLLAR@31..32 "$"
                        IDENT@32..33 "n"
                      WHITESPACE@33..34 " "
                      STAR@34..35 "*"
                      NUMBER_LITERAL@35..37
                        WHITESPACE@35..36 " "
                        NUMBER@36..37 "2"
                    SEMICOLON@37..38 ";"
                  WHITESPACE@38..39 " "
                  RBRACE@39..40 "}"
        "#]],
    );
}

#[test]
fn at_root_query_quoted_string() {
    check(
        "@at-root (without: \"media\") { a { b: c } }",
        expect![[r#"
            SOURCE_FILE@0..42
              AT_ROOT_RULE@0..42
                AT@0..1 "@"
                IDENT@1..8 "at-root"
                AT_ROOT_QUERY@8..27
                  WHITESPACE@8..9 " "
                  LPAREN@9..10 "("
                  IDENT@10..17 "without"
                  COLON@17..18 ":"
                  WHITESPACE@18..19 " "
                  QUOTED_STRING@19..26 "\"media\""
                  RPAREN@26..27 ")"
                BLOCK@27..42
                  WHITESPACE@27..28 " "
                  LBRACE@28..29 "{"
                  RULE_SET@29..40
                    SELECTOR_LIST@29..31
                      SELECTOR@29..31
                        SIMPLE_SELECTOR@29..31
                          WHITESPACE@29..30 " "
                          IDENT@30..31 "a"
                    BLOCK@31..40
                      WHITESPACE@31..32 " "
                      LBRACE@32..33 "{"
                      DECLARATION@33..38
                        PROPERTY@33..35
                          WHITESPACE@33..34 " "
                          IDENT@34..35 "b"
                        COLON@35..36 ":"
                        VALUE@36..38
                          VALUE@36..38
                            WHITESPACE@36..37 " "
                            IDENT@37..38 "c"
                      WHITESPACE@38..39 " "
                      RBRACE@39..40 "}"
                  WHITESPACE@40..41 " "
                  RBRACE@41..42 "}"
        "#]],
    );
}
