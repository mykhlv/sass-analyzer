//! `parse_sass_layout`: indented Sass with statement boundaries decided by the parser.
//!
//! Every input in `sass_lexer_tests.rs` is also parsed by both frontends there and must
//! give the same tree, unless it is on the allow-list.

use expect_test::{Expect, expect};

#[path = "common/layout_allow_list.rs"]
mod layout_allow_list;
#[path = "common/layout_classes.rs"]
mod layout_classes;
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
    // An unclosed bracket: one error, and the rules after it parse.
    let source = ".a\n  b: fn(1\n.c\n  d: e\n.f\n  g: h\n";
    assert_eq!(error_count(source), 1);
    let tree = render(parse_sass_layout, source);
    assert_eq!(tree.matches("RULE_SET@").count(), 3, "{tree}");
    assert_eq!(tree.matches("DECLARATION@").count(), 3, "{tree}");
    // An unterminated loud comment: the error before it, and the comment itself.
    assert_eq!(
        error_count("$a: 1 +\n  /* c\n    d\n    2\n.b\n  c: $a\n"),
        2
    );
    assert_eq!(error_count(".a /* x\n  y\n.b\n  c: d\n"), 1);
    // The loud comments of sass-spec's `loud.hrx`.
    let loud = layout_allow_list::allow_list()
        .into_iter()
        .find(|entry| entry.input.starts_with("/* Single-line"))
        .unwrap();
    let tree = render(parse_sass_layout, loud.input);
    assert_eq!(tree.matches("MULTI_LINE_COMMENT@").count(), 8, "{tree}");
    assert_eq!(error_count(loud.input), 0);
}

// ── Kinds of difference ──────────────────────────────────────────────

