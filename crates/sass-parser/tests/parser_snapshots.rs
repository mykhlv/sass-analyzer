mod common;

use common::check;
use expect_test::expect;

// ── Simple rules ────────────────────────────────────────────────────────

#[test]
fn simple_rule() {
    check(
        "div { color: red; }",
        expect![[r#"
            SOURCE_FILE@0..19
              RULE_SET@0..19
                SELECTOR_LIST@0..3
                  SELECTOR@0..3
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "div"
                BLOCK@3..19
                  WHITESPACE@3..4 " "
                  LBRACE@4..5 "{"
                  DECLARATION@5..17
                    PROPERTY@5..11
                      WHITESPACE@5..6 " "
                      IDENT@6..11 "color"
                    COLON@11..12 ":"
                    VALUE@12..16
                      VALUE@12..16
                        WHITESPACE@12..13 " "
                        IDENT@13..16 "red"
                    SEMICOLON@16..17 ";"
                  WHITESPACE@17..18 " "
                  RBRACE@18..19 "}"
        "#]],
    );
}

#[test]
fn multiple_declarations() {
    check(
        "p { color: red; font-size: 14px; }",
        expect![[r#"
            SOURCE_FILE@0..34
              RULE_SET@0..34
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                BLOCK@1..34
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  DECLARATION@3..15
                    PROPERTY@3..9
                      WHITESPACE@3..4 " "
                      IDENT@4..9 "color"
                    COLON@9..10 ":"
                    VALUE@10..14
                      VALUE@10..14
                        WHITESPACE@10..11 " "
                        IDENT@11..14 "red"
                    SEMICOLON@14..15 ";"
                  DECLARATION@15..32
                    PROPERTY@15..25
                      WHITESPACE@15..16 " "
                      IDENT@16..25 "font-size"
                    COLON@25..26 ":"
                    VALUE@26..31
                      DIMENSION@26..31
                        WHITESPACE@26..27 " "
                        NUMBER@27..29 "14"
                        IDENT@29..31 "px"
                    SEMICOLON@31..32 ";"
                  WHITESPACE@32..33 " "
                  RBRACE@33..34 "}"
        "#]],
    );
}

// ── Nested rules ────────────────────────────────────────────────────────

#[test]
fn nested_rule() {
    check(
        "nav { ul { margin: 0; } }",
        expect![[r#"
            SOURCE_FILE@0..25
              RULE_SET@0..25
                SELECTOR_LIST@0..3
                  SELECTOR@0..3
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "nav"
                BLOCK@3..25
                  WHITESPACE@3..4 " "
                  LBRACE@4..5 "{"
                  RULE_SET@5..23
                    SELECTOR_LIST@5..8
                      SELECTOR@5..8
                        SIMPLE_SELECTOR@5..8
                          WHITESPACE@5..6 " "
                          IDENT@6..8 "ul"
                    BLOCK@8..23
                      WHITESPACE@8..9 " "
                      LBRACE@9..10 "{"
                      DECLARATION@10..21
                        PROPERTY@10..17
                          WHITESPACE@10..11 " "
                          IDENT@11..17 "margin"
                        COLON@17..18 ":"
                        VALUE@18..20
                          NUMBER_LITERAL@18..20
                            WHITESPACE@18..19 " "
                            NUMBER@19..20 "0"
                        SEMICOLON@20..21 ";"
                      WHITESPACE@21..22 " "
                      RBRACE@22..23 "}"
                  WHITESPACE@23..24 " "
                  RBRACE@24..25 "}"
        "#]],
    );
}

// ── Multiple selectors (selector list) ──────────────────────────────────

#[test]
fn selector_list() {
    check(
        "h1, h2, h3 { margin: 0; }",
        expect![[r#"
            SOURCE_FILE@0..25
              RULE_SET@0..25
                SELECTOR_LIST@0..10
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      IDENT@0..2 "h1"
                  COMMA@2..3 ","
                  SELECTOR@3..6
                    SIMPLE_SELECTOR@3..6
                      WHITESPACE@3..4 " "
                      IDENT@4..6 "h2"
                  COMMA@6..7 ","
                  SELECTOR@7..10
                    SIMPLE_SELECTOR@7..10
                      WHITESPACE@7..8 " "
                      IDENT@8..10 "h3"
                BLOCK@10..25
                  WHITESPACE@10..11 " "
                  LBRACE@11..12 "{"
                  DECLARATION@12..23
                    PROPERTY@12..19
                      WHITESPACE@12..13 " "
                      IDENT@13..19 "margin"
                    COLON@19..20 ":"
                    VALUE@20..22
                      NUMBER_LITERAL@20..22
                        WHITESPACE@20..21 " "
                        NUMBER@21..22 "0"
                    SEMICOLON@22..23 ";"
                  WHITESPACE@23..24 " "
                  RBRACE@24..25 "}"
        "#]],
    );
}

// ── Compound selectors ──────────────────────────────────────────────────

#[test]
fn compound_selector_class_id() {
    check(
        "div.active#main { display: block; }",
        expect![[r##"
            SOURCE_FILE@0..35
              RULE_SET@0..35
                SELECTOR_LIST@0..15
                  SELECTOR@0..15
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "div"
                    SIMPLE_SELECTOR@3..10
                      DOT@3..4 "."
                      IDENT@4..10 "active"
                    SIMPLE_SELECTOR@10..15
                      HASH@10..11 "#"
                      IDENT@11..15 "main"
                BLOCK@15..35
                  WHITESPACE@15..16 " "
                  LBRACE@16..17 "{"
                  DECLARATION@17..33
                    PROPERTY@17..25
                      WHITESPACE@17..18 " "
                      IDENT@18..25 "display"
                    COLON@25..26 ":"
                    VALUE@26..32
                      VALUE@26..32
                        WHITESPACE@26..27 " "
                        IDENT@27..32 "block"
                    SEMICOLON@32..33 ";"
                  WHITESPACE@33..34 " "
                  RBRACE@34..35 "}"
        "##]],
    );
}

#[test]
fn compound_selector_full() {
    check(
        "div.class#id[attr]:hover::before { }",
        expect![[r##"
            SOURCE_FILE@0..36
              RULE_SET@0..36
                SELECTOR_LIST@0..32
                  SELECTOR@0..32
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "div"
                    SIMPLE_SELECTOR@3..9
                      DOT@3..4 "."
                      IDENT@4..9 "class"
                    SIMPLE_SELECTOR@9..12
                      HASH@9..10 "#"
                      IDENT@10..12 "id"
                    ATTR_SELECTOR@12..18
                      LBRACKET@12..13 "["
                      IDENT@13..17 "attr"
                      RBRACKET@17..18 "]"
                    PSEUDO_SELECTOR@18..24
                      COLON@18..19 ":"
                      IDENT@19..24 "hover"
                    PSEUDO_SELECTOR@24..32
                      COLON_COLON@24..26 "::"
                      IDENT@26..32 "before"
                BLOCK@32..36
                  WHITESPACE@32..33 " "
                  LBRACE@33..34 "{"
                  WHITESPACE@34..35 " "
                  RBRACE@35..36 "}"
        "##]],
    );
}

// ── Combinators ─────────────────────────────────────────────────────────

#[test]
fn child_combinator() {
    check(
        "ul > li { }",
        expect![[r#"
            SOURCE_FILE@0..11
              RULE_SET@0..11
                SELECTOR_LIST@0..7
                  SELECTOR@0..7
                    SIMPLE_SELECTOR@0..2
                      IDENT@0..2 "ul"
                    COMBINATOR@2..4
                      WHITESPACE@2..3 " "
                      GT@3..4 ">"
                    SIMPLE_SELECTOR@4..7
                      WHITESPACE@4..5 " "
                      IDENT@5..7 "li"
                BLOCK@7..11
                  WHITESPACE@7..8 " "
                  LBRACE@8..9 "{"
                  WHITESPACE@9..10 " "
                  RBRACE@10..11 "}"
        "#]],
    );
}

#[test]
fn adjacent_sibling_combinator() {
    check(
        "h1 + p { }",
        expect![[r#"
            SOURCE_FILE@0..10
              RULE_SET@0..10
                SELECTOR_LIST@0..6
                  SELECTOR@0..6
                    SIMPLE_SELECTOR@0..2
                      IDENT@0..2 "h1"
                    COMBINATOR@2..4
                      WHITESPACE@2..3 " "
                      PLUS@3..4 "+"
                    SIMPLE_SELECTOR@4..6
                      WHITESPACE@4..5 " "
                      IDENT@5..6 "p"
                BLOCK@6..10
                  WHITESPACE@6..7 " "
                  LBRACE@7..8 "{"
                  WHITESPACE@8..9 " "
                  RBRACE@9..10 "}"
        "#]],
    );
}

#[test]
fn general_sibling_combinator() {
    check(
        "h1 ~ p { }",
        expect![[r#"
            SOURCE_FILE@0..10
              RULE_SET@0..10
                SELECTOR_LIST@0..6
                  SELECTOR@0..6
                    SIMPLE_SELECTOR@0..2
                      IDENT@0..2 "h1"
                    COMBINATOR@2..4
                      WHITESPACE@2..3 " "
                      TILDE@3..4 "~"
                    SIMPLE_SELECTOR@4..6
                      WHITESPACE@4..5 " "
                      IDENT@5..6 "p"
                BLOCK@6..10
                  WHITESPACE@6..7 " "
                  LBRACE@7..8 "{"
                  WHITESPACE@8..9 " "
                  RBRACE@9..10 "}"
        "#]],
    );
}

#[test]
fn descendant_combinator() {
    check(
        "nav li { }",
        expect![[r#"
            SOURCE_FILE@0..10
              RULE_SET@0..10
                SELECTOR_LIST@0..6
                  SELECTOR@0..6
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "nav"
                    SIMPLE_SELECTOR@3..6
                      WHITESPACE@3..4 " "
                      IDENT@4..6 "li"
                BLOCK@6..10
                  WHITESPACE@6..7 " "
                  LBRACE@7..8 "{"
                  WHITESPACE@8..9 " "
                  RBRACE@9..10 "}"
        "#]],
    );
}

// ── Parent selector & ───────────────────────────────────────────────────

#[test]
fn parent_selector() {
    check(
        ".btn { &:hover { color: blue; } }",
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
                  RULE_SET@6..31
                    SELECTOR_LIST@6..14
                      SELECTOR@6..14
                        SIMPLE_SELECTOR@6..8
                          WHITESPACE@6..7 " "
                          AMP@7..8 "&"
                        PSEUDO_SELECTOR@8..14
                          COLON@8..9 ":"
                          IDENT@9..14 "hover"
                    BLOCK@14..31
                      WHITESPACE@14..15 " "
                      LBRACE@15..16 "{"
                      DECLARATION@16..29
                        PROPERTY@16..22
                          WHITESPACE@16..17 " "
                          IDENT@17..22 "color"
                        COLON@22..23 ":"
                        VALUE@23..28
                          VALUE@23..28
                            WHITESPACE@23..24 " "
                            IDENT@24..28 "blue"
                        SEMICOLON@28..29 ";"
                      WHITESPACE@29..30 " "
                      RBRACE@30..31 "}"
                  WHITESPACE@31..32 " "
                  RBRACE@32..33 "}"
        "#]],
    );
}

#[test]
fn parent_selector_suffix() {
    check(
        ".btn { &-primary { } }",
        expect![[r#"
            SOURCE_FILE@0..22
              RULE_SET@0..22
                SELECTOR_LIST@0..4
                  SELECTOR@0..4
                    SIMPLE_SELECTOR@0..4
                      DOT@0..1 "."
                      IDENT@1..4 "btn"
                BLOCK@4..22
                  WHITESPACE@4..5 " "
                  LBRACE@5..6 "{"
                  RULE_SET@6..20
                    SELECTOR_LIST@6..16
                      SELECTOR@6..16
                        SIMPLE_SELECTOR@6..16
                          WHITESPACE@6..7 " "
                          AMP@7..8 "&"
                          IDENT@8..16 "-primary"
                    BLOCK@16..20
                      WHITESPACE@16..17 " "
                      LBRACE@17..18 "{"
                      WHITESPACE@18..19 " "
                      RBRACE@19..20 "}"
                  WHITESPACE@20..21 " "
                  RBRACE@21..22 "}"
        "#]],
    );
}

// ── Placeholder selector ────────────────────────────────────────────────

#[test]
fn placeholder_selector() {
    check(
        "%placeholder { color: red; }",
        expect![[r#"
            SOURCE_FILE@0..28
              RULE_SET@0..28
                SELECTOR_LIST@0..12
                  SELECTOR@0..12
                    SIMPLE_SELECTOR@0..12
                      PERCENT@0..1 "%"
                      IDENT@1..12 "placeholder"
                BLOCK@12..28
                  WHITESPACE@12..13 " "
                  LBRACE@13..14 "{"
                  DECLARATION@14..26
                    PROPERTY@14..20
                      WHITESPACE@14..15 " "
                      IDENT@15..20 "color"
                    COLON@20..21 ":"
                    VALUE@21..25
                      VALUE@21..25
                        WHITESPACE@21..22 " "
                        IDENT@22..25 "red"
                    SEMICOLON@25..26 ";"
                  WHITESPACE@26..27 " "
                  RBRACE@27..28 "}"
        "#]],
    );
}

// ── Universal selector ──────────────────────────────────────────────────

#[test]
fn universal_selector() {
    check(
        "* { margin: 0; }",
        expect![[r#"
            SOURCE_FILE@0..16
              RULE_SET@0..16
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      STAR@0..1 "*"
                BLOCK@1..16
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  DECLARATION@3..14
                    PROPERTY@3..10
                      WHITESPACE@3..4 " "
                      IDENT@4..10 "margin"
                    COLON@10..11 ":"
                    VALUE@11..13
                      NUMBER_LITERAL@11..13
                        WHITESPACE@11..12 " "
                        NUMBER@12..13 "0"
                    SEMICOLON@13..14 ";"
                  WHITESPACE@14..15 " "
                  RBRACE@15..16 "}"
        "#]],
    );
}

// ── !important ──────────────────────────────────────────────────────────

#[test]
fn important_value() {
    check(
        "p { color: red !important; }",
        expect![[r#"
            SOURCE_FILE@0..28
              RULE_SET@0..28
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                BLOCK@1..28
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  DECLARATION@3..26
                    PROPERTY@3..9
                      WHITESPACE@3..4 " "
                      IDENT@4..9 "color"
                    COLON@9..10 ":"
                    VALUE@10..14
                      VALUE@10..14
                        WHITESPACE@10..11 " "
                        IDENT@11..14 "red"
                    IMPORTANT@14..25
                      WHITESPACE@14..15 " "
                      BANG@15..16 "!"
                      IDENT@16..25 "important"
                    SEMICOLON@25..26 ";"
                  WHITESPACE@26..27 " "
                  RBRACE@27..28 "}"
        "#]],
    );
}

// ── Comments ────────────────────────────────────────────────────────────

#[test]
fn comments_in_rule() {
    check(
        "/* heading */ h1 { /* color */ color: red; }",
        expect![[r#"
            SOURCE_FILE@0..44
              RULE_SET@0..44
                SELECTOR_LIST@0..16
                  SELECTOR@0..16
                    SIMPLE_SELECTOR@0..16
                      MULTI_LINE_COMMENT@0..13 "/* heading */"
                      WHITESPACE@13..14 " "
                      IDENT@14..16 "h1"
                BLOCK@16..44
                  WHITESPACE@16..17 " "
                  LBRACE@17..18 "{"
                  DECLARATION@18..42
                    PROPERTY@18..36
                      WHITESPACE@18..19 " "
                      MULTI_LINE_COMMENT@19..30 "/* color */"
                      WHITESPACE@30..31 " "
                      IDENT@31..36 "color"
                    COLON@36..37 ":"
                    VALUE@37..41
                      VALUE@37..41
                        WHITESPACE@37..38 " "
                        IDENT@38..41 "red"
                    SEMICOLON@41..42 ";"
                  WHITESPACE@42..43 " "
                  RBRACE@43..44 "}"
        "#]],
    );
}

#[test]
fn single_line_comment_in_rule() {
    check(
        "a {\n  // link color\n  color: blue;\n}",
        expect![[r#"
            SOURCE_FILE@0..36
              RULE_SET@0..36
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "a"
                BLOCK@1..36
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  DECLARATION@3..34
                    PROPERTY@3..27
                      WHITESPACE@3..6 "\n  "
                      SINGLE_LINE_COMMENT@6..19 "// link color"
                      WHITESPACE@19..22 "\n  "
                      IDENT@22..27 "color"
                    COLON@27..28 ":"
                    VALUE@28..33
                      VALUE@28..33
                        WHITESPACE@28..29 " "
                        IDENT@29..33 "blue"
                    SEMICOLON@33..34 ";"
                  WHITESPACE@34..35 "\n"
                  RBRACE@35..36 "}"
        "#]],
    );
}

// ── Custom properties ───────────────────────────────────────────────────

#[test]
fn custom_property() {
    check(
        ":root { --color: red; }",
        expect![[r#"
            SOURCE_FILE@0..23
              RULE_SET@0..23
                SELECTOR_LIST@0..5
                  SELECTOR@0..5
                    PSEUDO_SELECTOR@0..5
                      COLON@0..1 ":"
                      IDENT@1..5 "root"
                BLOCK@5..23
                  WHITESPACE@5..6 " "
                  LBRACE@6..7 "{"
                  CUSTOM_PROPERTY_DECL@7..21
                    PROPERTY@7..15
                      WHITESPACE@7..8 " "
                      IDENT@8..15 "--color"
                    COLON@15..16 ":"
                    VALUE@16..20
                      WHITESPACE@16..17 " "
                      IDENT@17..20 "red"
                    SEMICOLON@20..21 ";"
                  WHITESPACE@21..22 " "
                  RBRACE@22..23 "}"
        "#]],
    );
}

#[test]
fn custom_property_complex_value() {
    check(
        ":root { --grad: linear-gradient(to right, red, blue); }",
        expect![[r#"
            SOURCE_FILE@0..55
              RULE_SET@0..55
                SELECTOR_LIST@0..5
                  SELECTOR@0..5
                    PSEUDO_SELECTOR@0..5
                      COLON@0..1 ":"
                      IDENT@1..5 "root"
                BLOCK@5..55
                  WHITESPACE@5..6 " "
                  LBRACE@6..7 "{"
                  CUSTOM_PROPERTY_DECL@7..53
                    PROPERTY@7..14
                      WHITESPACE@7..8 " "
                      IDENT@8..14 "--grad"
                    COLON@14..15 ":"
                    VALUE@15..52
                      WHITESPACE@15..16 " "
                      IDENT@16..31 "linear-gradient"
                      LPAREN@31..32 "("
                      IDENT@32..34 "to"
                      WHITESPACE@34..35 " "
                      IDENT@35..40 "right"
                      COMMA@40..41 ","
                      WHITESPACE@41..42 " "
                      IDENT@42..45 "red"
                      COMMA@45..46 ","
                      WHITESPACE@46..47 " "
                      IDENT@47..51 "blue"
                      RPAREN@51..52 ")"
                    SEMICOLON@52..53 ";"
                  WHITESPACE@53..54 " "
                  RBRACE@54..55 "}"
        "#]],
    );
}

// ── Pseudo selectors ────────────────────────────────────────────────────

#[test]
fn pseudo_class_hover() {
    check(
        "a:hover { }",
        expect![[r##"
            SOURCE_FILE@0..11
              RULE_SET@0..11
                SELECTOR_LIST@0..7
                  SELECTOR@0..7
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "a"
                    PSEUDO_SELECTOR@1..7
                      COLON@1..2 ":"
                      IDENT@2..7 "hover"
                BLOCK@7..11
                  WHITESPACE@7..8 " "
                  LBRACE@8..9 "{"
                  WHITESPACE@9..10 " "
                  RBRACE@10..11 "}"
        "##]],
    );
}

#[test]
fn pseudo_element_before() {
    check(
        "p::before { }",
        expect![[r#"
            SOURCE_FILE@0..13
              RULE_SET@0..13
                SELECTOR_LIST@0..9
                  SELECTOR@0..9
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                    PSEUDO_SELECTOR@1..9
                      COLON_COLON@1..3 "::"
                      IDENT@3..9 "before"
                BLOCK@9..13
                  WHITESPACE@9..10 " "
                  LBRACE@10..11 "{"
                  WHITESPACE@11..12 " "
                  RBRACE@12..13 "}"
        "#]],
    );
}

#[test]
fn pseudo_vendor_prefixed() {
    check(
        "::-webkit-scrollbar { }",
        expect![[r#"
            SOURCE_FILE@0..23
              RULE_SET@0..23
                SELECTOR_LIST@0..19
                  SELECTOR@0..19
                    PSEUDO_SELECTOR@0..19
                      COLON_COLON@0..2 "::"
                      IDENT@2..19 "-webkit-scrollbar"
                BLOCK@19..23
                  WHITESPACE@19..20 " "
                  LBRACE@20..21 "{"
                  WHITESPACE@21..22 " "
                  RBRACE@22..23 "}"
        "#]],
    );
}

#[test]
fn pseudo_is() {
    check(
        ":is(h1, h2) { }",
        expect![[r#"
            SOURCE_FILE@0..15
              RULE_SET@0..15
                SELECTOR_LIST@0..11
                  SELECTOR@0..11
                    PSEUDO_SELECTOR@0..11
                      COLON@0..1 ":"
                      IDENT@1..3 "is"
                      LPAREN@3..4 "("
                      IDENT@4..6 "h1"
                      COMMA@6..7 ","
                      WHITESPACE@7..8 " "
                      IDENT@8..10 "h2"
                      RPAREN@10..11 ")"
                BLOCK@11..15
                  WHITESPACE@11..12 " "
                  LBRACE@12..13 "{"
                  WHITESPACE@13..14 " "
                  RBRACE@14..15 "}"
        "#]],
    );
}

#[test]
fn pseudo_where() {
    check(
        ":where(div, span) { }",
        expect![[r#"
            SOURCE_FILE@0..21
              RULE_SET@0..21
                SELECTOR_LIST@0..17
                  SELECTOR@0..17
                    PSEUDO_SELECTOR@0..17
                      COLON@0..1 ":"
                      IDENT@1..6 "where"
                      LPAREN@6..7 "("
                      IDENT@7..10 "div"
                      COMMA@10..11 ","
                      WHITESPACE@11..12 " "
                      IDENT@12..16 "span"
                      RPAREN@16..17 ")"
                BLOCK@17..21
                  WHITESPACE@17..18 " "
                  LBRACE@18..19 "{"
                  WHITESPACE@19..20 " "
                  RBRACE@20..21 "}"
        "#]],
    );
}

#[test]
fn pseudo_not() {
    check(
        ":not(.hidden) { }",
        expect![[r#"
            SOURCE_FILE@0..17
              RULE_SET@0..17
                SELECTOR_LIST@0..13
                  SELECTOR@0..13
                    PSEUDO_SELECTOR@0..13
                      COLON@0..1 ":"
                      IDENT@1..4 "not"
                      LPAREN@4..5 "("
                      DOT@5..6 "."
                      IDENT@6..12 "hidden"
                      RPAREN@12..13 ")"
                BLOCK@13..17
                  WHITESPACE@13..14 " "
                  LBRACE@14..15 "{"
                  WHITESPACE@15..16 " "
                  RBRACE@16..17 "}"
        "#]],
    );
}

// ── Attribute selectors ─────────────────────────────────────────────────

#[test]
fn attr_selector_presence() {
    check(
        "[disabled] { }",
        expect![[r#"
            SOURCE_FILE@0..14
              RULE_SET@0..14
                SELECTOR_LIST@0..10
                  SELECTOR@0..10
                    ATTR_SELECTOR@0..10
                      LBRACKET@0..1 "["
                      IDENT@1..9 "disabled"
                      RBRACKET@9..10 "]"
                BLOCK@10..14
                  WHITESPACE@10..11 " "
                  LBRACE@11..12 "{"
                  WHITESPACE@12..13 " "
                  RBRACE@13..14 "}"
        "#]],
    );
}

#[test]
fn attr_selector_value() {
    check(
        "[type=\"text\"] { }",
        expect![[r#"
            SOURCE_FILE@0..17
              RULE_SET@0..17
                SELECTOR_LIST@0..13
                  SELECTOR@0..13
                    ATTR_SELECTOR@0..13
                      LBRACKET@0..1 "["
                      IDENT@1..5 "type"
                      EQ@5..6 "="
                      QUOTED_STRING@6..12 "\"text\""
                      RBRACKET@12..13 "]"
                BLOCK@13..17
                  WHITESPACE@13..14 " "
                  LBRACE@14..15 "{"
                  WHITESPACE@15..16 " "
                  RBRACE@16..17 "}"
        "#]],
    );
}

// ── Nested properties ───────────────────────────────────────────────────

#[test]
fn nested_property_block() {
    check(
        "p { font: { weight: bold; size: 14px; } }",
        expect![[r#"
            SOURCE_FILE@0..41
              RULE_SET@0..41
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                BLOCK@1..41
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  NESTED_PROPERTY@3..39
                    PROPERTY@3..8
                      WHITESPACE@3..4 " "
                      IDENT@4..8 "font"
                    COLON@8..9 ":"
                    BLOCK@9..39
                      WHITESPACE@9..10 " "
                      LBRACE@10..11 "{"
                      DECLARATION@11..25
                        PROPERTY@11..18
                          WHITESPACE@11..12 " "
                          IDENT@12..18 "weight"
                        COLON@18..19 ":"
                        VALUE@19..24
                          VALUE@19..24
                            WHITESPACE@19..20 " "
                            IDENT@20..24 "bold"
                        SEMICOLON@24..25 ";"
                      DECLARATION@25..37
                        PROPERTY@25..30
                          WHITESPACE@25..26 " "
                          IDENT@26..30 "size"
                        COLON@30..31 ":"
                        VALUE@31..36
                          DIMENSION@31..36
                            WHITESPACE@31..32 " "
                            NUMBER@32..34 "14"
                            IDENT@34..36 "px"
                        SEMICOLON@36..37 ";"
                      WHITESPACE@37..38 " "
                      RBRACE@38..39 "}"
                  WHITESPACE@39..40 " "
                  RBRACE@40..41 "}"
        "#]],
    );
}

#[test]
fn nested_property_with_value() {
    check(
        "p { margin: 10px { top: 20px; } }",
        expect![[r#"
            SOURCE_FILE@0..33
              RULE_SET@0..33
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                BLOCK@1..33
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  NESTED_PROPERTY@3..31
                    PROPERTY@3..10
                      WHITESPACE@3..4 " "
                      IDENT@4..10 "margin"
                    COLON@10..11 ":"
                    VALUE@11..16
                      DIMENSION@11..16
                        WHITESPACE@11..12 " "
                        NUMBER@12..14 "10"
                        IDENT@14..16 "px"
                    BLOCK@16..31
                      WHITESPACE@16..17 " "
                      LBRACE@17..18 "{"
                      DECLARATION@18..29
                        PROPERTY@18..22
                          WHITESPACE@18..19 " "
                          IDENT@19..22 "top"
                        COLON@22..23 ":"
                        VALUE@23..28
                          DIMENSION@23..28
                            WHITESPACE@23..24 " "
                            NUMBER@24..26 "20"
                            IDENT@26..28 "px"
                        SEMICOLON@28..29 ";"
                      WHITESPACE@29..30 " "
                      RBRACE@30..31 "}"
                  WHITESPACE@31..32 " "
                  RBRACE@32..33 "}"
        "#]],
    );
}

#[test]
fn nested_property_value_and_block_zero() {
    // Value-and-block with bare 0 (no unit)
    check(
        ".a { margin: 0 { bottom: 15px; } }",
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
                  NESTED_PROPERTY@4..32
                    PROPERTY@4..11
                      WHITESPACE@4..5 " "
                      IDENT@5..11 "margin"
                    COLON@11..12 ":"
                    VALUE@12..14
                      NUMBER_LITERAL@12..14
                        WHITESPACE@12..13 " "
                        NUMBER@13..14 "0"
                    BLOCK@14..32
                      WHITESPACE@14..15 " "
                      LBRACE@15..16 "{"
                      DECLARATION@16..30
                        PROPERTY@16..23
                          WHITESPACE@16..17 " "
                          IDENT@17..23 "bottom"
                        COLON@23..24 ":"
                        VALUE@24..29
                          DIMENSION@24..29
                            WHITESPACE@24..25 " "
                            NUMBER@25..27 "15"
                            IDENT@27..29 "px"
                        SEMICOLON@29..30 ";"
                      WHITESPACE@30..31 " "
                      RBRACE@31..32 "}"
                  WHITESPACE@32..33 " "
                  RBRACE@33..34 "}"
        "#]],
    );
}

#[test]
fn nested_property_value_and_block_variable() {
    // Value-and-block with variable value
    check(
        ".b { font: $base { weight: bold; } }",
        expect![[r#"
            SOURCE_FILE@0..36
              RULE_SET@0..36
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "b"
                BLOCK@2..36
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  NESTED_PROPERTY@4..34
                    PROPERTY@4..9
                      WHITESPACE@4..5 " "
                      IDENT@5..9 "font"
                    COLON@9..10 ":"
                    VALUE@10..16
                      VARIABLE_REF@10..16
                        WHITESPACE@10..11 " "
                        DOLLAR@11..12 "$"
                        IDENT@12..16 "base"
                    BLOCK@16..34
                      WHITESPACE@16..17 " "
                      LBRACE@17..18 "{"
                      DECLARATION@18..32
                        PROPERTY@18..25
                          WHITESPACE@18..19 " "
                          IDENT@19..25 "weight"
                        COLON@25..26 ":"
                        VALUE@26..31
                          VALUE@26..31
                            WHITESPACE@26..27 " "
                            IDENT@27..31 "bold"
                        SEMICOLON@31..32 ";"
                      WHITESPACE@32..33 " "
                      RBRACE@33..34 "}"
                  WHITESPACE@34..35 " "
                  RBRACE@35..36 "}"
        "#]],
    );
}

#[test]
fn pseudo_selector_still_works() {
    // Ensure p:hover { } is still parsed as a selector, not a declaration
    check(
        "p:hover { color: red; }",
        expect![[r#"
            SOURCE_FILE@0..23
              RULE_SET@0..23
                SELECTOR_LIST@0..7
                  SELECTOR@0..7
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                    PSEUDO_SELECTOR@1..7
                      COLON@1..2 ":"
                      IDENT@2..7 "hover"
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
        "#]],
    );
}

// ── Interpolation ───────────────────────────────────────────────────────

#[test]
fn interpolation_in_selector() {
    check(
        "#{$tag} { }",
        expect![[r##"
            SOURCE_FILE@0..11
              RULE_SET@0..11
                SELECTOR_LIST@0..7
                  SELECTOR@0..7
                    INTERPOLATION@0..7
                      HASH_LBRACE@0..2 "#{"
                      VARIABLE_REF@2..6
                        DOLLAR@2..3 "$"
                        IDENT@3..6 "tag"
                      RBRACE@6..7 "}"
                BLOCK@7..11
                  WHITESPACE@7..8 " "
                  LBRACE@8..9 "{"
                  WHITESPACE@9..10 " "
                  RBRACE@10..11 "}"
        "##]],
    );
}

#[test]
fn interpolation_in_property() {
    check(
        "p { #{$prop}: red; }",
        expect![[r##"
            SOURCE_FILE@0..20
              RULE_SET@0..20
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                BLOCK@1..20
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  DECLARATION@3..18
                    PROPERTY@3..12
                      INTERPOLATION@3..12
                        WHITESPACE@3..4 " "
                        HASH_LBRACE@4..6 "#{"
                        VARIABLE_REF@6..11
                          DOLLAR@6..7 "$"
                          IDENT@7..11 "prop"
                        RBRACE@11..12 "}"
                    COLON@12..13 ":"
                    VALUE@13..17
                      VALUE@13..17
                        WHITESPACE@13..14 " "
                        IDENT@14..17 "red"
                    SEMICOLON@17..18 ";"
                  WHITESPACE@18..19 " "
                  RBRACE@19..20 "}"
        "##]],
    );
}

#[test]
fn interpolation_in_value() {
    check(
        "p { color: #{$c}; }",
        expect![[r##"
            SOURCE_FILE@0..19
              RULE_SET@0..19
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                BLOCK@1..19
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  DECLARATION@3..17
                    PROPERTY@3..9
                      WHITESPACE@3..4 " "
                      IDENT@4..9 "color"
                    COLON@9..10 ":"
                    VALUE@10..16
                      INTERPOLATION@10..16
                        WHITESPACE@10..11 " "
                        HASH_LBRACE@11..13 "#{"
                        VARIABLE_REF@13..15
                          DOLLAR@13..14 "$"
                          IDENT@14..15 "c"
                        RBRACE@15..16 "}"
                    SEMICOLON@16..17 ";"
                  WHITESPACE@17..18 " "
                  RBRACE@18..19 "}"
        "##]],
    );
}

// ── Multiple rules ──────────────────────────────────────────────────────

#[test]
fn multiple_rules() {
    check(
        "h1 { color: red; }\nh2 { color: blue; }",
        expect![[r#"
            SOURCE_FILE@0..38
              RULE_SET@0..18
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      IDENT@0..2 "h1"
                BLOCK@2..18
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  DECLARATION@4..16
                    PROPERTY@4..10
                      WHITESPACE@4..5 " "
                      IDENT@5..10 "color"
                    COLON@10..11 ":"
                    VALUE@11..15
                      VALUE@11..15
                        WHITESPACE@11..12 " "
                        IDENT@12..15 "red"
                    SEMICOLON@15..16 ";"
                  WHITESPACE@16..17 " "
                  RBRACE@17..18 "}"
              RULE_SET@18..38
                SELECTOR_LIST@18..21
                  SELECTOR@18..21
                    SIMPLE_SELECTOR@18..21
                      WHITESPACE@18..19 "\n"
                      IDENT@19..21 "h2"
                BLOCK@21..38
                  WHITESPACE@21..22 " "
                  LBRACE@22..23 "{"
                  DECLARATION@23..36
                    PROPERTY@23..29
                      WHITESPACE@23..24 " "
                      IDENT@24..29 "color"
                    COLON@29..30 ":"
                    VALUE@30..35
                      VALUE@30..35
                        WHITESPACE@30..31 " "
                        IDENT@31..35 "blue"
                    SEMICOLON@35..36 ";"
                  WHITESPACE@36..37 " "
                  RBRACE@37..38 "}"
        "#]],
    );
}

// ── Values with parens and brackets ─────────────────────────────────────

#[test]
fn value_with_parens() {
    check(
        "p { background: url(img.png); }",
        expect![[r#"
            SOURCE_FILE@0..31
              RULE_SET@0..31
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                BLOCK@1..31
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  DECLARATION@3..29
                    PROPERTY@3..14
                      WHITESPACE@3..4 " "
                      IDENT@4..14 "background"
                    COLON@14..15 ":"
                    VALUE@15..28
                      SPECIAL_FUNCTION_CALL@15..28
                        WHITESPACE@15..16 " "
                        IDENT@16..19 "url"
                        LPAREN@19..20 "("
                        URL_CONTENTS@20..27 "img.png"
                        RPAREN@27..28 ")"
                    SEMICOLON@28..29 ";"
                  WHITESPACE@29..30 " "
                  RBRACE@30..31 "}"
        "#]],
    );
}

#[test]
fn value_with_function_call() {
    check(
        "p { color: rgba(255, 0, 0, 0.5); }",
        expect![[r#"
            SOURCE_FILE@0..34
              RULE_SET@0..34
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                BLOCK@1..34
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  DECLARATION@3..32
                    PROPERTY@3..9
                      WHITESPACE@3..4 " "
                      IDENT@4..9 "color"
                    COLON@9..10 ":"
                    VALUE@10..31
                      FUNCTION_CALL@10..31
                        WHITESPACE@10..11 " "
                        IDENT@11..15 "rgba"
                        ARG_LIST@15..31
                          LPAREN@15..16 "("
                          ARG@16..19
                            NUMBER_LITERAL@16..19
                              NUMBER@16..19 "255"
                          COMMA@19..20 ","
                          ARG@20..22
                            NUMBER_LITERAL@20..22
                              WHITESPACE@20..21 " "
                              NUMBER@21..22 "0"
                          COMMA@22..23 ","
                          ARG@23..25
                            NUMBER_LITERAL@23..25
                              WHITESPACE@23..24 " "
                              NUMBER@24..25 "0"
                          COMMA@25..26 ","
                          ARG@26..30
                            NUMBER_LITERAL@26..30
                              WHITESPACE@26..27 " "
                              NUMBER@27..30 "0.5"
                          RPAREN@30..31 ")"
                    SEMICOLON@31..32 ";"
                  WHITESPACE@32..33 " "
                  RBRACE@33..34 "}"
        "#]],
    );
}

// ── Declaration without trailing semicolon ──────────────────────────────

#[test]
fn declaration_no_trailing_semicolon() {
    check(
        "p { color: red }",
        expect![[r#"
            SOURCE_FILE@0..16
              RULE_SET@0..16
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                BLOCK@1..16
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  DECLARATION@3..14
                    PROPERTY@3..9
                      WHITESPACE@3..4 " "
                      IDENT@4..9 "color"
                    COLON@9..10 ":"
                    VALUE@10..14
                      VALUE@10..14
                        WHITESPACE@10..11 " "
                        IDENT@11..14 "red"
                  WHITESPACE@14..15 " "
                  RBRACE@15..16 "}"
        "#]],
    );
}

// ── Empty rule ──────────────────────────────────────────────────────────

#[test]
fn empty_rule() {
    check(
        "div { }",
        expect![[r##"
            SOURCE_FILE@0..7
              RULE_SET@0..7
                SELECTOR_LIST@0..3
                  SELECTOR@0..3
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "div"
                BLOCK@3..7
                  WHITESPACE@3..4 " "
                  LBRACE@4..5 "{"
                  WHITESPACE@5..6 " "
                  RBRACE@6..7 "}"
        "##]],
    );
}

// ── Empty input ─────────────────────────────────────────────────────────

#[test]
fn empty_file() {
    check(
        "",
        expect![[r##"
            SOURCE_FILE@0..0
        "##]],
    );
}

// ── Class and ID selectors ──────────────────────────────────────────────

#[test]
fn class_selector() {
    check(
        ".active { }",
        expect![[r##"
            SOURCE_FILE@0..11
              RULE_SET@0..11
                SELECTOR_LIST@0..7
                  SELECTOR@0..7
                    SIMPLE_SELECTOR@0..7
                      DOT@0..1 "."
                      IDENT@1..7 "active"
                BLOCK@7..11
                  WHITESPACE@7..8 " "
                  LBRACE@8..9 "{"
                  WHITESPACE@9..10 " "
                  RBRACE@10..11 "}"
        "##]],
    );
}

#[test]
fn id_selector() {
    check(
        "#main { }",
        expect![[r##"
            SOURCE_FILE@0..9
              RULE_SET@0..9
                SELECTOR_LIST@0..5
                  SELECTOR@0..5
                    SIMPLE_SELECTOR@0..5
                      HASH@0..1 "#"
                      IDENT@1..5 "main"
                BLOCK@5..9
                  WHITESPACE@5..6 " "
                  LBRACE@6..7 "{"
                  WHITESPACE@7..8 " "
                  RBRACE@8..9 "}"
        "##]],
    );
}

// ── Nth-child pseudo ────────────────────────────────────────────────────

#[test]
fn pseudo_nth_child() {
    check(
        "li:nth-child(2n+1) { }",
        expect![[r#"
            SOURCE_FILE@0..22
              RULE_SET@0..22
                SELECTOR_LIST@0..18
                  SELECTOR@0..18
                    SIMPLE_SELECTOR@0..2
                      IDENT@0..2 "li"
                    PSEUDO_SELECTOR@2..18
                      COLON@2..3 ":"
                      IDENT@3..12 "nth-child"
                      LPAREN@12..13 "("
                      NUMBER@13..14 "2"
                      IDENT@14..15 "n"
                      PLUS@15..16 "+"
                      NUMBER@16..17 "1"
                      RPAREN@17..18 ")"
                BLOCK@18..22
                  WHITESPACE@18..19 " "
                  LBRACE@19..20 "{"
                  WHITESPACE@20..21 " "
                  RBRACE@21..22 "}"
        "#]],
    );
}

// ── Deeply nested ───────────────────────────────────────────────────────

#[test]
fn deeply_nested_rules() {
    check(
        "a { b { c { d: e; } } }",
        expect![[r#"
            SOURCE_FILE@0..23
              RULE_SET@0..23
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "a"
                BLOCK@1..23
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  RULE_SET@3..21
                    SELECTOR_LIST@3..5
                      SELECTOR@3..5
                        SIMPLE_SELECTOR@3..5
                          WHITESPACE@3..4 " "
                          IDENT@4..5 "b"
                    BLOCK@5..21
                      WHITESPACE@5..6 " "
                      LBRACE@6..7 "{"
                      RULE_SET@7..19
                        SELECTOR_LIST@7..9
                          SELECTOR@7..9
                            SIMPLE_SELECTOR@7..9
                              WHITESPACE@7..8 " "
                              IDENT@8..9 "c"
                        BLOCK@9..19
                          WHITESPACE@9..10 " "
                          LBRACE@10..11 "{"
                          DECLARATION@11..17
                            PROPERTY@11..13
                              WHITESPACE@11..12 " "
                              IDENT@12..13 "d"
                            COLON@13..14 ":"
                            VALUE@14..16
                              VALUE@14..16
                                WHITESPACE@14..15 " "
                                IDENT@15..16 "e"
                            SEMICOLON@16..17 ";"
                          WHITESPACE@17..18 " "
                          RBRACE@18..19 "}"
                      WHITESPACE@19..20 " "
                      RBRACE@20..21 "}"
                  WHITESPACE@21..22 " "
                  RBRACE@22..23 "}"
        "#]],
    );
}

// ── Mixed declarations and nested rules ─────────────────────────────────

#[test]
fn mixed_declarations_and_nested() {
    check(
        "nav { color: black; a { color: blue; } }",
        expect![[r#"
            SOURCE_FILE@0..40
              RULE_SET@0..40
                SELECTOR_LIST@0..3
                  SELECTOR@0..3
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "nav"
                BLOCK@3..40
                  WHITESPACE@3..4 " "
                  LBRACE@4..5 "{"
                  DECLARATION@5..19
                    PROPERTY@5..11
                      WHITESPACE@5..6 " "
                      IDENT@6..11 "color"
                    COLON@11..12 ":"
                    VALUE@12..18
                      VALUE@12..18
                        WHITESPACE@12..13 " "
                        IDENT@13..18 "black"
                    SEMICOLON@18..19 ";"
                  RULE_SET@19..38
                    SELECTOR_LIST@19..21
                      SELECTOR@19..21
                        SIMPLE_SELECTOR@19..21
                          WHITESPACE@19..20 " "
                          IDENT@20..21 "a"
                    BLOCK@21..38
                      WHITESPACE@21..22 " "
                      LBRACE@22..23 "{"
                      DECLARATION@23..36
                        PROPERTY@23..29
                          WHITESPACE@23..24 " "
                          IDENT@24..29 "color"
                        COLON@29..30 ":"
                        VALUE@30..35
                          VALUE@30..35
                            WHITESPACE@30..31 " "
                            IDENT@31..35 "blue"
                        SEMICOLON@35..36 ";"
                      WHITESPACE@36..37 " "
                      RBRACE@37..38 "}"
                  WHITESPACE@38..39 " "
                  RBRACE@39..40 "}"
        "#]],
    );
}

// ── Whitespace-only file ────────────────────────────────────────────────

#[test]
fn whitespace_only_file() {
    check(
        "   \n\t  ",
        expect![[r#"
            SOURCE_FILE@0..7
              WHITESPACE@0..7 "   \n\t  "
        "#]],
    );
}

// ── Complex real-world example ──────────────────────────────────────────

#[test]
fn complex_real_world() {
    check(
        ".container {\n  width: 100%;\n  > .row {\n    display: flex;\n  }\n}",
        expect![[r#"
            SOURCE_FILE@0..63
              RULE_SET@0..63
                SELECTOR_LIST@0..10
                  SELECTOR@0..10
                    SIMPLE_SELECTOR@0..10
                      DOT@0..1 "."
                      IDENT@1..10 "container"
                BLOCK@10..63
                  WHITESPACE@10..11 " "
                  LBRACE@11..12 "{"
                  DECLARATION@12..27
                    PROPERTY@12..20
                      WHITESPACE@12..15 "\n  "
                      IDENT@15..20 "width"
                    COLON@20..21 ":"
                    VALUE@21..26
                      DIMENSION@21..26
                        WHITESPACE@21..22 " "
                        NUMBER@22..25 "100"
                        PERCENT@25..26 "%"
                    SEMICOLON@26..27 ";"
                  RULE_SET@27..61
                    SELECTOR_LIST@27..36
                      SELECTOR@27..36
                        COMBINATOR@27..31
                          WHITESPACE@27..30 "\n  "
                          GT@30..31 ">"
                        SIMPLE_SELECTOR@31..36
                          WHITESPACE@31..32 " "
                          DOT@32..33 "."
                          IDENT@33..36 "row"
                    BLOCK@36..61
                      WHITESPACE@36..37 " "
                      LBRACE@37..38 "{"
                      DECLARATION@38..57
                        PROPERTY@38..50
                          WHITESPACE@38..43 "\n    "
                          IDENT@43..50 "display"
                        COLON@50..51 ":"
                        VALUE@51..56
                          VALUE@51..56
                            WHITESPACE@51..52 " "
                            IDENT@52..56 "flex"
                        SEMICOLON@56..57 ";"
                      WHITESPACE@57..60 "\n  "
                      RBRACE@60..61 "}"
                  WHITESPACE@61..62 "\n"
                  RBRACE@62..63 "}"
        "#]],
    );
}

// ═══════════════════════════════════════════════════════════════════════
// 2.16: Error recovery tests
// Quality criteria:
//   (a) error nodes are as small as possible (locality)
//   (b) correct nodes after error parse correctly (continuity)
//   (c) single syntax error ≤ 3 diagnostics (proportionality)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn error_missing_closing_brace() {
    check(
        "div { color: red;",
        expect![[r#"
            SOURCE_FILE@0..17
              RULE_SET@0..17
                SELECTOR_LIST@0..3
                  SELECTOR@0..3
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "div"
                BLOCK@3..17
                  WHITESPACE@3..4 " "
                  LBRACE@4..5 "{"
                  DECLARATION@5..17
                    PROPERTY@5..11
                      WHITESPACE@5..6 " "
                      IDENT@6..11 "color"
                    COLON@11..12 ":"
                    VALUE@12..16
                      VALUE@12..16
                        WHITESPACE@12..13 " "
                        IDENT@13..16 "red"
                    SEMICOLON@16..17 ";"
            errors:
              17..17: expected RBRACE
        "#]],
    );
}

#[test]
fn error_missing_semicolon() {
    check(
        "p { color: red font-size: 14px; }",
        expect![[r#"
            SOURCE_FILE@0..33
              RULE_SET@0..33
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                BLOCK@1..33
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  DECLARATION@3..31
                    PROPERTY@3..9
                      WHITESPACE@3..4 " "
                      IDENT@4..9 "color"
                    COLON@9..10 ":"
                    VALUE@10..30
                      VALUE@10..14
                        WHITESPACE@10..11 " "
                        IDENT@11..14 "red"
                      VALUE@14..24
                        WHITESPACE@14..15 " "
                        IDENT@15..24 "font-size"
                      COLON@24..25 ":"
                      DIMENSION@25..30
                        WHITESPACE@25..26 " "
                        NUMBER@26..28 "14"
                        IDENT@28..30 "px"
                    SEMICOLON@30..31 ";"
                  WHITESPACE@31..32 " "
                  RBRACE@32..33 "}"
        "#]],
    );
}

#[test]
fn error_missing_colon() {
    check(
        "p { color red; }",
        expect![[r#"
            SOURCE_FILE@0..16
              RULE_SET@0..16
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                BLOCK@1..16
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  RULE_SET@3..13
                    SELECTOR_LIST@3..13
                      SELECTOR@3..13
                        SIMPLE_SELECTOR@3..9
                          WHITESPACE@3..4 " "
                          IDENT@4..9 "color"
                        SIMPLE_SELECTOR@9..13
                          WHITESPACE@9..10 " "
                          IDENT@10..13 "red"
                  SEMICOLON@13..14 ";"
                  WHITESPACE@14..15 " "
                  RBRACE@15..16 "}"
            errors:
              13..14: expected `{`
        "#]],
    );
}

#[test]
fn error_garbage_tokens_between_rules() {
    check(
        "div { } @@@ h1 { }",
        expect![[r#"
            SOURCE_FILE@0..18
              RULE_SET@0..7
                SELECTOR_LIST@0..3
                  SELECTOR@0..3
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "div"
                BLOCK@3..7
                  WHITESPACE@3..4 " "
                  LBRACE@4..5 "{"
                  WHITESPACE@5..6 " "
                  RBRACE@6..7 "}"
              GENERIC_AT_RULE@7..18
                WHITESPACE@7..8 " "
                AT@8..9 "@"
                AT@9..10 "@"
                AT@10..11 "@"
                WHITESPACE@11..12 " "
                IDENT@12..14 "h1"
                BLOCK@14..18
                  WHITESPACE@14..15 " "
                  LBRACE@15..16 "{"
                  WHITESPACE@16..17 " "
                  RBRACE@17..18 "}"
        "#]],
    );
}

#[test]
fn error_stray_rbrace() {
    check(
        "} div { }",
        expect![[r#"
            SOURCE_FILE@0..9
              ERROR@0..1
                RBRACE@0..1 "}"
              RULE_SET@1..9
                SELECTOR_LIST@1..5
                  SELECTOR@1..5
                    SIMPLE_SELECTOR@1..5
                      WHITESPACE@1..2 " "
                      IDENT@2..5 "div"
                BLOCK@5..9
                  WHITESPACE@5..6 " "
                  LBRACE@6..7 "{"
                  WHITESPACE@7..8 " "
                  RBRACE@8..9 "}"
            errors:
              0..1: expected rule
        "#]],
    );
}

#[test]
fn error_selector_no_block() {
    check(
        "div",
        expect![[r#"
            SOURCE_FILE@0..3
              RULE_SET@0..3
                SELECTOR_LIST@0..3
                  SELECTOR@0..3
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "div"
            errors:
              3..3: expected `{`
        "#]],
    );
}

#[test]
fn error_nested_missing_brace() {
    check(
        "nav { ul { color: red; } h1 { }",
        expect![[r#"
            SOURCE_FILE@0..31
              RULE_SET@0..31
                SELECTOR_LIST@0..3
                  SELECTOR@0..3
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "nav"
                BLOCK@3..31
                  WHITESPACE@3..4 " "
                  LBRACE@4..5 "{"
                  RULE_SET@5..24
                    SELECTOR_LIST@5..8
                      SELECTOR@5..8
                        SIMPLE_SELECTOR@5..8
                          WHITESPACE@5..6 " "
                          IDENT@6..8 "ul"
                    BLOCK@8..24
                      WHITESPACE@8..9 " "
                      LBRACE@9..10 "{"
                      DECLARATION@10..22
                        PROPERTY@10..16
                          WHITESPACE@10..11 " "
                          IDENT@11..16 "color"
                        COLON@16..17 ":"
                        VALUE@17..21
                          VALUE@17..21
                            WHITESPACE@17..18 " "
                            IDENT@18..21 "red"
                        SEMICOLON@21..22 ";"
                      WHITESPACE@22..23 " "
                      RBRACE@23..24 "}"
                  RULE_SET@24..31
                    SELECTOR_LIST@24..27
                      SELECTOR@24..27
                        SIMPLE_SELECTOR@24..27
                          WHITESPACE@24..25 " "
                          IDENT@25..27 "h1"
                    BLOCK@27..31
                      WHITESPACE@27..28 " "
                      LBRACE@28..29 "{"
                      WHITESPACE@29..30 " "
                      RBRACE@30..31 "}"
            errors:
              31..31: expected RBRACE
        "#]],
    );
}

#[test]
fn error_at_without_ident() {
    check(
        "@ div { }",
        expect![[r#"
            SOURCE_FILE@0..9
              GENERIC_AT_RULE@0..9
                AT@0..1 "@"
                WHITESPACE@1..2 " "
                IDENT@2..5 "div"
                BLOCK@5..9
                  WHITESPACE@5..6 " "
                  LBRACE@6..7 "{"
                  WHITESPACE@7..8 " "
                  RBRACE@8..9 "}"
        "#]],
    );
}

#[test]
fn error_consecutive_combinators() {
    check(
        "div > > span { }",
        expect![[r#"
            SOURCE_FILE@0..16
              RULE_SET@0..16
                SELECTOR_LIST@0..12
                  SELECTOR@0..12
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "div"
                    COMBINATOR@3..5
                      WHITESPACE@3..4 " "
                      GT@4..5 ">"
                    COMBINATOR@5..7
                      WHITESPACE@5..6 " "
                      GT@6..7 ">"
                    SIMPLE_SELECTOR@7..12
                      WHITESPACE@7..8 " "
                      IDENT@8..12 "span"
                BLOCK@12..16
                  WHITESPACE@12..13 " "
                  LBRACE@13..14 "{"
                  WHITESPACE@14..15 " "
                  RBRACE@15..16 "}"
        "#]],
    );
}

#[test]
fn error_empty_selector_list_commas() {
    check(
        ",, { }",
        expect![[r#"
            SOURCE_FILE@0..6
              ERROR@0..6
                COMMA@0..1 ","
                COMMA@1..2 ","
                WHITESPACE@2..3 " "
                LBRACE@3..4 "{"
                WHITESPACE@4..5 " "
                RBRACE@5..6 "}"
            errors:
              0..1: expected rule
        "#]],
    );
}

#[test]
fn error_unterminated_attr() {
    check(
        "[attr { }",
        expect![[r#"
            SOURCE_FILE@0..9
              RULE_SET@0..9
                SELECTOR_LIST@0..9
                  SELECTOR@0..9
                    ATTR_SELECTOR@0..9
                      LBRACKET@0..1 "["
                      IDENT@1..5 "attr"
                      WHITESPACE@5..6 " "
                      LBRACE@6..7 "{"
                      WHITESPACE@7..8 " "
                      RBRACE@8..9 "}"
            errors:
              9..9: expected `{`
        "#]],
    );
}

#[test]
fn error_multiple_errors_same_file() {
    check(
        "div { } @@@ p { color; } h1 { }",
        expect![[r#"
            SOURCE_FILE@0..31
              RULE_SET@0..7
                SELECTOR_LIST@0..3
                  SELECTOR@0..3
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "div"
                BLOCK@3..7
                  WHITESPACE@3..4 " "
                  LBRACE@4..5 "{"
                  WHITESPACE@5..6 " "
                  RBRACE@6..7 "}"
              GENERIC_AT_RULE@7..24
                WHITESPACE@7..8 " "
                AT@8..9 "@"
                AT@9..10 "@"
                AT@10..11 "@"
                WHITESPACE@11..12 " "
                IDENT@12..13 "p"
                BLOCK@13..24
                  WHITESPACE@13..14 " "
                  LBRACE@14..15 "{"
                  RULE_SET@15..21
                    SELECTOR_LIST@15..21
                      SELECTOR@15..21
                        SIMPLE_SELECTOR@15..21
                          WHITESPACE@15..16 " "
                          IDENT@16..21 "color"
                  SEMICOLON@21..22 ";"
                  WHITESPACE@22..23 " "
                  RBRACE@23..24 "}"
              RULE_SET@24..31
                SELECTOR_LIST@24..27
                  SELECTOR@24..27
                    SIMPLE_SELECTOR@24..27
                      WHITESPACE@24..25 " "
                      IDENT@25..27 "h1"
                BLOCK@27..31
                  WHITESPACE@27..28 " "
                  LBRACE@28..29 "{"
                  WHITESPACE@29..30 " "
                  RBRACE@30..31 "}"
            errors:
              21..22: expected `{`
        "#]],
    );
}

#[test]
fn error_eof_mid_value() {
    check(
        "p { color:",
        expect![[r#"
            SOURCE_FILE@0..10
              RULE_SET@0..10
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                BLOCK@1..10
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  DECLARATION@3..10
                    PROPERTY@3..9
                      WHITESPACE@3..4 " "
                      IDENT@4..9 "color"
                    COLON@9..10 ":"
            errors:
              10..10: expected RBRACE
        "#]],
    );
}

#[test]
fn error_eof_mid_selector() {
    check(
        "div > ",
        expect![[r#"
            SOURCE_FILE@0..6
              RULE_SET@0..5
                SELECTOR_LIST@0..5
                  SELECTOR@0..5
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "div"
                    COMBINATOR@3..5
                      WHITESPACE@3..4 " "
                      GT@4..5 ">"
              WHITESPACE@5..6 " "
            errors:
              6..6: expected `{`
        "#]],
    );
}

#[test]
fn error_empty_braces() {
    check(
        "{ }",
        expect![[r#"
            SOURCE_FILE@0..3
              ERROR@0..3
                LBRACE@0..1 "{"
                WHITESPACE@1..2 " "
                RBRACE@2..3 "}"
            errors:
              0..1: expected rule
        "#]],
    );
}

#[test]
fn error_recovery_correct_after_error() {
    check(
        "@@@ h1 { color: red; }",
        expect![[r#"
            SOURCE_FILE@0..22
              GENERIC_AT_RULE@0..22
                AT@0..1 "@"
                AT@1..2 "@"
                AT@2..3 "@"
                WHITESPACE@3..4 " "
                IDENT@4..6 "h1"
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
fn error_unmatched_lbrace() {
    check(
        "div { { color: red; }",
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
                  ERROR@5..7
                    WHITESPACE@5..6 " "
                    LBRACE@6..7 "{"
                  DECLARATION@7..19
                    PROPERTY@7..13
                      WHITESPACE@7..8 " "
                      IDENT@8..13 "color"
                    COLON@13..14 ":"
                    VALUE@14..18
                      VALUE@14..18
                        WHITESPACE@14..15 " "
                        IDENT@15..18 "red"
                    SEMICOLON@18..19 ";"
                  WHITESPACE@19..20 " "
                  RBRACE@20..21 "}"
            errors:
              6..7: expected declaration or nested rule
        "#]],
    );
}

#[test]
fn error_declaration_missing_property() {
    check(
        "p { : red; }",
        expect![[r#"
            SOURCE_FILE@0..12
              RULE_SET@0..12
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                BLOCK@1..12
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  RULE_SET@3..9
                    SELECTOR_LIST@3..9
                      SELECTOR@3..9
                        PSEUDO_SELECTOR@3..9
                          WHITESPACE@3..4 " "
                          COLON@4..5 ":"
                          WHITESPACE@5..6 " "
                          IDENT@6..9 "red"
                  SEMICOLON@9..10 ";"
                  WHITESPACE@10..11 " "
                  RBRACE@11..12 "}"
            errors:
              9..10: expected `{`
        "#]],
    );
}

#[test]
fn error_trailing_comma_selector() {
    check(
        "h1, { }",
        expect![[r#"
            SOURCE_FILE@0..7
              RULE_SET@0..7
                SELECTOR_LIST@0..3
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      IDENT@0..2 "h1"
                  COMMA@2..3 ","
                BLOCK@3..7
                  WHITESPACE@3..4 " "
                  LBRACE@4..5 "{"
                  WHITESPACE@5..6 " "
                  RBRACE@6..7 "}"
            errors:
              4..5: expected selector after `,`
        "#]],
    );
}

#[test]
fn error_utf8_bom_then_error() {
    check(
        "\u{FEFF}@@@ div { }",
        expect![[r#"
            SOURCE_FILE@0..14
              GENERIC_AT_RULE@0..14
                WHITESPACE@0..3 "\u{feff}"
                AT@3..4 "@"
                AT@4..5 "@"
                AT@5..6 "@"
                WHITESPACE@6..7 " "
                IDENT@7..10 "div"
                BLOCK@10..14
                  WHITESPACE@10..11 " "
                  LBRACE@11..12 "{"
                  WHITESPACE@12..13 " "
                  RBRACE@13..14 "}"
        "#]],
    );
}

#[test]
fn error_eof_in_block() {
    check(
        "div {",
        expect![[r#"
            SOURCE_FILE@0..5
              RULE_SET@0..5
                SELECTOR_LIST@0..3
                  SELECTOR@0..3
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "div"
                BLOCK@3..5
                  WHITESPACE@3..4 " "
                  LBRACE@4..5 "{"
            errors:
              5..5: expected RBRACE
        "#]],
    );
}

#[test]
fn error_double_colon_in_property() {
    check(
        "p { color:: red; }",
        expect![[r#"
            SOURCE_FILE@0..18
              RULE_SET@0..18
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                BLOCK@1..18
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  RULE_SET@3..15
                    SELECTOR_LIST@3..15
                      SELECTOR@3..15
                        SIMPLE_SELECTOR@3..9
                          WHITESPACE@3..4 " "
                          IDENT@4..9 "color"
                        PSEUDO_SELECTOR@9..15
                          COLON_COLON@9..11 "::"
                          WHITESPACE@11..12 " "
                          IDENT@12..15 "red"
                  SEMICOLON@15..16 ";"
                  WHITESPACE@16..17 " "
                  RBRACE@17..18 "}"
            errors:
              15..16: expected `{`
        "#]],
    );
}

#[test]
fn error_recovery_continuity_after_bad_decl() {
    check(
        "p { col$or: red; font-size: 14px; }",
        expect![[r#"
            SOURCE_FILE@0..35
              RULE_SET@0..35
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                BLOCK@1..35
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  RULE_SET@3..7
                    SELECTOR_LIST@3..7
                      SELECTOR@3..7
                        SIMPLE_SELECTOR@3..7
                          WHITESPACE@3..4 " "
                          IDENT@4..7 "col"
                  VARIABLE_DECL@7..16
                    DOLLAR@7..8 "$"
                    IDENT@8..10 "or"
                    COLON@10..11 ":"
                    VALUE@11..15
                      WHITESPACE@11..12 " "
                      IDENT@12..15 "red"
                    SEMICOLON@15..16 ";"
                  DECLARATION@16..33
                    PROPERTY@16..26
                      WHITESPACE@16..17 " "
                      IDENT@17..26 "font-size"
                    COLON@26..27 ":"
                    VALUE@27..32
                      DIMENSION@27..32
                        WHITESPACE@27..28 " "
                        NUMBER@28..30 "14"
                        IDENT@30..32 "px"
                    SEMICOLON@32..33 ";"
                  WHITESPACE@33..34 " "
                  RBRACE@34..35 "}"
            errors:
              7..8: expected `{`
        "#]],
    );
}

#[test]
fn error_eof_after_property() {
    check(
        "p { color",
        expect![[r#"
            SOURCE_FILE@0..9
              RULE_SET@0..9
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                BLOCK@1..9
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  RULE_SET@3..9
                    SELECTOR_LIST@3..9
                      SELECTOR@3..9
                        SIMPLE_SELECTOR@3..9
                          WHITESPACE@3..4 " "
                          IDENT@4..9 "color"
            errors:
              9..9: expected `{`
              9..9: expected RBRACE
        "#]],
    );
}

#[test]
fn error_nested_error_recovery() {
    check(
        "nav { a { color; } b { font-size: 14px; } }",
        expect![[r#"
            SOURCE_FILE@0..43
              RULE_SET@0..43
                SELECTOR_LIST@0..3
                  SELECTOR@0..3
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "nav"
                BLOCK@3..43
                  WHITESPACE@3..4 " "
                  LBRACE@4..5 "{"
                  RULE_SET@5..18
                    SELECTOR_LIST@5..7
                      SELECTOR@5..7
                        SIMPLE_SELECTOR@5..7
                          WHITESPACE@5..6 " "
                          IDENT@6..7 "a"
                    BLOCK@7..18
                      WHITESPACE@7..8 " "
                      LBRACE@8..9 "{"
                      RULE_SET@9..15
                        SELECTOR_LIST@9..15
                          SELECTOR@9..15
                            SIMPLE_SELECTOR@9..15
                              WHITESPACE@9..10 " "
                              IDENT@10..15 "color"
                      SEMICOLON@15..16 ";"
                      WHITESPACE@16..17 " "
                      RBRACE@17..18 "}"
                  RULE_SET@18..41
                    SELECTOR_LIST@18..20
                      SELECTOR@18..20
                        SIMPLE_SELECTOR@18..20
                          WHITESPACE@18..19 " "
                          IDENT@19..20 "b"
                    BLOCK@20..41
                      WHITESPACE@20..21 " "
                      LBRACE@21..22 "{"
                      DECLARATION@22..39
                        PROPERTY@22..32
                          WHITESPACE@22..23 " "
                          IDENT@23..32 "font-size"
                        COLON@32..33 ":"
                        VALUE@33..38
                          DIMENSION@33..38
                            WHITESPACE@33..34 " "
                            NUMBER@34..36 "14"
                            IDENT@36..38 "px"
                        SEMICOLON@38..39 ";"
                      WHITESPACE@39..40 " "
                      RBRACE@40..41 "}"
                  WHITESPACE@41..42 " "
                  RBRACE@42..43 "}"
            errors:
              15..16: expected `{`
        "#]],
    );
}

// ── Stress: mid-declaration errors ─────────────────────────────────────

#[test]
fn error_empty_value_with_continuation() {
    check(
        "p { color: ; font-size: 14px; }",
        expect![[r#"
            SOURCE_FILE@0..31
              RULE_SET@0..31
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                BLOCK@1..31
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  DECLARATION@3..12
                    PROPERTY@3..9
                      WHITESPACE@3..4 " "
                      IDENT@4..9 "color"
                    COLON@9..10 ":"
                    WHITESPACE@10..11 " "
                    SEMICOLON@11..12 ";"
                  DECLARATION@12..29
                    PROPERTY@12..22
                      WHITESPACE@12..13 " "
                      IDENT@13..22 "font-size"
                    COLON@22..23 ":"
                    VALUE@23..28
                      DIMENSION@23..28
                        WHITESPACE@23..24 " "
                        NUMBER@24..26 "14"
                        IDENT@26..28 "px"
                    SEMICOLON@28..29 ";"
                  WHITESPACE@29..30 " "
                  RBRACE@30..31 "}"
        "#]],
    );
}

#[test]
fn error_missing_semicolon_between_decls() {
    check(
        "p { color: red font-size: 14px; }",
        expect![[r#"
            SOURCE_FILE@0..33
              RULE_SET@0..33
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                BLOCK@1..33
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  DECLARATION@3..31
                    PROPERTY@3..9
                      WHITESPACE@3..4 " "
                      IDENT@4..9 "color"
                    COLON@9..10 ":"
                    VALUE@10..30
                      VALUE@10..14
                        WHITESPACE@10..11 " "
                        IDENT@11..14 "red"
                      VALUE@14..24
                        WHITESPACE@14..15 " "
                        IDENT@15..24 "font-size"
                      COLON@24..25 ":"
                      DIMENSION@25..30
                        WHITESPACE@25..26 " "
                        NUMBER@26..28 "14"
                        IDENT@28..30 "px"
                    SEMICOLON@30..31 ";"
                  WHITESPACE@31..32 " "
                  RBRACE@32..33 "}"
        "#]],
    );
}

#[test]
fn error_extra_colon_in_value() {
    check(
        "p { color: : red; font-size: 14px; }",
        expect![[r#"
            SOURCE_FILE@0..36
              RULE_SET@0..36
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                BLOCK@1..36
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  DECLARATION@3..17
                    PROPERTY@3..9
                      WHITESPACE@3..4 " "
                      IDENT@4..9 "color"
                    COLON@9..10 ":"
                    VALUE@10..16
                      WHITESPACE@10..11 " "
                      COLON@11..12 ":"
                      VALUE@12..16
                        WHITESPACE@12..13 " "
                        IDENT@13..16 "red"
                    SEMICOLON@16..17 ";"
                  DECLARATION@17..34
                    PROPERTY@17..27
                      WHITESPACE@17..18 " "
                      IDENT@18..27 "font-size"
                    COLON@27..28 ":"
                    VALUE@28..33
                      DIMENSION@28..33
                        WHITESPACE@28..29 " "
                        NUMBER@29..31 "14"
                        IDENT@31..33 "px"
                    SEMICOLON@33..34 ";"
                  WHITESPACE@34..35 " "
                  RBRACE@35..36 "}"
        "#]],
    );
}

// ── Stress: malformed at-rules ─────────────────────────────────────────

#[test]
fn error_mixin_bad_params() {
    check(
        "@mixin m($, $b) { } .ok { color: red; }",
        expect![[r#"
            SOURCE_FILE@0..39
              MIXIN_RULE@0..19
                AT@0..1 "@"
                IDENT@1..6 "mixin"
                WHITESPACE@6..7 " "
                IDENT@7..8 "m"
                PARAM_LIST@8..15
                  LPAREN@8..9 "("
                  PARAM@9..10
                    DOLLAR@9..10 "$"
                  COMMA@10..11 ","
                  PARAM@11..14
                    WHITESPACE@11..12 " "
                    DOLLAR@12..13 "$"
                    IDENT@13..14 "b"
                  RPAREN@14..15 ")"
                BLOCK@15..19
                  WHITESPACE@15..16 " "
                  LBRACE@16..17 "{"
                  WHITESPACE@17..18 " "
                  RBRACE@18..19 "}"
              RULE_SET@19..39
                SELECTOR_LIST@19..23
                  SELECTOR@19..23
                    SIMPLE_SELECTOR@19..23
                      WHITESPACE@19..20 " "
                      DOT@20..21 "."
                      IDENT@21..23 "ok"
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
            errors:
              10..11: expected IDENT
        "#]],
    );
}

#[test]
fn error_for_missing_variable() {
    check(
        "@for from 1 through 10 { } .ok { }",
        expect![[r#"
            SOURCE_FILE@0..34
              FOR_RULE@0..26
                AT@0..1 "@"
                IDENT@1..4 "for"
                WHITESPACE@4..5 " "
                IDENT@5..9 "from"
                NUMBER_LITERAL@9..11
                  WHITESPACE@9..10 " "
                  NUMBER@10..11 "1"
                WHITESPACE@11..12 " "
                IDENT@12..19 "through"
                NUMBER_LITERAL@19..22
                  WHITESPACE@19..20 " "
                  NUMBER@20..22 "10"
                BLOCK@22..26
                  WHITESPACE@22..23 " "
                  LBRACE@23..24 "{"
                  WHITESPACE@24..25 " "
                  RBRACE@25..26 "}"
              RULE_SET@26..34
                SELECTOR_LIST@26..30
                  SELECTOR@26..30
                    SIMPLE_SELECTOR@26..30
                      WHITESPACE@26..27 " "
                      DOT@27..28 "."
                      IDENT@28..30 "ok"
                BLOCK@30..34
                  WHITESPACE@30..31 " "
                  LBRACE@31..32 "{"
                  WHITESPACE@32..33 " "
                  RBRACE@33..34 "}"
            errors:
              5..9: expected DOLLAR
              10..11: expected `from`
        "#]],
    );
}

#[test]
fn error_each_missing_list() {
    check(
        "@each $x { } .ok { color: red; }",
        expect![[r#"
            SOURCE_FILE@0..32
              EACH_RULE@0..12
                AT@0..1 "@"
                IDENT@1..5 "each"
                WHITESPACE@5..6 " "
                DOLLAR@6..7 "$"
                IDENT@7..8 "x"
                BLOCK@8..12
                  WHITESPACE@8..9 " "
                  LBRACE@9..10 "{"
                  WHITESPACE@10..11 " "
                  RBRACE@11..12 "}"
              RULE_SET@12..32
                SELECTOR_LIST@12..16
                  SELECTOR@12..16
                    SIMPLE_SELECTOR@12..16
                      WHITESPACE@12..13 " "
                      DOT@13..14 "."
                      IDENT@14..16 "ok"
                BLOCK@16..32
                  WHITESPACE@16..17 " "
                  LBRACE@17..18 "{"
                  DECLARATION@18..30
                    PROPERTY@18..24
                      WHITESPACE@18..19 " "
                      IDENT@19..24 "color"
                    COLON@24..25 ":"
                    VALUE@25..29
                      VALUE@25..29
                        WHITESPACE@25..26 " "
                        IDENT@26..29 "red"
                    SEMICOLON@29..30 ";"
                  WHITESPACE@30..31 " "
                  RBRACE@31..32 "}"
            errors:
              9..10: expected `in`
              9..10: expected expression
        "#]],
    );
}

#[test]
fn error_include_missing_name() {
    check(
        ".x { @include; color: red; }",
        expect![[r#"
            SOURCE_FILE@0..28
              RULE_SET@0..28
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "x"
                BLOCK@2..28
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  INCLUDE_RULE@4..14
                    WHITESPACE@4..5 " "
                    AT@5..6 "@"
                    IDENT@6..13 "include"
                    SEMICOLON@13..14 ";"
                  DECLARATION@14..26
                    PROPERTY@14..20
                      WHITESPACE@14..15 " "
                      IDENT@15..20 "color"
                    COLON@20..21 ":"
                    VALUE@21..25
                      VALUE@21..25
                        WHITESPACE@21..22 " "
                        IDENT@22..25 "red"
                    SEMICOLON@25..26 ";"
                  WHITESPACE@26..27 " "
                  RBRACE@27..28 "}"
            errors:
              13..14: expected IDENT
        "#]],
    );
}

// ── Stress: nested error recovery ──────────────────────────────────────

#[test]
fn error_inner_block_no_corruption_of_sibling() {
    check(
        "nav { .inner { color; } .sibling { font-size: 14px; } }",
        expect![[r#"
            SOURCE_FILE@0..55
              RULE_SET@0..55
                SELECTOR_LIST@0..3
                  SELECTOR@0..3
                    SIMPLE_SELECTOR@0..3
                      IDENT@0..3 "nav"
                BLOCK@3..55
                  WHITESPACE@3..4 " "
                  LBRACE@4..5 "{"
                  RULE_SET@5..23
                    SELECTOR_LIST@5..12
                      SELECTOR@5..12
                        SIMPLE_SELECTOR@5..12
                          WHITESPACE@5..6 " "
                          DOT@6..7 "."
                          IDENT@7..12 "inner"
                    BLOCK@12..23
                      WHITESPACE@12..13 " "
                      LBRACE@13..14 "{"
                      RULE_SET@14..20
                        SELECTOR_LIST@14..20
                          SELECTOR@14..20
                            SIMPLE_SELECTOR@14..20
                              WHITESPACE@14..15 " "
                              IDENT@15..20 "color"
                      SEMICOLON@20..21 ";"
                      WHITESPACE@21..22 " "
                      RBRACE@22..23 "}"
                  RULE_SET@23..53
                    SELECTOR_LIST@23..32
                      SELECTOR@23..32
                        SIMPLE_SELECTOR@23..32
                          WHITESPACE@23..24 " "
                          DOT@24..25 "."
                          IDENT@25..32 "sibling"
                    BLOCK@32..53
                      WHITESPACE@32..33 " "
                      LBRACE@33..34 "{"
                      DECLARATION@34..51
                        PROPERTY@34..44
                          WHITESPACE@34..35 " "
                          IDENT@35..44 "font-size"
                        COLON@44..45 ":"
                        VALUE@45..50
                          DIMENSION@45..50
                            WHITESPACE@45..46 " "
                            NUMBER@46..48 "14"
                            IDENT@48..50 "px"
                        SEMICOLON@50..51 ";"
                      WHITESPACE@51..52 " "
                      RBRACE@52..53 "}"
                  WHITESPACE@53..54 " "
                  RBRACE@54..55 "}"
            errors:
              20..21: expected `{`
        "#]],
    );
}

#[test]
fn error_three_levels_deep_outer_intact() {
    check(
        "a { b { c { color; } font-size: 14px; } margin: 0; }",
        expect![[r#"
            SOURCE_FILE@0..52
              RULE_SET@0..52
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "a"
                BLOCK@1..52
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  RULE_SET@3..39
                    SELECTOR_LIST@3..5
                      SELECTOR@3..5
                        SIMPLE_SELECTOR@3..5
                          WHITESPACE@3..4 " "
                          IDENT@4..5 "b"
                    BLOCK@5..39
                      WHITESPACE@5..6 " "
                      LBRACE@6..7 "{"
                      RULE_SET@7..20
                        SELECTOR_LIST@7..9
                          SELECTOR@7..9
                            SIMPLE_SELECTOR@7..9
                              WHITESPACE@7..8 " "
                              IDENT@8..9 "c"
                        BLOCK@9..20
                          WHITESPACE@9..10 " "
                          LBRACE@10..11 "{"
                          RULE_SET@11..17
                            SELECTOR_LIST@11..17
                              SELECTOR@11..17
                                SIMPLE_SELECTOR@11..17
                                  WHITESPACE@11..12 " "
                                  IDENT@12..17 "color"
                          SEMICOLON@17..18 ";"
                          WHITESPACE@18..19 " "
                          RBRACE@19..20 "}"
                      DECLARATION@20..37
                        PROPERTY@20..30
                          WHITESPACE@20..21 " "
                          IDENT@21..30 "font-size"
                        COLON@30..31 ":"
                        VALUE@31..36
                          DIMENSION@31..36
                            WHITESPACE@31..32 " "
                            NUMBER@32..34 "14"
                            IDENT@34..36 "px"
                        SEMICOLON@36..37 ";"
                      WHITESPACE@37..38 " "
                      RBRACE@38..39 "}"
                  DECLARATION@39..50
                    PROPERTY@39..46
                      WHITESPACE@39..40 " "
                      IDENT@40..46 "margin"
                    COLON@46..47 ":"
                    VALUE@47..49
                      NUMBER_LITERAL@47..49
                        WHITESPACE@47..48 " "
                        NUMBER@48..49 "0"
                    SEMICOLON@49..50 ";"
                  WHITESPACE@50..51 " "
                  RBRACE@51..52 "}"
            errors:
              17..18: expected `{`
        "#]],
    );
}

// ── Stress: multiple sequential errors ─────────────────────────────────

#[test]
fn error_two_broken_rules_third_correct() {
    check(
        "p { color; } q { font; } h1 { margin: 0; }",
        expect![[r#"
            SOURCE_FILE@0..42
              RULE_SET@0..12
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                BLOCK@1..12
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  RULE_SET@3..9
                    SELECTOR_LIST@3..9
                      SELECTOR@3..9
                        SIMPLE_SELECTOR@3..9
                          WHITESPACE@3..4 " "
                          IDENT@4..9 "color"
                  SEMICOLON@9..10 ";"
                  WHITESPACE@10..11 " "
                  RBRACE@11..12 "}"
              RULE_SET@12..24
                SELECTOR_LIST@12..14
                  SELECTOR@12..14
                    SIMPLE_SELECTOR@12..14
                      WHITESPACE@12..13 " "
                      IDENT@13..14 "q"
                BLOCK@14..24
                  WHITESPACE@14..15 " "
                  LBRACE@15..16 "{"
                  RULE_SET@16..21
                    SELECTOR_LIST@16..21
                      SELECTOR@16..21
                        SIMPLE_SELECTOR@16..21
                          WHITESPACE@16..17 " "
                          IDENT@17..21 "font"
                  SEMICOLON@21..22 ";"
                  WHITESPACE@22..23 " "
                  RBRACE@23..24 "}"
              RULE_SET@24..42
                SELECTOR_LIST@24..27
                  SELECTOR@24..27
                    SIMPLE_SELECTOR@24..27
                      WHITESPACE@24..25 " "
                      IDENT@25..27 "h1"
                BLOCK@27..42
                  WHITESPACE@27..28 " "
                  LBRACE@28..29 "{"
                  DECLARATION@29..40
                    PROPERTY@29..36
                      WHITESPACE@29..30 " "
                      IDENT@30..36 "margin"
                    COLON@36..37 ":"
                    VALUE@37..39
                      NUMBER_LITERAL@37..39
                        WHITESPACE@37..38 " "
                        NUMBER@38..39 "0"
                    SEMICOLON@39..40 ";"
                  WHITESPACE@40..41 " "
                  RBRACE@41..42 "}"
            errors:
              9..10: expected `{`
              21..22: expected `{`
        "#]],
    );
}

#[test]
fn error_garbage_then_valid_rule() {
    check(
        "@@@ { } %%% { } h1 { color: red; }",
        expect![[r#"
            SOURCE_FILE@0..34
              GENERIC_AT_RULE@0..7
                AT@0..1 "@"
                AT@1..2 "@"
                AT@2..3 "@"
                BLOCK@3..7
                  WHITESPACE@3..4 " "
                  LBRACE@4..5 "{"
                  WHITESPACE@5..6 " "
                  RBRACE@6..7 "}"
              RULE_SET@7..15
                SELECTOR_LIST@7..11
                  SELECTOR@7..11
                    SIMPLE_SELECTOR@7..9
                      WHITESPACE@7..8 " "
                      PERCENT@8..9 "%"
                    SIMPLE_SELECTOR@9..10
                      PERCENT@9..10 "%"
                    SIMPLE_SELECTOR@10..11
                      PERCENT@10..11 "%"
                BLOCK@11..15
                  WHITESPACE@11..12 " "
                  LBRACE@12..13 "{"
                  WHITESPACE@13..14 " "
                  RBRACE@14..15 "}"
              RULE_SET@15..34
                SELECTOR_LIST@15..18
                  SELECTOR@15..18
                    SIMPLE_SELECTOR@15..18
                      WHITESPACE@15..16 " "
                      IDENT@16..18 "h1"
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
            errors:
              9..10: expected IDENT
              10..11: expected IDENT
              12..13: expected IDENT
        "#]],
    );
}

// ── Stress: interpolation errors ───────────────────────────────────────

#[test]
fn error_unclosed_interpolation_selector() {
    check(
        "#{$tag { color: red; }",
        expect![[r##"
            SOURCE_FILE@0..22
              RULE_SET@0..22
                SELECTOR_LIST@0..6
                  SELECTOR@0..6
                    INTERPOLATION@0..6
                      HASH_LBRACE@0..2 "#{"
                      VARIABLE_REF@2..6
                        DOLLAR@2..3 "$"
                        IDENT@3..6 "tag"
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
            errors:
              7..8: expected RBRACE
        "##]],
    );
}

#[test]
fn error_unclosed_interpolation_value() {
    check(
        "p { color: #{$c; }",
        expect![[r##"
            SOURCE_FILE@0..18
              RULE_SET@0..18
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                BLOCK@1..18
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  DECLARATION@3..16
                    PROPERTY@3..9
                      WHITESPACE@3..4 " "
                      IDENT@4..9 "color"
                    COLON@9..10 ":"
                    VALUE@10..15
                      INTERPOLATION@10..15
                        WHITESPACE@10..11 " "
                        HASH_LBRACE@11..13 "#{"
                        VARIABLE_REF@13..15
                          DOLLAR@13..14 "$"
                          IDENT@14..15 "c"
                    SEMICOLON@15..16 ";"
                  WHITESPACE@16..17 " "
                  RBRACE@17..18 "}"
            errors:
              15..16: expected RBRACE
        "##]],
    );
}

#[test]
fn error_bad_expr_inside_interpolation() {
    check(
        "p { color: #{$ + }; font-size: 14px; }",
        expect![[r##"
            SOURCE_FILE@0..38
              RULE_SET@0..38
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "p"
                BLOCK@1..38
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  DECLARATION@3..19
                    PROPERTY@3..9
                      WHITESPACE@3..4 " "
                      IDENT@4..9 "color"
                    COLON@9..10 ":"
                    VALUE@10..18
                      INTERPOLATION@10..18
                        WHITESPACE@10..11 " "
                        HASH_LBRACE@11..13 "#{"
                        BINARY_EXPR@13..16
                          VARIABLE_REF@13..14
                            DOLLAR@13..14 "$"
                          WHITESPACE@14..15 " "
                          PLUS@15..16 "+"
                        WHITESPACE@16..17 " "
                        RBRACE@17..18 "}"
                    SEMICOLON@18..19 ";"
                  DECLARATION@19..36
                    PROPERTY@19..29
                      WHITESPACE@19..20 " "
                      IDENT@20..29 "font-size"
                    COLON@29..30 ":"
                    VALUE@30..35
                      DIMENSION@30..35
                        WHITESPACE@30..31 " "
                        NUMBER@31..33 "14"
                        IDENT@33..35 "px"
                    SEMICOLON@35..36 ";"
                  WHITESPACE@36..37 " "
                  RBRACE@37..38 "}"
            errors:
              15..16: expected IDENT
              17..18: expected expression
        "##]],
    );
}
