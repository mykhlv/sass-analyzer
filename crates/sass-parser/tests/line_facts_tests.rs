use std::fmt::Write;

use expect_test::{Expect, expect};
use sass_parser::input::{IndentKind, Input, LineBreak};
use sass_parser::syntax_kind::SyntaxKind;

#[path = "common/layout_allow_list.rs"]
mod layout_allow_list;
#[path = "common/layout_differential.rs"]
mod layout_differential;
#[path = "common/line_facts.rs"]
mod line_facts;
#[path = "common/virtual_tokens.rs"]
mod virtual_tokens;

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
    render_input(&Input::from_source_indented(source), source)
}

fn render_input(input: &Input, source: &str) -> String {
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
    assert_layout_matches_legacy(source);
}

/// `parse_sass` gives legacy's tree, or the input is on the allow-list.
#[track_caller]
fn assert_layout_matches_legacy(source: &str) {
    if let Some((legacy, layout)) = layout_differential::layout_mismatch(source) {
        assert!(
            layout_allow_list::allowed(source).is_some(),
            "parse_sass disagrees with parse_sass_legacy on {source:?}:\n{}",
            layout_differential::first_difference(&legacy, &layout),
        );
    }
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
        ".a\n  b: c\n/* x */\n    // y\n  d: e\n",
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
        ".a\n  b: c /* never closed\n  d: e\n",
        expect![[r#"
            DOT "."
            IDENT "a"
            IDENT "b" <- 2 spaces
            COLON ":"
            IDENT "c"
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
fn a_loud_comment_starting_a_statement_ends_at_the_dedent() {
    check(
        "/* a\n  b\n.c\n  d: e\n",
        expect![[r#"
        DOT "." <- 0
        IDENT "c"
        IDENT "d" <- 2 spaces
        COLON ":"
        IDENT "e"
        EOF <- 0
    "#]],
    );
}

#[test]
fn a_silent_comment_starting_a_statement_takes_the_deeper_lines() {
    check(
        "// a\n  b: c\n.d\n  e: f\n",
        expect![[r#"
        DOT "." <- 2 spaces, 0
        IDENT "d"
        IDENT "e" <- 2 spaces
        COLON ":"
        IDENT "f"
        EOF <- 0
    "#]],
    );
}

/// The legacy lexer knows that a comment starting a continued line does not start a
/// statement. The layout alone cannot, so these facts read it as one.
#[test]
fn the_layout_alone_reads_a_comment_on_a_continued_line_as_a_statement() {
    let mismatches = line_facts::line_fact_mismatches(".a,\n// c\n  .b\n    x: y\n");
    expect![[r#"
        [
            "layout comment 11..12 \".\"",
            "layout comment 12..13 \"b\"",
            "layout comment 18..19 \"x\"",
            "layout comment 19..20 \":\"",
            "layout comment 20..21 \" \"",
            "layout comment 21..22 \"y\"",
        ]
    "#]]
    .assert_debug_eq(&mismatches);
}

/// Once the parser knows the line goes on, re-lexing it reads the comment as an ordinary
/// one: the lines below are tokens again, and the facts are rebuilt with them.
#[test]
fn relexing_a_continued_line_ends_its_comment_with_the_line() {
    let source = ".a,\n// c\n  .b\n    x: y\n";
    let mut input = Input::from_source_indented(source);
    assert_eq!(input.len(), 3, "`.b` and `x: y` are inside the comment");
    let (first_break, _) = input.line_break_range(2);
    assert!(input.relex_continued(source, first_break, true));
    expect![[r#"
        DOT "."
        IDENT "a"
        COMMA ","
        DOT "." <- 0, 2 spaces
        IDENT "b"
        IDENT "x" <- 4 spaces
        COLON ":"
        IDENT "y"
        EOF <- 0
    "#]]
    .assert_eq(&render_input(&input, source));
    assert!(
        !input.relex_continued(source, first_break, true),
        "a second pass changes nothing",
    );
}

/// Re-lexing in any order, the indices that tokens and line breaks hold into the trivia
/// stay right: splicing a segment in shifts those of everything after it.
#[test]
fn relexed_lines_keep_every_index_right_in_any_order() {
    let source = "$a: 1 +\n  // c\n    2\n$b: 3 +\n  // d\n    4\n$e: 5 +\n  // f\n    6\n";
    let mut input = Input::from_source_indented(source);
    let comment_breaks: Vec<u32> = (0..input.line_break_count())
        .filter(|&i| {
            let after = input.line_break(i).trivia() as usize + 1;
            input
                .all_trivia()
                .get(after)
                .is_some_and(|(_, range)| source[*range].starts_with("//"))
        })
        .collect();
    assert_eq!(comment_breaks.len(), 3);
    // Last first, so that each one shifts the indices of those already re-lexed.
    for &index in comment_breaks.iter().rev() {
        let line_break = input.line_break(index);
        assert!(input.relex_continued(source, index, true));
        assert_eq!(
            input.line_break(index),
            line_break,
            "the break itself stays put"
        );
        assert_indices_hold(&input, source);
    }
    expect![[r#"
        DOLLAR "$"
        IDENT "a"
        COLON ":"
        NUMBER "1"
        PLUS "+"
        NUMBER "2" <- 2 spaces, 4 spaces
        DOLLAR "$" <- 0
        IDENT "b"
        COLON ":"
        NUMBER "3"
        PLUS "+"
        NUMBER "4" <- 2 spaces, 4 spaces
        DOLLAR "$" <- 0
        IDENT "e"
        COLON ":"
        NUMBER "5"
        PLUS "+"
        NUMBER "6" <- 2 spaces, 4 spaces
        EOF <- 0
    "#]]
    .assert_eq(&render_input(&input, source));
}

fn assert_indices_hold(input: &Input, source: &str) {
    let mut line_breaks = 0;
    for pos in 0..=input.len() {
        let trivia = if pos < input.len() {
            input.trivia_before(pos)
        } else {
            input.trailing_trivia()
        };
        let (start, end) = input.line_break_range(pos);
        assert_eq!(start, line_breaks, "line breaks before token {pos}");
        line_breaks = end;
        for index in start..end {
            let line_break = input.line_break(index);
            let (kind, range) = input.all_trivia()[line_break.trivia() as usize];
            assert_eq!(kind, SyntaxKind::WHITESPACE);
            assert!(source[range].contains('\n'), "line break {index}");
            assert!(
                trivia.contains(&(kind, range)),
                "line break {index} before token {pos}"
            );
            assert_eq!(input.line_break_owner(index), pos);
        }
    }
    assert_eq!(line_breaks, input.line_break_count());
}

#[test]
fn tokens_are_those_of_the_braced_input_where_no_comment_reaches_further() {
    let source = ".a\n  b: c\n  // x\n  @include d(1,\n    2)\n";
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
/// skip those inside brackets, and report every other one, the line breaks of a `//`
/// comment's continuation lines and trailing whitespace included.
#[test]
fn legacy_reports_the_newlines_it_measures() {
    let source = "// c\n  more\n.a\n  b: fn(1,\n2)\n    ";
    let seen: Vec<String> = sass_parser::sass_lexer::newline_indents(source)
        .into_iter()
        .map(|(offset, column)| format!("{offset:?}={column}"))
        .collect();
    expect![[r#"["4=2", "11=0", "14=2", "28=4"]"#]].assert_eq(&format!("{seen:?}"));
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

#[test]
fn layout_frontend_matches_legacy_on_layout_inputs() {
    for source in LAYOUT_INPUTS {
        assert_layout_matches_legacy(source);
    }
}

#[test]
fn virtual_tokens_match_legacy_on_layout_inputs() {
    let failures: Vec<String> = LAYOUT_INPUTS
        .iter()
        .filter_map(|source| {
            virtual_tokens::virtual_token_mismatch(source).map(|m| format!("{source:?} {m}"))
        })
        .collect();
    assert!(failures.is_empty(), "{failures:#?}");
}
