//! A declaration's value holds no `:`, `)` or `]` of its own: Dart Sass expects the
//! declaration to end there (`expected ";"`, or `expected newline` in the indented
//! syntax). That is one error, which takes the rest of the value, and the statements
//! after it parse as usual. Verified against Dart Sass 1.105.1.

mod common;
#[path = "common/render.rs"]
mod render;

use common::check;
use expect_test::{Expect, expect};

#[allow(clippy::needless_pass_by_value)]
#[track_caller]
fn check_sass(source: &str, expect: Expect) {
    expect.assert_eq(&render::render(sass_parser::parse_sass, source));
}

fn errors(parse: render::ParseFn, source: &str) -> Vec<String> {
    parse(source).1.into_iter().map(|(msg, _)| msg).collect()
}

// ── Failing, with recovery ──────────────────────────────────────────────

#[test]
fn stray_paren_ends_the_value() {
    check(
        ".b { c: d) }\n.e { f: g }\n",
        expect![[r#"
        SOURCE_FILE@0..25
          RULE_SET@0..12
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "b"
            BLOCK@2..12
              WHITESPACE@2..3 " "
              LBRACE@3..4 "{"
              DECLARATION@4..10
                PROPERTY@4..6
                  WHITESPACE@4..5 " "
                  IDENT@5..6 "c"
                COLON@6..7 ":"
                VALUE@7..10
                  VALUE@7..9
                    WHITESPACE@7..8 " "
                    IDENT@8..9 "d"
                  ERROR@9..10
                    RPAREN@9..10 ")"
              WHITESPACE@10..11 " "
              RBRACE@11..12 "}"
          RULE_SET@12..24
            SELECTOR_LIST@12..15
              SELECTOR@12..15
                SIMPLE_SELECTOR@12..15
                  WHITESPACE@12..13 "\n"
                  DOT@13..14 "."
                  IDENT@14..15 "e"
            BLOCK@15..24
              WHITESPACE@15..16 " "
              LBRACE@16..17 "{"
              DECLARATION@17..22
                PROPERTY@17..19
                  WHITESPACE@17..18 " "
                  IDENT@18..19 "f"
                COLON@19..20 ":"
                VALUE@20..22
                  VALUE@20..22
                    WHITESPACE@20..21 " "
                    IDENT@21..22 "g"
              WHITESPACE@22..23 " "
              RBRACE@23..24 "}"
          WHITESPACE@24..25 "\n"
        errors:
          9..10: expected SEMICOLON
    "#]],
    );
}

#[test]
fn colon_in_a_value_ends_it() {
    check(
        "a { b: c:d; e: f; }\n",
        expect![[r#"
        SOURCE_FILE@0..20
          RULE_SET@0..19
            SELECTOR_LIST@0..1
              SELECTOR@0..1
                SIMPLE_SELECTOR@0..1
                  IDENT@0..1 "a"
            BLOCK@1..19
              WHITESPACE@1..2 " "
              LBRACE@2..3 "{"
              DECLARATION@3..11
                PROPERTY@3..5
                  WHITESPACE@3..4 " "
                  IDENT@4..5 "b"
                COLON@5..6 ":"
                VALUE@6..10
                  VALUE@6..8
                    WHITESPACE@6..7 " "
                    IDENT@7..8 "c"
                  ERROR@8..10
                    COLON@8..9 ":"
                    IDENT@9..10 "d"
                SEMICOLON@10..11 ";"
              DECLARATION@11..17
                PROPERTY@11..13
                  WHITESPACE@11..12 " "
                  IDENT@12..13 "e"
                COLON@13..14 ":"
                VALUE@14..16
                  VALUE@14..16
                    WHITESPACE@14..15 " "
                    IDENT@15..16 "f"
                SEMICOLON@16..17 ";"
              WHITESPACE@17..18 " "
              RBRACE@18..19 "}"
          WHITESPACE@19..20 "\n"
        errors:
          8..9: expected SEMICOLON
    "#]],
    );
}

#[test]
fn colon_as_the_value_is_a_missing_expression() {
    // Dart Sass: `Expected expression.`
    check(
        "a { b: :d; e: f; }\n",
        expect![[r#"
        SOURCE_FILE@0..19
          RULE_SET@0..18
            SELECTOR_LIST@0..1
              SELECTOR@0..1
                SIMPLE_SELECTOR@0..1
                  IDENT@0..1 "a"
            BLOCK@1..18
              WHITESPACE@1..2 " "
              LBRACE@2..3 "{"
              DECLARATION@3..10
                PROPERTY@3..5
                  WHITESPACE@3..4 " "
                  IDENT@4..5 "b"
                COLON@5..6 ":"
                VALUE@6..9
                  ERROR@6..9
                    WHITESPACE@6..7 " "
                    COLON@7..8 ":"
                    IDENT@8..9 "d"
                SEMICOLON@9..10 ";"
              DECLARATION@10..16
                PROPERTY@10..12
                  WHITESPACE@10..11 " "
                  IDENT@11..12 "e"
                COLON@12..13 ":"
                VALUE@13..15
                  VALUE@13..15
                    WHITESPACE@13..14 " "
                    IDENT@14..15 "f"
                SEMICOLON@15..16 ";"
              WHITESPACE@16..17 " "
              RBRACE@17..18 "}"
          WHITESPACE@18..19 "\n"
        errors:
          7..8: expected expression
    "#]],
    );
}

#[test]
fn name_and_colon_after_a_value_is_the_next_declaration() {
    // Dart Sass: `expected ";"`, at the second colon.
    check(
        "a { b: c d: e; f: g; }\n",
        expect![[r#"
        SOURCE_FILE@0..23
          RULE_SET@0..22
            SELECTOR_LIST@0..1
              SELECTOR@0..1
                SIMPLE_SELECTOR@0..1
                  IDENT@0..1 "a"
            BLOCK@1..22
              WHITESPACE@1..2 " "
              LBRACE@2..3 "{"
              DECLARATION@3..8
                PROPERTY@3..5
                  WHITESPACE@3..4 " "
                  IDENT@4..5 "b"
                COLON@5..6 ":"
                VALUE@6..8
                  VALUE@6..8
                    WHITESPACE@6..7 " "
                    IDENT@7..8 "c"
              DECLARATION@8..14
                PROPERTY@8..10
                  WHITESPACE@8..9 " "
                  IDENT@9..10 "d"
                COLON@10..11 ":"
                VALUE@11..13
                  VALUE@11..13
                    WHITESPACE@11..12 " "
                    IDENT@12..13 "e"
                SEMICOLON@13..14 ";"
              DECLARATION@14..20
                PROPERTY@14..16
                  WHITESPACE@14..15 " "
                  IDENT@15..16 "f"
                COLON@16..17 ":"
                VALUE@17..19
                  VALUE@17..19
                    WHITESPACE@17..18 " "
                    IDENT@18..19 "g"
                SEMICOLON@19..20 ";"
              WHITESPACE@20..21 " "
              RBRACE@21..22 "}"
          WHITESPACE@22..23 "\n"
        errors:
          9..10: expected SEMICOLON
    "#]],
    );
}

#[test]
fn stray_paren_ends_the_line_in_the_indented_syntax() {
    check_sass(
        ".b\n  c: d)\n  e: f\n.g\n  h: i\n",
        expect![[r#"
        SOURCE_FILE@0..28
          RULE_SET@0..17
            SELECTOR_LIST@0..2
              SELECTOR@0..2
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "b"
            BLOCK@2..17
              LBRACE@2..2 ""
              DECLARATION@2..10
                PROPERTY@2..6
                  WHITESPACE@2..5 "\n  "
                  IDENT@5..6 "c"
                COLON@6..7 ":"
                VALUE@7..10
                  VALUE@7..9
                    WHITESPACE@7..8 " "
                    IDENT@8..9 "d"
                  ERROR@9..10
                    RPAREN@9..10 ")"
                SEMICOLON@10..10 ""
              DECLARATION@10..17
                PROPERTY@10..14
                  WHITESPACE@10..13 "\n  "
                  IDENT@13..14 "e"
                COLON@14..15 ":"
                VALUE@15..17
                  VALUE@15..17
                    WHITESPACE@15..16 " "
                    IDENT@16..17 "f"
                SEMICOLON@17..17 ""
              RBRACE@17..17 ""
          RULE_SET@17..27
            SELECTOR_LIST@17..20
              SELECTOR@17..20
                SIMPLE_SELECTOR@17..20
                  WHITESPACE@17..18 "\n"
                  DOT@18..19 "."
                  IDENT@19..20 "g"
            BLOCK@20..27
              LBRACE@20..20 ""
              DECLARATION@20..27
                PROPERTY@20..24
                  WHITESPACE@20..23 "\n  "
                  IDENT@23..24 "h"
                COLON@24..25 ":"
                VALUE@25..27
                  VALUE@25..27
                    WHITESPACE@25..26 " "
                    IDENT@26..27 "i"
                SEMICOLON@27..27 ""
              RBRACE@27..27 ""
          WHITESPACE@27..28 "\n"
        error 9..10: expected newline
    "#]],
    );
}

#[test]
fn colon_in_a_value_ends_the_line_in_the_indented_syntax() {
    check_sass(
        "a\n  b: c:d\n  e: f\n",
        expect![[r#"
        SOURCE_FILE@0..18
          RULE_SET@0..17
            SELECTOR_LIST@0..1
              SELECTOR@0..1
                SIMPLE_SELECTOR@0..1
                  IDENT@0..1 "a"
            BLOCK@1..17
              LBRACE@1..1 ""
              DECLARATION@1..10
                PROPERTY@1..5
                  WHITESPACE@1..4 "\n  "
                  IDENT@4..5 "b"
                COLON@5..6 ":"
                VALUE@6..10
                  VALUE@6..8
                    WHITESPACE@6..7 " "
                    IDENT@7..8 "c"
                  ERROR@8..10
                    COLON@8..9 ":"
                    IDENT@9..10 "d"
                SEMICOLON@10..10 ""
              DECLARATION@10..17
                PROPERTY@10..14
                  WHITESPACE@10..13 "\n  "
                  IDENT@13..14 "e"
                COLON@14..15 ":"
                VALUE@15..17
                  VALUE@15..17
                    WHITESPACE@15..16 " "
                    IDENT@16..17 "f"
                SEMICOLON@17..17 ""
              RBRACE@17..17 ""
          WHITESPACE@17..18 "\n"
        error 8..9: expected newline
    "#]],
    );
}

#[test]
fn dot_that_starts_no_number_ends_the_value() {
    // Dart Sass: `Expected digit.` at the `.`; joined to a word, `expected "("`.
    assert_eq!(
        errors(sass_parser::parse_scss, "a { b: c .d; e: f; }\n"),
        ["expected digit"]
    );
    assert_eq!(
        errors(sass_parser::parse_scss, "a { b: c.d; e: f; }\n"),
        ["expected `(`"]
    );
    assert_eq!(
        errors(
            sass_parser::parse_scss,
            "a { b: 1.5 .5; c: math.div(1, 2); }\n"
        ),
        [] as [String; 0]
    );
}

// ── Valid, untouched ────────────────────────────────────────────────────

#[test]
fn colons_that_values_hold_are_no_error() {
    for source in [
        "a { b: c; }\n",
        "a { filter: progid:DXImageTransform.Microsoft.gradient(startColorstr=#fff, endColorstr=#000); }\n",
        "a { filter: alpha(opacity=50) progid:DXImageTransform.Microsoft.Alpha(Opacity=80); }\n",
        "a { b: url(a:b); }\n",
        "a { b: url(\"a:b\"); }\n",
        "a { b: \"c:d\"; }\n",
        "a { b: c\\:d; }\n",
        "a { --x: a:b; }\n",
        "a { b: c { d: e; } }\n",
        "@supports (a: b) { c { d: e } }\n",
        "@media (min-width: 1px) and (max-width: 2px) { a { b: c } }\n",
        "a { b: if(sass($c): d; else: e); }\n",
        "a { &:hover { b: c } }\n",
        "a { b:c:hover { d: e } }\n",
    ] {
        assert_eq!(
            errors(sass_parser::parse_scss, source),
            [] as [String; 0],
            "{source}"
        );
    }
    for source in [
        "a\n  filter: progid:DXImageTransform.Microsoft.Alpha(Opacity=80)\n",
        "a\n  b: url(c:d)\n",
        "a\n  --x: a:b\n",
        "a\n  b:c:d\n",
        "a\n  b:c:hover\n    d: e\n",
    ] {
        assert_eq!(
            errors(sass_parser::parse_sass, source),
            [] as [String; 0],
            "{source}"
        );
    }
}
