use std::fmt::Write;

use expect_test::{Expect, expect};
use sass_parser::input::{IndentKind, Input, LineBreak};

#[path = "common/line_facts.rs"]
mod line_facts;

// ── Helpers ──────────────────────────────────────────────────────────

fn describe(line_break: LineBreak) -> String {
    let kind = match line_break.indent_kind() {
        IndentKind::None => "",
        IndentKind::Spaces => " spaces",
        IndentKind::Tabs => " tabs",
        IndentKind::Mixed => " mixed",
    };
    let blank = if line_break.is_blank_line() {
        " blank"
    } else {
        ""
    };
    format!("{}{kind}{blank}", line_break.column())
}

/// One line per significant token, then EOF, each with the line breaks before it.
fn render(source: &str) -> String {
    let input = Input::from_source_indented(source);
    let mut buf = String::new();
    for pos in 0..=input.len() {
        let token = if pos < input.len() {
            let range = input.range(pos);
            format!("{:?} {:?}", input.kind(pos), &source[range])
        } else {
            "EOF".to_owned()
        };
        let breaks: Vec<String> = input
            .line_breaks_before(pos)
            .iter()
            .map(|b| describe(*b))
            .collect();
        if breaks.is_empty() {
            let _ = writeln!(buf, "{token}");
        } else {
            let _ = writeln!(buf, "{token} <- {}", breaks.join(", "));
        }
    }
    buf
}

#[allow(clippy::needless_pass_by_value)]
#[track_caller]
fn check(source: &str, expect: Expect) {
    expect.assert_eq(&render(source));
    let mismatches = line_facts::line_fact_mismatches(source);
    assert!(mismatches.is_empty(), "{source:?}: {mismatches:#?}");
}

// ── Facts ────────────────────────────────────────────────────────────

