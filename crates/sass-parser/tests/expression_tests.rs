use expect_test::{Expect, expect};
use sass_parser::syntax::{SyntaxNode, debug_tree};

#[allow(clippy::needless_pass_by_value)]
fn check(source: &str, expect: Expect) {
    let (green, errors) = sass_parser::parse(source);
    let tree = SyntaxNode::new_root(green);

    assert_eq!(
        tree.text().to_string(),
        source,
        "lossless round-trip failed"
    );

    let mut buf = debug_tree(&tree);
    if !errors.is_empty() {
        buf.push_str("errors:\n");
        for (msg, range) in &errors {
            use std::fmt::Write;
            let _ = writeln!(buf, "  {range:?}: {msg}");
        }
    }
    expect.assert_eq(&buf);
}

// ── Variable declarations ──────────────────────────────────────────

#[test]
fn variable_simple() {
    check(
        "$color: red;",
        expect![[r#"
        SOURCE_FILE@0..12
          VARIABLE_DECL@0..12
            DOLLAR@0..1 "$"
            IDENT@1..6 "color"
            COLON@6..7 ":"
            VALUE@7..11
              WHITESPACE@7..8 " "
              IDENT@8..11 "red"
            SEMICOLON@11..12 ";"
    "#]],
    );
}

#[test]
fn variable_number() {
    check(
        "$width: 100px;",
        expect![[r#"
        SOURCE_FILE@0..14
          VARIABLE_DECL@0..14
            DOLLAR@0..1 "$"
            IDENT@1..6 "width"
            COLON@6..7 ":"
            DIMENSION@7..13
              WHITESPACE@7..8 " "
              NUMBER@8..11 "100"
              IDENT@11..13 "px"
            SEMICOLON@13..14 ";"
    "#]],
    );
}

#[test]
fn variable_default() {
    check(
        "$color: blue !default;",
        expect![[r#"
        SOURCE_FILE@0..22
          VARIABLE_DECL@0..22
            DOLLAR@0..1 "$"
            IDENT@1..6 "color"
            COLON@6..7 ":"
            VALUE@7..12
              WHITESPACE@7..8 " "
              IDENT@8..12 "blue"
            SASS_FLAG@12..21
              WHITESPACE@12..13 " "
              BANG@13..14 "!"
              IDENT@14..21 "default"
            SEMICOLON@21..22 ";"
    "#]],
    );
}

#[test]
fn variable_global() {
    check(
        "$color: red !global;",
        expect![[r#"
        SOURCE_FILE@0..20
          VARIABLE_DECL@0..20
            DOLLAR@0..1 "$"
            IDENT@1..6 "color"
            COLON@6..7 ":"
            VALUE@7..11
              WHITESPACE@7..8 " "
              IDENT@8..11 "red"
            SASS_FLAG@11..19
              WHITESPACE@11..12 " "
              BANG@12..13 "!"
              IDENT@13..19 "global"
            SEMICOLON@19..20 ";"
    "#]],
    );
}

#[test]
fn variable_default_global() {
    check(
        "$x: 1 !default !global;",
        expect![[r#"
        SOURCE_FILE@0..23
          VARIABLE_DECL@0..23
            DOLLAR@0..1 "$"
            IDENT@1..2 "x"
            COLON@2..3 ":"
            NUMBER_LITERAL@3..5
              WHITESPACE@3..4 " "
              NUMBER@4..5 "1"
            SASS_FLAG@5..14
              WHITESPACE@5..6 " "
              BANG@6..7 "!"
              IDENT@7..14 "default"
            SASS_FLAG@14..22
              WHITESPACE@14..15 " "
              BANG@15..16 "!"
              IDENT@16..22 "global"
            SEMICOLON@22..23 ";"
    "#]],
    );
}

#[test]
fn variable_inside_block() {
    check(
        ".box { $size: 10px; }",
        expect![[r#"
        SOURCE_FILE@0..21
          RULE_SET@0..21
            SELECTOR_LIST@0..4
              SELECTOR@0..4
                SIMPLE_SELECTOR@0..4
                  DOT@0..1 "."
                  IDENT@1..4 "box"
            BLOCK@4..21
              WHITESPACE@4..5 " "
              LBRACE@5..6 "{"
              VARIABLE_DECL@6..19
                WHITESPACE@6..7 " "
                DOLLAR@7..8 "$"
                IDENT@8..12 "size"
                COLON@12..13 ":"
                DIMENSION@13..18
                  WHITESPACE@13..14 " "
                  NUMBER@14..16 "10"
                  IDENT@16..18 "px"
                SEMICOLON@18..19 ";"
              WHITESPACE@19..20 " "
              RBRACE@20..21 "}"
    "#]],
    );
}

// ── Variable references ────────────────────────────────────────────

#[test]
fn variable_ref_in_value() {
    check(
        "div { color: $color; }",
        expect![[r#"
        SOURCE_FILE@0..22
          RULE_SET@0..22
            SELECTOR_LIST@0..3
              SELECTOR@0..3
                SIMPLE_SELECTOR@0..3
                  IDENT@0..3 "div"
            BLOCK@3..22
              WHITESPACE@3..4 " "
              LBRACE@4..5 "{"
              DECLARATION@5..20
                PROPERTY@5..11
                  WHITESPACE@5..6 " "
                  IDENT@6..11 "color"
                COLON@11..12 ":"
                VALUE@12..19
                  VARIABLE_REF@12..19
                    WHITESPACE@12..13 " "
                    DOLLAR@13..14 "$"
                    IDENT@14..19 "color"
                SEMICOLON@19..20 ";"
              WHITESPACE@20..21 " "
              RBRACE@21..22 "}"
    "#]],
    );
}

// ── Arithmetic expressions ─────────────────────────────────────────

#[test]
fn expr_addition() {
    check(
        "$a: 1 + 2;",
        expect![[r#"
        SOURCE_FILE@0..10
          VARIABLE_DECL@0..10
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            BINARY_EXPR@3..9
              NUMBER_LITERAL@3..5
                WHITESPACE@3..4 " "
                NUMBER@4..5 "1"
              WHITESPACE@5..6 " "
              PLUS@6..7 "+"
              NUMBER_LITERAL@7..9
                WHITESPACE@7..8 " "
                NUMBER@8..9 "2"
            SEMICOLON@9..10 ";"
    "#]],
    );
}

#[test]
fn expr_multiplication_precedence() {
    check(
        "$a: 1 + 2 * 3;",
        expect![[r#"
        SOURCE_FILE@0..14
          VARIABLE_DECL@0..14
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            BINARY_EXPR@3..13
              NUMBER_LITERAL@3..5
                WHITESPACE@3..4 " "
                NUMBER@4..5 "1"
              WHITESPACE@5..6 " "
              PLUS@6..7 "+"
              BINARY_EXPR@7..13
                NUMBER_LITERAL@7..9
                  WHITESPACE@7..8 " "
                  NUMBER@8..9 "2"
                WHITESPACE@9..10 " "
                STAR@10..11 "*"
                NUMBER_LITERAL@11..13
                  WHITESPACE@11..12 " "
                  NUMBER@12..13 "3"
            SEMICOLON@13..14 ";"
    "#]],
    );
}

#[test]
fn expr_left_associativity() {
    check(
        "$a: 1 - 2 - 3;",
        expect![[r#"
        SOURCE_FILE@0..14
          VARIABLE_DECL@0..14
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            BINARY_EXPR@3..13
              BINARY_EXPR@3..9
                NUMBER_LITERAL@3..5
                  WHITESPACE@3..4 " "
                  NUMBER@4..5 "1"
                WHITESPACE@5..6 " "
                MINUS@6..7 "-"
                NUMBER_LITERAL@7..9
                  WHITESPACE@7..8 " "
                  NUMBER@8..9 "2"
              WHITESPACE@9..10 " "
              MINUS@10..11 "-"
              NUMBER_LITERAL@11..13
                WHITESPACE@11..12 " "
                NUMBER@12..13 "3"
            SEMICOLON@13..14 ";"
    "#]],
    );
}

#[test]
fn expr_division_in_sass_context() {
    check(
        "$a: 10 / 2;",
        expect![[r#"
        SOURCE_FILE@0..11
          VARIABLE_DECL@0..11
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            BINARY_EXPR@3..10
              NUMBER_LITERAL@3..6
                WHITESPACE@3..4 " "
                NUMBER@4..6 "10"
              WHITESPACE@6..7 " "
              SLASH@7..8 "/"
              NUMBER_LITERAL@8..10
                WHITESPACE@8..9 " "
                NUMBER@9..10 "2"
            SEMICOLON@10..11 ";"
    "#]],
    );
}

#[test]
fn expr_modulo() {
    check(
        "$a: 10 % 3;",
        expect![[r#"
        SOURCE_FILE@0..11
          VARIABLE_DECL@0..11
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            BINARY_EXPR@3..10
              NUMBER_LITERAL@3..6
                WHITESPACE@3..4 " "
                NUMBER@4..6 "10"
              WHITESPACE@6..7 " "
              PERCENT@7..8 "%"
              NUMBER_LITERAL@8..10
                WHITESPACE@8..9 " "
                NUMBER@9..10 "3"
            SEMICOLON@10..11 ";"
    "#]],
    );
}

#[test]
fn expr_all_operators() {
    check(
        "$a: 2 * 3 + 4 - 1;",
        expect![[r#"
        SOURCE_FILE@0..18
          VARIABLE_DECL@0..18
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            BINARY_EXPR@3..17
              BINARY_EXPR@3..13
                BINARY_EXPR@3..9
                  NUMBER_LITERAL@3..5
                    WHITESPACE@3..4 " "
                    NUMBER@4..5 "2"
                  WHITESPACE@5..6 " "
                  STAR@6..7 "*"
                  NUMBER_LITERAL@7..9
                    WHITESPACE@7..8 " "
                    NUMBER@8..9 "3"
                WHITESPACE@9..10 " "
                PLUS@10..11 "+"
                NUMBER_LITERAL@11..13
                  WHITESPACE@11..12 " "
                  NUMBER@12..13 "4"
              WHITESPACE@13..14 " "
              MINUS@14..15 "-"
              NUMBER_LITERAL@15..17
                WHITESPACE@15..16 " "
                NUMBER@16..17 "1"
            SEMICOLON@17..18 ";"
    "#]],
    );
}

// ── Comparison and logical operators ───────────────────────────────

#[test]
fn expr_comparison_eq() {
    check(
        "$a: $x == $y;",
        expect![[r#"
        SOURCE_FILE@0..13
          VARIABLE_DECL@0..13
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            BINARY_EXPR@3..12
              VARIABLE_REF@3..6
                WHITESPACE@3..4 " "
                DOLLAR@4..5 "$"
                IDENT@5..6 "x"
              WHITESPACE@6..7 " "
              EQ_EQ@7..9 "=="
              VARIABLE_REF@9..12
                WHITESPACE@9..10 " "
                DOLLAR@10..11 "$"
                IDENT@11..12 "y"
            SEMICOLON@12..13 ";"
    "#]],
    );
}

#[test]
fn expr_comparison_neq() {
    check(
        "$a: $x != $y;",
        expect![[r#"
        SOURCE_FILE@0..13
          VARIABLE_DECL@0..13
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            BINARY_EXPR@3..12
              VARIABLE_REF@3..6
                WHITESPACE@3..4 " "
                DOLLAR@4..5 "$"
                IDENT@5..6 "x"
              WHITESPACE@6..7 " "
              BANG_EQ@7..9 "!="
              VARIABLE_REF@9..12
                WHITESPACE@9..10 " "
                DOLLAR@10..11 "$"
                IDENT@11..12 "y"
            SEMICOLON@12..13 ";"
    "#]],
    );
}

#[test]
fn expr_less_than() {
    check(
        "$a: $x < 10;",
        expect![[r#"
        SOURCE_FILE@0..12
          VARIABLE_DECL@0..12
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            BINARY_EXPR@3..11
              VARIABLE_REF@3..6
                WHITESPACE@3..4 " "
                DOLLAR@4..5 "$"
                IDENT@5..6 "x"
              WHITESPACE@6..7 " "
              LT@7..8 "<"
              NUMBER_LITERAL@8..11
                WHITESPACE@8..9 " "
                NUMBER@9..11 "10"
            SEMICOLON@11..12 ";"
    "#]],
    );
}

#[test]
fn expr_greater_eq() {
    check(
        "$a: $x >= 10;",
        expect![[r#"
        SOURCE_FILE@0..13
          VARIABLE_DECL@0..13
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            BINARY_EXPR@3..12
              VARIABLE_REF@3..6
                WHITESPACE@3..4 " "
                DOLLAR@4..5 "$"
                IDENT@5..6 "x"
              WHITESPACE@6..7 " "
              GT_EQ@7..9 ">="
              NUMBER_LITERAL@9..12
                WHITESPACE@9..10 " "
                NUMBER@10..12 "10"
            SEMICOLON@12..13 ";"
    "#]],
    );
}

#[test]
fn expr_less_eq() {
    check(
        "$a: $x <= 10;",
        expect![[r#"
        SOURCE_FILE@0..13
          VARIABLE_DECL@0..13
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            BINARY_EXPR@3..12
              VARIABLE_REF@3..6
                WHITESPACE@3..4 " "
                DOLLAR@4..5 "$"
                IDENT@5..6 "x"
              WHITESPACE@6..7 " "
              LT_EQ@7..9 "<="
              NUMBER_LITERAL@9..12
                WHITESPACE@9..10 " "
                NUMBER@10..12 "10"
            SEMICOLON@12..13 ";"
    "#]],
    );
}

#[test]
fn expr_logical_and_or() {
    check(
        "$a: $x and $y or $z;",
        expect![[r#"
        SOURCE_FILE@0..20
          VARIABLE_DECL@0..20
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            BINARY_EXPR@3..19
              BINARY_EXPR@3..13
                VARIABLE_REF@3..6
                  WHITESPACE@3..4 " "
                  DOLLAR@4..5 "$"
                  IDENT@5..6 "x"
                WHITESPACE@6..7 " "
                IDENT@7..10 "and"
                VARIABLE_REF@10..13
                  WHITESPACE@10..11 " "
                  DOLLAR@11..12 "$"
                  IDENT@12..13 "y"
              WHITESPACE@13..14 " "
              IDENT@14..16 "or"
              VARIABLE_REF@16..19
                WHITESPACE@16..17 " "
                DOLLAR@17..18 "$"
                IDENT@18..19 "z"
            SEMICOLON@19..20 ";"
    "#]],
    );
}

#[test]
fn expr_precedence_comparison_vs_arithmetic() {
    check(
        "$a: $x + 1 > $y - 2;",
        expect![[r#"
        SOURCE_FILE@0..20
          VARIABLE_DECL@0..20
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            BINARY_EXPR@3..19
              BINARY_EXPR@3..10
                VARIABLE_REF@3..6
                  WHITESPACE@3..4 " "
                  DOLLAR@4..5 "$"
                  IDENT@5..6 "x"
                WHITESPACE@6..7 " "
                PLUS@7..8 "+"
                NUMBER_LITERAL@8..10
                  WHITESPACE@8..9 " "
                  NUMBER@9..10 "1"
              WHITESPACE@10..11 " "
              GT@11..12 ">"
              BINARY_EXPR@12..19
                VARIABLE_REF@12..15
                  WHITESPACE@12..13 " "
                  DOLLAR@13..14 "$"
                  IDENT@14..15 "y"
                WHITESPACE@15..16 " "
                MINUS@16..17 "-"
                NUMBER_LITERAL@17..19
                  WHITESPACE@17..18 " "
                  NUMBER@18..19 "2"
            SEMICOLON@19..20 ";"
    "#]],
    );
}

#[test]
fn expr_precedence_logical_vs_comparison() {
    check(
        "$a: $x == 1 and $y != 2;",
        expect![[r#"
        SOURCE_FILE@0..24
          VARIABLE_DECL@0..24
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            BINARY_EXPR@3..23
              BINARY_EXPR@3..11
                VARIABLE_REF@3..6
                  WHITESPACE@3..4 " "
                  DOLLAR@4..5 "$"
                  IDENT@5..6 "x"
                WHITESPACE@6..7 " "
                EQ_EQ@7..9 "=="
                NUMBER_LITERAL@9..11
                  WHITESPACE@9..10 " "
                  NUMBER@10..11 "1"
              WHITESPACE@11..12 " "
              IDENT@12..15 "and"
              BINARY_EXPR@15..23
                VARIABLE_REF@15..18
                  WHITESPACE@15..16 " "
                  DOLLAR@16..17 "$"
                  IDENT@17..18 "y"
                WHITESPACE@18..19 " "
                BANG_EQ@19..21 "!="
                NUMBER_LITERAL@21..23
                  WHITESPACE@21..22 " "
                  NUMBER@22..23 "2"
            SEMICOLON@23..24 ";"
    "#]],
    );
}

// ── Unary operators ────────────────────────────────────────────────

#[test]
fn expr_unary_minus() {
    check(
        "$a: -$x;",
        expect![[r#"
        SOURCE_FILE@0..8
          VARIABLE_DECL@0..8
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            UNARY_EXPR@3..7
              WHITESPACE@3..4 " "
              MINUS@4..5 "-"
              VARIABLE_REF@5..7
                DOLLAR@5..6 "$"
                IDENT@6..7 "x"
            SEMICOLON@7..8 ";"
    "#]],
    );
}

#[test]
fn expr_unary_plus() {
    check(
        "$a: +$x;",
        expect![[r#"
        SOURCE_FILE@0..8
          VARIABLE_DECL@0..8
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            UNARY_EXPR@3..7
              WHITESPACE@3..4 " "
              PLUS@4..5 "+"
              VARIABLE_REF@5..7
                DOLLAR@5..6 "$"
                IDENT@6..7 "x"
            SEMICOLON@7..8 ";"
    "#]],
    );
}

#[test]
fn expr_not() {
    check(
        "$a: not $x;",
        expect![[r#"
        SOURCE_FILE@0..11
          VARIABLE_DECL@0..11
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            UNARY_EXPR@3..10
              WHITESPACE@3..4 " "
              IDENT@4..7 "not"
              VARIABLE_REF@7..10
                WHITESPACE@7..8 " "
                DOLLAR@8..9 "$"
                IDENT@9..10 "x"
            SEMICOLON@10..11 ";"
    "#]],
    );
}

#[test]
fn expr_unary_minus_in_binary() {
    check(
        "$a: 10 + -$x;",
        expect![[r#"
        SOURCE_FILE@0..13
          VARIABLE_DECL@0..13
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            BINARY_EXPR@3..12
              NUMBER_LITERAL@3..6
                WHITESPACE@3..4 " "
                NUMBER@4..6 "10"
              WHITESPACE@6..7 " "
              PLUS@7..8 "+"
              UNARY_EXPR@8..12
                WHITESPACE@8..9 " "
                MINUS@9..10 "-"
                VARIABLE_REF@10..12
                  DOLLAR@10..11 "$"
                  IDENT@11..12 "x"
            SEMICOLON@12..13 ";"
    "#]],
    );
}

// ── Whitespace disambiguation ──────────────────────────────────────

#[test]
fn ws_space_both_sides_is_infix() {
    check(
        "$a: $x - $y;",
        expect![[r#"
        SOURCE_FILE@0..12
          VARIABLE_DECL@0..12
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            BINARY_EXPR@3..11
              VARIABLE_REF@3..6
                WHITESPACE@3..4 " "
                DOLLAR@4..5 "$"
                IDENT@5..6 "x"
              WHITESPACE@6..7 " "
              MINUS@7..8 "-"
              VARIABLE_REF@8..11
                WHITESPACE@8..9 " "
                DOLLAR@9..10 "$"
                IDENT@10..11 "y"
            SEMICOLON@11..12 ";"
    "#]],
    );
}

#[test]
fn ws_no_space_is_infix() {
    check(
        "$a: $x-$y;",
        expect![[r#"
            SOURCE_FILE@0..10
              VARIABLE_DECL@0..10
                DOLLAR@0..1 "$"
                IDENT@1..2 "a"
                COLON@2..3 ":"
                VARIABLE_REF@3..7
                  WHITESPACE@3..4 " "
                  DOLLAR@4..5 "$"
                  IDENT@5..7 "x-"
                VARIABLE_REF@7..9
                  DOLLAR@7..8 "$"
                  IDENT@8..9 "y"
                SEMICOLON@9..10 ";"
        "#]],
    );
}

// ── Literals ───────────────────────────────────────────────────────

#[test]
fn literal_boolean_true() {
    check(
        "$a: true;",
        expect![[r#"
        SOURCE_FILE@0..9
          VARIABLE_DECL@0..9
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            BOOL_LITERAL@3..8
              WHITESPACE@3..4 " "
              IDENT@4..8 "true"
            SEMICOLON@8..9 ";"
    "#]],
    );
}

#[test]
fn literal_boolean_false() {
    check(
        "$a: false;",
        expect![[r#"
        SOURCE_FILE@0..10
          VARIABLE_DECL@0..10
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            BOOL_LITERAL@3..9
              WHITESPACE@3..4 " "
              IDENT@4..9 "false"
            SEMICOLON@9..10 ";"
    "#]],
    );
}

#[test]
fn literal_null() {
    check(
        "$a: null;",
        expect![[r#"
        SOURCE_FILE@0..9
          VARIABLE_DECL@0..9
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            NULL_LITERAL@3..8
              WHITESPACE@3..4 " "
              IDENT@4..8 "null"
            SEMICOLON@8..9 ";"
    "#]],
    );
}

#[test]
fn literal_color_hex3() {
    check(
        "$c: #ff0;",
        expect![[r##"
        SOURCE_FILE@0..9
          VARIABLE_DECL@0..9
            DOLLAR@0..1 "$"
            IDENT@1..2 "c"
            COLON@2..3 ":"
            COLOR_LITERAL@3..8
              WHITESPACE@3..4 " "
              HASH@4..5 "#"
              IDENT@5..8 "ff0"
            SEMICOLON@8..9 ";"
    "##]],
    );
}

#[test]
fn literal_color_hex6() {
    check(
        "$c: #aabbcc;",
        expect![[r##"
        SOURCE_FILE@0..12
          VARIABLE_DECL@0..12
            DOLLAR@0..1 "$"
            IDENT@1..2 "c"
            COLON@2..3 ":"
            COLOR_LITERAL@3..11
              WHITESPACE@3..4 " "
              HASH@4..5 "#"
              IDENT@5..11 "aabbcc"
            SEMICOLON@11..12 ";"
    "##]],
    );
}

#[test]
fn literal_string_quoted() {
    check(
        "$s: \"hello\";",
        expect![[r#"
        SOURCE_FILE@0..12
          VARIABLE_DECL@0..12
            DOLLAR@0..1 "$"
            IDENT@1..2 "s"
            COLON@2..3 ":"
            STRING_LITERAL@3..11
              WHITESPACE@3..4 " "
              QUOTED_STRING@4..11 "\"hello\""
            SEMICOLON@11..12 ";"
    "#]],
    );
}

#[test]
fn literal_dimension_px() {
    check(
        "$w: 10px;",
        expect![[r#"
        SOURCE_FILE@0..9
          VARIABLE_DECL@0..9
            DOLLAR@0..1 "$"
            IDENT@1..2 "w"
            COLON@2..3 ":"
            DIMENSION@3..8
              WHITESPACE@3..4 " "
              NUMBER@4..6 "10"
              IDENT@6..8 "px"
            SEMICOLON@8..9 ";"
    "#]],
    );
}

#[test]
fn literal_dimension_em() {
    check(
        "$w: 2em;",
        expect![[r#"
        SOURCE_FILE@0..8
          VARIABLE_DECL@0..8
            DOLLAR@0..1 "$"
            IDENT@1..2 "w"
            COLON@2..3 ":"
            DIMENSION@3..7
              WHITESPACE@3..4 " "
              NUMBER@4..5 "2"
              IDENT@5..7 "em"
            SEMICOLON@7..8 ";"
    "#]],
    );
}

#[test]
fn literal_dimension_percent() {
    check(
        "$w: 50%;",
        expect![[r#"
        SOURCE_FILE@0..8
          VARIABLE_DECL@0..8
            DOLLAR@0..1 "$"
            IDENT@1..2 "w"
            COLON@2..3 ":"
            DIMENSION@3..7
              WHITESPACE@3..4 " "
              NUMBER@4..6 "50"
              PERCENT@6..7 "%"
            SEMICOLON@7..8 ";"
    "#]],
    );
}

#[test]
fn literal_number_plain() {
    check(
        "$n: 42;",
        expect![[r#"
        SOURCE_FILE@0..7
          VARIABLE_DECL@0..7
            DOLLAR@0..1 "$"
            IDENT@1..2 "n"
            COLON@2..3 ":"
            NUMBER_LITERAL@3..6
              WHITESPACE@3..4 " "
              NUMBER@4..6 "42"
            SEMICOLON@6..7 ";"
    "#]],
    );
}

#[test]
fn literal_number_decimal() {
    check(
        "$n: 3.14;",
        expect![[r#"
        SOURCE_FILE@0..9
          VARIABLE_DECL@0..9
            DOLLAR@0..1 "$"
            IDENT@1..2 "n"
            COLON@2..3 ":"
            NUMBER_LITERAL@3..8
              WHITESPACE@3..4 " "
              NUMBER@4..8 "3.14"
            SEMICOLON@8..9 ";"
    "#]],
    );
}

// ── Function calls ─────────────────────────────────────────────────

#[test]
fn function_call_simple() {
    check(
        "$a: darken($color, 10%);",
        expect![[r#"
        SOURCE_FILE@0..24
          VARIABLE_DECL@0..24
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            FUNCTION_CALL@3..23
              WHITESPACE@3..4 " "
              IDENT@4..10 "darken"
              ARG_LIST@10..23
                LPAREN@10..11 "("
                ARG@11..17
                  VARIABLE_REF@11..17
                    DOLLAR@11..12 "$"
                    IDENT@12..17 "color"
                COMMA@17..18 ","
                ARG@18..22
                  DIMENSION@18..22
                    WHITESPACE@18..19 " "
                    NUMBER@19..21 "10"
                    PERCENT@21..22 "%"
                RPAREN@22..23 ")"
            SEMICOLON@23..24 ";"
    "#]],
    );
}

#[test]
fn function_call_keyword_arg() {
    check(
        "$a: rgba($red: 255, $green: 0, $blue: 0);",
        expect![[r#"
        SOURCE_FILE@0..41
          VARIABLE_DECL@0..41
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            FUNCTION_CALL@3..40
              WHITESPACE@3..4 " "
              IDENT@4..8 "rgba"
              ARG_LIST@8..40
                LPAREN@8..9 "("
                ARG@9..18
                  DOLLAR@9..10 "$"
                  IDENT@10..13 "red"
                  COLON@13..14 ":"
                  NUMBER_LITERAL@14..18
                    WHITESPACE@14..15 " "
                    NUMBER@15..18 "255"
                COMMA@18..19 ","
                ARG@19..29
                  WHITESPACE@19..20 " "
                  DOLLAR@20..21 "$"
                  IDENT@21..26 "green"
                  COLON@26..27 ":"
                  NUMBER_LITERAL@27..29
                    WHITESPACE@27..28 " "
                    NUMBER@28..29 "0"
                COMMA@29..30 ","
                ARG@30..39
                  WHITESPACE@30..31 " "
                  DOLLAR@31..32 "$"
                  IDENT@32..36 "blue"
                  COLON@36..37 ":"
                  NUMBER_LITERAL@37..39
                    WHITESPACE@37..38 " "
                    NUMBER@38..39 "0"
                RPAREN@39..40 ")"
            SEMICOLON@40..41 ";"
    "#]],
    );
}

#[test]
fn function_call_if() {
    check(
        "$x: if($cond, $a, $b);",
        expect![[r#"
        SOURCE_FILE@0..22
          VARIABLE_DECL@0..22
            DOLLAR@0..1 "$"
            IDENT@1..2 "x"
            COLON@2..3 ":"
            FUNCTION_CALL@3..21
              WHITESPACE@3..4 " "
              IDENT@4..6 "if"
              ARG_LIST@6..21
                LPAREN@6..7 "("
                ARG@7..12
                  VARIABLE_REF@7..12
                    DOLLAR@7..8 "$"
                    IDENT@8..12 "cond"
                COMMA@12..13 ","
                ARG@13..16
                  VARIABLE_REF@13..16
                    WHITESPACE@13..14 " "
                    DOLLAR@14..15 "$"
                    IDENT@15..16 "a"
                COMMA@16..17 ","
                ARG@17..20
                  VARIABLE_REF@17..20
                    WHITESPACE@17..18 " "
                    DOLLAR@18..19 "$"
                    IDENT@19..20 "b"
                RPAREN@20..21 ")"
            SEMICOLON@21..22 ";"
    "#]],
    );
}

#[test]
fn function_call_nested() {
    check(
        "$a: lighten(darken($base, 10%), 5%);",
        expect![[r#"
        SOURCE_FILE@0..36
          VARIABLE_DECL@0..36
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            FUNCTION_CALL@3..35
              WHITESPACE@3..4 " "
              IDENT@4..11 "lighten"
              ARG_LIST@11..35
                LPAREN@11..12 "("
                ARG@12..30
                  FUNCTION_CALL@12..30
                    IDENT@12..18 "darken"
                    ARG_LIST@18..30
                      LPAREN@18..19 "("
                      ARG@19..24
                        VARIABLE_REF@19..24
                          DOLLAR@19..20 "$"
                          IDENT@20..24 "base"
                      COMMA@24..25 ","
                      ARG@25..29
                        DIMENSION@25..29
                          WHITESPACE@25..26 " "
                          NUMBER@26..28 "10"
                          PERCENT@28..29 "%"
                      RPAREN@29..30 ")"
                COMMA@30..31 ","
                ARG@31..34
                  DIMENSION@31..34
                    WHITESPACE@31..32 " "
                    NUMBER@32..33 "5"
                    PERCENT@33..34 "%"
                RPAREN@34..35 ")"
            SEMICOLON@35..36 ";"
    "#]],
    );
}

#[test]
fn function_call_no_args() {
    check(
        "$a: unique-id();",
        expect![[r#"
        SOURCE_FILE@0..16
          VARIABLE_DECL@0..16
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            FUNCTION_CALL@3..15
              WHITESPACE@3..4 " "
              IDENT@4..13 "unique-id"
              ARG_LIST@13..15
                LPAREN@13..14 "("
                RPAREN@14..15 ")"
            SEMICOLON@15..16 ";"
    "#]],
    );
}

// ── Calculation functions ──────────────────────────────────────────

#[test]
fn calc_simple() {
    check(
        "div { width: calc(100% - 20px); }",
        expect![[r#"
        SOURCE_FILE@0..33
          RULE_SET@0..33
            SELECTOR_LIST@0..3
              SELECTOR@0..3
                SIMPLE_SELECTOR@0..3
                  IDENT@0..3 "div"
            BLOCK@3..33
              WHITESPACE@3..4 " "
              LBRACE@4..5 "{"
              DECLARATION@5..31
                PROPERTY@5..11
                  WHITESPACE@5..6 " "
                  IDENT@6..11 "width"
                COLON@11..12 ":"
                VALUE@12..30
                  CALCULATION@12..30
                    WHITESPACE@12..13 " "
                    IDENT@13..17 "calc"
                    LPAREN@17..18 "("
                    CALC_SUM@18..29
                      CALC_VALUE@18..22
                        DIMENSION@18..22
                          NUMBER@18..21 "100"
                          PERCENT@21..22 "%"
                      WHITESPACE@22..23 " "
                      MINUS@23..24 "-"
                      CALC_VALUE@24..29
                        DIMENSION@24..29
                          WHITESPACE@24..25 " "
                          NUMBER@25..27 "20"
                          IDENT@27..29 "px"
                    RPAREN@29..30 ")"
                SEMICOLON@30..31 ";"
              WHITESPACE@31..32 " "
              RBRACE@32..33 "}"
    "#]],
    );
}

#[test]
fn calc_min() {
    check(
        "div { width: min(100px, 50%); }",
        expect![[r#"
        SOURCE_FILE@0..31
          RULE_SET@0..31
            SELECTOR_LIST@0..3
              SELECTOR@0..3
                SIMPLE_SELECTOR@0..3
                  IDENT@0..3 "div"
            BLOCK@3..31
              WHITESPACE@3..4 " "
              LBRACE@4..5 "{"
              DECLARATION@5..29
                PROPERTY@5..11
                  WHITESPACE@5..6 " "
                  IDENT@6..11 "width"
                COLON@11..12 ":"
                VALUE@12..28
                  CALCULATION@12..28
                    WHITESPACE@12..13 " "
                    IDENT@13..16 "min"
                    LPAREN@16..17 "("
                    CALC_VALUE@17..22
                      DIMENSION@17..22
                        NUMBER@17..20 "100"
                        IDENT@20..22 "px"
                    COMMA@22..23 ","
                    CALC_VALUE@23..27
                      DIMENSION@23..27
                        WHITESPACE@23..24 " "
                        NUMBER@24..26 "50"
                        PERCENT@26..27 "%"
                    RPAREN@27..28 ")"
                SEMICOLON@28..29 ";"
              WHITESPACE@29..30 " "
              RBRACE@30..31 "}"
    "#]],
    );
}

#[test]
fn calc_clamp() {
    check(
        "div { width: clamp(200px, 50%, 800px); }",
        expect![[r#"
        SOURCE_FILE@0..40
          RULE_SET@0..40
            SELECTOR_LIST@0..3
              SELECTOR@0..3
                SIMPLE_SELECTOR@0..3
                  IDENT@0..3 "div"
            BLOCK@3..40
              WHITESPACE@3..4 " "
              LBRACE@4..5 "{"
              DECLARATION@5..38
                PROPERTY@5..11
                  WHITESPACE@5..6 " "
                  IDENT@6..11 "width"
                COLON@11..12 ":"
                VALUE@12..37
                  CALCULATION@12..37
                    WHITESPACE@12..13 " "
                    IDENT@13..18 "clamp"
                    LPAREN@18..19 "("
                    CALC_VALUE@19..24
                      DIMENSION@19..24
                        NUMBER@19..22 "200"
                        IDENT@22..24 "px"
                    COMMA@24..25 ","
                    CALC_VALUE@25..29
                      DIMENSION@25..29
                        WHITESPACE@25..26 " "
                        NUMBER@26..28 "50"
                        PERCENT@28..29 "%"
                    COMMA@29..30 ","
                    CALC_VALUE@30..36
                      DIMENSION@30..36
                        WHITESPACE@30..31 " "
                        NUMBER@31..34 "800"
                        IDENT@34..36 "px"
                    RPAREN@36..37 ")"
                SEMICOLON@37..38 ";"
              WHITESPACE@38..39 " "
              RBRACE@39..40 "}"
    "#]],
    );
}

#[test]
fn calc_with_variable() {
    check(
        "$w: calc($base + 10px);",
        expect![[r#"
        SOURCE_FILE@0..23
          VARIABLE_DECL@0..23
            DOLLAR@0..1 "$"
            IDENT@1..2 "w"
            COLON@2..3 ":"
            CALCULATION@3..22
              WHITESPACE@3..4 " "
              IDENT@4..8 "calc"
              LPAREN@8..9 "("
              CALC_SUM@9..21
                CALC_VALUE@9..14
                  VARIABLE_REF@9..14
                    DOLLAR@9..10 "$"
                    IDENT@10..14 "base"
                WHITESPACE@14..15 " "
                PLUS@15..16 "+"
                CALC_VALUE@16..21
                  DIMENSION@16..21
                    WHITESPACE@16..17 " "
                    NUMBER@17..19 "10"
                    IDENT@19..21 "px"
              RPAREN@21..22 ")"
            SEMICOLON@22..23 ";"
    "#]],
    );
}

#[test]
fn calc_product() {
    check(
        "div { width: calc(100% * 0.5); }",
        expect![[r#"
        SOURCE_FILE@0..32
          RULE_SET@0..32
            SELECTOR_LIST@0..3
              SELECTOR@0..3
                SIMPLE_SELECTOR@0..3
                  IDENT@0..3 "div"
            BLOCK@3..32
              WHITESPACE@3..4 " "
              LBRACE@4..5 "{"
              DECLARATION@5..30
                PROPERTY@5..11
                  WHITESPACE@5..6 " "
                  IDENT@6..11 "width"
                COLON@11..12 ":"
                VALUE@12..29
                  CALCULATION@12..29
                    WHITESPACE@12..13 " "
                    IDENT@13..17 "calc"
                    LPAREN@17..18 "("
                    CALC_PRODUCT@18..28
                      CALC_VALUE@18..22
                        DIMENSION@18..22
                          NUMBER@18..21 "100"
                          PERCENT@21..22 "%"
                      WHITESPACE@22..23 " "
                      STAR@23..24 "*"
                      CALC_VALUE@24..28
                        NUMBER_LITERAL@24..28
                          WHITESPACE@24..25 " "
                          NUMBER@25..28 "0.5"
                    RPAREN@28..29 ")"
                SEMICOLON@29..30 ";"
              WHITESPACE@30..31 " "
              RBRACE@31..32 "}"
    "#]],
    );
}

// ── Maps ───────────────────────────────────────────────────────────

#[test]
fn map_simple() {
    check(
        "$map: (a: 1, b: 2);",
        expect![[r#"
        SOURCE_FILE@0..19
          VARIABLE_DECL@0..19
            DOLLAR@0..1 "$"
            IDENT@1..4 "map"
            COLON@4..5 ":"
            MAP_EXPR@5..18
              WHITESPACE@5..6 " "
              LPAREN@6..7 "("
              MAP_ENTRY@7..11
                VALUE@7..8
                  IDENT@7..8 "a"
                COLON@8..9 ":"
                NUMBER_LITERAL@9..11
                  WHITESPACE@9..10 " "
                  NUMBER@10..11 "1"
              COMMA@11..12 ","
              MAP_ENTRY@12..17
                VALUE@12..14
                  WHITESPACE@12..13 " "
                  IDENT@13..14 "b"
                COLON@14..15 ":"
                NUMBER_LITERAL@15..17
                  WHITESPACE@15..16 " "
                  NUMBER@16..17 "2"
              RPAREN@17..18 ")"
            SEMICOLON@18..19 ";"
    "#]],
    );
}

#[test]
fn map_nested() {
    check(
        "$m: (colors: (r: 1, g: 2), sizes: (s: 3));",
        expect![[r#"
        SOURCE_FILE@0..42
          VARIABLE_DECL@0..42
            DOLLAR@0..1 "$"
            IDENT@1..2 "m"
            COLON@2..3 ":"
            MAP_EXPR@3..41
              WHITESPACE@3..4 " "
              LPAREN@4..5 "("
              MAP_ENTRY@5..25
                VALUE@5..11
                  IDENT@5..11 "colors"
                COLON@11..12 ":"
                MAP_EXPR@12..25
                  WHITESPACE@12..13 " "
                  LPAREN@13..14 "("
                  MAP_ENTRY@14..18
                    VALUE@14..15
                      IDENT@14..15 "r"
                    COLON@15..16 ":"
                    NUMBER_LITERAL@16..18
                      WHITESPACE@16..17 " "
                      NUMBER@17..18 "1"
                  COMMA@18..19 ","
                  MAP_ENTRY@19..24
                    VALUE@19..21
                      WHITESPACE@19..20 " "
                      IDENT@20..21 "g"
                    COLON@21..22 ":"
                    NUMBER_LITERAL@22..24
                      WHITESPACE@22..23 " "
                      NUMBER@23..24 "2"
                  RPAREN@24..25 ")"
              COMMA@25..26 ","
              MAP_ENTRY@26..40
                VALUE@26..32
                  WHITESPACE@26..27 " "
                  IDENT@27..32 "sizes"
                COLON@32..33 ":"
                MAP_EXPR@33..40
                  WHITESPACE@33..34 " "
                  LPAREN@34..35 "("
                  MAP_ENTRY@35..39
                    VALUE@35..36
                      IDENT@35..36 "s"
                    COLON@36..37 ":"
                    NUMBER_LITERAL@37..39
                      WHITESPACE@37..38 " "
                      NUMBER@38..39 "3"
                  RPAREN@39..40 ")"
              RPAREN@40..41 ")"
            SEMICOLON@41..42 ";"
    "#]],
    );
}

#[test]
fn map_trailing_comma() {
    check(
        "$m: (a: 1, b: 2,);",
        expect![[r#"
        SOURCE_FILE@0..18
          VARIABLE_DECL@0..18
            DOLLAR@0..1 "$"
            IDENT@1..2 "m"
            COLON@2..3 ":"
            MAP_EXPR@3..17
              WHITESPACE@3..4 " "
              LPAREN@4..5 "("
              MAP_ENTRY@5..9
                VALUE@5..6
                  IDENT@5..6 "a"
                COLON@6..7 ":"
                NUMBER_LITERAL@7..9
                  WHITESPACE@7..8 " "
                  NUMBER@8..9 "1"
              COMMA@9..10 ","
              MAP_ENTRY@10..15
                VALUE@10..12
                  WHITESPACE@10..11 " "
                  IDENT@11..12 "b"
                COLON@12..13 ":"
                NUMBER_LITERAL@13..15
                  WHITESPACE@13..14 " "
                  NUMBER@14..15 "2"
              COMMA@15..16 ","
              RPAREN@16..17 ")"
            SEMICOLON@17..18 ";"
    "#]],
    );
}

// ── Lists ──────────────────────────────────────────────────────────

#[test]
fn bracketed_list() {
    check(
        "$l: [1, 2, 3];",
        expect![[r#"
        SOURCE_FILE@0..14
          VARIABLE_DECL@0..14
            DOLLAR@0..1 "$"
            IDENT@1..2 "l"
            COLON@2..3 ":"
            BRACKETED_LIST@3..13
              WHITESPACE@3..4 " "
              LBRACKET@4..5 "["
              NUMBER_LITERAL@5..6
                NUMBER@5..6 "1"
              COMMA@6..7 ","
              NUMBER_LITERAL@7..9
                WHITESPACE@7..8 " "
                NUMBER@8..9 "2"
              COMMA@9..10 ","
              NUMBER_LITERAL@10..12
                WHITESPACE@10..11 " "
                NUMBER@11..12 "3"
              RBRACKET@12..13 "]"
            SEMICOLON@13..14 ";"
    "#]],
    );
}

#[test]
fn empty_list() {
    check(
        "$l: ();",
        expect![[r#"
        SOURCE_FILE@0..7
          VARIABLE_DECL@0..7
            DOLLAR@0..1 "$"
            IDENT@1..2 "l"
            COLON@2..3 ":"
            LIST_EXPR@3..6
              WHITESPACE@3..4 " "
              LPAREN@4..5 "("
              RPAREN@5..6 ")"
            SEMICOLON@6..7 ";"
    "#]],
    );
}

#[test]
fn paren_list_comma() {
    check(
        "$l: (1, 2, 3);",
        expect![[r#"
        SOURCE_FILE@0..14
          VARIABLE_DECL@0..14
            DOLLAR@0..1 "$"
            IDENT@1..2 "l"
            COLON@2..3 ":"
            LIST_EXPR@3..13
              WHITESPACE@3..4 " "
              LPAREN@4..5 "("
              NUMBER_LITERAL@5..6
                NUMBER@5..6 "1"
              COMMA@6..7 ","
              NUMBER_LITERAL@7..9
                WHITESPACE@7..8 " "
                NUMBER@8..9 "2"
              COMMA@9..10 ","
              NUMBER_LITERAL@10..12
                WHITESPACE@10..11 " "
                NUMBER@11..12 "3"
              RPAREN@12..13 ")"
            SEMICOLON@13..14 ";"
    "#]],
    );
}

// ── Space-separated and comma-separated lists in variable values ──

#[test]
fn var_space_separated_list() {
    check(
        "$x: 1px 2px 3px;",
        expect![[r#"
            SOURCE_FILE@0..16
              VARIABLE_DECL@0..16
                DOLLAR@0..1 "$"
                IDENT@1..2 "x"
                COLON@2..3 ":"
                DIMENSION@3..7
                  WHITESPACE@3..4 " "
                  NUMBER@4..5 "1"
                  IDENT@5..7 "px"
                DIMENSION@7..11
                  WHITESPACE@7..8 " "
                  NUMBER@8..9 "2"
                  IDENT@9..11 "px"
                DIMENSION@11..15
                  WHITESPACE@11..12 " "
                  NUMBER@12..13 "3"
                  IDENT@13..15 "px"
                SEMICOLON@15..16 ";"
        "#]],
    );
}

#[test]
fn var_comma_separated_list() {
    check(
        "$fonts: Arial, sans-serif;",
        expect![[r#"
            SOURCE_FILE@0..26
              VARIABLE_DECL@0..26
                DOLLAR@0..1 "$"
                IDENT@1..6 "fonts"
                COLON@6..7 ":"
                LIST_EXPR@7..25
                  VALUE@7..13
                    WHITESPACE@7..8 " "
                    IDENT@8..13 "Arial"
                  COMMA@13..14 ","
                  VALUE@14..25
                    WHITESPACE@14..15 " "
                    IDENT@15..25 "sans-serif"
                SEMICOLON@25..26 ";"
        "#]],
    );
}

#[test]
fn var_comma_of_space_lists() {
    check(
        "$x: 1px 2px, 3px 4px;",
        expect![[r#"
            SOURCE_FILE@0..21
              VARIABLE_DECL@0..21
                DOLLAR@0..1 "$"
                IDENT@1..2 "x"
                COLON@2..3 ":"
                LIST_EXPR@3..20
                  DIMENSION@3..7
                    WHITESPACE@3..4 " "
                    NUMBER@4..5 "1"
                    IDENT@5..7 "px"
                  DIMENSION@7..11
                    WHITESPACE@7..8 " "
                    NUMBER@8..9 "2"
                    IDENT@9..11 "px"
                  COMMA@11..12 ","
                  DIMENSION@12..16
                    WHITESPACE@12..13 " "
                    NUMBER@13..14 "3"
                    IDENT@14..16 "px"
                  DIMENSION@16..20
                    WHITESPACE@16..17 " "
                    NUMBER@17..18 "4"
                    IDENT@18..20 "px"
                SEMICOLON@20..21 ";"
        "#]],
    );
}

#[test]
fn function_arg_space_list() {
    check(
        "$x: if($a, 1px 2px, none);",
        expect![[r#"
            SOURCE_FILE@0..26
              VARIABLE_DECL@0..26
                DOLLAR@0..1 "$"
                IDENT@1..2 "x"
                COLON@2..3 ":"
                FUNCTION_CALL@3..25
                  WHITESPACE@3..4 " "
                  IDENT@4..6 "if"
                  ARG_LIST@6..25
                    LPAREN@6..7 "("
                    ARG@7..9
                      VARIABLE_REF@7..9
                        DOLLAR@7..8 "$"
                        IDENT@8..9 "a"
                    COMMA@9..10 ","
                    ARG@10..18
                      DIMENSION@10..14
                        WHITESPACE@10..11 " "
                        NUMBER@11..12 "1"
                        IDENT@12..14 "px"
                      DIMENSION@14..18
                        WHITESPACE@14..15 " "
                        NUMBER@15..16 "2"
                        IDENT@16..18 "px"
                    COMMA@18..19 ","
                    ARG@19..24
                      VALUE@19..24
                        WHITESPACE@19..20 " "
                        IDENT@20..24 "none"
                    RPAREN@24..25 ")"
                SEMICOLON@25..26 ";"
        "#]],
    );
}

#[test]
fn nested_rule_leading_combinator() {
    check(
        ".parent { > .child { color: red; } }",
        expect![[r#"
            SOURCE_FILE@0..36
              RULE_SET@0..36
                SELECTOR_LIST@0..7
                  SELECTOR@0..7
                    SIMPLE_SELECTOR@0..7
                      DOT@0..1 "."
                      IDENT@1..7 "parent"
                BLOCK@7..36
                  WHITESPACE@7..8 " "
                  LBRACE@8..9 "{"
                  RULE_SET@9..34
                    SELECTOR_LIST@9..18
                      SELECTOR@9..18
                        COMBINATOR@9..11
                          WHITESPACE@9..10 " "
                          GT@10..11 ">"
                        SIMPLE_SELECTOR@11..18
                          WHITESPACE@11..12 " "
                          DOT@12..13 "."
                          IDENT@13..18 "child"
                    BLOCK@18..34
                      WHITESPACE@18..19 " "
                      LBRACE@19..20 "{"
                      DECLARATION@20..32
                        PROPERTY@20..26
                          WHITESPACE@20..21 " "
                          IDENT@21..26 "color"
                        COLON@26..27 ":"
                        VALUE@27..31
                          VALUE@27..31
                            WHITESPACE@27..28 " "
                            IDENT@28..31 "red"
                        SEMICOLON@31..32 ";"
                      WHITESPACE@32..33 " "
                      RBRACE@33..34 "}"
                  WHITESPACE@34..35 " "
                  RBRACE@35..36 "}"
        "#]],
    );
}

// ── Parenthesized expressions ──────────────────────────────────────

#[test]
fn paren_expr() {
    check(
        "$a: (1 + 2) * 3;",
        expect![[r#"
        SOURCE_FILE@0..16
          VARIABLE_DECL@0..16
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            BINARY_EXPR@3..15
              PAREN_EXPR@3..11
                WHITESPACE@3..4 " "
                LPAREN@4..5 "("
                BINARY_EXPR@5..10
                  NUMBER_LITERAL@5..6
                    NUMBER@5..6 "1"
                  WHITESPACE@6..7 " "
                  PLUS@7..8 "+"
                  NUMBER_LITERAL@8..10
                    WHITESPACE@8..9 " "
                    NUMBER@9..10 "2"
                RPAREN@10..11 ")"
              WHITESPACE@11..12 " "
              STAR@12..13 "*"
              NUMBER_LITERAL@13..15
                WHITESPACE@13..14 " "
                NUMBER@14..15 "3"
            SEMICOLON@15..16 ";"
    "#]],
    );
}

#[test]
fn paren_expr_nested() {
    check(
        "$a: ((1 + 2) * (3 - 4));",
        expect![[r#"
        SOURCE_FILE@0..24
          VARIABLE_DECL@0..24
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            PAREN_EXPR@3..23
              WHITESPACE@3..4 " "
              LPAREN@4..5 "("
              BINARY_EXPR@5..22
                PAREN_EXPR@5..12
                  LPAREN@5..6 "("
                  BINARY_EXPR@6..11
                    NUMBER_LITERAL@6..7
                      NUMBER@6..7 "1"
                    WHITESPACE@7..8 " "
                    PLUS@8..9 "+"
                    NUMBER_LITERAL@9..11
                      WHITESPACE@9..10 " "
                      NUMBER@10..11 "2"
                  RPAREN@11..12 ")"
                WHITESPACE@12..13 " "
                STAR@13..14 "*"
                PAREN_EXPR@14..22
                  WHITESPACE@14..15 " "
                  LPAREN@15..16 "("
                  BINARY_EXPR@16..21
                    NUMBER_LITERAL@16..17
                      NUMBER@16..17 "3"
                    WHITESPACE@17..18 " "
                    MINUS@18..19 "-"
                    NUMBER_LITERAL@19..21
                      WHITESPACE@19..20 " "
                      NUMBER@20..21 "4"
                  RPAREN@21..22 ")"
              RPAREN@22..23 ")"
            SEMICOLON@23..24 ";"
    "#]],
    );
}

// ── Interpolation (Phase 3 upgraded) ───────────────────────────────

#[test]
fn interpolation_with_expression() {
    check(
        "div { color: #{$base + 1}; }",
        expect![[r##"
            SOURCE_FILE@0..28
              RULE_SET@0..28
                SELECTOR_LIST@0..3
                  SELECTOR@0..3
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "div"
                BLOCK@3..28
                  WHITESPACE@3..4 " "
                  LBRACE@4..5 "{"
                  DECLARATION@5..26
                    PROPERTY@5..11
                      WHITESPACE@5..6 " "
                      IDENT@6..11 "color"
                    COLON@11..12 ":"
                    VALUE@12..25
                      INTERPOLATION@12..25
                        WHITESPACE@12..13 " "
                        HASH_LBRACE@13..15 "#{"
                        BINARY_EXPR@15..24
                          VARIABLE_REF@15..20
                            DOLLAR@15..16 "$"
                            IDENT@16..20 "base"
                          WHITESPACE@20..21 " "
                          PLUS@21..22 "+"
                          NUMBER_LITERAL@22..24
                            WHITESPACE@22..23 " "
                            NUMBER@23..24 "1"
                        RBRACE@24..25 "}"
                    SEMICOLON@25..26 ";"
                  WHITESPACE@26..27 " "
                  RBRACE@27..28 "}"
        "##]],
    );
}

#[test]
fn interpolation_in_selector_with_var() {
    check(
        "#{$tag} { color: red; }",
        expect![[r##"
        SOURCE_FILE@0..23
          RULE_SET@0..23
            SELECTOR_LIST@0..7
              SELECTOR@0..7
                INTERPOLATION@0..7
                  HASH_LBRACE@0..2 "#{"
                  VARIABLE_REF@2..6
                    DOLLAR@2..3 "$"
                    IDENT@3..6 "tag"
                  RBRACE@6..7 "}"
            BLOCK@7..23
              WHITESPACE@7..8 " "
              LBRACE@8..9 "{"
              DECLARATION@9..21
                PROPERTY@9..15
                  WHITESPACE@9..10 " "
                  IDENT@10..15 "color"
                COLON@15..16 ":"
                VALUE@16..20
                  VALUE@16..20
                    WHITESPACE@16..17 " "
                    IDENT@17..20 "red"
                SEMICOLON@20..21 ";"
              WHITESPACE@21..22 " "
              RBRACE@22..23 "}"
    "##]],
    );
}

#[test]
fn interpolation_in_property() {
    check(
        "div { #{$prop}: red; }",
        expect![[r##"
        SOURCE_FILE@0..22
          RULE_SET@0..22
            SELECTOR_LIST@0..3
              SELECTOR@0..3
                SIMPLE_SELECTOR@0..3
                  IDENT@0..3 "div"
            BLOCK@3..22
              WHITESPACE@3..4 " "
              LBRACE@4..5 "{"
              DECLARATION@5..20
                PROPERTY@5..14
                  INTERPOLATION@5..14
                    WHITESPACE@5..6 " "
                    HASH_LBRACE@6..8 "#{"
                    VARIABLE_REF@8..13
                      DOLLAR@8..9 "$"
                      IDENT@9..13 "prop"
                    RBRACE@13..14 "}"
                COLON@14..15 ":"
                VALUE@15..19
                  VALUE@15..19
                    WHITESPACE@15..16 " "
                    IDENT@16..19 "red"
                SEMICOLON@19..20 ";"
              WHITESPACE@20..21 " "
              RBRACE@21..22 "}"
    "##]],
    );
}

// ── String interpolation ───────────────────────────────────────────

#[test]
fn string_interpolation_simple() {
    check(
        "$s: \"hello #{$name}!\";",
        expect![[r##"
            SOURCE_FILE@0..22
              VARIABLE_DECL@0..22
                DOLLAR@0..1 "$"
                IDENT@1..2 "s"
                COLON@2..3 ":"
                INTERPOLATED_STRING@3..21
                  WHITESPACE@3..4 " "
                  STRING_START@4..11 "\"hello "
                  INTERPOLATION@11..19
                    HASH_LBRACE@11..13 "#{"
                    VARIABLE_REF@13..18
                      DOLLAR@13..14 "$"
                      IDENT@14..18 "name"
                    RBRACE@18..19 "}"
                  STRING_END@19..21 "!\""
                SEMICOLON@21..22 ";"
        "##]],
    );
}

#[test]
fn string_interpolation_multiple() {
    check(
        "$s: \"#{$a}-#{$b}\";",
        expect![[r##"
            SOURCE_FILE@0..18
              VARIABLE_DECL@0..18
                DOLLAR@0..1 "$"
                IDENT@1..2 "s"
                COLON@2..3 ":"
                INTERPOLATED_STRING@3..17
                  WHITESPACE@3..4 " "
                  STRING_START@4..5 "\""
                  INTERPOLATION@5..10
                    HASH_LBRACE@5..7 "#{"
                    VARIABLE_REF@7..9
                      DOLLAR@7..8 "$"
                      IDENT@8..9 "a"
                    RBRACE@9..10 "}"
                  STRING_MID@10..11 "-"
                  INTERPOLATION@11..16
                    HASH_LBRACE@11..13 "#{"
                    VARIABLE_REF@13..15
                      DOLLAR@13..14 "$"
                      IDENT@14..15 "b"
                    RBRACE@15..16 "}"
                  STRING_END@16..17 "\""
                SEMICOLON@17..18 ";"
        "##]],
    );
}

// ── Special functions ──────────────────────────────────────────────

#[test]
fn url_unquoted() {
    check(
        "div { background: url(img.png); }",
        expect![[r#"
        SOURCE_FILE@0..33
          RULE_SET@0..33
            SELECTOR_LIST@0..3
              SELECTOR@0..3
                SIMPLE_SELECTOR@0..3
                  IDENT@0..3 "div"
            BLOCK@3..33
              WHITESPACE@3..4 " "
              LBRACE@4..5 "{"
              DECLARATION@5..31
                PROPERTY@5..16
                  WHITESPACE@5..6 " "
                  IDENT@6..16 "background"
                COLON@16..17 ":"
                VALUE@17..30
                  SPECIAL_FUNCTION_CALL@17..30
                    WHITESPACE@17..18 " "
                    IDENT@18..21 "url"
                    LPAREN@21..22 "("
                    URL_CONTENTS@22..29 "img.png"
                    RPAREN@29..30 ")"
                SEMICOLON@30..31 ";"
              WHITESPACE@31..32 " "
              RBRACE@32..33 "}"
    "#]],
    );
}

#[test]
fn url_quoted() {
    check(
        "div { background: url(\"img.png\"); }",
        expect![[r#"
        SOURCE_FILE@0..35
          RULE_SET@0..35
            SELECTOR_LIST@0..3
              SELECTOR@0..3
                SIMPLE_SELECTOR@0..3
                  IDENT@0..3 "div"
            BLOCK@3..35
              WHITESPACE@3..4 " "
              LBRACE@4..5 "{"
              DECLARATION@5..33
                PROPERTY@5..16
                  WHITESPACE@5..6 " "
                  IDENT@6..16 "background"
                COLON@16..17 ":"
                VALUE@17..32
                  SPECIAL_FUNCTION_CALL@17..32
                    WHITESPACE@17..18 " "
                    IDENT@18..21 "url"
                    LPAREN@21..22 "("
                    QUOTED_STRING@22..31 "\"img.png\""
                    RPAREN@31..32 ")"
                SEMICOLON@32..33 ";"
              WHITESPACE@33..34 " "
              RBRACE@34..35 "}"
    "#]],
    );
}

#[test]
fn url_with_interpolation() {
    check(
        "div { background: url(#{$path}/img.png); }",
        expect![[r##"
        SOURCE_FILE@0..42
          RULE_SET@0..42
            SELECTOR_LIST@0..3
              SELECTOR@0..3
                SIMPLE_SELECTOR@0..3
                  IDENT@0..3 "div"
            BLOCK@3..42
              WHITESPACE@3..4 " "
              LBRACE@4..5 "{"
              DECLARATION@5..40
                PROPERTY@5..16
                  WHITESPACE@5..6 " "
                  IDENT@6..16 "background"
                COLON@16..17 ":"
                VALUE@17..39
                  SPECIAL_FUNCTION_CALL@17..39
                    WHITESPACE@17..18 " "
                    IDENT@18..21 "url"
                    LPAREN@21..22 "("
                    INTERPOLATION@22..30
                      HASH_LBRACE@22..24 "#{"
                      VARIABLE_REF@24..29
                        DOLLAR@24..25 "$"
                        IDENT@25..29 "path"
                      RBRACE@29..30 "}"
                    URL_CONTENTS@30..38 "/img.png"
                    RPAREN@38..39 ")"
                SEMICOLON@39..40 ";"
              WHITESPACE@40..41 " "
              RBRACE@41..42 "}"
    "##]],
    );
}

// ── CSS property values (CssValue context) ─────────────────────────

#[test]
fn css_value_slash_separator() {
    check(
        "div { font: bold 14px/1.5 sans-serif; }",
        expect![[r#"
        SOURCE_FILE@0..39
          RULE_SET@0..39
            SELECTOR_LIST@0..3
              SELECTOR@0..3
                SIMPLE_SELECTOR@0..3
                  IDENT@0..3 "div"
            BLOCK@3..39
              WHITESPACE@3..4 " "
              LBRACE@4..5 "{"
              DECLARATION@5..37
                PROPERTY@5..10
                  WHITESPACE@5..6 " "
                  IDENT@6..10 "font"
                COLON@10..11 ":"
                VALUE@11..36
                  VALUE@11..16
                    WHITESPACE@11..12 " "
                    IDENT@12..16 "bold"
                  DIMENSION@16..21
                    WHITESPACE@16..17 " "
                    NUMBER@17..19 "14"
                    IDENT@19..21 "px"
                  SLASH@21..22 "/"
                  NUMBER_LITERAL@22..25
                    NUMBER@22..25 "1.5"
                  VALUE@25..36
                    WHITESPACE@25..26 " "
                    IDENT@26..36 "sans-serif"
                SEMICOLON@36..37 ";"
              WHITESPACE@37..38 " "
              RBRACE@38..39 "}"
    "#]],
    );
}

#[test]
fn css_value_space_separated() {
    check(
        "div { margin: 10px 20px; }",
        expect![[r#"
        SOURCE_FILE@0..26
          RULE_SET@0..26
            SELECTOR_LIST@0..3
              SELECTOR@0..3
                SIMPLE_SELECTOR@0..3
                  IDENT@0..3 "div"
            BLOCK@3..26
              WHITESPACE@3..4 " "
              LBRACE@4..5 "{"
              DECLARATION@5..24
                PROPERTY@5..12
                  WHITESPACE@5..6 " "
                  IDENT@6..12 "margin"
                COLON@12..13 ":"
                VALUE@13..23
                  DIMENSION@13..18
                    WHITESPACE@13..14 " "
                    NUMBER@14..16 "10"
                    IDENT@16..18 "px"
                  DIMENSION@18..23
                    WHITESPACE@18..19 " "
                    NUMBER@19..21 "20"
                    IDENT@21..23 "px"
                SEMICOLON@23..24 ";"
              WHITESPACE@24..25 " "
              RBRACE@25..26 "}"
    "#]],
    );
}

#[test]
fn css_value_four_values() {
    check(
        "div { margin: 10px 20px 30px 40px; }",
        expect![[r#"
        SOURCE_FILE@0..36
          RULE_SET@0..36
            SELECTOR_LIST@0..3
              SELECTOR@0..3
                SIMPLE_SELECTOR@0..3
                  IDENT@0..3 "div"
            BLOCK@3..36
              WHITESPACE@3..4 " "
              LBRACE@4..5 "{"
              DECLARATION@5..34
                PROPERTY@5..12
                  WHITESPACE@5..6 " "
                  IDENT@6..12 "margin"
                COLON@12..13 ":"
                VALUE@13..33
                  DIMENSION@13..18
                    WHITESPACE@13..14 " "
                    NUMBER@14..16 "10"
                    IDENT@16..18 "px"
                  DIMENSION@18..23
                    WHITESPACE@18..19 " "
                    NUMBER@19..21 "20"
                    IDENT@21..23 "px"
                  DIMENSION@23..28
                    WHITESPACE@23..24 " "
                    NUMBER@24..26 "30"
                    IDENT@26..28 "px"
                  DIMENSION@28..33
                    WHITESPACE@28..29 " "
                    NUMBER@29..31 "40"
                    IDENT@31..33 "px"
                SEMICOLON@33..34 ";"
              WHITESPACE@34..35 " "
              RBRACE@35..36 "}"
    "#]],
    );
}

#[test]
fn css_value_comma_separated() {
    check(
        "div { font-family: Arial, sans-serif; }",
        expect![[r#"
        SOURCE_FILE@0..39
          RULE_SET@0..39
            SELECTOR_LIST@0..3
              SELECTOR@0..3
                SIMPLE_SELECTOR@0..3
                  IDENT@0..3 "div"
            BLOCK@3..39
              WHITESPACE@3..4 " "
              LBRACE@4..5 "{"
              DECLARATION@5..37
                PROPERTY@5..17
                  WHITESPACE@5..6 " "
                  IDENT@6..17 "font-family"
                COLON@17..18 ":"
                VALUE@18..36
                  VALUE@18..24
                    WHITESPACE@18..19 " "
                    IDENT@19..24 "Arial"
                  COMMA@24..25 ","
                  VALUE@25..36
                    WHITESPACE@25..26 " "
                    IDENT@26..36 "sans-serif"
                SEMICOLON@36..37 ";"
              WHITESPACE@37..38 " "
              RBRACE@38..39 "}"
    "#]],
    );
}

#[test]
fn css_important() {
    check(
        "div { color: red !important; }",
        expect![[r#"
        SOURCE_FILE@0..30
          RULE_SET@0..30
            SELECTOR_LIST@0..3
              SELECTOR@0..3
                SIMPLE_SELECTOR@0..3
                  IDENT@0..3 "div"
            BLOCK@3..30
              WHITESPACE@3..4 " "
              LBRACE@4..5 "{"
              DECLARATION@5..28
                PROPERTY@5..11
                  WHITESPACE@5..6 " "
                  IDENT@6..11 "color"
                COLON@11..12 ":"
                VALUE@12..16
                  VALUE@12..16
                    WHITESPACE@12..13 " "
                    IDENT@13..16 "red"
                IMPORTANT@16..27
                  WHITESPACE@16..17 " "
                  BANG@17..18 "!"
                  IDENT@18..27 "important"
                SEMICOLON@27..28 ";"
              WHITESPACE@28..29 " "
              RBRACE@29..30 "}"
    "#]],
    );
}

// ── Standalone percent ─────────────────────────────────────────────

#[test]
fn standalone_percent() {
    check(
        "$a: %;",
        expect![[r#"
        SOURCE_FILE@0..6
          VARIABLE_DECL@0..6
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            STANDALONE_PERCENT@3..5
              WHITESPACE@3..4 " "
              PERCENT@4..5 "%"
            SEMICOLON@5..6 ";"
    "#]],
    );
}

// ── Error cases ────────────────────────────────────────────────────

#[test]
fn error_variable_missing_value() {
    check(
        "$x: ;",
        expect![[r#"
        SOURCE_FILE@0..5
          VARIABLE_DECL@0..5
            DOLLAR@0..1 "$"
            IDENT@1..2 "x"
            COLON@2..3 ":"
            WHITESPACE@3..4 " "
            SEMICOLON@4..5 ";"
        errors:
          4..5: expected expression
    "#]],
    );
}

#[test]
fn error_variable_missing_colon() {
    check(
        "$x red;",
        expect![[r#"
        SOURCE_FILE@0..7
          VARIABLE_DECL@0..7
            DOLLAR@0..1 "$"
            IDENT@1..2 "x"
            VALUE@2..6
              WHITESPACE@2..3 " "
              IDENT@3..6 "red"
            SEMICOLON@6..7 ";"
        errors:
          3..6: expected COLON
    "#]],
    );
}

// ── Integration tests ──────────────────────────────────────────────

#[test]
fn integration_var_in_function_in_decl() {
    check(
        ".box { .inner { color: darken($base, 20%); } }",
        expect![[r#"
        SOURCE_FILE@0..46
          RULE_SET@0..46
            SELECTOR_LIST@0..4
              SELECTOR@0..4
                SIMPLE_SELECTOR@0..4
                  DOT@0..1 "."
                  IDENT@1..4 "box"
            BLOCK@4..46
              WHITESPACE@4..5 " "
              LBRACE@5..6 "{"
              RULE_SET@6..44
                SELECTOR_LIST@6..13
                  SELECTOR@6..13
                    SIMPLE_SELECTOR@6..13
                      WHITESPACE@6..7 " "
                      DOT@7..8 "."
                      IDENT@8..13 "inner"
                BLOCK@13..44
                  WHITESPACE@13..14 " "
                  LBRACE@14..15 "{"
                  DECLARATION@15..42
                    PROPERTY@15..21
                      WHITESPACE@15..16 " "
                      IDENT@16..21 "color"
                    COLON@21..22 ":"
                    VALUE@22..41
                      FUNCTION_CALL@22..41
                        WHITESPACE@22..23 " "
                        IDENT@23..29 "darken"
                        ARG_LIST@29..41
                          LPAREN@29..30 "("
                          ARG@30..35
                            VARIABLE_REF@30..35
                              DOLLAR@30..31 "$"
                              IDENT@31..35 "base"
                          COMMA@35..36 ","
                          ARG@36..40
                            DIMENSION@36..40
                              WHITESPACE@36..37 " "
                              NUMBER@37..39 "20"
                              PERCENT@39..40 "%"
                          RPAREN@40..41 ")"
                    SEMICOLON@41..42 ";"
                  WHITESPACE@42..43 " "
                  RBRACE@43..44 "}"
              WHITESPACE@44..45 " "
              RBRACE@45..46 "}"
    "#]],
    );
}

#[test]
fn integration_expr_in_declaration() {
    check(
        "div { width: $base + 10px; }",
        expect![[r#"
        SOURCE_FILE@0..28
          RULE_SET@0..28
            SELECTOR_LIST@0..3
              SELECTOR@0..3
                SIMPLE_SELECTOR@0..3
                  IDENT@0..3 "div"
            BLOCK@3..28
              WHITESPACE@3..4 " "
              LBRACE@4..5 "{"
              DECLARATION@5..26
                PROPERTY@5..11
                  WHITESPACE@5..6 " "
                  IDENT@6..11 "width"
                COLON@11..12 ":"
                VALUE@12..25
                  BINARY_EXPR@12..25
                    VARIABLE_REF@12..18
                      WHITESPACE@12..13 " "
                      DOLLAR@13..14 "$"
                      IDENT@14..18 "base"
                    WHITESPACE@18..19 " "
                    PLUS@19..20 "+"
                    DIMENSION@20..25
                      WHITESPACE@20..21 " "
                      NUMBER@21..23 "10"
                      IDENT@23..25 "px"
                SEMICOLON@25..26 ";"
              WHITESPACE@26..27 " "
              RBRACE@27..28 "}"
    "#]],
    );
}

#[test]
fn integration_multiple_vars_and_decls() {
    check(
        "$primary: #333;\n$size: 16px;\ndiv { color: $primary; font-size: $size; }",
        expect![[r##"
            SOURCE_FILE@0..71
              VARIABLE_DECL@0..15
                DOLLAR@0..1 "$"
                IDENT@1..8 "primary"
                COLON@8..9 ":"
                COLOR_LITERAL@9..14
                  WHITESPACE@9..10 " "
                  HASH@10..11 "#"
                  NUMBER@11..14 "333"
                SEMICOLON@14..15 ";"
              VARIABLE_DECL@15..28
                WHITESPACE@15..16 "\n"
                DOLLAR@16..17 "$"
                IDENT@17..21 "size"
                COLON@21..22 ":"
                DIMENSION@22..27
                  WHITESPACE@22..23 " "
                  NUMBER@23..25 "16"
                  IDENT@25..27 "px"
                SEMICOLON@27..28 ";"
              RULE_SET@28..71
                SELECTOR_LIST@28..32
                  SELECTOR@28..32
                    SIMPLE_SELECTOR@28..32
                      WHITESPACE@28..29 "\n"
                      IDENT@29..32 "div"
                BLOCK@32..71
                  WHITESPACE@32..33 " "
                  LBRACE@33..34 "{"
                  DECLARATION@34..51
                    PROPERTY@34..40
                      WHITESPACE@34..35 " "
                      IDENT@35..40 "color"
                    COLON@40..41 ":"
                    VALUE@41..50
                      VARIABLE_REF@41..50
                        WHITESPACE@41..42 " "
                        DOLLAR@42..43 "$"
                        IDENT@43..50 "primary"
                    SEMICOLON@50..51 ";"
                  DECLARATION@51..69
                    PROPERTY@51..61
                      WHITESPACE@51..52 " "
                      IDENT@52..61 "font-size"
                    COLON@61..62 ":"
                    VALUE@62..68
                      VARIABLE_REF@62..68
                        WHITESPACE@62..63 " "
                        DOLLAR@63..64 "$"
                        IDENT@64..68 "size"
                    SEMICOLON@68..69 ";"
                  WHITESPACE@69..70 " "
                  RBRACE@70..71 "}"
        "##]],
    );
}

#[test]
fn integration_calc_in_declaration() {
    check(
        "div { width: calc(100% - 2 * $gap); }",
        expect![[r#"
        SOURCE_FILE@0..37
          RULE_SET@0..37
            SELECTOR_LIST@0..3
              SELECTOR@0..3
                SIMPLE_SELECTOR@0..3
                  IDENT@0..3 "div"
            BLOCK@3..37
              WHITESPACE@3..4 " "
              LBRACE@4..5 "{"
              DECLARATION@5..35
                PROPERTY@5..11
                  WHITESPACE@5..6 " "
                  IDENT@6..11 "width"
                COLON@11..12 ":"
                VALUE@12..34
                  CALCULATION@12..34
                    WHITESPACE@12..13 " "
                    IDENT@13..17 "calc"
                    LPAREN@17..18 "("
                    CALC_SUM@18..33
                      CALC_VALUE@18..22
                        DIMENSION@18..22
                          NUMBER@18..21 "100"
                          PERCENT@21..22 "%"
                      WHITESPACE@22..23 " "
                      MINUS@23..24 "-"
                      CALC_PRODUCT@24..33
                        CALC_VALUE@24..26
                          NUMBER_LITERAL@24..26
                            WHITESPACE@24..25 " "
                            NUMBER@25..26 "2"
                        WHITESPACE@26..27 " "
                        STAR@27..28 "*"
                        CALC_VALUE@28..33
                          VARIABLE_REF@28..33
                            WHITESPACE@28..29 " "
                            DOLLAR@29..30 "$"
                            IDENT@30..33 "gap"
                    RPAREN@33..34 ")"
                SEMICOLON@34..35 ";"
              WHITESPACE@35..36 " "
              RBRACE@36..37 "}"
    "#]],
    );
}

#[test]
fn integration_nested_function_calls() {
    check(
        "$a: darken(mix($c1, $c2, 50%), 10%);",
        expect![[r#"
        SOURCE_FILE@0..36
          VARIABLE_DECL@0..36
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            FUNCTION_CALL@3..35
              WHITESPACE@3..4 " "
              IDENT@4..10 "darken"
              ARG_LIST@10..35
                LPAREN@10..11 "("
                ARG@11..29
                  FUNCTION_CALL@11..29
                    IDENT@11..14 "mix"
                    ARG_LIST@14..29
                      LPAREN@14..15 "("
                      ARG@15..18
                        VARIABLE_REF@15..18
                          DOLLAR@15..16 "$"
                          IDENT@16..18 "c1"
                      COMMA@18..19 ","
                      ARG@19..23
                        VARIABLE_REF@19..23
                          WHITESPACE@19..20 " "
                          DOLLAR@20..21 "$"
                          IDENT@21..23 "c2"
                      COMMA@23..24 ","
                      ARG@24..28
                        DIMENSION@24..28
                          WHITESPACE@24..25 " "
                          NUMBER@25..27 "50"
                          PERCENT@27..28 "%"
                      RPAREN@28..29 ")"
                COMMA@29..30 ","
                ARG@30..34
                  DIMENSION@30..34
                    WHITESPACE@30..31 " "
                    NUMBER@31..33 "10"
                    PERCENT@33..34 "%"
                RPAREN@34..35 ")"
            SEMICOLON@35..36 ";"
    "#]],
    );
}

#[test]
fn integration_complex_expression() {
    check(
        "$result: ($a + $b) * 2 - ($c / 4);",
        expect![[r#"
        SOURCE_FILE@0..34
          VARIABLE_DECL@0..34
            DOLLAR@0..1 "$"
            IDENT@1..7 "result"
            COLON@7..8 ":"
            BINARY_EXPR@8..33
              BINARY_EXPR@8..22
                PAREN_EXPR@8..18
                  WHITESPACE@8..9 " "
                  LPAREN@9..10 "("
                  BINARY_EXPR@10..17
                    VARIABLE_REF@10..12
                      DOLLAR@10..11 "$"
                      IDENT@11..12 "a"
                    WHITESPACE@12..13 " "
                    PLUS@13..14 "+"
                    VARIABLE_REF@14..17
                      WHITESPACE@14..15 " "
                      DOLLAR@15..16 "$"
                      IDENT@16..17 "b"
                  RPAREN@17..18 ")"
                WHITESPACE@18..19 " "
                STAR@19..20 "*"
                NUMBER_LITERAL@20..22
                  WHITESPACE@20..21 " "
                  NUMBER@21..22 "2"
              WHITESPACE@22..23 " "
              MINUS@23..24 "-"
              PAREN_EXPR@24..33
                WHITESPACE@24..25 " "
                LPAREN@25..26 "("
                BINARY_EXPR@26..32
                  VARIABLE_REF@26..28
                    DOLLAR@26..27 "$"
                    IDENT@27..28 "c"
                  WHITESPACE@28..29 " "
                  SLASH@29..30 "/"
                  NUMBER_LITERAL@30..32
                    WHITESPACE@30..31 " "
                    NUMBER@31..32 "4"
                RPAREN@32..33 ")"
            SEMICOLON@33..34 ";"
    "#]],
    );
}

#[test]
fn integration_var_with_function_and_map() {
    check(
        "$theme: (primary: darken($blue, 10%), secondary: lighten($red, 5%));",
        expect![[r#"
        SOURCE_FILE@0..68
          VARIABLE_DECL@0..68
            DOLLAR@0..1 "$"
            IDENT@1..6 "theme"
            COLON@6..7 ":"
            MAP_EXPR@7..67
              WHITESPACE@7..8 " "
              LPAREN@8..9 "("
              MAP_ENTRY@9..36
                VALUE@9..16
                  IDENT@9..16 "primary"
                COLON@16..17 ":"
                FUNCTION_CALL@17..36
                  WHITESPACE@17..18 " "
                  IDENT@18..24 "darken"
                  ARG_LIST@24..36
                    LPAREN@24..25 "("
                    ARG@25..30
                      VARIABLE_REF@25..30
                        DOLLAR@25..26 "$"
                        IDENT@26..30 "blue"
                    COMMA@30..31 ","
                    ARG@31..35
                      DIMENSION@31..35
                        WHITESPACE@31..32 " "
                        NUMBER@32..34 "10"
                        PERCENT@34..35 "%"
                    RPAREN@35..36 ")"
              COMMA@36..37 ","
              MAP_ENTRY@37..66
                VALUE@37..47
                  WHITESPACE@37..38 " "
                  IDENT@38..47 "secondary"
                COLON@47..48 ":"
                FUNCTION_CALL@48..66
                  WHITESPACE@48..49 " "
                  IDENT@49..56 "lighten"
                  ARG_LIST@56..66
                    LPAREN@56..57 "("
                    ARG@57..61
                      VARIABLE_REF@57..61
                        DOLLAR@57..58 "$"
                        IDENT@58..61 "red"
                    COMMA@61..62 ","
                    ARG@62..65
                      DIMENSION@62..65
                        WHITESPACE@62..63 " "
                        NUMBER@63..64 "5"
                        PERCENT@64..65 "%"
                    RPAREN@65..66 ")"
              RPAREN@66..67 ")"
            SEMICOLON@67..68 ";"
    "#]],
    );
}

#[test]
fn integration_url_with_interpolation() {
    check(
        "div { background: url(#{$path}/img.png); }",
        expect![[r##"
        SOURCE_FILE@0..42
          RULE_SET@0..42
            SELECTOR_LIST@0..3
              SELECTOR@0..3
                SIMPLE_SELECTOR@0..3
                  IDENT@0..3 "div"
            BLOCK@3..42
              WHITESPACE@3..4 " "
              LBRACE@4..5 "{"
              DECLARATION@5..40
                PROPERTY@5..16
                  WHITESPACE@5..6 " "
                  IDENT@6..16 "background"
                COLON@16..17 ":"
                VALUE@17..39
                  SPECIAL_FUNCTION_CALL@17..39
                    WHITESPACE@17..18 " "
                    IDENT@18..21 "url"
                    LPAREN@21..22 "("
                    INTERPOLATION@22..30
                      HASH_LBRACE@22..24 "#{"
                      VARIABLE_REF@24..29
                        DOLLAR@24..25 "$"
                        IDENT@25..29 "path"
                      RBRACE@29..30 "}"
                    URL_CONTENTS@30..38 "/img.png"
                    RPAREN@38..39 ")"
                SEMICOLON@39..40 ";"
              WHITESPACE@40..41 " "
              RBRACE@41..42 "}"
    "##]],
    );
}

#[test]
fn integration_property_interpolation_with_value() {
    check(
        "div { #{$prop}-color: $val; }",
        expect![[r##"
        SOURCE_FILE@0..29
          RULE_SET@0..29
            SELECTOR_LIST@0..3
              SELECTOR@0..3
                SIMPLE_SELECTOR@0..3
                  IDENT@0..3 "div"
            BLOCK@3..29
              WHITESPACE@3..4 " "
              LBRACE@4..5 "{"
              DECLARATION@5..27
                PROPERTY@5..20
                  INTERPOLATION@5..14
                    WHITESPACE@5..6 " "
                    HASH_LBRACE@6..8 "#{"
                    VARIABLE_REF@8..13
                      DOLLAR@8..9 "$"
                      IDENT@9..13 "prop"
                    RBRACE@13..14 "}"
                  IDENT@14..20 "-color"
                COLON@20..21 ":"
                VALUE@21..26
                  VARIABLE_REF@21..26
                    WHITESPACE@21..22 " "
                    DOLLAR@22..23 "$"
                    IDENT@23..26 "val"
                SEMICOLON@26..27 ";"
              WHITESPACE@27..28 " "
              RBRACE@28..29 "}"
    "##]],
    );
}

#[test]
fn integration_css_multi_value() {
    check(
        "div { border: 1px solid #333; }",
        expect![[r##"
            SOURCE_FILE@0..31
              RULE_SET@0..31
                SELECTOR_LIST@0..3
                  SELECTOR@0..3
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "div"
                BLOCK@3..31
                  WHITESPACE@3..4 " "
                  LBRACE@4..5 "{"
                  DECLARATION@5..29
                    PROPERTY@5..12
                      WHITESPACE@5..6 " "
                      IDENT@6..12 "border"
                    COLON@12..13 ":"
                    VALUE@13..28
                      DIMENSION@13..17
                        WHITESPACE@13..14 " "
                        NUMBER@14..15 "1"
                        IDENT@15..17 "px"
                      VALUE@17..23
                        WHITESPACE@17..18 " "
                        IDENT@18..23 "solid"
                      COLOR_LITERAL@23..28
                        WHITESPACE@23..24 " "
                        HASH@24..25 "#"
                        NUMBER@25..28 "333"
                    SEMICOLON@28..29 ";"
                  WHITESPACE@29..30 " "
                  RBRACE@30..31 "}"
        "##]],
    );
}

// ── Round-trip tests ───────────────────────────────────────────────

#[test]
fn round_trip_complex_expr() {
    let source = "$result: ($a + $b) * 2 - ($c / 4);";
    let (green, _) = sass_parser::parse(source);
    let tree = SyntaxNode::new_root(green);
    assert_eq!(tree.text().to_string(), source);
}

#[test]
fn round_trip_map_nested() {
    let source = "$m: (colors: (red: #f00, blue: #00f), sizes: (sm: 12px, lg: 24px));";
    let (green, _) = sass_parser::parse(source);
    let tree = SyntaxNode::new_root(green);
    assert_eq!(tree.text().to_string(), source);
}

#[test]
fn round_trip_calc_nested() {
    let source = "div { width: calc(100% - min(20px, 5vw)); }";
    let (green, _) = sass_parser::parse(source);
    let tree = SyntaxNode::new_root(green);
    assert_eq!(tree.text().to_string(), source);
}

#[test]
fn round_trip_string_interpolation() {
    let source = "$s: \"#{$a}-#{$b}\";";
    let (green, _) = sass_parser::parse(source);
    let tree = SyntaxNode::new_root(green);
    assert_eq!(tree.text().to_string(), source);
}

#[test]
fn round_trip_variable_flags() {
    let source = "$x: 1 !default !global;";
    let (green, _) = sass_parser::parse(source);
    let tree = SyntaxNode::new_root(green);
    assert_eq!(tree.text().to_string(), source);
}

#[test]
fn round_trip_url_interpolation() {
    let source = "div { background: url(#{$path}/img.png); }";
    let (green, _) = sass_parser::parse(source);
    let tree = SyntaxNode::new_root(green);
    assert_eq!(tree.text().to_string(), source);
}

#[test]
fn round_trip_function_keyword_args() {
    let source = "$x: rgba($red: 255, $green: 128, $blue: 0);";
    let (green, _) = sass_parser::parse(source);
    let tree = SyntaxNode::new_root(green);
    assert_eq!(tree.text().to_string(), source);
}

#[test]
fn round_trip_bracketed_list() {
    let source = "$l: [1, 2, 3];";
    let (green, _) = sass_parser::parse(source);
    let tree = SyntaxNode::new_root(green);
    assert_eq!(tree.text().to_string(), source);
}

#[test]
fn round_trip_boolean_ops() {
    let source = "$a: not $x and $y or $z;";
    let (green, _) = sass_parser::parse(source);
    let tree = SyntaxNode::new_root(green);
    assert_eq!(tree.text().to_string(), source);
}

// ── Regression tests (code review fixes) ─────────────────────────

#[test]
fn calc_deeply_nested_parens() {
    // Regression: calc_value had no depth_guard → stack overflow on deep nesting
    let mut source = String::from("div { width: calc(");
    for _ in 0..260 {
        source.push('(');
    }
    source.push_str("1px");
    for _ in 0..260 {
        source.push(')');
    }
    source.push_str("); }");
    let (green, errors) = sass_parser::parse(&source);
    let tree = SyntaxNode::new_root(green);
    // Should not stack-overflow — depth limit triggers an error instead
    assert!(
        !errors.is_empty(),
        "deeply nested calc should produce nesting error"
    );
    assert_eq!(tree.text().to_string(), source, "lossless round-trip");
}

#[test]
fn css_value_not_is_plain_ident() {
    // Regression: `not` was always parsed as unary prefix, even in CssValue context
    check(
        "div { content: not; }",
        expect![[r#"
            SOURCE_FILE@0..21
              RULE_SET@0..21
                SELECTOR_LIST@0..3
                  SELECTOR@0..3
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "div"
                BLOCK@3..21
                  WHITESPACE@3..4 " "
                  LBRACE@4..5 "{"
                  DECLARATION@5..19
                    PROPERTY@5..13
                      WHITESPACE@5..6 " "
                      IDENT@6..13 "content"
                    COLON@13..14 ":"
                    VALUE@14..18
                      VALUE@14..18
                        WHITESPACE@14..15 " "
                        IDENT@15..18 "not"
                    SEMICOLON@18..19 ";"
                  WHITESPACE@19..20 " "
                  RBRACE@20..21 "}"
        "#]],
    );
}

#[test]
fn css_value_and_or_are_plain_idents() {
    // Regression: `and`/`or` returned None from ident_or_call even in CssValue context
    check(
        "div { content: and; }",
        expect![[r#"
            SOURCE_FILE@0..21
              RULE_SET@0..21
                SELECTOR_LIST@0..3
                  SELECTOR@0..3
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "div"
                BLOCK@3..21
                  WHITESPACE@3..4 " "
                  LBRACE@4..5 "{"
                  DECLARATION@5..19
                    PROPERTY@5..13
                      WHITESPACE@5..6 " "
                      IDENT@6..13 "content"
                    COLON@13..14 ":"
                    VALUE@14..18
                      VALUE@14..18
                        WHITESPACE@14..15 " "
                        IDENT@15..18 "and"
                    SEMICOLON@18..19 ";"
                  WHITESPACE@19..20 " "
                  RBRACE@20..21 "}"
        "#]],
    );
}

// ── Namespace member access ───────────────────────────────────────

#[test]
fn namespace_variable_ref() {
    check(
        "$x: ns.$var;",
        expect![[r#"
            SOURCE_FILE@0..12
              VARIABLE_DECL@0..12
                DOLLAR@0..1 "$"
                IDENT@1..2 "x"
                COLON@2..3 ":"
                NAMESPACE_REF@3..11
                  WHITESPACE@3..4 " "
                  IDENT@4..6 "ns"
                  DOT@6..7 "."
                  DOLLAR@7..8 "$"
                  IDENT@8..11 "var"
                SEMICOLON@11..12 ";"
        "#]],
    );
}

#[test]
fn namespace_function_call() {
    check(
        "$x: math.floor(4.7);",
        expect![[r#"
            SOURCE_FILE@0..20
              VARIABLE_DECL@0..20
                DOLLAR@0..1 "$"
                IDENT@1..2 "x"
                COLON@2..3 ":"
                NAMESPACE_REF@3..19
                  WHITESPACE@3..4 " "
                  IDENT@4..8 "math"
                  DOT@8..9 "."
                  FUNCTION_CALL@9..19
                    IDENT@9..14 "floor"
                    ARG_LIST@14..19
                      LPAREN@14..15 "("
                      ARG@15..18
                        NUMBER_LITERAL@15..18
                          NUMBER@15..18 "4.7"
                      RPAREN@18..19 ")"
                SEMICOLON@19..20 ";"
        "#]],
    );
}

#[test]
fn namespace_function_with_args() {
    check(
        "$x: color.adjust($c, $lightness: 10%);",
        expect![[r#"
            SOURCE_FILE@0..38
              VARIABLE_DECL@0..38
                DOLLAR@0..1 "$"
                IDENT@1..2 "x"
                COLON@2..3 ":"
                NAMESPACE_REF@3..37
                  WHITESPACE@3..4 " "
                  IDENT@4..9 "color"
                  DOT@9..10 "."
                  FUNCTION_CALL@10..37
                    IDENT@10..16 "adjust"
                    ARG_LIST@16..37
                      LPAREN@16..17 "("
                      ARG@17..19
                        VARIABLE_REF@17..19
                          DOLLAR@17..18 "$"
                          IDENT@18..19 "c"
                      COMMA@19..20 ","
                      ARG@20..36
                        WHITESPACE@20..21 " "
                        DOLLAR@21..22 "$"
                        IDENT@22..31 "lightness"
                        COLON@31..32 ":"
                        DIMENSION@32..36
                          WHITESPACE@32..33 " "
                          NUMBER@33..35 "10"
                          PERCENT@35..36 "%"
                      RPAREN@36..37 ")"
                SEMICOLON@37..38 ";"
        "#]],
    );
}

#[test]
fn include_namespace_mixin() {
    check(
        ".foo { @include ns.mixin(1px); }",
        expect![[r#"
            SOURCE_FILE@0..32
              RULE_SET@0..32
                SELECTOR_LIST@0..4
                  SELECTOR@0..4
                    SIMPLE_SELECTOR@0..4
                      DOT@0..1 "."
                      IDENT@1..4 "foo"
                BLOCK@4..32
                  WHITESPACE@4..5 " "
                  LBRACE@5..6 "{"
                  INCLUDE_RULE@6..30
                    WHITESPACE@6..7 " "
                    AT@7..8 "@"
                    IDENT@8..15 "include"
                    NAMESPACE_REF@15..24
                      WHITESPACE@15..16 " "
                      IDENT@16..18 "ns"
                      DOT@18..19 "."
                      IDENT@19..24 "mixin"
                    ARG_LIST@24..29
                      LPAREN@24..25 "("
                      ARG@25..28
                        DIMENSION@25..28
                          NUMBER@25..26 "1"
                          IDENT@26..28 "px"
                      RPAREN@28..29 ")"
                    SEMICOLON@29..30 ";"
                  WHITESPACE@30..31 " "
                  RBRACE@31..32 "}"
        "#]],
    );
}

// ── min()/max() SassScript fallback ───────────────────────────────

#[test]
fn min_sass_fallback_modulo() {
    check(
        "$x: min($a, $b % 2);",
        expect![[r#"
            SOURCE_FILE@0..20
              VARIABLE_DECL@0..20
                DOLLAR@0..1 "$"
                IDENT@1..2 "x"
                COLON@2..3 ":"
                FUNCTION_CALL@3..19
                  WHITESPACE@3..4 " "
                  IDENT@4..7 "min"
                  ARG_LIST@7..19
                    LPAREN@7..8 "("
                    ARG@8..10
                      VARIABLE_REF@8..10
                        DOLLAR@8..9 "$"
                        IDENT@9..10 "a"
                    COMMA@10..11 ","
                    ARG@11..18
                      BINARY_EXPR@11..18
                        VARIABLE_REF@11..14
                          WHITESPACE@11..12 " "
                          DOLLAR@12..13 "$"
                          IDENT@13..14 "b"
                        WHITESPACE@14..15 " "
                        PERCENT@15..16 "%"
                        NUMBER_LITERAL@16..18
                          WHITESPACE@16..17 " "
                          NUMBER@17..18 "2"
                    RPAREN@18..19 ")"
                SEMICOLON@19..20 ";"
        "#]],
    );
}

#[test]
fn min_sass_fallback_comparison() {
    check(
        "$x: min($a > $b, $c);",
        expect![[r#"
            SOURCE_FILE@0..21
              VARIABLE_DECL@0..21
                DOLLAR@0..1 "$"
                IDENT@1..2 "x"
                COLON@2..3 ":"
                FUNCTION_CALL@3..20
                  WHITESPACE@3..4 " "
                  IDENT@4..7 "min"
                  ARG_LIST@7..20
                    LPAREN@7..8 "("
                    ARG@8..15
                      BINARY_EXPR@8..15
                        VARIABLE_REF@8..10
                          DOLLAR@8..9 "$"
                          IDENT@9..10 "a"
                        WHITESPACE@10..11 " "
                        GT@11..12 ">"
                        VARIABLE_REF@12..15
                          WHITESPACE@12..13 " "
                          DOLLAR@13..14 "$"
                          IDENT@14..15 "b"
                    COMMA@15..16 ","
                    ARG@16..19
                      VARIABLE_REF@16..19
                        WHITESPACE@16..17 " "
                        DOLLAR@17..18 "$"
                        IDENT@18..19 "c"
                    RPAREN@19..20 ")"
                SEMICOLON@20..21 ";"
        "#]],
    );
}

#[test]
fn max_css_calculation() {
    check(
        "div { width: max(100px, 50%); }",
        expect![[r#"
            SOURCE_FILE@0..31
              RULE_SET@0..31
                SELECTOR_LIST@0..3
                  SELECTOR@0..3
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "div"
                BLOCK@3..31
                  WHITESPACE@3..4 " "
                  LBRACE@4..5 "{"
                  DECLARATION@5..29
                    PROPERTY@5..11
                      WHITESPACE@5..6 " "
                      IDENT@6..11 "width"
                    COLON@11..12 ":"
                    VALUE@12..28
                      CALCULATION@12..28
                        WHITESPACE@12..13 " "
                        IDENT@13..16 "max"
                        LPAREN@16..17 "("
                        CALC_VALUE@17..22
                          DIMENSION@17..22
                            NUMBER@17..20 "100"
                            IDENT@20..22 "px"
                        COMMA@22..23 ","
                        CALC_VALUE@23..27
                          DIMENSION@23..27
                            WHITESPACE@23..24 " "
                            NUMBER@24..26 "50"
                            PERCENT@26..27 "%"
                        RPAREN@27..28 ")"
                    SEMICOLON@28..29 ";"
                  WHITESPACE@29..30 " "
                  RBRACE@30..31 "}"
        "#]],
    );
}

// ── Stress: semantic AST accuracy ──────────────────────────────────────

#[test]
fn deep_nested_string_interpolation() {
    check(
        r#"$s: "a #{"b #{$c} d"} e";"#,
        expect![[r##"
            SOURCE_FILE@0..25
              VARIABLE_DECL@0..25
                DOLLAR@0..1 "$"
                IDENT@1..2 "s"
                COLON@2..3 ":"
                INTERPOLATED_STRING@3..24
                  WHITESPACE@3..4 " "
                  STRING_START@4..7 "\"a "
                  INTERPOLATION@7..21
                    HASH_LBRACE@7..9 "#{"
                    INTERPOLATED_STRING@9..20
                      STRING_START@9..12 "\"b "
                      INTERPOLATION@12..17
                        HASH_LBRACE@12..14 "#{"
                        VARIABLE_REF@14..16
                          DOLLAR@14..15 "$"
                          IDENT@15..16 "c"
                        RBRACE@16..17 "}"
                      STRING_END@17..20 " d\""
                    RBRACE@20..21 "}"
                  STRING_END@21..24 " e\""
                SEMICOLON@24..25 ";"
        "##]],
    );
}

#[test]
fn calc_nested_min_subtraction() {
    check(
        "div { width: calc(min(100%, 50vw) - 2rem); }",
        expect![[r#"
            SOURCE_FILE@0..44
              RULE_SET@0..44
                SELECTOR_LIST@0..3
                  SELECTOR@0..3
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "div"
                BLOCK@3..44
                  WHITESPACE@3..4 " "
                  LBRACE@4..5 "{"
                  DECLARATION@5..42
                    PROPERTY@5..11
                      WHITESPACE@5..6 " "
                      IDENT@6..11 "width"
                    COLON@11..12 ":"
                    VALUE@12..41
                      CALCULATION@12..41
                        WHITESPACE@12..13 " "
                        IDENT@13..17 "calc"
                        LPAREN@17..18 "("
                        CALC_SUM@18..40
                          CALCULATION@18..33
                            IDENT@18..21 "min"
                            LPAREN@21..22 "("
                            CALC_VALUE@22..26
                              DIMENSION@22..26
                                NUMBER@22..25 "100"
                                PERCENT@25..26 "%"
                            COMMA@26..27 ","
                            CALC_VALUE@27..32
                              DIMENSION@27..32
                                WHITESPACE@27..28 " "
                                NUMBER@28..30 "50"
                                IDENT@30..32 "vw"
                            RPAREN@32..33 ")"
                          WHITESPACE@33..34 " "
                          MINUS@34..35 "-"
                          CALC_VALUE@35..40
                            DIMENSION@35..40
                              WHITESPACE@35..36 " "
                              NUMBER@36..37 "2"
                              IDENT@37..40 "rem"
                        RPAREN@40..41 ")"
                    SEMICOLON@41..42 ";"
                  WHITESPACE@42..43 " "
                  RBRACE@43..44 "}"
        "#]],
    );
}

#[test]
fn max_with_calc_inside() {
    check(
        "div { width: max(calc(100% - 40px), 300px); }",
        expect![[r#"
            SOURCE_FILE@0..45
              RULE_SET@0..45
                SELECTOR_LIST@0..3
                  SELECTOR@0..3
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "div"
                BLOCK@3..45
                  WHITESPACE@3..4 " "
                  LBRACE@4..5 "{"
                  DECLARATION@5..43
                    PROPERTY@5..11
                      WHITESPACE@5..6 " "
                      IDENT@6..11 "width"
                    COLON@11..12 ":"
                    VALUE@12..42
                      CALCULATION@12..42
                        WHITESPACE@12..13 " "
                        IDENT@13..16 "max"
                        LPAREN@16..17 "("
                        CALCULATION@17..34
                          IDENT@17..21 "calc"
                          LPAREN@21..22 "("
                          CALC_SUM@22..33
                            CALC_VALUE@22..26
                              DIMENSION@22..26
                                NUMBER@22..25 "100"
                                PERCENT@25..26 "%"
                            WHITESPACE@26..27 " "
                            MINUS@27..28 "-"
                            CALC_VALUE@28..33
                              DIMENSION@28..33
                                WHITESPACE@28..29 " "
                                NUMBER@29..31 "40"
                                IDENT@31..33 "px"
                          RPAREN@33..34 ")"
                        COMMA@34..35 ","
                        CALC_VALUE@35..41
                          DIMENSION@35..41
                            WHITESPACE@35..36 " "
                            NUMBER@36..39 "300"
                            IDENT@39..41 "px"
                        RPAREN@41..42 ")"
                    SEMICOLON@42..43 ";"
                  WHITESPACE@43..44 " "
                  RBRACE@44..45 "}"
        "#]],
    );
}

#[test]
fn interpolation_in_selector_property_value() {
    check(
        "#{$sel} { #{$prop}: #{$val}; }",
        expect![[r##"
            SOURCE_FILE@0..30
              RULE_SET@0..30
                SELECTOR_LIST@0..7
                  SELECTOR@0..7
                    INTERPOLATION@0..7
                      HASH_LBRACE@0..2 "#{"
                      VARIABLE_REF@2..6
                        DOLLAR@2..3 "$"
                        IDENT@3..6 "sel"
                      RBRACE@6..7 "}"
                BLOCK@7..30
                  WHITESPACE@7..8 " "
                  LBRACE@8..9 "{"
                  DECLARATION@9..28
                    PROPERTY@9..18
                      INTERPOLATION@9..18
                        WHITESPACE@9..10 " "
                        HASH_LBRACE@10..12 "#{"
                        VARIABLE_REF@12..17
                          DOLLAR@12..13 "$"
                          IDENT@13..17 "prop"
                        RBRACE@17..18 "}"
                    COLON@18..19 ":"
                    VALUE@19..27
                      INTERPOLATION@19..27
                        WHITESPACE@19..20 " "
                        HASH_LBRACE@20..22 "#{"
                        VARIABLE_REF@22..26
                          DOLLAR@22..23 "$"
                          IDENT@23..26 "val"
                        RBRACE@26..27 "}"
                    SEMICOLON@27..28 ";"
                  WHITESPACE@28..29 " "
                  RBRACE@29..30 "}"
        "##]],
    );
}
