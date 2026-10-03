//! `parse_sass_layout`: indented Sass with statement boundaries decided by the parser.
//!
//! Every input in `sass_lexer_tests.rs` is also parsed by both frontends there and must
//! give the same tree, unless it is on the allow-list.

use expect_test::{Expect, expect};

#[path = "common/layout_allow_list.rs"]
mod layout_allow_list;
#[path = "common/layout_differential.rs"]
mod layout_differential;

use layout_differential::render;
use sass_parser::parse_sass_layout;

#[allow(clippy::needless_pass_by_value)]
#[track_caller]
fn check(source: &str, expect: Expect) {
    expect.assert_eq(&render(parse_sass_layout, source));
}

/// The layout frontend gives the same tree as legacy here.
#[track_caller]
fn check_same(source: &str) -> String {
    if let Some((legacy, layout)) = layout_differential::layout_mismatch(source) {
        panic!(
            "parse_sass_layout and parse_sass_legacy disagree on {source:?}:\n{}",
            layout_differential::first_difference(&legacy, &layout),
        );
    }
    render(parse_sass_layout, source)
}

fn error_count(source: &str) -> usize {
    parse_sass_layout(source).1.len()
}

// ── The allow-list ───────────────────────────────────────────────────

#[test]
fn allow_listed_inputs_reach_their_target() {
    for entry in layout_allow_list::allow_list() {
        entry
            .expected
            .assert_eq(&render(parse_sass_layout, entry.input));
    }
}

#[test]
fn allow_list_has_no_duplicates() {
    let list = layout_allow_list::allow_list();
    for (i, entry) in list.iter().enumerate() {
        assert!(
            list[..i].iter().all(|other| other.input != entry.input),
            "{:?} is on the allow-list twice",
            entry.input,
        );
        assert!(
            layout_allow_list::allowed(entry.input).is_some_and(|found| !found.change.is_empty()),
            "{:?} has no description",
            entry.input,
        );
    }
}

/// The defects with a stated number of diagnostics.
#[test]
fn allow_listed_defects_report_what_they_should() {
    // A dedent to no open column, a line of the other indentation, mixed indentation.
    assert_eq!(error_count(".a\n    .b\n      c: d\n  .e\n    f: g\n"), 1);
    assert_eq!(error_count(".a\n\tcolor: red\n        b: c\n"), 1);
    assert_eq!(error_count(".a\n  \tcolor: red\n  \tb: c\n"), 1);
    // The loud comments of sass-spec's `loud.hrx`.
    let loud = layout_allow_list::allow_list()
        .into_iter()
        .find(|entry| entry.input.starts_with("/* Single-line"))
        .unwrap();
    let tree = render(parse_sass_layout, loud.input);
    assert_eq!(tree.matches("MULTI_LINE_COMMENT@").count(), 8, "{tree}");
    assert_eq!(error_count(loud.input), 0);
}

// ── Forms Dart Sass accepts ──────────────────────────────────────────

#[test]
fn dart_valid_continuations_parse_without_errors() {
    for source in [
        "$a: 1 +\n  2\n",
        "@each $a\n  , $b in (1 2)\n  .x\n    c: $a\n",
        "@function f\n  ($a)\n  @return $a\n",
        "@if\n  $a\n  b: c\n",
        "@function f($a)\n  @return $a +\n    1\n",
        "$m: (\na: 1\n)\n",
        ".a\n  b: fn(1,\n2)\n",
        "@for $i\n  from 1 through 3\n  .a\n    b: $i\n",
        ".a,\n.b\n  c: d\n",
        "$a:\n  1px\n",
    ] {
        let tree = check_same(source);
        assert!(!tree.contains("\nerror "), "{source:?}:\n{tree}");
    }
}

/// Accepted although Dart Sass rejects them; kept until the indented frontend is aligned
/// with Dart, so that only that step changes what is accepted.
#[test]
fn lenient_forms_still_parse_without_errors() {
    for source in [
        "@mixin foo($x: 1)\n  a: $x\n.a\n  @include foo\n    ($x)\n",
        "@use \"x\"\n  as y\n",
        "@media screen and\n  (min-width: 1px)\n  .a\n    b: c\n",
        "@container card (width > 400px) and\n  (height > 100px)\n  .a\n    b: c\n",
        "@container card\n  (width > 400px)\n  .a\n    b: c\n",
        "@scope (.a) to\n  (.b)\n  .c\n    d: e\n",
        ".a\n  @at-root\n    (without: media)\n    .b\n      c: d\n",
        "$b: true\n@if false\n  a: b\n@else\n  if $b\n  c: d\n",
        "$x: true\n$y: true\n@if $x\n  and $y\n  a: b\n",
    ] {
        let tree = check_same(source);
        assert!(!tree.contains("\nerror "), "{source:?}:\n{tree}");
    }
}