#[test]
fn nested_rule_columns() {
    check(
        ".a\n  b: c\n  .d\n    e: f\n.g\n",
        expect![[r#"
            DOT "."
            IDENT "a"
            IDENT "b" <- 2 spaces
            COLON ":"
            IDENT "c"
            DOT "." <- 2 spaces
            IDENT "d"
            IDENT "e" <- 4 spaces
            COLON ":"
            IDENT "f"
            DOT "." <- 0
            IDENT "g"
            EOF <- 0
        "#]],
    );
}

#[test]
fn first_token_does_not_start_a_line_even_when_indented() {
    check(
        "  .a\n  b: c",
        expect![[r#"
            DOT "."
            IDENT "a"
            IDENT "b" <- 2 spaces
            COLON ":"
            IDENT "c"
            EOF
        "#]],
    );
}

#[test]
fn blank_line_counts_line_feeds_only() {
    check(
        "a\n\n  b\r\n  c\r\n\r\nd\r\re",
        expect![[r#"
            IDENT "a"
            IDENT "b" <- 2 spaces blank
            IDENT "c" <- 2 spaces
            IDENT "d" <- 0 blank
            IDENT "e" <- 0
            EOF
        "#]],
    );
}

#[test]
fn form_feed_and_lone_carriage_return_break_lines() {
    check(
        "a\x0c  b\r c",
        expect![[r#"
            IDENT "a"
            IDENT "b" <- 2 spaces
            IDENT "c" <- 1 spaces
            EOF
        "#]],
    );
}

#[test]
fn tabs_count_one_column_each() {
    check(
        ".a\n\tb: c\n\t\t.d\n  \te: f\n",
        expect![[r#"
            DOT "."
            IDENT "a"
            IDENT "b" <- 1 tabs
            COLON ":"
            IDENT "c"
            DOT "." <- 2 tabs
            IDENT "d"
            IDENT "e" <- 3 mixed
            COLON ":"
            IDENT "f"
            EOF <- 0
        "#]],
    );
}

#[test]
fn comment_only_lines_keep_a_break_of_their_own() {
    check(
        ".a\n  b: c\n// x\n    /* y */\n  d: e\n",
        expect![[r#"
            DOT "."
            IDENT "a"
            IDENT "b" <- 2 spaces
            COLON ":"
            IDENT "c"
            IDENT "d" <- 0, 4 spaces, 2 spaces
            COLON ":"
            IDENT "e"
            EOF <- 0
        "#]],
    );
}

#[test]
fn newlines_inside_comments_are_not_line_breaks() {
    check(
        ".a /* x\n    y */ .b\n  c: d",
        expect![[r#"
            DOT "."
            IDENT "a"
            DOT "."
            IDENT "b"
            IDENT "c" <- 2 spaces
            COLON ":"
            IDENT "d"
            EOF
        "#]],
    );
}

#[test]
fn trailing_indentation_is_reported_before_eof() {
    check(
        ".a\n  b: c\n    ",
        expect![[r#"
            DOT "."
            IDENT "a"
            IDENT "b" <- 2 spaces
            COLON ":"
            IDENT "c"
            EOF <- 4 spaces
        "#]],
    );
}

#[test]
fn empty_and_whitespace_only_input() {
    check(
        "",
        expect![[r#"
        EOF
    "#]],
    );
    check(
        " \n\n ",
        expect![[r#"
        EOF <- 1 spaces blank
    "#]],
    );
}

#[test]
fn lexing_errors_do_not_disturb_the_facts() {
    check(
        ".a\n  b: c\n  /* never closed\n  d: e\n",
        expect![[r#"
            DOT "."
            IDENT "a"
            IDENT "b" <- 2 spaces
            COLON ":"
            IDENT "c"
            ERROR "/* never closed\n  d: e\n" <- 2 spaces
            EOF
        "#]],
    );
    // Newlines inside an error token are not line breaks either.
    check(
        ".a\n  b: \"open\n  c: d\n",
        expect![[r#"
            DOT "."
            IDENT "a"
            IDENT "b" <- 2 spaces
            COLON ":"
            ERROR "\"open\n  c: d\n"
            EOF
        "#]],
    );
}

#[test]
fn tokens_are_those_of_the_braced_input() {
    let source = ".a\n  b: c\n// x\n  @include d(1,\n    2)\n";
    let braced = Input::from_source(source);
    let indented = Input::from_source_indented(source);
    assert_eq!(braced.len(), indented.len());
    for pos in 0..braced.len() {
        assert_eq!(braced.kind(pos), indented.kind(pos));
        assert_eq!(braced.range(pos), indented.range(pos));
        assert_eq!(braced.trivia_before(pos), indented.trivia_before(pos));
    }
    assert_eq!(braced.trailing_trivia(), indented.trailing_trivia());
}

#[test]
fn line_start_and_starts_line_follow_the_last_break() {
    let input = Input::from_source_indented("a\n    // x\n  b c");
    assert!(!input.starts_line(0));
    assert!(input.starts_line(1));
    assert!(!input.starts_line(2));
    assert_eq!(input.line_start(1).map(LineBreak::column), Some(2));
    assert_eq!(input.line_start(2), None);
}

#[test]
fn measure_rejects_whitespace_without_a_newline() {
    assert_eq!(LineBreak::measure("  \t "), None);
    assert_eq!(LineBreak::measure(""), None);
    assert_eq!(LineBreak::measure("\n").map(LineBreak::column), Some(0));
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "layout facts exist only for indented input")]
fn braced_input_has_no_layout_facts() {
    let _ = Input::from_source(".a { b: c; }").line_breaks_before(0);
}

// ── Cross-check against the legacy lexer ─────────────────────────────

/// Inputs chosen for layout, beyond those of `sass_lexer_tests.rs` and
/// `parse_sass_legacy_tests.rs`, whose helpers run the same cross-check on every
/// input: the known indented-Sass defects (silent misparses, tabs and spaces,
/// inconsistent indentation, unclosed brackets), the Dart-valid continuation forms,
/// and the `.sass` documents of the language-server tests.
const LAYOUT_INPUTS: &[&str] = &[
    ".a:hover > *\n  color: red\n",
    ".a:focus,\n.b *\n  c: d\n",
    ".a:hover .not\n  b: c\n",
    ".a\n  content: not\n  b: c\n",
    ".a\n  font:\n    family: fantasy\n",
    ".a\n  font: bold\n    family: x\n",
    "@mixin foo\n  // c\n.a\n  b: c\n",
    "@media screen\n  /* c */\n.a\n  b: c\n",
    "/* loud\n  still comment\n.a\n  b: c\n.d\n  e: f\n",
    ".a\n\tcolor: red\n        b: c\n",
    ".a\n  \tcolor: red\n  \tb: c\n",
    ".a\n    .b\n      c: d\n  .e\n    f: g\n",
    ".a\n  color:red\n",
    ".a\n  b: fn(1\n.c\n  d: e\n.f\n  g: h\n",
    "@namespace svg\n  \"http://www.w3.org/2000/svg\"\n",
    "$a: 1 +\n  2\n",
    "@each $a\n  , $b in (1 2)\n  c: $a\n",
    "@function f\n  ($a)\n  @return $a +\n    1\n",
    "@if\n  $a\n  b: c\n",
    "$m: (\na: 1\n)\n",
    ".a\n  b: fn(1,\n2)\n",
    "// c\n  continued\n    deeper\n.a\n  // body\n\n  b: c\n",
    "// c\r\n  crlf continuation\r\n.a\r\n  b: c\r\n",
    "\u{feff}.a\n  b: c\n",
    ".a\n  b: c\n    \n",
    "@mixin m\n  b: c\n// dedented comment\n  d: e\n",
    "=m\n  b: c\n.a\n  +m\n",
    ".nav\n  color: red\n  .item\n    margin: 0\n    &:hover\n      color: blue\n.footer\n  padding: 0\n",
    "@media screen and (min-width: 100px),\n  print\n  .a\n    color: red\n@include button($bg,\n  $border)\n",
    "// leading comment\n// second line\n.a\n  // only a comment\n  color: red\n@mixin empty\n  // comment-only body\n.b\n  /* loud */\n  margin: 0\n",
];

/// The cross-check is only as strong as the set of newlines legacy reports: it must
/// skip those inside brackets and `//` continuations, and report every other one,
/// trailing whitespace included.
#[test]
fn legacy_reports_the_newlines_it_measures() {
    let source = "// c\n  more\n.a\n  b: fn(1,\n2)\n    ";
    let seen: Vec<String> = sass_parser::sass_lexer::newline_indents(source)
        .into_iter()
        .map(|(offset, column)| format!("{offset:?}={column}"))
        .collect();
    expect![[r#"["11=0", "14=2", "28=4"]"#]].assert_eq(&format!("{seen:?}"));
}

#[test]
fn facts_match_legacy_on_layout_inputs() {
    let failures: Vec<String> = LAYOUT_INPUTS
        .iter()
        .flat_map(|source| {
            line_facts::line_fact_mismatches(source)
                .into_iter()
                .map(move |m| format!("{source:?} {m}"))
        })
        .collect();
    assert!(failures.is_empty(), "{failures:#?}");
}
