//! A quoted string left unclosed ends at the end of its line, as a CSS bad string does,
//! and is reported once; what follows it parses as usual. Dart Sass rejects it with
//! `Expected "."` (or `'`) at the newline, in both syntaxes.

mod common;
#[path = "common/layout_classes.rs"]
mod layout_classes;
#[path = "common/layout_differential.rs"]
mod layout_differential;

use expect_test::{Expect, expect};
use layout_differential::render;
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

// ── Extent ───────────────────────────────────────────────────────────

#[test]
fn ends_at_the_end_of_its_line_scss() {
    // A declaration still runs to its `;`, as CSS recovers from a bad string, so the next
    // line joins the value; the rule's `}` and everything after it are untouched.
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
                  DECLARATION@4..20
                    PROPERTY@4..8
                      WHITESPACE@4..7 "\n  "
                      IDENT@7..8 "b"
                    COLON@8..9 ":"
                    VALUE@9..19
                      STRING_LITERAL@9..12
                        WHITESPACE@9..10 " "
                        QUOTED_STRING@10..12 "\"x"
                      VALUE@12..16
                        WHITESPACE@12..15 "\n  "
                        IDENT@15..16 "c"
                      COLON@16..17 ":"
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
              DECLARATION@4..21
                PROPERTY@4..8
                  WHITESPACE@4..7 "\n  "
                  IDENT@7..8 "b"
                COLON@8..9 ":"
                VALUE@9..20
                  STRING_LITERAL@9..13
                    WHITESPACE@9..10 " "
                    QUOTED_STRING@10..13 "'x;"
                  VALUE@13..17
                    WHITESPACE@13..16 "\n  "
                    IDENT@16..17 "c"
                  COLON@17..18 ":"
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
                  DECLARATION@4..28
                    PROPERTY@4..8
                      WHITESPACE@4..7 "\n  "
                      IDENT@7..8 "b"
                    COLON@8..9 ":"
                    VALUE@9..27
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
                      VALUE@20..24
                        WHITESPACE@20..23 "\n  "
                        IDENT@23..24 "c"
                      COLON@24..25 ":"
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
                  DECLARATION@4..25
                    PROPERTY@4..8
                      WHITESPACE@4..7 "\n  "
                      IDENT@7..8 "b"
                    COLON@8..9 ":"
                    VALUE@9..24
                      INTERPOLATED_STRING@9..17
                        WHITESPACE@9..10 " "
                        STRING_START@10..13 "\"x "
                        INTERPOLATION@13..17
                          HASH_LBRACE@13..15 "#{"
                          STRING_LITERAL@15..17
                            QUOTED_STRING@15..17 "\"y"
                      VALUE@17..21
                        WHITESPACE@17..20 "\n  "
                        IDENT@20..21 "c"
                      COLON@21..22 ":"
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
    // One more error than the string: the next line is read as the next argument.
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
              DECLARATION@2..20
                PROPERTY@2..6
                  WHITESPACE@2..5 "\n  "
                  IDENT@5..6 "b"
                COLON@6..7 ":"
                VALUE@7..20
                  FUNCTION_CALL@7..17
                    WHITESPACE@7..8 " "
                    IDENT@8..9 "f"
                    ARG_LIST@9..17
                      LPAREN@9..10 "("
                      ARG@10..13
                        STRING_LITERAL@10..13
                          QUOTED_STRING@10..13 "\"x)"
                      ARG@13..17
                        VALUE@13..17
                          WHITESPACE@13..16 "\n  "
                          IDENT@16..17 "c"
                  COLON@17..18 ":"
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
        error 17..18: expected RPAREN
    "#]],
    );
    let scss = ".a {\n  b: f(\"x);\n  c: d;\n}\n.e { f: g; }\n";
    assert_eq!(errors(parse_scss, scss).len(), 2);
    assert_eq!(top_level_rules(parse_scss, scss), [".a", ".e"]);
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
                  DECLARATION@4..24
                    PROPERTY@4..8
                      WHITESPACE@4..7 "\n  "
                      IDENT@7..8 "b"
                    COLON@8..9 ":"
                    VALUE@9..23
                      SPECIAL_FUNCTION_CALL@9..16
                        WHITESPACE@9..10 " "
                        IDENT@10..13 "url"
                        LPAREN@13..14 "("
                        QUOTED_STRING@14..16 "'x"
                      VALUE@16..20
                        WHITESPACE@16..19 "\n  "
                        IDENT@19..20 "c"
                      COLON@20..21 ":"
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
    // The string takes ` {` too, so the rule has no block: one more error at its `}`.
    common::check(
        "[a=\"b] {\n  c: d;\n}\n.e { f: g; }\n",
        expect![[r#"
        SOURCE_FILE@0..32
          RULE_SET@0..15
            SELECTOR_LIST@0..15
              SELECTOR@0..15
                ATTR_SELECTOR@0..8
                  LBRACKET@0..1 "["
                  IDENT@1..2 "a"
                  EQ@2..3 "="
                  QUOTED_STRING@3..8 "\"b] {"
                SIMPLE_SELECTOR@8..12
                  WHITESPACE@8..11 "\n  "
                  IDENT@11..12 "c"
                PSEUDO_SELECTOR@12..15
                  COLON@12..13 ":"
                  WHITESPACE@13..14 " "
                  IDENT@14..15 "d"
          SEMICOLON@15..16 ";"
          ERROR@16..18
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
          17..18: expected rule
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

// ── Both indented frontends ──────────────────────────────────────────

#[test]
fn the_legacy_indented_frontend_agrees() {
    for source in [
        ".a\n  b: \"x\n  c: d\n",
        ".a\n  b: 'x\n",
        ".a\n  b: \"x #{$y} z\n  c: d\n",
        ".a\n  b: \"x #{$y}\n  c: d\n",
        "$m: (\n  a: \"x,\n  b: 2,\n)\n.e\n  f: g\n",
        "@use \"a\n@import 'b\n.e\n  f: g\n",
        ".a\n  b: f(\"x, 1,\n    2)\n.e\n  f: g\n",
    ] {
        if let Some((legacy, layout)) = layout_differential::layout_mismatch(source) {
            panic!(
                "parse_sass and parse_sass_legacy disagree on {source:?}:\n{}",
                layout_differential::first_difference(&legacy, &layout),
            );
        }
    }
}

/// A string that takes a `]` or `)` with it, or ends an interpolation, leaves legacy's count of
/// open brackets up for the rest of the file, as after any bracket left unclosed;
/// `parse_sass` recovers at the next line.
#[test]
fn legacy_does_not_recover_from_a_bracket_the_string_left_open() {
    for (source, rules) in [
        ("[a=\"b]\n  c: d\n.e\n  f: g\n", ["[a=\"b]", ".e"]),
        (".a\n  b: url(\"x\n  c: d\n.e\n  f: g\n", [".a", ".e"]),
        (
            ".a\n  b: \"x #{\"y\n// c\n  d e\n.f\n  g: h\n",
            [".a", "// c\n  d e\n.f"],
        ),
    ] {
        assert_eq!(top_level_rules(parse_sass, source), rules, "{source:?}");
        assert_eq!(
            layout_classes::classify(source),
            [layout_classes::Class::BracketRecovery],
            "{source:?}",
        );
    }
}