// ── Line breaks the grammar absorbs ──────────────────────────────────

#[test]
fn an_operator_at_the_end_of_a_line_continues_the_expression() {
    check(
        "$a: 1 +\n2\n",
        expect![[r#"
            SOURCE_FILE@0..10
              VARIABLE_DECL@0..9
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
                    WHITESPACE@7..8 "\n"
                    NUMBER@8..9 "2"
                SEMICOLON@9..9 ""
              WHITESPACE@9..10 "\n"
        "#]],
    );
}

#[test]
fn a_blank_line_ends_a_continued_expression() {
    check(
        "$a: 1 +\n\n.b\n  c: d\n",
        expect![[r#"
            SOURCE_FILE@0..19
              VARIABLE_DECL@0..7
                DOLLAR@0..1 "$"
                IDENT@1..2 "a"
                COLON@2..3 ":"
                NUMBER_LITERAL@3..5
                  WHITESPACE@3..4 " "
                  NUMBER@4..5 "1"
                UNARY_EXPR@5..7
                  WHITESPACE@5..6 " "
                  PLUS@6..7 "+"
                SEMICOLON@7..7 ""
              RULE_SET@7..18
                SELECTOR_LIST@7..11
                  SELECTOR@7..11
                    SIMPLE_SELECTOR@7..11
                      WHITESPACE@7..9 "\n\n"
                      DOT@9..10 "."
                      IDENT@10..11 "b"
                BLOCK@11..18
                  LBRACE@11..11 ""
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
            error 7..7: expected expression
        "#]],
    );
}

#[test]
fn a_comma_continues_across_a_blank_line() {
    check_same(".a,\n\n.b\n  c: d\n");
}

#[test]
fn a_selector_list_scanned_across_a_continued_line_stays_a_selector() {
    // The declaration-or-selector scan sees past the comma on the first line.
    check(
        ".x\n  a:hover,\n  b:focus\n    c: d\n",
        expect![[r#"
            SOURCE_FILE@0..33
              RULE_SET@0..32
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "x"
                BLOCK@2..32
                  LBRACE@2..2 ""
                  RULE_SET@2..32
                    SELECTOR_LIST@2..23
                      SELECTOR@2..12
                        SIMPLE_SELECTOR@2..6
                          WHITESPACE@2..5 "\n  "
                          IDENT@5..6 "a"
                        PSEUDO_SELECTOR@6..12
                          COLON@6..7 ":"
                          IDENT@7..12 "hover"
                      COMMA@12..13 ","
                      SELECTOR@13..23
                        SIMPLE_SELECTOR@13..17
                          WHITESPACE@13..16 "\n  "
                          IDENT@16..17 "b"
                        PSEUDO_SELECTOR@17..23
                          COLON@17..18 ":"
                          IDENT@18..23 "focus"
                    BLOCK@23..32
                      LBRACE@23..23 ""
                      DECLARATION@23..32
                        PROPERTY@23..29
                          WHITESPACE@23..28 "\n    "
                          IDENT@28..29 "c"
                        COLON@29..30 ":"
                        VALUE@30..32
                          VALUE@30..32
                            WHITESPACE@30..31 " "
                            IDENT@31..32 "d"
                        SEMICOLON@32..32 ""
                      RBRACE@32..32 ""
                  RBRACE@32..32 ""
              WHITESPACE@32..33 "\n"
        "#]],
    );
}

#[test]
fn a_comment_on_a_continued_line_is_an_ordinary_comment() {
    // Read as starting a statement, the comment would take the deeper line below it.
    check(
        "$a: 1 +\n  // c\n    2\n.b\n  c: $a\n",
        expect![[r#"
            SOURCE_FILE@0..32
              VARIABLE_DECL@0..20
                DOLLAR@0..1 "$"
                IDENT@1..2 "a"
                COLON@2..3 ":"
                BINARY_EXPR@3..20
                  NUMBER_LITERAL@3..5
                    WHITESPACE@3..4 " "
                    NUMBER@4..5 "1"
                  WHITESPACE@5..6 " "
                  PLUS@6..7 "+"
                  NUMBER_LITERAL@7..20
                    WHITESPACE@7..10 "\n  "
                    SINGLE_LINE_COMMENT@10..14 "// c"
                    WHITESPACE@14..19 "\n    "
                    NUMBER@19..20 "2"
                SEMICOLON@20..20 ""
              RULE_SET@20..31
                SELECTOR_LIST@20..23
                  SELECTOR@20..23
                    SIMPLE_SELECTOR@20..23
                      WHITESPACE@20..21 "\n"
                      DOT@21..22 "."
                      IDENT@22..23 "b"
                BLOCK@23..31
                  LBRACE@23..23 ""
                  DECLARATION@23..31
                    PROPERTY@23..27
                      WHITESPACE@23..26 "\n  "
                      IDENT@26..27 "c"
                    COLON@27..28 ":"
                    VALUE@28..31
                      VARIABLE_REF@28..31
                        WHITESPACE@28..29 " "
                        DOLLAR@29..30 "$"
                        IDENT@30..31 "a"
                    SEMICOLON@31..31 ""
                  RBRACE@31..31 ""
              WHITESPACE@31..32 "\n"
        "#]],
    );
}

#[test]
fn a_header_goes_on_on_a_deeper_line_that_starts_like_its_rest() {
    check(
        "@for $i\n  from 1 through 3\n  .a\n    b: $i\n",
        expect![[r#"
            SOURCE_FILE@0..42
              FOR_RULE@0..41
                AT@0..1 "@"
                IDENT@1..4 "for"
                WHITESPACE@4..5 " "
                DOLLAR@5..6 "$"
                IDENT@6..7 "i"
                WHITESPACE@7..10 "\n  "
                IDENT@10..14 "from"
                NUMBER_LITERAL@14..16
                  WHITESPACE@14..15 " "
                  NUMBER@15..16 "1"
                WHITESPACE@16..17 " "
                IDENT@17..24 "through"
                NUMBER_LITERAL@24..26
                  WHITESPACE@24..25 " "
                  NUMBER@25..26 "3"
                BLOCK@26..41
                  LBRACE@26..26 ""
                  RULE_SET@26..41
                    SELECTOR_LIST@26..31
                      SELECTOR@26..31
                        SIMPLE_SELECTOR@26..31
                          WHITESPACE@26..29 "\n  "
                          DOT@29..30 "."
                          IDENT@30..31 "a"
                    BLOCK@31..41
                      LBRACE@31..31 ""
                      DECLARATION@31..41
                        PROPERTY@31..37
                          WHITESPACE@31..36 "\n    "
                          IDENT@36..37 "b"
                        COLON@37..38 ":"
                        VALUE@38..41
                          VARIABLE_REF@38..41
                            WHITESPACE@38..39 " "
                            DOLLAR@39..40 "$"
                            IDENT@40..41 "i"
                        SEMICOLON@41..41 ""
                      RBRACE@41..41 ""
                  RBRACE@41..41 ""
              WHITESPACE@41..42 "\n"
        "#]],
    );
}

#[test]
fn shorthands_read_as_at_rules() {
    check(
        "=m\n  b: c\n.a\n  +m\n",
        expect![[r#"
            SOURCE_FILE@0..18
              MIXIN_RULE@0..9
                AT@0..0 ""
                IDENT@0..1 "="
                IDENT@1..2 "m"
                BLOCK@2..9
                  LBRACE@2..2 ""
                  DECLARATION@2..9
                    PROPERTY@2..6
                      WHITESPACE@2..5 "\n  "
                      IDENT@5..6 "b"
                    COLON@6..7 ":"
                    VALUE@7..9
                      VALUE@7..9
                        WHITESPACE@7..8 " "
                        IDENT@8..9 "c"
                    SEMICOLON@9..9 ""
                  RBRACE@9..9 ""
              RULE_SET@9..17
                SELECTOR_LIST@9..12
                  SELECTOR@9..12
                    SIMPLE_SELECTOR@9..12
                      WHITESPACE@9..10 "\n"
                      DOT@10..11 "."
                      IDENT@11..12 "a"
                BLOCK@12..17
                  LBRACE@12..12 ""
                  INCLUDE_RULE@12..17
                    WHITESPACE@12..15 "\n  "
                    AT@15..15 ""
                    IDENT@15..16 "+"
                    IDENT@16..17 "m"
                    SEMICOLON@17..17 ""
                  RBRACE@17..17 ""
              WHITESPACE@17..18 "\n"
        "#]],
    );
}

#[test]
fn a_combinator_alone_on_its_line_does_not_continue_it() {
    check_same(".a\n  >\n    b: c\n");
}
