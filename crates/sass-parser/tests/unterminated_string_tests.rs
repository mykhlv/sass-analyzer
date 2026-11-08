//! A quoted string left unclosed ends at the end of its line, as a CSS bad string does,
//! and is reported once; what follows it parses as usual. Dart Sass rejects it with
//! `Expected "."` (or `'`) at the newline, in both syntaxes.

mod common;
#[path = "common/render.rs"]
mod render;

use expect_test::{Expect, expect};
use render::render;
use sass_parser::syntax::SyntaxNode;
use sass_parser::syntax_kind::SyntaxKind;
use sass_parser::{parse_sass, parse_scss};

#[allow(clippy::needless_pass_by_value)]
#[track_caller]
fn check_sass(source: &str, expect: Expect) {
    expect.assert_eq(&render(parse_sass, source));
}

type ParseFn = fn(
    &str,
) -> (
    rowan::GreenNode,
    Vec<(String, sass_parser::text_range::TextRange)>,
);

fn errors(parse: ParseFn, source: &str) -> Vec<String> {
    parse(source).1.into_iter().map(|(msg, _)| msg).collect()
}

/// The selectors of the rules at the top level of `source`, as written.
fn top_level_rules(parse: ParseFn, source: &str) -> Vec<String> {
    SyntaxNode::new_root(parse(source).0)
        .children()
        .filter(|node| node.kind() == SyntaxKind::RULE_SET)
        .filter_map(|rule| rule.first_child())
        .map(|selector| selector.text().to_string().trim().to_owned())
        .collect()
}

/// The text of every declaration in `source`, variables and custom properties included.
fn declarations(parse: ParseFn, source: &str) -> Vec<String> {
    let root = SyntaxNode::new_root(parse(source).0);
    assert_eq!(
        root.text().to_string(),
        source,
        "lossless round-trip failed"
    );
    root.descendants()
        .filter(|node| {
            matches!(
                node.kind(),
                SyntaxKind::DECLARATION
                    | SyntaxKind::CUSTOM_PROPERTY_DECL
                    | SyntaxKind::VARIABLE_DECL
            )
        })
        .map(|node| node.text().to_string().trim().to_owned())
        .collect()
}

// ── Extent ───────────────────────────────────────────────────────────