/// The kinds the differences on the fuzz corpora must be of, each told on a small input
/// that makes it; all of these but the comment across a blank line are inputs Dart Sass
/// rejects. An input can make several, found one after another.
#[test]
fn differences_are_told_apart_by_kind() {
    use layout_classes::Class::{
        AtRuleAfterABrokenStatement, BracketRecovery, CommentAcrossBlankLine, DedentToNoOpenBlock,
        IndentationDiagnostics, OperatorContinuesALine, OperatorOrColonEndsALine,
        StrayColonEndsALine, TrailingIndentation, UnbalancedBrackets, UnterminatedComment,
        VariableNamedLikeAHeaderWord, WordOperatorBeforeAStatement,
    };
    for (source, kinds) in [
        (".a\n  b: c\n    ", vec![TrailingIndentation]),
        (".a\n  b: c /* d\n", vec![UnterminatedComment]),
        (
            ".a\n  b: c /* d\n  ",
            vec![TrailingIndentation, UnterminatedComment],
        ),
        (
            ".a\n    .b\n      c: d\n  .e\n    f: g\n",
            vec![IndentationDiagnostics],
        ),
        (
            ".a\n  b: c and\n    d: e\n",
            vec![WordOperatorBeforeAStatement],
        ),
        (
            ".x\n  @if $and\n    a: b\n",
            vec![VariableNamedLikeAHeaderWord],
        ),
        (".a\n  b: fn(1\n.c\n  d: e\n", vec![BracketRecovery]),
        ("\x0e[:\n}+\n0", vec![UnbalancedBrackets]),
        ("$||{z:\x0c,,", vec![UnbalancedBrackets]),
        ("//\n\n //\n 4", vec![CommentAcrossBlankLine]),
        ("$0*\n", vec![OperatorContinuesALine]),
        ("$||:\x0c", vec![StrayColonEndsALine]),
        (":*\n", vec![OperatorOrColonEndsALine]),
        ("=\n\n\t//\n", vec![OperatorOrColonEndsALine]),
        ("$+*//\n", vec![OperatorOrColonEndsALine]),
        ("$@\n ''", vec![AtRuleAfterABrokenStatement]),
        ("$\n\t //\n #\n", vec![DedentToNoOpenBlock]),
        ("@:*\n", vec![OperatorOrColonEndsALine]),
    ] {
        assert_eq!(layout_classes::classify(source), kinds, "{source:?}");
    }
    assert!(layout_classes::classify(".a\n  b: c\n").is_empty());
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

/// Accepted although Dart Sass rejects them, or reads them otherwise (a trailing comma
/// before a line at the same column); kept until the indented frontend is aligned with
/// Dart, so that only that step changes what is accepted. Each is marked `lenient:` in
/// the grammar.
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
        ".a\n  b: c,\n    d\n",
        ".a\n  b: c,\n  d\n",
        "$a: 1,\n  2\n",
        "@function f()\n  @return 1,\n    2\n",
        "$a: 1\n  2\n",
        "$a: 1\n  , 2\n",
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

/// A declaration's value reads `and`, `or` and `not` as words, but Dart Sass reads them
/// as operators: after `and` or `or` with an operand before it, or after `not`, the value
/// goes on on the next line, as it did in legacy.
#[test]
fn a_word_operator_at_the_end_of_a_value_continues_it() {
    for source in [
        ".a\n  b: c and\n    d\n",
        ".a\n  b: c and\n  d\n",
        ".a\n  b: c or\n    d\n",
        ".a\n  b: 1 and\n    2\n",
        ".a\n  b: c not\n    d\n",
        ".a\n  content: not\n    d\n",
        ".a\n  b: c and\n    d e\n",
        ".a\n  b: c, d and\n  e\n",
        ".a\n  b: c and\n    d\n  e: f\n",
    ] {
        let tree = check_same(source);
        assert!(!tree.contains("\nerror "), "{source:?}:\n{tree}");
    }
    check(
        ".a\n  b: c and\n    d\n",
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
                  DECLARATION@2..19
                    PROPERTY@2..6
                      WHITESPACE@2..5 "\n  "
                      IDENT@5..6 "b"
                    COLON@6..7 ":"
                    VALUE@7..19
                      VALUE@7..9
                        WHITESPACE@7..8 " "
                        IDENT@8..9 "c"
                      VALUE@9..13
                        WHITESPACE@9..10 " "
                        IDENT@10..13 "and"
                      VALUE@13..19
                        WHITESPACE@13..18 "\n    "
                        IDENT@18..19 "d"
                    SEMICOLON@19..19 ""
                  RBRACE@19..19 ""
              WHITESPACE@19..20 "\n"
        "#]],
    );
}

/// The failing cases: a line that starts like a declaration is no operand, and `and` or
/// `or` with nothing before it is a plain word. Both end the value, and the next line
/// has a property of its own, with no diagnostic yet (Dart reports `expected newline.`
/// for the first).
#[test]
fn a_word_operator_does_not_take_a_declaration_into_the_value() {
    for source in [
        ".a\n  b: c and\n    d: e\n",
        ".a\n  b: c or\n  d: e\n",
        ".a\n  content: not\n  b: c\n",
        ".a\n  content: and\n  b: c\n",
    ] {
        let tree = render(parse_sass_layout, source);
        assert_eq!(tree.matches(" PROPERTY@").count(), 2, "{source:?}:\n{tree}");
        assert!(!tree.contains("\nerror "), "{source:?}:\n{tree}");
    }
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

/// Every list the grammar reads outside brackets goes on after a comma at the end of a
/// line, with or without a blank line after it, as in legacy.
#[test]
fn every_comma_separated_list_goes_on_after_a_comma() {
    for source in [
        ".a,\n  .b\n  c: d\n",
        ".a\n  @extend .b,\n    .c\n",
        "@each $a,\n  $b in c\n  d: e\n",
        "@each $a in b,\n  c\n  d: e\n",
        ".a\n  b: c,\n    d\n",
        "$a: 1,\n  2\n",
        "@use \"a\" show b,\n  c\n",
        "@import \"a\",\n  \"b\"\n",
        "@layer a,\n  b\n",
        "@keyframes k\n  10%,\n    20%\n    a: b\n",
        "@foo\n  10%,\n  20%\n    a: b\n",
        "@media screen,\n  print\n  .a\n    b: c\n",
        "@foo a,\n  b\n  c: d\n",
        ".a\n  --x: a,\n    b\n",
    ] {
        check_same(source);
        check_same(&source.replacen(",\n", ",\n\n", 1));
    }
}

/// The keyframe selectors of an unknown at-rule's block: the comma joins `20%` to `10%`,
/// and the block below belongs to both.
#[test]
fn a_comma_continues_a_keyframe_list_in_an_unknown_at_rule() {
    check_same("@foo\n  10%,\n  20%\n    a: b\n");
    check(
        "@foo\n  10%,\n  20%\n    a: b\n",
        expect![[r#"
            SOURCE_FILE@0..27
              GENERIC_AT_RULE@0..26
                AT@0..1 "@"
                IDENT@1..4 "foo"
                BLOCK@4..26
                  LBRACE@4..4 ""
                  KEYFRAME_SELECTOR@4..26
                    WHITESPACE@4..7 "\n  "
                    NUMBER@7..9 "10"
                    PERCENT@9..10 "%"
                    COMMA@10..11 ","
                    WHITESPACE@11..14 "\n  "
                    NUMBER@14..16 "20"
                    PERCENT@16..17 "%"
                    BLOCK@17..26
                      LBRACE@17..17 ""
                      DECLARATION@17..26
                        PROPERTY@17..23
                          WHITESPACE@17..22 "\n    "
                          IDENT@22..23 "a"
                        COLON@23..24 ":"
                        VALUE@24..26
                          VALUE@24..26
                            WHITESPACE@24..25 " "
                            IDENT@25..26 "b"
                        SEMICOLON@26..26 ""
                      RBRACE@26..26 ""
                  RBRACE@26..26 ""
              WHITESPACE@26..27 "\n"
        "#]],
    );
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

/// A header goes on after one of its words at the end of a line; a variable named like
/// one is no such word, and the line below is the rule's body, as in Dart Sass.
#[test]
fn a_variable_named_like_a_header_word_ends_the_header() {
    check_same(".x\n  @for $i from 1 through\n    3\n    a: $i\n");
    check_same(".x\n  @each $v in\n    1 2\n    a: $v\n");
    for (source, rule) in [
        (".x\n  @for $i from 1 through $to\n    a: $i\n", "FOR_RULE@"),
        (".x\n  @for $i from 1 to $through\n    a: $i\n", "FOR_RULE@"),
        (
            ".x\n  @for $i from $from through $to\n    a: $i\n",
            "FOR_RULE@",
        ),
        (".x\n  @each $v in $in\n    a: $v\n", "EACH_RULE@"),
        (".x\n  @if $and\n    a: b\n", "IF_RULE@"),
        (".x\n  @if true and $or\n    a: b\n", "IF_RULE@"),
        (".x\n  @while $not\n    a: b\n", "WHILE_RULE@"),
    ] {
        let tree = render(parse_sass_layout, source);
        let body = tree
            .split_once(rule)
            .map(|(_, rest)| rest.contains("BLOCK@") && rest.contains("DECLARATION@"));
        assert_eq!(body, Some(true), "{source:?}:\n{tree}");
        assert!(!tree.contains("\nerror "), "{source:?}:\n{tree}");
    }
    check(
        ".x\n  @for $i from 1 through $to\n    a: $i\n",
        expect![[r#"
            SOURCE_FILE@0..42
              RULE_SET@0..41
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "x"
                BLOCK@2..41
                  LBRACE@2..2 ""
                  FOR_RULE@2..41
                    WHITESPACE@2..5 "\n  "
                    AT@5..6 "@"
                    IDENT@6..9 "for"
                    WHITESPACE@9..10 " "
                    DOLLAR@10..11 "$"
                    IDENT@11..12 "i"
                    WHITESPACE@12..13 " "
                    IDENT@13..17 "from"
                    NUMBER_LITERAL@17..19
                      WHITESPACE@17..18 " "
                      NUMBER@18..19 "1"
                    WHITESPACE@19..20 " "
                    IDENT@20..27 "through"
                    VARIABLE_REF@27..31
                      WHITESPACE@27..28 " "
                      DOLLAR@28..29 "$"
                      IDENT@29..31 "to"
                    BLOCK@31..41
                      LBRACE@31..31 ""
                      DECLARATION@31..41
                        PROPERTY@31..37
                          WHITESPACE@31..36 "\n    "
                          IDENT@36..37 "a"
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

/// An error ends a statement before its line does; an at-rule after it on the line is a
/// statement of its own, and the header of the broken one no longer decides whether the
/// deeper line below goes on: here it opens the at-rule's block, as in legacy.
#[test]
fn an_at_rule_after_a_broken_statement_ends_its_header() {
    check_same("+J@\n\t(\n");
    check_same("=@\n ,\n");
    // A comma went on over the line break before the at-rule.
    check_same("=,\x0c@\x0c ,");
    check(
        "+m@\n  (a)\n",
        expect![[r#"
            SOURCE_FILE@0..10
              INCLUDE_RULE@0..2
                AT@0..0 ""
                IDENT@0..1 "+"
                IDENT@1..2 "m"
              GENERIC_AT_RULE@2..9
                AT@2..3 "@"
                BLOCK@3..9
                  LBRACE@3..3 ""
                  ERROR@3..7
                    WHITESPACE@3..6 "\n  "
                    LPAREN@6..7 "("
                  RULE_SET@7..8
                    SELECTOR_LIST@7..8
                      SELECTOR@7..8
                        SIMPLE_SELECTOR@7..8
                          IDENT@7..8 "a"
                  ERROR@8..9
                    RPAREN@8..9 ")"
                  SEMICOLON@9..9 ""
                  RBRACE@9..9 ""
              WHITESPACE@9..10 "\n"
            error 2..3: expected SEMICOLON
            error 6..7: expected declaration or nested rule
            error 8..9: expected declaration or nested rule
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

// ── Brackets ─────────────────────────────────────────────────────────

#[test]
fn line_breaks_inside_brackets_are_whitespace() {
    check_same("$m: (\n  a: 1,\n    b: 2\n)\n.x\n  c: map-get($m,\n'a')\n");
}

#[test]
fn an_unclosed_bracket_without_an_error_runs_to_the_end_of_the_input() {
    // No error inside the bracket, so nothing recovers early.
    check_same(".a:not(.b\n  c: d\n");
}

#[test]
fn an_error_inside_a_bracket_that_closes_changes_nothing() {
    // The error makes the bracket stop at a line no deeper than the one it opened on, but
    // it closes before any such line.
    check_same(".a\n  b: fn(1, !\n    2)\n  c: d\n");
}

#[test]
fn unclosed_brackets_inside_a_closed_one_unwind_with_it() {
    check(
        ".a\n  b: f(g(1 ]\n.c\n  d: e\n",
        expect![[r#"
            SOURCE_FILE@0..26
              RULE_SET@0..15
                SELECTOR_LIST@0..2
                  SELECTOR@0..2
                    SIMPLE_SELECTOR@0..2
                      DOT@0..1 "."
                      IDENT@1..2 "a"
                BLOCK@2..15
                  LBRACE@2..2 ""
                  DECLARATION@2..15
                    PROPERTY@2..6
                      WHITESPACE@2..5 "\n  "
                      IDENT@5..6 "b"
                    COLON@6..7 ":"
                    VALUE@7..15
                      FUNCTION_CALL@7..13
                        WHITESPACE@7..8 " "
                        IDENT@8..9 "f"
                        ARG_LIST@9..13
                          LPAREN@9..10 "("
                          ARG@10..13
                            FUNCTION_CALL@10..13
                              IDENT@10..11 "g"
                              ARG_LIST@11..13
                                LPAREN@11..12 "("
                                ARG@12..13
                                  NUMBER_LITERAL@12..13
                                    NUMBER@12..13 "1"
                      WHITESPACE@13..14 " "
                      RBRACKET@14..15 "]"
                    SEMICOLON@15..15 ""
                  RBRACE@15..15 ""
              RULE_SET@15..25
                SELECTOR_LIST@15..18
                  SELECTOR@15..18
                    SIMPLE_SELECTOR@15..18
                      WHITESPACE@15..16 "\n"
                      DOT@16..17 "."
                      IDENT@17..18 "c"
                BLOCK@18..25
                  LBRACE@18..18 ""
                  DECLARATION@18..25
                    PROPERTY@18..22
                      WHITESPACE@18..21 "\n  "
                      IDENT@21..22 "d"
                    COLON@22..23 ":"
                    VALUE@23..25
                      VALUE@23..25
                        WHITESPACE@23..24 " "
                        IDENT@24..25 "e"
                    SEMICOLON@25..25 ""
                  RBRACE@25..25 ""
              WHITESPACE@25..26 "\n"
            error 14..15: expected RPAREN
            error 14..15: expected RPAREN
        "#]],
    );
}
