use std::fmt::Write;

use expect_test::{Expect, expect};
use sass_parser::input::{IndentKind, Input, LineBreak};

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