#[test]
fn ends_at_the_end_of_its_line_scss() {
    // The declaration ends with the string's line, though the string took no `;`: the next
    // line is a declaration of its own, and the rule's `}` and what follows are untouched.
    common::check(
        ".a {\n  b: \"x\n  c: d;\n}\n",
        expect![[r#"
            SOURCE_FILE@0..23
              RULE_SET@0..22
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..22
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  DECLARATION@4..12
                    PROPERTY@4..8
                      WHITESPACE@4..7 "\n  "
                      IDENT@7..8 "b"
                    COLON@8..9 ":"
                    VALUE@9..12
                      STRING_LITERAL@9..12
                        WHITESPACE@9..10 " "
                        QUOTED_STRING@10..12 "\"x"
                  DECLARATION@12..20
                    PROPERTY@12..16
                      WHITESPACE@12..15 "\n  "
                      IDENT@15..16 "c"
                    COLON@16..17 ":"
                    VALUE@17..19
                      VALUE@17..19
                        WHITESPACE@17..18 " "
                        IDENT@18..19 "d"
                    SEMICOLON@19..20 ";"
                  WHITESPACE@20..21 "\n"
                  RBRACE@21..22 "}"
              WHITESPACE@22..23 "\n"
            errors:
              10..12: unterminated string
        "#]],
    );
}

#[test]
fn ends_at_the_end_of_its_line_sass() {
    check_sass(
        ".a\n  b: \"x\n  c: d\n",
        expect![[r#"
        SOURCE_FILE@0..18
          RULE_SET@0..17
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..17
              LBRACE@2..2 ""
              DECLARATION@2..10
                PROPERTY@2..6
                  WHITESPACE@2..5 "\n  "
                  IDENT@5..6 "b"
                COLON@6..7 ":"
                VALUE@7..10
                  STRING_LITERAL@7..10
                    WHITESPACE@7..8 " "
                    QUOTED_STRING@8..10 "\"x"
                SEMICOLON@10..10 ""
              DECLARATION@10..17
                PROPERTY@10..14
                  WHITESPACE@10..13 "\n  "
                  IDENT@13..14 "c"
                COLON@14..15 ":"
                VALUE@15..17
                  VALUE@15..17
                    WHITESPACE@15..16 " "
                    IDENT@16..17 "d"
                SEMICOLON@17..17 ""
              RBRACE@17..17 ""
          WHITESPACE@17..18 "\n"
        error 8..10: unterminated string
    "#]],
    );
}

#[test]
fn single_quotes() {
    common::check(
        ".a {\n  b: 'x;\n  c: d;\n}\n",
        expect![[r#"
        SOURCE_FILE@0..24
          RULE_SET@0..23
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..23
              WHITESPACE@2..3 " "
              LBRACE@3..4 "{"
              DECLARATION@4..13
                PROPERTY@4..8
                  WHITESPACE@4..7 "\n  "
                  IDENT@7..8 "b"
                COLON@8..9 ":"
                VALUE@9..13
                  STRING_LITERAL@9..13
                    WHITESPACE@9..10 " "
                    QUOTED_STRING@10..13 "'x;"
              DECLARATION@13..21
                PROPERTY@13..17
                  WHITESPACE@13..16 "\n  "
                  IDENT@16..17 "c"
                COLON@17..18 ":"
                VALUE@18..20
                  VALUE@18..20
                    WHITESPACE@18..19 " "
                    IDENT@19..20 "d"
                SEMICOLON@20..21 ";"
              WHITESPACE@21..22 "\n"
              RBRACE@22..23 "}"
          WHITESPACE@23..24 "\n"
        errors:
          10..13: unterminated string
    "#]],
    );
    check_sass(
        ".a\n  b: 'x\n  c: d\n",
        expect![[r#"
        SOURCE_FILE@0..18
          RULE_SET@0..17
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..17
              LBRACE@2..2 ""
              DECLARATION@2..10
                PROPERTY@2..6
                  WHITESPACE@2..5 "\n  "
                  IDENT@5..6 "b"
                COLON@6..7 ":"
                VALUE@7..10
                  STRING_LITERAL@7..10
                    WHITESPACE@7..8 " "
                    QUOTED_STRING@8..10 "'x"
                SEMICOLON@10..10 ""
              DECLARATION@10..17
                PROPERTY@10..14
                  WHITESPACE@10..13 "\n  "
                  IDENT@13..14 "c"
                COLON@14..15 ":"
                VALUE@15..17
                  VALUE@15..17
                    WHITESPACE@15..16 " "
                    IDENT@16..17 "d"
                SEMICOLON@17..17 ""
              RBRACE@17..17 ""
          WHITESPACE@17..18 "\n"
        error 8..10: unterminated string
    "#]],
    );
}

#[test]
fn the_other_quote_does_not_close_it() {
    check_sass(
        ".a\n  b: \"x'\n  c: d\n",
        expect![[r#"
        SOURCE_FILE@0..19
          RULE_SET@0..18
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..18
              LBRACE@2..2 ""
              DECLARATION@2..11
                PROPERTY@2..6
                  WHITESPACE@2..5 "\n  "
                  IDENT@5..6 "b"
                COLON@6..7 ":"
                VALUE@7..11
                  STRING_LITERAL@7..11
                    WHITESPACE@7..8 " "
                    QUOTED_STRING@8..11 "\"x'"
                SEMICOLON@11..11 ""
              DECLARATION@11..18
                PROPERTY@11..15
                  WHITESPACE@11..14 "\n  "
                  IDENT@14..15 "c"
                COLON@15..16 ":"
                VALUE@16..18
                  VALUE@16..18
                    WHITESPACE@16..17 " "
                    IDENT@17..18 "d"
                SEMICOLON@18..18 ""
              RBRACE@18..18 ""
          WHITESPACE@18..19 "\n"
        error 8..11: unterminated string
    "#]],
    );
}

#[test]
fn at_the_end_of_input_without_a_newline() {
    common::check(
        ".a {\n  b: \"x",
        expect![[r#"
        SOURCE_FILE@0..12
          RULE_SET@0..12
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..12
              WHITESPACE@2..3 " "
              LBRACE@3..4 "{"
              DECLARATION@4..12
                PROPERTY@4..8
                  WHITESPACE@4..7 "\n  "
                  IDENT@7..8 "b"
                COLON@8..9 ":"
                VALUE@9..12
                  STRING_LITERAL@9..12
                    WHITESPACE@9..10 " "
                    QUOTED_STRING@10..12 "\"x"
        errors:
          10..12: unterminated string
          12..12: expected RBRACE
    "#]],
    );
    check_sass(
        ".a\n  b: 'x",
        expect![[r#"
        SOURCE_FILE@0..10
          RULE_SET@0..10
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..10
              LBRACE@2..2 ""
              DECLARATION@2..10
                PROPERTY@2..6
                  WHITESPACE@2..5 "\n  "
                  IDENT@5..6 "b"
                COLON@6..7 ":"
                VALUE@7..10
                  STRING_LITERAL@7..10
                    WHITESPACE@7..8 " "
                    QUOTED_STRING@8..10 "'x"
                SEMICOLON@10..10 ""
              RBRACE@10..10 ""
        error 8..10: unterminated string
    "#]],
    );
}

#[test]
fn every_newline_ends_it() {
    for newline in ["\r\n", "\r", "\x0C"] {
        let source = format!(".a\n  b: \"x{newline}  c: d\n");
        assert_eq!(
            errors(parse_sass, &source),
            ["unterminated string"],
            "{newline:?}"
        );
        let source = format!(".a {{ b: \"x{newline}}}\n.e {{ f: g; }}\n");
        assert_eq!(
            errors(parse_scss, &source),
            ["unterminated string"],
            "{newline:?}"
        );
        assert_eq!(
            top_level_rules(parse_scss, &source),
            [".a", ".e"],
            "{newline:?}"
        );
    }
}

// ── Escapes ──────────────────────────────────────────────────────────

#[test]
fn an_escaped_newline_continues_it() {
    common::check(
        ".a {\n  b: \"x\\\ny\";\n}\n",
        expect![[r#"
        SOURCE_FILE@0..20
          RULE_SET@0..19
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..19
              WHITESPACE@2..3 " "
              LBRACE@3..4 "{"
              DECLARATION@4..17
                PROPERTY@4..8
                  WHITESPACE@4..7 "\n  "
                  IDENT@7..8 "b"
                COLON@8..9 ":"
                VALUE@9..16
                  STRING_LITERAL@9..16
                    WHITESPACE@9..10 " "
                    QUOTED_STRING@10..16 "\"x\\\ny\""
                SEMICOLON@16..17 ";"
              WHITESPACE@17..18 "\n"
              RBRACE@18..19 "}"
          WHITESPACE@19..20 "\n"
    "#]],
    );
    check_sass(
        ".a\n  b: \"x\\\ny\"\n  c: d\n",
        expect![[r#"
        SOURCE_FILE@0..22
          RULE_SET@0..21
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..21
              LBRACE@2..2 ""
              DECLARATION@2..14
                PROPERTY@2..6
                  WHITESPACE@2..5 "\n  "
                  IDENT@5..6 "b"
                COLON@6..7 ":"
                VALUE@7..14
                  STRING_LITERAL@7..14
                    WHITESPACE@7..8 " "
                    QUOTED_STRING@8..14 "\"x\\\ny\""
                SEMICOLON@14..14 ""
              DECLARATION@14..21
                PROPERTY@14..18
                  WHITESPACE@14..17 "\n  "
                  IDENT@17..18 "c"
                COLON@18..19 ":"
                VALUE@19..21
                  VALUE@19..21
                    WHITESPACE@19..20 " "
                    IDENT@20..21 "d"
                SEMICOLON@21..21 ""
              RBRACE@21..21 ""
          WHITESPACE@21..22 "\n"
    "#]],
    );
    assert!(errors(parse_scss, ".a { b: \"x\\\r\ny\"; }").is_empty());
}

#[test]
fn an_escaped_quote_does_not_close_it() {
    check_sass(
        ".a\n  b: \"x\\\"\n  c: d\n",
        expect![[r#"
        SOURCE_FILE@0..20
          RULE_SET@0..19
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..19
              LBRACE@2..2 ""
              DECLARATION@2..12
                PROPERTY@2..6
                  WHITESPACE@2..5 "\n  "
                  IDENT@5..6 "b"
                COLON@6..7 ":"
                VALUE@7..12
                  STRING_LITERAL@7..12
                    WHITESPACE@7..8 " "
                    QUOTED_STRING@8..12 "\"x\\\""
                SEMICOLON@12..12 ""
              DECLARATION@12..19
                PROPERTY@12..16
                  WHITESPACE@12..15 "\n  "
                  IDENT@15..16 "c"
                COLON@16..17 ":"
                VALUE@17..19
                  VALUE@17..19
                    WHITESPACE@17..18 " "
                    IDENT@18..19 "d"
                SEMICOLON@19..19 ""
              RBRACE@19..19 ""
          WHITESPACE@19..20 "\n"
        error 8..12: unterminated string
    "#]],
    );
    // An escaped backslash before the quote leaves the quote to close it.
    assert!(errors(parse_sass, ".a\n  b: \"x\\\\\"\n  c: d\n").is_empty());
    // As does a hex escape, even one taking a space after it.
    assert!(errors(parse_sass, ".a\n  b: \"\\22 \"\n  c: d\n").is_empty());
}

// ── Interpolation ────────────────────────────────────────────────────

#[test]
fn after_an_interpolation() {
    common::check(
        ".a {\n  b: \"x #{$y} z\n  c: d;\n}\n",
        expect![[r##"
            SOURCE_FILE@0..31
              RULE_SET@0..30
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..30
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  DECLARATION@4..20
                    PROPERTY@4..8
                      WHITESPACE@4..7 "\n  "
                      IDENT@7..8 "b"
                    COLON@8..9 ":"
                    VALUE@9..20
                      INTERPOLATED_STRING@9..20
                        WHITESPACE@9..10 " "
                        STRING_START@10..13 "\"x "
                        INTERPOLATION@13..18
                          HASH_LBRACE@13..15 "#{"
                          VARIABLE_REF@15..17
                            DOLLAR@15..16 "$"
                            IDENT@16..17 "y"
                          RBRACE@17..18 "}"
                        STRING_END@18..20 " z"
                  DECLARATION@20..28
                    PROPERTY@20..24
                      WHITESPACE@20..23 "\n  "
                      IDENT@23..24 "c"
                    COLON@24..25 ":"
                    VALUE@25..27
                      VALUE@25..27
                        WHITESPACE@25..26 " "
                        IDENT@26..27 "d"
                    SEMICOLON@27..28 ";"
                  WHITESPACE@28..29 "\n"
                  RBRACE@29..30 "}"
              WHITESPACE@30..31 "\n"
            errors:
              10..20: unterminated string
        "##]],
    );
    check_sass(
        ".a\n  b: \"x #{$y} z\n  c: d\n",
        expect![[r##"
        SOURCE_FILE@0..26
          RULE_SET@0..25
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..25
              LBRACE@2..2 ""
              DECLARATION@2..18
                PROPERTY@2..6
                  WHITESPACE@2..5 "\n  "
                  IDENT@5..6 "b"
                COLON@6..7 ":"
                VALUE@7..18
                  INTERPOLATED_STRING@7..18
                    WHITESPACE@7..8 " "
                    STRING_START@8..11 "\"x "
                    INTERPOLATION@11..16
                      HASH_LBRACE@11..13 "#{"
                      VARIABLE_REF@13..15
                        DOLLAR@13..14 "$"
                        IDENT@14..15 "y"
                      RBRACE@15..16 "}"
                    STRING_END@16..18 " z"
                SEMICOLON@18..18 ""
              DECLARATION@18..25
                PROPERTY@18..22
                  WHITESPACE@18..21 "\n  "
                  IDENT@21..22 "c"
                COLON@22..23 ":"
                VALUE@23..25
                  VALUE@23..25
                    WHITESPACE@23..24 " "
                    IDENT@24..25 "d"
                SEMICOLON@25..25 ""
              RBRACE@25..25 ""
          WHITESPACE@25..26 "\n"
        error 8..18: unterminated string
    "##]],
    );
}

#[test]
fn right_after_an_interpolation() {
    // Nothing of the string is left after `}` to make a token of.
    check_sass(
        ".a\n  b: \"x #{$y}\n  c: d\n",
        expect![[r##"
        SOURCE_FILE@0..24
          RULE_SET@0..23
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..23
              LBRACE@2..2 ""
              DECLARATION@2..16
                PROPERTY@2..6
                  WHITESPACE@2..5 "\n  "
                  IDENT@5..6 "b"
                COLON@6..7 ":"
                VALUE@7..16
                  INTERPOLATED_STRING@7..16
                    WHITESPACE@7..8 " "
                    STRING_START@8..11 "\"x "
                    INTERPOLATION@11..16
                      HASH_LBRACE@11..13 "#{"
                      VARIABLE_REF@13..15
                        DOLLAR@13..14 "$"
                        IDENT@14..15 "y"
                      RBRACE@15..16 "}"
                SEMICOLON@16..16 ""
              DECLARATION@16..23
                PROPERTY@16..20
                  WHITESPACE@16..19 "\n  "
                  IDENT@19..20 "c"
                COLON@20..21 ":"
                VALUE@21..23
                  VALUE@21..23
                    WHITESPACE@21..22 " "
                    IDENT@22..23 "d"
                SEMICOLON@23..23 ""
              RBRACE@23..23 ""
          WHITESPACE@23..24 "\n"
        error 8..16: unterminated string
    "##]],
    );
    assert_eq!(
        errors(parse_scss, ".a { b: \"x #{$y}\n}\n.e { f: g; }\n"),
        ["unterminated string"]
    );
}

#[test]
fn inside_an_interpolation_ends_the_string_around_it() {
    common::check(
        ".a {\n  b: \"x #{\"y\n  c: d;\n}\n.e { f: g; }\n",
        expect![[r##"
            SOURCE_FILE@0..41
              RULE_SET@0..27
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..27
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  DECLARATION@4..17
                    PROPERTY@4..8
                      WHITESPACE@4..7 "\n  "
                      IDENT@7..8 "b"
                    COLON@8..9 ":"
                    VALUE@9..17
                      INTERPOLATED_STRING@9..17
                        WHITESPACE@9..10 " "
                        STRING_START@10..13 "\"x "
                        INTERPOLATION@13..17
                          HASH_LBRACE@13..15 "#{"
                          STRING_LITERAL@15..17
                            QUOTED_STRING@15..17 "\"y"
                  DECLARATION@17..25
                    PROPERTY@17..21
                      WHITESPACE@17..20 "\n  "
                      IDENT@20..21 "c"
                    COLON@21..22 ":"
                    VALUE@22..24
                      VALUE@22..24
                        WHITESPACE@22..23 " "
                        IDENT@23..24 "d"
                    SEMICOLON@24..25 ";"
                  WHITESPACE@25..26 "\n"
                  RBRACE@26..27 "}"
              RULE_SET@27..40
                SELECTOR_LIST@27..30
                  SELECTOR@27..30
                    SIMPLE_SELECTOR@27..30
                      WHITESPACE@27..28 "\n"
                      DOT@28..29 "."
                      IDENT@29..30 "e"
                BLOCK@30..40
                  WHITESPACE@30..31 " "
                  LBRACE@31..32 "{"
                  DECLARATION@32..38
                    PROPERTY@32..34
                      WHITESPACE@32..33 " "
                      IDENT@33..34 "f"
                    COLON@34..35 ":"
                    VALUE@35..37
                      VALUE@35..37
                        WHITESPACE@35..36 " "
                        IDENT@36..37 "g"
                    SEMICOLON@37..38 ";"
                  WHITESPACE@38..39 " "
                  RBRACE@39..40 "}"
              WHITESPACE@40..41 "\n"
            errors:
              15..17: unterminated string
        "##]],
    );
    check_sass(
        ".a\n  b: \"x #{'y\n  c: d\n.e\n  f: g\n",
        expect![[r##"
        SOURCE_FILE@0..33
          RULE_SET@0..22
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..22
              LBRACE@2..2 ""
              DECLARATION@2..15
                PROPERTY@2..6
                  WHITESPACE@2..5 "\n  "
                  IDENT@5..6 "b"
                COLON@6..7 ":"
                VALUE@7..15
                  INTERPOLATED_STRING@7..15
                    WHITESPACE@7..8 " "
                    STRING_START@8..11 "\"x "
                    INTERPOLATION@11..15
                      HASH_LBRACE@11..13 "#{"
                      STRING_LITERAL@13..15
                        QUOTED_STRING@13..15 "'y"
                SEMICOLON@15..15 ""
              DECLARATION@15..22
                PROPERTY@15..19
                  WHITESPACE@15..18 "\n  "
                  IDENT@18..19 "c"
                COLON@19..20 ":"
                VALUE@20..22
                  VALUE@20..22
                    WHITESPACE@20..21 " "
                    IDENT@21..22 "d"
                SEMICOLON@22..22 ""
              RBRACE@22..22 ""
          RULE_SET@22..32
            SELECTOR_LIST@22..25
              SELECTOR@22..25
                SIMPLE_SELECTOR@22..25
                  WHITESPACE@22..23 "\n"
                  DOT@23..24 "."
                  IDENT@24..25 "e"
            BLOCK@25..32
              LBRACE@25..25 ""
              DECLARATION@25..32
                PROPERTY@25..29
                  WHITESPACE@25..28 "\n  "
                  IDENT@28..29 "f"
                COLON@29..30 ":"
                VALUE@30..32
                  VALUE@30..32
                    WHITESPACE@30..31 " "
                    IDENT@31..32 "g"
                SEMICOLON@32..32 ""
              RBRACE@32..32 ""
          WHITESPACE@32..33 "\n"
        error 13..15: unterminated string
    "##]],
    );
}

#[test]
fn the_interpolation_it_ended_does_not_keep_later_lines_in_brackets() {
    // The silent comment still reaches over the line indented beneath it, rather than
    // `d e` becoming a selector as it would inside brackets.
    let source = ".a\n  b: \"x #{\"y\n// c\n  d e\n.f\n  g: h\n";
    assert_eq!(errors(parse_sass, source), ["unterminated string"]);
    assert_eq!(
        top_level_rules(parse_sass, source),
        [".a", "// c\n  d e\n.f"]
    );
}

#[test]
fn an_interpolation_spanning_lines_is_not_a_newline_in_the_string() {
    let source = "$y: 1\n.a\n  b: \"x #{\n    $y\n  } z\"\n  c: d\n";
    assert!(errors(parse_sass, source).is_empty());
    let source = ".a {\n  b: \"x #{\n    $y\n  } z\";\n}\n";
    assert!(errors(parse_scss, source).is_empty());
}

// ── Inside brackets ──────────────────────────────────────────────────

#[test]
fn in_a_map_spanning_lines() {
    common::check(
        "$m: (\n  a: \"x,\n  b: 2,\n);\n.e { f: g; }\n",
        expect![[r#"
            SOURCE_FILE@0..39
              VARIABLE_DECL@0..25
                DOLLAR@0..1 "$"
                IDENT@1..2 "m"
                COLON@2..3 ":"
                MAP_EXPR@3..24
                  WHITESPACE@3..4 " "
                  LPAREN@4..5 "("
                  MAP_ENTRY@5..14
                    VALUE@5..9
                      WHITESPACE@5..8 "\n  "
                      IDENT@8..9 "a"
                    COLON@9..10 ":"
                    STRING_LITERAL@10..14
                      WHITESPACE@10..11 " "
                      QUOTED_STRING@11..14 "\"x,"
                  MAP_ENTRY@14..21
                    VALUE@14..18
                      WHITESPACE@14..17 "\n  "
                      IDENT@17..18 "b"
                    COLON@18..19 ":"
                    NUMBER_LITERAL@19..21
                      WHITESPACE@19..20 " "
                      NUMBER@20..21 "2"
                  COMMA@21..22 ","
                  WHITESPACE@22..23 "\n"
                  RPAREN@23..24 ")"
                SEMICOLON@24..25 ";"
              RULE_SET@25..38
                SELECTOR_LIST@25..28
                  SELECTOR@25..28
                    SIMPLE_SELECTOR@25..28
                      WHITESPACE@25..26 "\n"
                      DOT@26..27 "."
                      IDENT@27..28 "e"
                BLOCK@28..38
                  WHITESPACE@28..29 " "
                  LBRACE@29..30 "{"
                  DECLARATION@30..36
                    PROPERTY@30..32
                      WHITESPACE@30..31 " "
                      IDENT@31..32 "f"
                    COLON@32..33 ":"
                    VALUE@33..35
                      VALUE@33..35
                        WHITESPACE@33..34 " "
                        IDENT@34..35 "g"
                    SEMICOLON@35..36 ";"
                  WHITESPACE@36..37 " "
                  RBRACE@37..38 "}"
              WHITESPACE@38..39 "\n"
            errors:
              11..14: unterminated string
        "#]],
    );
    check_sass(
        "$m: (\n  a: \"x,\n  b: 2,\n)\n.e\n  f: g\n",
        expect![[r#"
        SOURCE_FILE@0..35
          VARIABLE_DECL@0..24
            DOLLAR@0..1 "$"
            IDENT@1..2 "m"
            COLON@2..3 ":"
            MAP_EXPR@3..24
              WHITESPACE@3..4 " "
              LPAREN@4..5 "("
              MAP_ENTRY@5..14
                VALUE@5..9
                  WHITESPACE@5..8 "\n  "
                  IDENT@8..9 "a"
                COLON@9..10 ":"
                STRING_LITERAL@10..14
                  WHITESPACE@10..11 " "
                  QUOTED_STRING@11..14 "\"x,"
              MAP_ENTRY@14..21
                VALUE@14..18
                  WHITESPACE@14..17 "\n  "
                  IDENT@17..18 "b"
                COLON@18..19 ":"
                NUMBER_LITERAL@19..21
                  WHITESPACE@19..20 " "
                  NUMBER@20..21 "2"
              COMMA@21..22 ","
              WHITESPACE@22..23 "\n"
              RPAREN@23..24 ")"
            SEMICOLON@24..24 ""
          RULE_SET@24..34
            SELECTOR_LIST@24..27
              SELECTOR@24..27
                SIMPLE_SELECTOR@24..27
                  WHITESPACE@24..25 "\n"
                  DOT@25..26 "."
                  IDENT@26..27 "e"
            BLOCK@27..34
              LBRACE@27..27 ""
              DECLARATION@27..34
                PROPERTY@27..31
                  WHITESPACE@27..30 "\n  "
                  IDENT@30..31 "f"
                COLON@31..32 ":"
                VALUE@32..34
                  VALUE@32..34
                    WHITESPACE@32..33 " "
                    IDENT@33..34 "g"
                SEMICOLON@34..34 ""
              RBRACE@34..34 ""
          WHITESPACE@34..35 "\n"
        error 11..14: unterminated string
    "#]],
    );
}

#[test]
fn in_an_argument_list_spanning_lines() {
    common::check(
        ".a {\n  b: f(\"x, 1,\n    2);\n}\n.e { f: g; }\n",
        expect![[r#"
            SOURCE_FILE@0..42
              RULE_SET@0..28
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..28
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  DECLARATION@4..26
                    PROPERTY@4..8
                      WHITESPACE@4..7 "\n  "
                      IDENT@7..8 "b"
                    COLON@8..9 ":"
                    VALUE@9..25
                      FUNCTION_CALL@9..25
                        WHITESPACE@9..10 " "
                        IDENT@10..11 "f"
                        ARG_LIST@11..25
                          LPAREN@11..12 "("
                          ARG@12..18
                            STRING_LITERAL@12..18
                              QUOTED_STRING@12..18 "\"x, 1,"
                          ARG@18..24
                            NUMBER_LITERAL@18..24
                              WHITESPACE@18..23 "\n    "
                              NUMBER@23..24 "2"
                          RPAREN@24..25 ")"
                    SEMICOLON@25..26 ";"
                  WHITESPACE@26..27 "\n"
                  RBRACE@27..28 "}"
              RULE_SET@28..41
                SELECTOR_LIST@28..31
                  SELECTOR@28..31
                    SIMPLE_SELECTOR@28..31
                      WHITESPACE@28..29 "\n"
                      DOT@29..30 "."
                      IDENT@30..31 "e"
                BLOCK@31..41
                  WHITESPACE@31..32 " "
                  LBRACE@32..33 "{"
                  DECLARATION@33..39
                    PROPERTY@33..35
                      WHITESPACE@33..34 " "
                      IDENT@34..35 "f"
                    COLON@35..36 ":"
                    VALUE@36..38
                      VALUE@36..38
                        WHITESPACE@36..37 " "
                        IDENT@37..38 "g"
                    SEMICOLON@38..39 ";"
                  WHITESPACE@39..40 " "
                  RBRACE@40..41 "}"
              WHITESPACE@41..42 "\n"
            errors:
              12..18: unterminated string
        "#]],
    );
    check_sass(
        ".a\n  b: f('x, 1,\n    2)\n.e\n  f: g\n",
        expect![[r#"
        SOURCE_FILE@0..34
          RULE_SET@0..23
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..23
              LBRACE@2..2 ""
              DECLARATION@2..23
                PROPERTY@2..6
                  WHITESPACE@2..5 "\n  "
                  IDENT@5..6 "b"
                COLON@6..7 ":"
                VALUE@7..23
                  FUNCTION_CALL@7..23
                    WHITESPACE@7..8 " "
                    IDENT@8..9 "f"
                    ARG_LIST@9..23
                      LPAREN@9..10 "("
                      ARG@10..16
                        STRING_LITERAL@10..16
                          QUOTED_STRING@10..16 "'x, 1,"
                      ARG@16..22
                        NUMBER_LITERAL@16..22
                          WHITESPACE@16..21 "\n    "
                          NUMBER@21..22 "2"
                      RPAREN@22..23 ")"
                SEMICOLON@23..23 ""
              RBRACE@23..23 ""
          RULE_SET@23..33
            SELECTOR_LIST@23..26
              SELECTOR@23..26
                SIMPLE_SELECTOR@23..26
                  WHITESPACE@23..24 "\n"
                  DOT@24..25 "."
                  IDENT@25..26 "e"
            BLOCK@26..33
              LBRACE@26..26 ""
              DECLARATION@26..33
                PROPERTY@26..30
                  WHITESPACE@26..29 "\n  "
                  IDENT@29..30 "f"
                COLON@30..31 ":"
                VALUE@31..33
                  VALUE@31..33
                    WHITESPACE@31..32 " "
                    IDENT@32..33 "g"
                SEMICOLON@33..33 ""
              RBRACE@33..33 ""
          WHITESPACE@33..34 "\n"
        error 10..16: unterminated string
    "#]],
    );
}

#[test]
fn in_a_list_spanning_lines() {
    let scss = "$l: (\n  \"x,\n  2\n);\n$b: [\n  \"x,\n  2\n];\n.e { f: g; }\n";
    assert_eq!(
        errors(parse_scss, scss),
        ["unterminated string", "unterminated string"]
    );
    assert_eq!(top_level_rules(parse_scss, scss), [".e"]);
}

#[test]
fn that_took_the_closing_parenthesis() {
    // The call ends at the line break, as the string took its `)`; the next line is a
    // declaration of its own.
    check_sass(
        ".a\n  b: f(\"x)\n  c: d\n.e\n  f: g\n",
        expect![[r#"
            SOURCE_FILE@0..31
              RULE_SET@0..20
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..20
                  LBRACE@2..2 ""
                  DECLARATION@2..13
                    PROPERTY@2..6
                      WHITESPACE@2..5 "\n  "
                      IDENT@5..6 "b"
                    COLON@6..7 ":"
                    VALUE@7..13
                      FUNCTION_CALL@7..13
                        WHITESPACE@7..8 " "
                        IDENT@8..9 "f"
                        ARG_LIST@9..13
                          LPAREN@9..10 "("
                          ARG@10..13
                            STRING_LITERAL@10..13
                              QUOTED_STRING@10..13 "\"x)"
                    SEMICOLON@13..13 ""
                  DECLARATION@13..20
                    PROPERTY@13..17
                      WHITESPACE@13..16 "\n  "
                      IDENT@16..17 "c"
                    COLON@17..18 ":"
                    VALUE@18..20
                      VALUE@18..20
                        WHITESPACE@18..19 " "
                        IDENT@19..20 "d"
                    SEMICOLON@20..20 ""
                  RBRACE@20..20 ""
              RULE_SET@20..30
                SELECTOR_LIST@20..23
                  SELECTOR@20..23
                    SIMPLE_SELECTOR@20..23
                      WHITESPACE@20..21 "\n"
                      DOT@21..22 "."
                      IDENT@22..23 "e"
                BLOCK@23..30
                  LBRACE@23..23 ""
                  DECLARATION@23..30
                    PROPERTY@23..27
                      WHITESPACE@23..26 "\n  "
                      IDENT@26..27 "f"
                    COLON@27..28 ":"
                    VALUE@28..30
                      VALUE@28..30
                        WHITESPACE@28..29 " "
                        IDENT@29..30 "g"
                    SEMICOLON@30..30 ""
                  RBRACE@30..30 ""
              WHITESPACE@30..31 "\n"
            error 10..13: unterminated string
        "#]],
    );
    let scss = ".a {\n  b: f(\"x);\n  c: d;\n}\n.e { f: g; }\n";
    assert_eq!(errors(parse_scss, scss), ["unterminated string"]);
    assert_eq!(top_level_rules(parse_scss, scss), [".a", ".e"]);
}

#[test]
fn that_took_the_end_of_an_include() {
    common::check(
        ".a {\n  @include m(\"x);\n  e: f;\n}\n",
        expect![[r#"
            SOURCE_FILE@0..33
              RULE_SET@0..32
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..32
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  INCLUDE_RULE@4..22
                    WHITESPACE@4..7 "\n  "
                    AT@7..8 "@"
                    IDENT@8..15 "include"
                    WHITESPACE@15..16 " "
                    IDENT@16..17 "m"
                    ARG_LIST@17..22
                      LPAREN@17..18 "("
                      ARG@18..22
                        STRING_LITERAL@18..22
                          QUOTED_STRING@18..22 "\"x);"
                  DECLARATION@22..30
                    PROPERTY@22..26
                      WHITESPACE@22..25 "\n  "
                      IDENT@25..26 "e"
                    COLON@26..27 ":"
                    VALUE@27..29
                      VALUE@27..29
                        WHITESPACE@27..28 " "
                        IDENT@28..29 "f"
                    SEMICOLON@29..30 ";"
                  WHITESPACE@30..31 "\n"
                  RBRACE@31..32 "}"
              WHITESPACE@32..33 "\n"
            errors:
              18..22: unterminated string
        "#]],
    );
    check_sass(
        ".a\n  +m(\"x)\n  e: f\n",
        expect![[r#"
        SOURCE_FILE@0..19
          RULE_SET@0..18
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..18
              LBRACE@2..2 ""
              INCLUDE_RULE@2..11
                WHITESPACE@2..5 "\n  "
                AT@5..5 ""
                IDENT@5..6 "+"
                IDENT@6..7 "m"
                ARG_LIST@7..11
                  LPAREN@7..8 "("
                  ARG@8..11
                    STRING_LITERAL@8..11
                      QUOTED_STRING@8..11 "\"x)"
                SEMICOLON@11..11 ""
              DECLARATION@11..18
                PROPERTY@11..15
                  WHITESPACE@11..14 "\n  "
                  IDENT@14..15 "e"
                COLON@15..16 ":"
                VALUE@16..18
                  VALUE@16..18
                    WHITESPACE@16..17 " "
                    IDENT@17..18 "f"
                SEMICOLON@18..18 ""
              RBRACE@18..18 ""
          WHITESPACE@18..19 "\n"
        error 8..11: unterminated string
    "#]],
    );
    // Whatever else of the statement the string took with it.
    for source in [
        "@include m(\"x\n.e { f: g; }\n",
        "@include m(\"x;\n.e { f: g; }\n",
        "@include m('x);\n.e { f: g; }\n",
        "@include m(\"x, 2);\n.e { f: g; }\n",
        "@include m($a: \"x);\n.e { f: g; }\n",
        ".a { b: calc(1px + \"x);\n  c: d;\n}\n.e { f: g; }\n",
        ".a { b: f(f(\"x), 2);\n}\n.e { f: g; }\n",
        "$l: [a \"x];\n.e { f: g; }\n",
    ] {
        assert_eq!(
            errors(parse_scss, source),
            ["unterminated string"],
            "{source:?}"
        );
        assert_eq!(
            top_level_rules(parse_scss, source)
                .last()
                .map(String::as_str),
            Some(".e"),
            "{source:?}"
        );
    }
}

#[test]
fn that_took_the_line_before_a_closing_token() {
    // The token at the start of the next line goes on with the bracket, as written.
    common::check(
        ".a {\n  b: f(\"x\n  );\n  c: d;\n}\n",
        expect![[r#"
        SOURCE_FILE@0..30
          RULE_SET@0..29
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..29
              WHITESPACE@2..3 " "
              LBRACE@3..4 "{"
              DECLARATION@4..19
                PROPERTY@4..8
                  WHITESPACE@4..7 "\n  "
                  IDENT@7..8 "b"
                COLON@8..9 ":"
                VALUE@9..18
                  FUNCTION_CALL@9..18
                    WHITESPACE@9..10 " "
                    IDENT@10..11 "f"
                    ARG_LIST@11..18
                      LPAREN@11..12 "("
                      ARG@12..14
                        STRING_LITERAL@12..14
                          QUOTED_STRING@12..14 "\"x"
                      WHITESPACE@14..17 "\n  "
                      RPAREN@17..18 ")"
                SEMICOLON@18..19 ";"
              DECLARATION@19..27
                PROPERTY@19..23
                  WHITESPACE@19..22 "\n  "
                  IDENT@22..23 "c"
                COLON@23..24 ":"
                VALUE@24..26
                  VALUE@24..26
                    WHITESPACE@24..25 " "
                    IDENT@25..26 "d"
                SEMICOLON@26..27 ";"
              WHITESPACE@27..28 "\n"
              RBRACE@28..29 "}"
          WHITESPACE@29..30 "\n"
        errors:
          12..14: unterminated string
    "#]],
    );
    common::check(
        ".a {\n  b: f(\"x\n  , 2);\n  c: d;\n}\n",
        expect![[r#"
        SOURCE_FILE@0..33
          RULE_SET@0..32
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..32
              WHITESPACE@2..3 " "
              LBRACE@3..4 "{"
              DECLARATION@4..22
                PROPERTY@4..8
                  WHITESPACE@4..7 "\n  "
                  IDENT@7..8 "b"
                COLON@8..9 ":"
                VALUE@9..21
                  FUNCTION_CALL@9..21
                    WHITESPACE@9..10 " "
                    IDENT@10..11 "f"
                    ARG_LIST@11..21
                      LPAREN@11..12 "("
                      ARG@12..14
                        STRING_LITERAL@12..14
                          QUOTED_STRING@12..14 "\"x"
                      WHITESPACE@14..17 "\n  "
                      COMMA@17..18 ","
                      ARG@18..20
                        NUMBER_LITERAL@18..20
                          WHITESPACE@18..19 " "
                          NUMBER@19..20 "2"
                      RPAREN@20..21 ")"
                SEMICOLON@21..22 ";"
              DECLARATION@22..30
                PROPERTY@22..26
                  WHITESPACE@22..25 "\n  "
                  IDENT@25..26 "c"
                COLON@26..27 ":"
                VALUE@27..29
                  VALUE@27..29
                    WHITESPACE@27..28 " "
                    IDENT@28..29 "d"
                SEMICOLON@29..30 ";"
              WHITESPACE@30..31 "\n"
              RBRACE@31..32 "}"
          WHITESPACE@32..33 "\n"
        errors:
          12..14: unterminated string
    "#]],
    );
    let source = "$l: [a \"x\n];\n.e { f: g; }\n";
    assert_eq!(errors(parse_scss, source), ["unterminated string"]);
    assert_eq!(top_level_rules(parse_scss, source), [".e"]);
}

#[test]
fn that_took_the_end_of_its_block() {
    // The block ends at the line break, with a `}` of no width, as the next line is no
    // deeper than the line the block opened on.
    common::check(
        ".a { b: \"x }\n.c { d: e; }\n",
        expect![[r#"
            SOURCE_FILE@0..26
              RULE_SET@0..12
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..12
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  DECLARATION@4..12
                    PROPERTY@4..6
                      WHITESPACE@4..5 " "
                      IDENT@5..6 "b"
                    COLON@6..7 ":"
                    VALUE@7..12
                      STRING_LITERAL@7..12
                        WHITESPACE@7..8 " "
                        QUOTED_STRING@8..12 "\"x }"
                  RBRACE@12..12 ""
              RULE_SET@12..25
                SELECTOR_LIST@12..15
                  SELECTOR@12..15
                    SIMPLE_SELECTOR@12..15
                      WHITESPACE@12..13 "\n"
                      DOT@13..14 "."
                      IDENT@14..15 "c"
                BLOCK@15..25
                  WHITESPACE@15..16 " "
                  LBRACE@16..17 "{"
                  DECLARATION@17..23
                    PROPERTY@17..19
                      WHITESPACE@17..18 " "
                      IDENT@18..19 "d"
                    COLON@19..20 ":"
                    VALUE@20..22
                      VALUE@20..22
                        WHITESPACE@20..21 " "
                        IDENT@21..22 "e"
                    SEMICOLON@22..23 ";"
                  WHITESPACE@23..24 " "
                  RBRACE@24..25 "}"
              WHITESPACE@25..26 "\n"
            errors:
              8..12: unterminated string
        "#]],
    );
    // A block opened on an earlier line, an inner one, the end of input, and a string
    // that ends after an interpolation.
    for (source, rules) in [
        (
            "a {\n  b: f(f(\"x), 2); }\n.c { d: e; }\n",
            &["a", ".c"][..],
        ),
        (".a {\n  b: \"x; }\n.c { d: e; }\n", &[".a", ".c"]),
        (".a {\n  .b { c: \"x }\n}\n.c { d: e; }\n", &[".a", ".c"]),
        (
            ".a {\n  .b { c: \"x }\n  .d { e: f; }\n}\n.c { d: e; }\n",
            &[".a", ".c"],
        ),
        (".a {\n  --b: \"x }\n.c { d: e; }\n", &[".a", ".c"]),
        (".a {\n  b: \"x }", &[".a"]),
        (".a { b: \"#{$x} }\n.c { d: e; }\n", &[".a", ".c"]),
    ] {
        assert_eq!(
            errors(parse_scss, source),
            ["unterminated string"],
            "{source:?}"
        );
        assert_eq!(top_level_rules(parse_scss, source), rules, "{source:?}");
    }
}

#[test]
fn that_took_a_closing_brace_its_block_goes_on_past() {
    // A `}` level with the line the block opened on closes it as written: the string's
    // was text.
    common::check(
        ".a {\n  content: \"}\n}\n.c { d: e; }\n",
        expect![[r#"
            SOURCE_FILE@0..34
              RULE_SET@0..20
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..20
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  DECLARATION@4..18
                    PROPERTY@4..14
                      WHITESPACE@4..7 "\n  "
                      IDENT@7..14 "content"
                    COLON@14..15 ":"
                    VALUE@15..18
                      STRING_LITERAL@15..18
                        WHITESPACE@15..16 " "
                        QUOTED_STRING@16..18 "\"}"
                  WHITESPACE@18..19 "\n"
                  RBRACE@19..20 "}"
              RULE_SET@20..33
                SELECTOR_LIST@20..23
                  SELECTOR@20..23
                    SIMPLE_SELECTOR@20..23
                      WHITESPACE@20..21 "\n"
                      DOT@21..22 "."
                      IDENT@22..23 "c"
                BLOCK@23..33
                  WHITESPACE@23..24 " "
                  LBRACE@24..25 "{"
                  DECLARATION@25..31
                    PROPERTY@25..27
                      WHITESPACE@25..26 " "
                      IDENT@26..27 "d"
                    COLON@27..28 ":"
                    VALUE@28..30
                      VALUE@28..30
                        WHITESPACE@28..29 " "
                        IDENT@29..30 "e"
                    SEMICOLON@30..31 ";"
                  WHITESPACE@31..32 " "
                  RBRACE@32..33 "}"
              WHITESPACE@33..34 "\n"
            errors:
              16..18: unterminated string
        "#]],
    );
    for source in [
        // The next line is deeper, still inside the block.
        ".a {\n  b: \"x }\n  c: d;\n}\n.c { d: e; }\n",
        // As many braces opened as closed in what the string took.
        ".a {\n  @include m(\"x) { a: b; }\n}\n.c { d: e; }\n",
        // No block to close.
        "$a: \"x }\n.c { d: e; }\n",
    ] {
        assert_eq!(
            errors(parse_scss, source),
            ["unterminated string"],
            "{source:?}"
        );
        assert_eq!(
            top_level_rules(parse_scss, source)
                .last()
                .map(String::as_str),
            Some(".c"),
            "{source:?}"
        );
    }
    // The `}` of an interpolation is the interpolation's: the block stays open.
    assert_eq!(
        errors(parse_scss, ".a { b: #{\"x }\n.c { d: e; }\n"),
        ["unterminated string", "expected RBRACE"]
    );
    // The indented syntax has no braces to put back.
    check_sass(
        ".a\n  b: \"x }\n.c\n  d: e\n",
        expect![[r#"
        SOURCE_FILE@0..23
          RULE_SET@0..12
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..12
              LBRACE@2..2 ""
              DECLARATION@2..12
                PROPERTY@2..6
                  WHITESPACE@2..5 "\n  "
                  IDENT@5..6 "b"
                COLON@6..7 ":"
                VALUE@7..12
                  STRING_LITERAL@7..12
                    WHITESPACE@7..8 " "
                    QUOTED_STRING@8..12 "\"x }"
                SEMICOLON@12..12 ""
              RBRACE@12..12 ""
          RULE_SET@12..22
            SELECTOR_LIST@12..15
              SELECTOR@12..15
                SIMPLE_SELECTOR@12..15
                  WHITESPACE@12..13 "\n"
                  DOT@13..14 "."
                  IDENT@14..15 "c"
            BLOCK@15..22
              LBRACE@15..15 ""
              DECLARATION@15..22
                PROPERTY@15..19
                  WHITESPACE@15..18 "\n  "
                  IDENT@18..19 "d"
                COLON@19..20 ":"
                VALUE@20..22
                  VALUE@20..22
                    WHITESPACE@20..21 " "
                    IDENT@21..22 "e"
                SEMICOLON@22..22 ""
              RBRACE@22..22 ""
          WHITESPACE@22..23 "\n"
        error 8..12: unterminated string
    "#]],
    );
}

#[test]
fn that_took_the_start_of_its_block() {
    // The body indented beneath the line opens with a `{` of no width, and its `}` closes
    // it.
    common::check(
        "@include m(\"x) {\n  a: b;\n}\n.c { d: e; }\n",
        expect![[r#"
            SOURCE_FILE@0..40
              INCLUDE_RULE@0..26
                AT@0..1 "@"
                IDENT@1..8 "include"
                WHITESPACE@8..9 " "
                IDENT@9..10 "m"
                ARG_LIST@10..16
                  LPAREN@10..11 "("
                  ARG@11..16
                    STRING_LITERAL@11..16
                      QUOTED_STRING@11..16 "\"x) {"
                BLOCK@16..26
                  LBRACE@16..16 ""
                  DECLARATION@16..24
                    PROPERTY@16..20
                      WHITESPACE@16..19 "\n  "
                      IDENT@19..20 "a"
                    COLON@20..21 ":"
                    VALUE@21..23
                      VALUE@21..23
                        WHITESPACE@21..22 " "
                        IDENT@22..23 "b"
                    SEMICOLON@23..24 ";"
                  WHITESPACE@24..25 "\n"
                  RBRACE@25..26 "}"
              RULE_SET@26..39
                SELECTOR_LIST@26..29
                  SELECTOR@26..29
                    SIMPLE_SELECTOR@26..29
                      WHITESPACE@26..27 "\n"
                      DOT@27..28 "."
                      IDENT@28..29 "c"
                BLOCK@29..39
                  WHITESPACE@29..30 " "
                  LBRACE@30..31 "{"
                  DECLARATION@31..37
                    PROPERTY@31..33
                      WHITESPACE@31..32 " "
                      IDENT@32..33 "d"
                    COLON@33..34 ":"
                    VALUE@34..36
                      VALUE@34..36
                        WHITESPACE@34..35 " "
                        IDENT@35..36 "e"
                    SEMICOLON@36..37 ";"
                  WHITESPACE@37..38 " "
                  RBRACE@38..39 "}"
              WHITESPACE@39..40 "\n"
            errors:
              11..16: unterminated string
        "#]],
    );
    for source in [
        ".a:not(\"x) {\n  a: b;\n}\n.c { d: e; }\n",
        ".a, .b[c=\"x] {\n  a: b;\n}\n.c { d: e; }\n",
        "@if $a == \"x {\n  a: b;\n} @else {\n  c: d;\n}\n.c { d: e; }\n",
        "@each $a in \"x {\n  a: b;\n}\n.c { d: e; }\n",
        "@mixin m($a: \"x) {\n  a: b;\n}\n.c { d: e; }\n",
        ".a {\n  @include m(\"x) {\n    a: b;\n  }\n}\n.c { d: e; }\n",
        ".a {\n  b: \"x {\n    c: d;\n  }\n}\n.c { d: e; }\n",
        // An empty body: its `}` is level with the line.
        "@include m(\"x) {\n}\n.c { d: e; }\n",
    ] {
        assert_eq!(
            errors(parse_scss, source),
            ["unterminated string"],
            "{source:?}"
        );
        assert_eq!(
            top_level_rules(parse_scss, source)
                .last()
                .map(String::as_str),
            Some(".c"),
            "{source:?}"
        );
    }
    // A body that the end of input cuts short still misses its `}`.
    assert_eq!(
        errors(parse_scss, "@include m(\"x) {\n  a: b;\n"),
        ["unterminated string", "expected RBRACE"]
    );
}

#[test]
fn that_took_an_opening_brace_with_nothing_beneath() {
    // A `}` less indented than the line closes an outer block: the string's `{` was text.
    common::check(
        ".a {\n  b: \"x {\n}\n.c { d: e; }\n",
        expect![[r#"
        SOURCE_FILE@0..30
          RULE_SET@0..16
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..16
              WHITESPACE@2..3 " "
              LBRACE@3..4 "{"
              DECLARATION@4..14
                PROPERTY@4..8
                  WHITESPACE@4..7 "\n  "
                  IDENT@7..8 "b"
                COLON@8..9 ":"
                VALUE@9..14
                  STRING_LITERAL@9..14
                    WHITESPACE@9..10 " "
                    QUOTED_STRING@10..14 "\"x {"
              WHITESPACE@14..15 "\n"
              RBRACE@15..16 "}"
          RULE_SET@16..29
            SELECTOR_LIST@16..19
              SELECTOR@16..19
                SIMPLE_SELECTOR@16..19
                  WHITESPACE@16..17 "\n"
                  DOT@17..18 "."
                  IDENT@18..19 "c"
            BLOCK@19..29
              WHITESPACE@19..20 " "
              LBRACE@20..21 "{"
              DECLARATION@21..27
                PROPERTY@21..23
                  WHITESPACE@21..22 " "
                  IDENT@22..23 "d"
                COLON@23..24 ":"
                VALUE@24..26
                  VALUE@24..26
                    WHITESPACE@24..25 " "
                    IDENT@25..26 "e"
                SEMICOLON@26..27 ";"
              WHITESPACE@27..28 " "
              RBRACE@28..29 "}"
          WHITESPACE@29..30 "\n"
        errors:
          10..14: unterminated string
    "#]],
    );
    for source in [
        ".a {\n  b: \"x {\n  c: d;\n}\n.c { d: e; }\n",
        "@include m(\"x) { a: b; }\n.c { d: e; }\n",
        "@include m(\"x) {",
    ] {
        assert_eq!(
            errors(parse_scss, source),
            ["unterminated string"],
            "{source:?}"
        );
    }
}

#[test]
fn that_took_a_comma() {
    // A comma inside what the string took does not make its line break one.
    check_sass(
        ".a\n  b: f(\"x, 2)\n  c: d\n",
        expect![[r#"
        SOURCE_FILE@0..24
          RULE_SET@0..23
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..23
              LBRACE@2..2 ""
              DECLARATION@2..16
                PROPERTY@2..6
                  WHITESPACE@2..5 "\n  "
                  IDENT@5..6 "b"
                COLON@6..7 ":"
                VALUE@7..16
                  FUNCTION_CALL@7..16
                    WHITESPACE@7..8 " "
                    IDENT@8..9 "f"
                    ARG_LIST@9..16
                      LPAREN@9..10 "("
                      ARG@10..16
                        STRING_LITERAL@10..16
                          QUOTED_STRING@10..16 "\"x, 2)"
                SEMICOLON@16..16 ""
              DECLARATION@16..23
                PROPERTY@16..20
                  WHITESPACE@16..19 "\n  "
                  IDENT@19..20 "c"
                COLON@20..21 ":"
                VALUE@21..23
                  VALUE@21..23
                    WHITESPACE@21..22 " "
                    IDENT@22..23 "d"
                SEMICOLON@23..23 ""
              RBRACE@23..23 ""
          WHITESPACE@23..24 "\n"
        error 10..16: unterminated string
    "#]],
    );
    // A trailing one is a trailing comma.
    common::check(
        ".a {\n  b: f(\"x,\n  );\n}\n",
        expect![[r#"
        SOURCE_FILE@0..23
          RULE_SET@0..22
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..22
              WHITESPACE@2..3 " "
              LBRACE@3..4 "{"
              DECLARATION@4..20
                PROPERTY@4..8
                  WHITESPACE@4..7 "\n  "
                  IDENT@7..8 "b"
                COLON@8..9 ":"
                VALUE@9..19
                  FUNCTION_CALL@9..19
                    WHITESPACE@9..10 " "
                    IDENT@10..11 "f"
                    ARG_LIST@11..19
                      LPAREN@11..12 "("
                      ARG@12..15
                        STRING_LITERAL@12..15
                          QUOTED_STRING@12..15 "\"x,"
                      WHITESPACE@15..18 "\n  "
                      RPAREN@18..19 ")"
                SEMICOLON@19..20 ";"
              WHITESPACE@20..21 "\n"
              RBRACE@21..22 "}"
          WHITESPACE@22..23 "\n"
        errors:
          12..15: unterminated string
    "#]],
    );
    // Followed by more than the bracket's next item: the next line is still read as one.
    let source = ".a\n  +m(\"x,\n  e: f\n.b\n  c: d\n";
    assert_eq!(
        errors(parse_sass, source),
        [
            "unterminated string",
            "expected RPAREN",
            "expected SEMICOLON"
        ]
    );
    assert_eq!(top_level_rules(parse_sass, source), [".a", ".b"]);
}

#[test]
fn in_brackets_opened_on_an_earlier_line() {
    // One item per line: the line break stands for the comma, not for the `)`.
    common::check(
        ".a {\n  b: f(\n    \"x\n    2\n  );\n}\n",
        expect![[r#"
        SOURCE_FILE@0..33
          RULE_SET@0..32
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..32
              WHITESPACE@2..3 " "
              LBRACE@3..4 "{"
              DECLARATION@4..30
                PROPERTY@4..8
                  WHITESPACE@4..7 "\n  "
                  IDENT@7..8 "b"
                COLON@8..9 ":"
                VALUE@9..29
                  FUNCTION_CALL@9..29
                    WHITESPACE@9..10 " "
                    IDENT@10..11 "f"
                    ARG_LIST@11..29
                      LPAREN@11..12 "("
                      ARG@12..19
                        STRING_LITERAL@12..19
                          WHITESPACE@12..17 "\n    "
                          QUOTED_STRING@17..19 "\"x"
                      ARG@19..25
                        NUMBER_LITERAL@19..25
                          WHITESPACE@19..24 "\n    "
                          NUMBER@24..25 "2"
                      WHITESPACE@25..28 "\n  "
                      RPAREN@28..29 ")"
                SEMICOLON@29..30 ";"
              WHITESPACE@30..31 "\n"
              RBRACE@31..32 "}"
          WHITESPACE@32..33 "\n"
        errors:
          17..19: unterminated string
    "#]],
    );
    check_sass(
        ".a\n  b: f(\n    \"x\n    2\n  )\n  c: d\n",
        expect![[r#"
        SOURCE_FILE@0..35
          RULE_SET@0..34
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..34
              LBRACE@2..2 ""
              DECLARATION@2..27
                PROPERTY@2..6
                  WHITESPACE@2..5 "\n  "
                  IDENT@5..6 "b"
                COLON@6..7 ":"
                VALUE@7..27
                  FUNCTION_CALL@7..27
                    WHITESPACE@7..8 " "
                    IDENT@8..9 "f"
                    ARG_LIST@9..27
                      LPAREN@9..10 "("
                      ARG@10..17
                        STRING_LITERAL@10..17
                          WHITESPACE@10..15 "\n    "
                          QUOTED_STRING@15..17 "\"x"
                      ARG@17..23
                        NUMBER_LITERAL@17..23
                          WHITESPACE@17..22 "\n    "
                          NUMBER@22..23 "2"
                      WHITESPACE@23..26 "\n  "
                      RPAREN@26..27 ")"
                SEMICOLON@27..27 ""
              DECLARATION@27..34
                PROPERTY@27..31
                  WHITESPACE@27..30 "\n  "
                  IDENT@30..31 "c"
                COLON@31..32 ":"
                VALUE@32..34
                  VALUE@32..34
                    WHITESPACE@32..33 " "
                    IDENT@33..34 "d"
                SEMICOLON@34..34 ""
              RBRACE@34..34 ""
          WHITESPACE@34..35 "\n"
        error 15..17: unterminated string
    "#]],
    );
    // Each bracket decides as it ends: `f(` opened on the string's line stops, the map
    // goes on.
    common::check(
        "$m: (\n  a: f(\"x\n  b: 2\n);\n.e { f: g; }\n",
        expect![[r#"
        SOURCE_FILE@0..39
          VARIABLE_DECL@0..25
            DOLLAR@0..1 "$"
            IDENT@1..2 "m"
            COLON@2..3 ":"
            MAP_EXPR@3..24
              WHITESPACE@3..4 " "
              LPAREN@4..5 "("
              MAP_ENTRY@5..15
                VALUE@5..9
                  WHITESPACE@5..8 "\n  "
                  IDENT@8..9 "a"
                COLON@9..10 ":"
                FUNCTION_CALL@10..15
                  WHITESPACE@10..11 " "
                  IDENT@11..12 "f"
                  ARG_LIST@12..15
                    LPAREN@12..13 "("
                    ARG@13..15
                      STRING_LITERAL@13..15
                        QUOTED_STRING@13..15 "\"x"
              MAP_ENTRY@15..22
                VALUE@15..19
                  WHITESPACE@15..18 "\n  "
                  IDENT@18..19 "b"
                COLON@19..20 ":"
                NUMBER_LITERAL@20..22
                  WHITESPACE@20..21 " "
                  NUMBER@21..22 "2"
              WHITESPACE@22..23 "\n"
              RPAREN@23..24 ")"
            SEMICOLON@24..25 ";"
          RULE_SET@25..38
            SELECTOR_LIST@25..28
              SELECTOR@25..28
                SIMPLE_SELECTOR@25..28
                  WHITESPACE@25..26 "\n"
                  DOT@26..27 "."
                  IDENT@27..28 "e"
            BLOCK@28..38
              WHITESPACE@28..29 " "
              LBRACE@29..30 "{"
              DECLARATION@30..36
                PROPERTY@30..32
                  WHITESPACE@30..31 " "
                  IDENT@31..32 "f"
                COLON@32..33 ":"
                VALUE@33..35
                  VALUE@33..35
                    WHITESPACE@33..34 " "
                    IDENT@34..35 "g"
                SEMICOLON@35..36 ";"
              WHITESPACE@36..37 " "
              RBRACE@37..38 "}"
          WHITESPACE@38..39 "\n"
        errors:
          13..15: unterminated string
    "#]],
    );
    for source in [
        "$m: (\n  a: \"x\n  b: 2\n);\n.e { f: g; }\n",
        "$m: (\n  a: (b: \"x\n  c: 2\n);\n.e { f: g; }\n",
        "$l: [\n  \"x\n  \"y\"\n];\n.e { f: g; }\n",
        ".a { b: f(1,\n  \"x\n  2); }\n.e { f: g; }\n",
        ".a { b: f(\n  \"#{1}x\n  2); }\n.e { f: g; }\n",
        ".a { b: f(\n  \"x#{1}\n  2); }\n.e { f: g; }\n",
    ] {
        assert_eq!(
            errors(parse_scss, source),
            ["unterminated string"],
            "{source:?}"
        );
        assert_eq!(
            top_level_rules(parse_scss, source)
                .last()
                .map(String::as_str),
            Some(".e"),
            "{source:?}"
        );
    }
}

#[test]
fn in_brackets_opened_on_an_earlier_line_before_a_statement() {
    // A next line shaped like a statement, no deeper than the line that opened the
    // bracket, is the statement after the string's, not the next item: `c: d` keeps its
    // declaration instead of becoming an argument of `f`.
    common::check(
        "a {\n  b: f(\n    \"x\n  c: d;\n}\n.e { f: g; }\n",
        expect![[r#"
            SOURCE_FILE@0..42
              RULE_SET@0..28
                SELECTOR_LIST@0..1
                  SELECTOR@0..1
                    SIMPLE_SELECTOR@0..1
                      IDENT@0..1 "a"
                BLOCK@1..28
                  WHITESPACE@1..2 " "
                  LBRACE@2..3 "{"
                  DECLARATION@3..18
                    PROPERTY@3..7
                      WHITESPACE@3..6 "\n  "
                      IDENT@6..7 "b"
                    COLON@7..8 ":"
                    VALUE@8..18
                      FUNCTION_CALL@8..18
                        WHITESPACE@8..9 " "
                        IDENT@9..10 "f"
                        ARG_LIST@10..18
                          LPAREN@10..11 "("
                          ARG@11..18
                            STRING_LITERAL@11..18
                              WHITESPACE@11..16 "\n    "
                              QUOTED_STRING@16..18 "\"x"
                  DECLARATION@18..26
                    PROPERTY@18..22
                      WHITESPACE@18..21 "\n  "
                      IDENT@21..22 "c"
                    COLON@22..23 ":"
                    VALUE@23..25
                      VALUE@23..25
                        WHITESPACE@23..24 " "
                        IDENT@24..25 "d"
                    SEMICOLON@25..26 ";"
                  WHITESPACE@26..27 "\n"
                  RBRACE@27..28 "}"
              RULE_SET@28..41
                SELECTOR_LIST@28..31
                  SELECTOR@28..31
                    SIMPLE_SELECTOR@28..31
                      WHITESPACE@28..29 "\n"
                      DOT@29..30 "."
                      IDENT@30..31 "e"
                BLOCK@31..41
                  WHITESPACE@31..32 " "
                  LBRACE@32..33 "{"
                  DECLARATION@33..39
                    PROPERTY@33..35
                      WHITESPACE@33..34 " "
                      IDENT@34..35 "f"
                    COLON@35..36 ":"
                    VALUE@36..38
                      VALUE@36..38
                        WHITESPACE@36..37 " "
                        IDENT@37..38 "g"
                    SEMICOLON@38..39 ";"
                  WHITESPACE@39..40 " "
                  RBRACE@40..41 "}"
              WHITESPACE@41..42 "\n"
            errors:
              16..18: unterminated string
        "#]],
    );
    for (source, declared) in [
        ("a {\n  b: f(\n    \"x\n  $c: d;\n}\n", &["$c: d;"][..]),
        (
            "a {\n  b: f(\n    \"x\n  @include m;\n  c: d;\n}\n",
            &["c: d;"],
        ),
        ("a {\n  b: f(\n    \"x\n}\n.e { f: g; }\n", &["f: g;"]),
        ("$m: (\n  a: \"x\n$c: d;\n", &["$c: d;"]),
        // Deeper than the opening line: the next item of the map, as before.
        ("$m: (\n  a: \"x\n  b: 2\n);\n$c: d;\n", &["$c: d;"]),
    ] {
        assert_eq!(
            errors(parse_scss, source),
            ["unterminated string"],
            "{source:?}"
        );
        let mut found = declarations(parse_scss, source);
        found.retain(|text| !text.starts_with("b:") && !text.starts_with("$m:"));
        assert_eq!(found, declared, "{source:?}");
    }
}

#[test]
fn in_brackets_opened_on_its_line() {
    // `f("x` ends at the line break, and the declaration with it, as when the next line is
    // a declaration of its own; what the string should have closed there is left over.
    common::check(
        ".a {\n  b: f(\"x\n  2);\n  c: d;\n}\n",
        expect![[r#"
        SOURCE_FILE@0..31
          RULE_SET@0..30
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..30
              WHITESPACE@2..3 " "
              LBRACE@3..4 "{"
              DECLARATION@4..14
                PROPERTY@4..8
                  WHITESPACE@4..7 "\n  "
                  IDENT@7..8 "b"
                COLON@8..9 ":"
                VALUE@9..14
                  FUNCTION_CALL@9..14
                    WHITESPACE@9..10 " "
                    IDENT@10..11 "f"
                    ARG_LIST@11..14
                      LPAREN@11..12 "("
                      ARG@12..14
                        STRING_LITERAL@12..14
                          QUOTED_STRING@12..14 "\"x"
              ERROR@14..18
                WHITESPACE@14..17 "\n  "
                NUMBER@17..18 "2"
              ERROR@18..19
                RPAREN@18..19 ")"
              SEMICOLON@19..20 ";"
              DECLARATION@20..28
                PROPERTY@20..24
                  WHITESPACE@20..23 "\n  "
                  IDENT@23..24 "c"
                COLON@24..25 ":"
                VALUE@25..27
                  VALUE@25..27
                    WHITESPACE@25..26 " "
                    IDENT@26..27 "d"
                SEMICOLON@27..28 ";"
              WHITESPACE@28..29 "\n"
              RBRACE@29..30 "}"
          WHITESPACE@30..31 "\n"
        errors:
          12..14: unterminated string
          17..18: expected declaration or nested rule
          18..19: expected declaration or nested rule
    "#]],
    );
    // A whole line taken in the indented syntax ends the call with the statement.
    let source = ".a\n  b: f(\"x\n  c: d\n";
    assert_eq!(errors(parse_sass, source), ["unterminated string"]);
}

#[test]
fn in_a_url() {
    // `url(` takes no arguments to read the next line as, so it ends with the string.
    check_sass(
        ".a\n  b: url(\"x\n  c: d\n.e\n  f: g\n",
        expect![[r#"
            SOURCE_FILE@0..32
              RULE_SET@0..21
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..21
                  LBRACE@2..2 ""
                  DECLARATION@2..14
                    PROPERTY@2..6
                      WHITESPACE@2..5 "\n  "
                      IDENT@5..6 "b"
                    COLON@6..7 ":"
                    VALUE@7..14
                      SPECIAL_FUNCTION_CALL@7..14
                        WHITESPACE@7..8 " "
                        IDENT@8..11 "url"
                        LPAREN@11..12 "("
                        QUOTED_STRING@12..14 "\"x"
                    SEMICOLON@14..14 ""
                  DECLARATION@14..21
                    PROPERTY@14..18
                      WHITESPACE@14..17 "\n  "
                      IDENT@17..18 "c"
                    COLON@18..19 ":"
                    VALUE@19..21
                      VALUE@19..21
                        WHITESPACE@19..20 " "
                        IDENT@20..21 "d"
                    SEMICOLON@21..21 ""
                  RBRACE@21..21 ""
              RULE_SET@21..31
                SELECTOR_LIST@21..24
                  SELECTOR@21..24
                    SIMPLE_SELECTOR@21..24
                      WHITESPACE@21..22 "\n"
                      DOT@22..23 "."
                      IDENT@23..24 "e"
                BLOCK@24..31
                  LBRACE@24..24 ""
                  DECLARATION@24..31
                    PROPERTY@24..28
                      WHITESPACE@24..27 "\n  "
                      IDENT@27..28 "f"
                    COLON@28..29 ":"
                    VALUE@29..31
                      VALUE@29..31
                        WHITESPACE@29..30 " "
                        IDENT@30..31 "g"
                    SEMICOLON@31..31 ""
                  RBRACE@31..31 ""
              WHITESPACE@31..32 "\n"
            error 12..14: unterminated string
        "#]],
    );
    common::check(
        ".a {\n  b: url('x\n  c: d;\n}\n.e { f: g; }\n",
        expect![[r#"
            SOURCE_FILE@0..40
              RULE_SET@0..26
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..26
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  DECLARATION@4..16
                    PROPERTY@4..8
                      WHITESPACE@4..7 "\n  "
                      IDENT@7..8 "b"
                    COLON@8..9 ":"
                    VALUE@9..16
                      SPECIAL_FUNCTION_CALL@9..16
                        WHITESPACE@9..10 " "
                        IDENT@10..13 "url"
                        LPAREN@13..14 "("
                        QUOTED_STRING@14..16 "'x"
                  DECLARATION@16..24
                    PROPERTY@16..20
                      WHITESPACE@16..19 "\n  "
                      IDENT@19..20 "c"
                    COLON@20..21 ":"
                    VALUE@21..23
                      VALUE@21..23
                        WHITESPACE@21..22 " "
                        IDENT@22..23 "d"
                    SEMICOLON@23..24 ";"
                  WHITESPACE@24..25 "\n"
                  RBRACE@25..26 "}"
              RULE_SET@26..39
                SELECTOR_LIST@26..29
                  SELECTOR@26..29
                    SIMPLE_SELECTOR@26..29
                      WHITESPACE@26..27 "\n"
                      DOT@27..28 "."
                      IDENT@28..29 "e"
                BLOCK@29..39
                  WHITESPACE@29..30 " "
                  LBRACE@30..31 "{"
                  DECLARATION@31..37
                    PROPERTY@31..33
                      WHITESPACE@31..32 " "
                      IDENT@32..33 "f"
                    COLON@33..34 ":"
                    VALUE@34..36
                      VALUE@34..36
                        WHITESPACE@34..35 " "
                        IDENT@35..36 "g"
                    SEMICOLON@36..37 ";"
                  WHITESPACE@37..38 " "
                  RBRACE@38..39 "}"
              WHITESPACE@39..40 "\n"
            errors:
              14..16: unterminated string
        "#]],
    );
}

// ── The statement around it ──────────────────────────────────────────

#[test]
fn that_took_the_semicolon_of_a_declaration() {
    // The declaration ends at the line break, the `;` the string took not reported again,
    // and the next line is a declaration of its own.
    common::check(
        ".a {\n  b: \"x;\n  c: d;\n}\n.e { f: g; }\n",
        expect![[r#"
            SOURCE_FILE@0..37
              RULE_SET@0..23
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..23
                  WHITESPACE@2..3 " "
                  LBRACE@3..4 "{"
                  DECLARATION@4..13
                    PROPERTY@4..8
                      WHITESPACE@4..7 "\n  "
                      IDENT@7..8 "b"
                    COLON@8..9 ":"
                    VALUE@9..13
                      STRING_LITERAL@9..13
                        WHITESPACE@9..10 " "
                        QUOTED_STRING@10..13 "\"x;"
                  DECLARATION@13..21
                    PROPERTY@13..17
                      WHITESPACE@13..16 "\n  "
                      IDENT@16..17 "c"
                    COLON@17..18 ":"
                    VALUE@18..20
                      VALUE@18..20
                        WHITESPACE@18..19 " "
                        IDENT@19..20 "d"
                    SEMICOLON@20..21 ";"
                  WHITESPACE@21..22 "\n"
                  RBRACE@22..23 "}"
              RULE_SET@23..36
                SELECTOR_LIST@23..26
                  SELECTOR@23..26
                    SIMPLE_SELECTOR@23..26
                      WHITESPACE@23..24 "\n"
                      DOT@24..25 "."
                      IDENT@25..26 "e"
                BLOCK@26..36
                  WHITESPACE@26..27 " "
                  LBRACE@27..28 "{"
                  DECLARATION@28..34
                    PROPERTY@28..30
                      WHITESPACE@28..29 " "
                      IDENT@29..30 "f"
                    COLON@30..31 ":"
                    VALUE@31..33
                      VALUE@31..33
                        WHITESPACE@31..32 " "
                        IDENT@32..33 "g"
                    SEMICOLON@33..34 ";"
                  WHITESPACE@34..35 " "
                  RBRACE@35..36 "}"
              WHITESPACE@36..37 "\n"
            errors:
              10..13: unterminated string
        "#]],
    );
    // Whatever else the string took with it, and wherever in the value it starts.
    for source in [
        ".a {\n  b: f(\"x);\n  c: d;\n}\n",
        ".a {\n  b: \"x !important;\n  c: d;\n}\n",
        ".a {\n  b: e \"x;\n  c: d;\n}\n",
        ".a {\n  font: {\n    b: \"x;\n    c: d;\n  }\n}\n",
        ".a {\n  font: 1px {\n    b: \"x;\n    c: d;\n  }\n}\n",
    ] {
        assert_eq!(
            errors(parse_scss, source),
            ["unterminated string"],
            "{source:?}"
        );
        let declared = declarations(parse_scss, source);
        assert_eq!(
            declared.last().map(String::as_str),
            Some("c: d;"),
            "{source:?}"
        );
    }
    // The indented syntax ends it at the line break anyway.
    assert_eq!(
        declarations(parse_sass, ".a\n  b: \"x;\n  c: d\n"),
        ["b: \"x;", "c: d"]
    );
}

#[test]
fn that_left_the_semicolon_to_the_next_line() {
    // A `;` starting the next line still ends the declaration it belongs to.
    common::check(
        ".a {\n  b: \"x\n  ;\n  c: d;\n}\n",
        expect![[r#"
        SOURCE_FILE@0..27
          RULE_SET@0..26
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
            BLOCK@2..26
              WHITESPACE@2..3 " "
              LBRACE@3..4 "{"
              DECLARATION@4..16
                PROPERTY@4..8
                  WHITESPACE@4..7 "\n  "
                  IDENT@7..8 "b"
                COLON@8..9 ":"
                VALUE@9..12
                  STRING_LITERAL@9..12
                    WHITESPACE@9..10 " "
                    QUOTED_STRING@10..12 "\"x"
                WHITESPACE@12..15 "\n  "
                SEMICOLON@15..16 ";"
              DECLARATION@16..24
                PROPERTY@16..20
                  WHITESPACE@16..19 "\n  "
                  IDENT@19..20 "c"
                COLON@20..21 ":"
                VALUE@21..23
                  VALUE@21..23
                    WHITESPACE@21..22 " "
                    IDENT@22..23 "d"
                SEMICOLON@23..24 ";"
              WHITESPACE@24..25 "\n"
              RBRACE@25..26 "}"
          WHITESPACE@26..27 "\n"
        errors:
          10..12: unterminated string
    "#]],
    );
}

#[test]
fn that_took_a_comma_of_a_declaration() {
    // A comma the string took goes on with the list on the next line, as in brackets: the
    // `c: d` there joins the value, with no further diagnostics.
    let source = ".a {\n  b: \"x,\n  c: d;\n}\n.e { f: g; }\n";
    assert_eq!(errors(parse_scss, source), ["unterminated string"]);
    assert_eq!(
        declarations(parse_scss, source),
        ["b: \"x,\n  c: d;", "f: g;"]
    );
}

#[test]
fn that_took_the_semicolon_of_a_custom_property() {
    // An opaque value ends at the line break all the same.
    common::check(
        ".a {\n  --b: \"x;\n  c: d;\n}\n",
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
              CUSTOM_PROPERTY_DECL@4..15
                PROPERTY@4..10
                  WHITESPACE@4..7 "\n  "
                  IDENT@7..10 "--b"
                COLON@10..11 ":"
                VALUE@11..15
                  WHITESPACE@11..12 " "
                  QUOTED_STRING@12..15 "\"x;"
              DECLARATION@15..23
                PROPERTY@15..19
                  WHITESPACE@15..18 "\n  "
                  IDENT@18..19 "c"
                COLON@19..20 ":"
                VALUE@20..22
                  VALUE@20..22
                    WHITESPACE@20..21 " "
                    IDENT@21..22 "d"
                SEMICOLON@22..23 ";"
              WHITESPACE@23..24 "\n"
              RBRACE@24..25 "}"
          WHITESPACE@25..26 "\n"
        errors:
          12..15: unterminated string
    "#]],
    );
    // Brackets opened on the string's line too, whose ends it took with the `;`...
    let source = ".a {\n  --b: f(\"x);\n  c: d;\n}\n";
    assert_eq!(errors(parse_scss, source), ["unterminated string"]);
    assert_eq!(declarations(parse_scss, source), ["--b: f(\"x);", "c: d;"]);
    // ...unless the next line closes them.
    let source = ".a {\n  --b: f(\"x\n  );\n  c: d;\n}\n";
    assert_eq!(errors(parse_scss, source), ["unterminated string"]);
    assert_eq!(
        declarations(parse_scss, source),
        ["--b: f(\"x\n  );", "c: d;"]
    );
    // A comma the string took goes on with the value.
    let source = ".a {\n  --b: \"x,\n  c: d;\n}\n";
    assert_eq!(errors(parse_scss, source), ["unterminated string"]);
    assert_eq!(declarations(parse_scss, source), ["--b: \"x,\n  c: d;"]);
}

#[test]
fn that_took_the_semicolon_of_a_statement() {
    for (source, declared) in [
        ("$a: \"x;\n$b: c;\n", &["$a: \"x;", "$b: c;"][..]),
        (
            "$a: \"x !default;\n$b: c;\n",
            &["$a: \"x !default;", "$b: c;"],
        ),
        ("@include m(\"x);\n$b: c;\n", &["$b: c;"]),
        (
            "@function f() {\n  @return \"x;\n  $b: c;\n}\n",
            &["$b: c;"],
        ),
        ("@debug \"x;\n$b: c;\n", &["$b: c;"]),
        ("@warn \"x;\n$b: c;\n", &["$b: c;"]),
        ("@error \"x;\n$b: c;\n", &["$b: c;"]),
    ] {
        assert_eq!(
            errors(parse_scss, source),
            ["unterminated string"],
            "{source:?}"
        );
        assert_eq!(declarations(parse_scss, source), declared, "{source:?}");
    }
}

#[test]
fn that_took_the_semicolon_of_an_unknown_at_rule() {
    // The prelude of an unknown at-rule runs on to the next `;` or `{`: without the
    // string's line break ending it, `.a` would read as the prelude and its block as
    // the at-rule's.
    common::check(
        "@foo \"x;\n.a { b: c; }\n",
        expect![[r#"
            SOURCE_FILE@0..22
              GENERIC_AT_RULE@0..8
                AT@0..1 "@"
                IDENT@1..4 "foo"
                WHITESPACE@4..5 " "
                QUOTED_STRING@5..8 "\"x;"
              RULE_SET@8..21
                SELECTOR_LIST@8..11
                  SELECTOR@8..11
                    SIMPLE_SELECTOR@8..11
                      WHITESPACE@8..9 "\n"
                      DOT@9..10 "."
                      IDENT@10..11 "a"
                BLOCK@11..21
                  WHITESPACE@11..12 " "
                  LBRACE@12..13 "{"
                  DECLARATION@13..19
                    PROPERTY@13..15
                      WHITESPACE@13..14 " "
                      IDENT@14..15 "b"
                    COLON@15..16 ":"
                    VALUE@16..18
                      VALUE@16..18
                        WHITESPACE@16..17 " "
                        IDENT@17..18 "c"
                    SEMICOLON@18..19 ";"
                  WHITESPACE@19..20 " "
                  RBRACE@20..21 "}"
              WHITESPACE@21..22 "\n"
            errors:
              5..8: unterminated string
        "#]],
    );
    for (source, rules, declared) in [
        ("@foo (\"x);\n.a { b: c; }\n", &[".a"][..], &["b: c;"][..]),
        ("@foo \"x\n.a { b: c; }\n", &[".a"], &["b: c;"]),
        (".r {\n  @foo \"x;\n  b: c;\n}\n", &[".r"], &["b: c;"]),
        // A comma the string took leaves the prelude going on.
        ("@foo \"x,\n  y;\n.a { b: c; }\n", &[".a"], &["b: c;"]),
    ] {
        assert_eq!(
            errors(parse_scss, source),
            ["unterminated string"],
            "{source:?}"
        );
        assert_eq!(top_level_rules(parse_scss, source), rules, "{source:?}");
        assert_eq!(declarations(parse_scss, source), declared, "{source:?}");
    }
}

// ── Selectors and at-rules ───────────────────────────────────────────

#[test]
fn in_an_attribute_selector() {
    check_sass(
        "[a=\"b]\n  c: d\n.e\n  f: g\n",
        expect![[r#"
        SOURCE_FILE@0..24
          RULE_SET@0..13
            SELECTOR_LIST@0..6
              SELECTOR@0..6
                ATTR_SELECTOR@0..6
                  LBRACKET@0..1 "["
                  IDENT@1..2 "a"
                  EQ@2..3 "="
                  QUOTED_STRING@3..6 "\"b]"
            BLOCK@6..13
              LBRACE@6..6 ""
              DECLARATION@6..13
                PROPERTY@6..10
                  WHITESPACE@6..9 "\n  "
                  IDENT@9..10 "c"
                COLON@10..11 ":"
                VALUE@11..13
                  VALUE@11..13
                    WHITESPACE@11..12 " "
                    IDENT@12..13 "d"
                SEMICOLON@13..13 ""
              RBRACE@13..13 ""
          RULE_SET@13..23
            SELECTOR_LIST@13..16
              SELECTOR@13..16
                SIMPLE_SELECTOR@13..16
                  WHITESPACE@13..14 "\n"
                  DOT@14..15 "."
                  IDENT@15..16 "e"
            BLOCK@16..23
              LBRACE@16..16 ""
              DECLARATION@16..23
                PROPERTY@16..20
                  WHITESPACE@16..19 "\n  "
                  IDENT@19..20 "f"
                COLON@20..21 ":"
                VALUE@21..23
                  VALUE@21..23
                    WHITESPACE@21..22 " "
                    IDENT@22..23 "g"
                SEMICOLON@23..23 ""
              RBRACE@23..23 ""
          WHITESPACE@23..24 "\n"
        error 3..6: unterminated string
    "#]],
    );
    // The string takes ` {` too, which the line indented beneath it puts back.
    common::check(
        "[a=\"b] {\n  c: d;\n}\n.e { f: g; }\n",
        expect![[r#"
            SOURCE_FILE@0..32
              RULE_SET@0..18
                SELECTOR_LIST@0..8
                  SELECTOR@0..8
                    ATTR_SELECTOR@0..8
                      LBRACKET@0..1 "["
                      IDENT@1..2 "a"
                      EQ@2..3 "="
                      QUOTED_STRING@3..8 "\"b] {"
                BLOCK@8..18
                  LBRACE@8..8 ""
                  DECLARATION@8..16
                    PROPERTY@8..12
                      WHITESPACE@8..11 "\n  "
                      IDENT@11..12 "c"
                    COLON@12..13 ":"
                    VALUE@13..15
                      VALUE@13..15
                        WHITESPACE@13..14 " "
                        IDENT@14..15 "d"
                    SEMICOLON@15..16 ";"
                  WHITESPACE@16..17 "\n"
                  RBRACE@17..18 "}"
              RULE_SET@18..31
                SELECTOR_LIST@18..21
                  SELECTOR@18..21
                    SIMPLE_SELECTOR@18..21
                      WHITESPACE@18..19 "\n"
                      DOT@19..20 "."
                      IDENT@20..21 "e"
                BLOCK@21..31
                  WHITESPACE@21..22 " "
                  LBRACE@22..23 "{"
                  DECLARATION@23..29
                    PROPERTY@23..25
                      WHITESPACE@23..24 " "
                      IDENT@24..25 "f"
                    COLON@25..26 ":"
                    VALUE@26..28
                      VALUE@26..28
                        WHITESPACE@26..27 " "
                        IDENT@27..28 "g"
                    SEMICOLON@28..29 ";"
                  WHITESPACE@29..30 " "
                  RBRACE@30..31 "}"
              WHITESPACE@31..32 "\n"
            errors:
              3..8: unterminated string
        "#]],
    );
}

#[test]
fn in_use_and_import_urls() {
    check_sass(
        "@use \"a\n@import 'b\n.e\n  f: g\n",
        expect![[r#"
        SOURCE_FILE@0..29
          USE_RULE@0..7
            AT@0..1 "@"
            IDENT@1..4 "use"
            WHITESPACE@4..5 " "
            QUOTED_STRING@5..7 "\"a"
            SEMICOLON@7..7 ""
          IMPORT_RULE@7..18
            WHITESPACE@7..8 "\n"
            AT@8..9 "@"
            IDENT@9..15 "import"
            WHITESPACE@15..16 " "
            QUOTED_STRING@16..18 "'b"
            SEMICOLON@18..18 ""
          RULE_SET@18..28
            SELECTOR_LIST@18..21
              SELECTOR@18..21
                SIMPLE_SELECTOR@18..21
                  WHITESPACE@18..19 "\n"
                  DOT@19..20 "."
                  IDENT@20..21 "e"
            BLOCK@21..28
              LBRACE@21..21 ""
              DECLARATION@21..28
                PROPERTY@21..25
                  WHITESPACE@21..24 "\n  "
                  IDENT@24..25 "f"
                COLON@25..26 ":"
                VALUE@26..28
                  VALUE@26..28
                    WHITESPACE@26..27 " "
                    IDENT@27..28 "g"
                SEMICOLON@28..28 ""
              RBRACE@28..28 ""
          WHITESPACE@28..29 "\n"
        error 5..7: unterminated string
        error 16..18: unterminated string
    "#]],
    );
    common::check(
        "@use \"a\n;\n@import 'b\n;\n.e { f: g; }\n",
        expect![[r#"
        SOURCE_FILE@0..36
          USE_RULE@0..9
            AT@0..1 "@"
            IDENT@1..4 "use"
            WHITESPACE@4..5 " "
            QUOTED_STRING@5..7 "\"a"
            WHITESPACE@7..8 "\n"
            SEMICOLON@8..9 ";"
          IMPORT_RULE@9..22
            WHITESPACE@9..10 "\n"
            AT@10..11 "@"
            IDENT@11..17 "import"
            WHITESPACE@17..18 " "
            QUOTED_STRING@18..20 "'b"
            WHITESPACE@20..21 "\n"
            SEMICOLON@21..22 ";"
          RULE_SET@22..35
            SELECTOR_LIST@22..25
              SELECTOR@22..25
                SIMPLE_SELECTOR@22..25
                  WHITESPACE@22..23 "\n"
                  DOT@23..24 "."
                  IDENT@24..25 "e"
            BLOCK@25..35
              WHITESPACE@25..26 " "
              LBRACE@26..27 "{"
              DECLARATION@27..33
                PROPERTY@27..29
                  WHITESPACE@27..28 " "
                  IDENT@28..29 "f"
                COLON@29..30 ":"
                VALUE@30..32
                  VALUE@30..32
                    WHITESPACE@30..31 " "
                    IDENT@31..32 "g"
                SEMICOLON@32..33 ";"
              WHITESPACE@33..34 " "
              RBRACE@34..35 "}"
          WHITESPACE@35..36 "\n"
        errors:
          5..7: unterminated string
          18..20: unterminated string
    "#]],
    );
}

#[test]
fn module_paths_of_unclosed_strings_drop_only_the_opening_quote() {
    let (green, _) = parse_sass("@use \"a/b\n");
    let tree = SyntaxNode::new_root(green);
    let use_rule = tree
        .descendants()
        .find_map(<sass_parser::ast::UseRule as sass_parser::ast::AstNode>::cast)
        .expect("a @use rule");
    assert_eq!(use_rule.module_path().as_deref(), Some("a/b"));
    assert_eq!(sass_parser::ast::unquote("\"a\\\""), "a\\\"");
    assert_eq!(sass_parser::ast::unquote("'a'"), "a");
    assert_eq!(sass_parser::ast::unquote("'"), "");
}

// ── Not a string ─────────────────────────────────────────────────────

#[test]
fn a_quote_in_a_comment_is_not_a_string() {
    assert!(errors(parse_scss, ".a { b: c; } // \"x\n.e { f: g; }\n").is_empty());
    // On the lines a silent comment reaches over, too.
    assert!(errors(parse_sass, "// a\n  \"b\n.e\n  f: g\n").is_empty());
}

// ── Brackets a string left open ──────────────────────────────────────

/// A string that takes a `]` or `)` with it, or ends an interpolation, leaves a bracket
/// open, as any bracket left unclosed does; `parse_sass` recovers at the next line.
#[test]
fn a_bracket_the_string_left_open_closes_at_the_next_line() {
    for (source, rules) in [
        ("[a=\"b]\n  c: d\n.e\n  f: g\n", ["[a=\"b]", ".e"]),
        (".a\n  b: url(\"x\n  c: d\n.e\n  f: g\n", [".a", ".e"]),
        (
            ".a\n  b: \"x #{\"y\n// c\n  d e\n.f\n  g: h\n",
            [".a", "// c\n  d e\n.f"],
        ),
    ] {
        assert_eq!(top_level_rules(parse_sass, source), rules, "{source:?}");
    }
}
