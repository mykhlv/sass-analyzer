use std::fmt::Write;

use expect_test::{Expect, expect};
use sass_parser::input::{IndentKind, Input, LineBreak};
use sass_parser::syntax_kind::SyntaxKind;

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
fn blank_line_counts_every_line_ending_once() {
    // `\r\n` is one line break, as a lone `\n`, `\r` or form feed is.
    check(
        "a\n\n  b\r\n  c\r\n\r\nd\r\re\x0c\x0cf\r\n\rg",
        expect![[r#"
            IDENT "a"
            IDENT "b" <- 2 spaces blank
            IDENT "c" <- 2 spaces
            IDENT "d" <- 0 blank
            IDENT "e" <- 0 blank
            IDENT "f" <- 0 blank
            IDENT "g" <- 0 blank
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
    // A string left unclosed ends at its line's end, and the line break after it is one.
    check(
        ".a\n  b: \"open\n  c: d\n",
        expect![[r#"
            DOT "."
            IDENT "a"
            IDENT "b" <- 2 spaces
            COLON ":"
            QUOTED_STRING "\"open"
            IDENT "c" <- 2 spaces
            COLON ":"
            IDENT "d"
            EOF <- 0
        "#]],
    );
    // An escaped newline inside a string is not a line break.
    check(
        ".a\n  b: \"x\\\n  y\"\n  c: d\n",
        expect![[r#"
            DOT "."
            IDENT "a"
            IDENT "b" <- 2 spaces
            COLON ":"
            QUOTED_STRING "\"x\\\n  y\""
            IDENT "c" <- 2 spaces
            COLON ":"
            IDENT "d"
            EOF <- 0
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
