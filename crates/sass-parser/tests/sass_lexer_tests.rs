use expect_test::{Expect, expect};
use sass_parser::sass_lexer::sass_tokenize;
use sass_parser::syntax::SassLanguage;
use sass_parser::syntax_kind::SyntaxKind::{self, *};

// ── Helpers ──────────────────────────────────────────────────────────

fn tokens(source: &str) -> Vec<(SyntaxKind, &str)> {
    sass_tokenize(source)
}

fn significant(source: &str) -> Vec<(SyntaxKind, &str)> {
    sass_tokenize(source)
        .into_iter()
        .filter(|(k, _)| !k.is_trivia())
        .collect()
}

fn format_tokens(toks: &[(SyntaxKind, &str)]) -> String {
    let mut buf = String::new();
    for (kind, text) in toks {
        if text.is_empty() {
            buf.push_str(&format!("{kind:?}(virtual)\n"));
        } else {
            let escaped = text.replace('\n', "\\n").replace('\t', "\\t");
            buf.push_str(&format!("{kind:?} \"{escaped}\"\n"));
        }
    }
    buf
}

#[allow(clippy::needless_pass_by_value)]
fn check_tokens(source: &str, expect: Expect) {
    let toks = tokens(source);
    expect.assert_eq(&format_tokens(&toks));
}

#[allow(clippy::needless_pass_by_value)]
fn check_significant(source: &str, expect: Expect) {
    let toks = significant(source);
    expect.assert_eq(&format_tokens(&toks));
}

fn check_parse(source: &str) -> String {
    let (green, errs) = sass_parser::parse_sass(source);
    let tree = rowan::SyntaxNode::<SassLanguage>::new_root(green);
    assert_eq!(
        tree.text().to_string(),
        source,
        "lossless round-trip failed"
    );
    let mut buf = sass_parser::syntax::debug_tree(&tree);
    if !errs.is_empty() {
        buf.push_str("errors:\n");
        for (msg, range) in &errs {
            let _ = std::fmt::Write::write_fmt(&mut buf, format_args!("  {range:?}: {msg}\n"));
        }
    }
    buf
}

#[allow(clippy::needless_pass_by_value)]
fn check_tree(source: &str, expect: Expect) {
    expect.assert_eq(&check_parse(source));
}

// ── Empty input ──────────────────────────────────────────────────────

#[test]
fn empty_input() {
    assert!(tokens("").is_empty());
}

#[test]
fn whitespace_only() {
    check_tokens(
        "  \n",
        expect![[r#"
        WHITESPACE "  \n"
    "#]],
    );
}

// ── Basic rule set ───────────────────────────────────────────────────

#[test]
fn single_rule_one_property() {
    check_significant(
        ".foo\n  color: red\n",
        expect![[r#"
        DOT "."
        IDENT "foo"
        LBRACE(virtual)
        IDENT "color"
        COLON ":"
        IDENT "red"
        SEMICOLON(virtual)
        RBRACE(virtual)
    "#]],
    );
}

#[test]
fn single_rule_multiple_properties() {
    check_significant(
        ".foo\n  color: red\n  font-size: 14px\n",
        expect![[r#"
        DOT "."
        IDENT "foo"
        LBRACE(virtual)
        IDENT "color"
        COLON ":"
        IDENT "red"
        SEMICOLON(virtual)
        IDENT "font-size"
        COLON ":"
        NUMBER "14"
        IDENT "px"
        SEMICOLON(virtual)
        RBRACE(virtual)
    "#]],
    );
}

// ── Nested rules ─────────────────────────────────────────────────────

#[test]
fn nested_rule() {
    check_significant(
        ".foo\n  color: red\n  .bar\n    font-size: 14px\n",
        expect![[r#"
        DOT "."
        IDENT "foo"
        LBRACE(virtual)
        IDENT "color"
        COLON ":"
        IDENT "red"
        SEMICOLON(virtual)
        DOT "."
        IDENT "bar"
        LBRACE(virtual)
        IDENT "font-size"
        COLON ":"
        NUMBER "14"
        IDENT "px"
        SEMICOLON(virtual)
        RBRACE(virtual)
        RBRACE(virtual)
    "#]],
    );
}

#[test]
fn nested_then_sibling() {
    let src = ".foo\n  .bar\n    color: red\n  margin: 0\n";
    check_significant(
        src,
        expect![[r#"
        DOT "."
        IDENT "foo"
        LBRACE(virtual)
        DOT "."
        IDENT "bar"
        LBRACE(virtual)
        IDENT "color"
        COLON ":"
        IDENT "red"
        SEMICOLON(virtual)
        RBRACE(virtual)
        IDENT "margin"
        COLON ":"
        NUMBER "0"
        SEMICOLON(virtual)
        RBRACE(virtual)
    "#]],
    );
}

// ── Multiple top-level rules ─────────────────────────────────────────

#[test]
fn two_top_level_rules() {
    let src = ".foo\n  color: red\n.bar\n  font-size: 14px\n";
    check_significant(
        src,
        expect![[r#"
        DOT "."
        IDENT "foo"
        LBRACE(virtual)
        IDENT "color"
        COLON ":"
        IDENT "red"
        SEMICOLON(virtual)
        RBRACE(virtual)
        DOT "."
        IDENT "bar"
        LBRACE(virtual)
        IDENT "font-size"
        COLON ":"
        NUMBER "14"
        IDENT "px"
        SEMICOLON(virtual)
        RBRACE(virtual)
    "#]],
    );
}

// ── Comma continuation ───────────────────────────────────────────────

#[test]
fn comma_selector_continuation() {
    let src = ".foo,\n.bar\n  color: red\n";
    check_significant(
        src,
        expect![[r#"
        DOT "."
        IDENT "foo"
        COMMA ","
        DOT "."
        IDENT "bar"
        LBRACE(virtual)
        IDENT "color"
        COLON ":"
        IDENT "red"
        SEMICOLON(virtual)
        RBRACE(virtual)
    "#]],
    );
}

#[test]
fn multi_line_comma_continuation() {
    let src = ".a,\n.b,\n.c\n  color: red\n";
    check_significant(
        src,
        expect![[r#"
        DOT "."
        IDENT "a"
        COMMA ","
        DOT "."
        IDENT "b"
        COMMA ","
        DOT "."
        IDENT "c"
        LBRACE(virtual)
        IDENT "color"
        COLON ":"
        IDENT "red"
        SEMICOLON(virtual)
        RBRACE(virtual)
    "#]],
    );
}

// ── Parentheses suppress virtual tokens ──────────────────────────────

#[test]
fn paren_suppresses_newline_handling() {
    let src = ".foo\n  color: rgb(\n    255,\n    0,\n    0\n  )\n";
    check_significant(
        src,
        expect![[r#"
        DOT "."
        IDENT "foo"
        LBRACE(virtual)
        IDENT "color"
        COLON ":"
        IDENT "rgb"
        LPAREN "("
        NUMBER "255"
        COMMA ","
        NUMBER "0"
        COMMA ","
        NUMBER "0"
        RPAREN ")"
        SEMICOLON(virtual)
        RBRACE(virtual)
    "#]],
    );
}

// ── At-rules ─────────────────────────────────────────────────────────

#[test]
fn mixin_and_include() {
    let src = "@mixin foo\n  color: red\n.bar\n  @include foo\n";
    check_significant(
        src,
        expect![[r#"
        AT "@"
        IDENT "mixin"
        IDENT "foo"
        LBRACE(virtual)
        IDENT "color"
        COLON ":"
        IDENT "red"
        SEMICOLON(virtual)
        RBRACE(virtual)
        DOT "."
        IDENT "bar"
        LBRACE(virtual)
        AT "@"
        IDENT "include"
        IDENT "foo"
        SEMICOLON(virtual)
        RBRACE(virtual)
    "#]],
    );
}

#[test]
fn if_else() {
    let src = "@if $cond\n  color: red\n@else\n  color: blue\n";
    check_significant(
        src,
        expect![[r#"
        AT "@"
        IDENT "if"
        DOLLAR "$"
        IDENT "cond"
        LBRACE(virtual)
        IDENT "color"
        COLON ":"
        IDENT "red"
        SEMICOLON(virtual)
        RBRACE(virtual)
        AT "@"
        IDENT "else"
        LBRACE(virtual)
        IDENT "color"
        COLON ":"
        IDENT "blue"
        SEMICOLON(virtual)
        RBRACE(virtual)
    "#]],
    );
}

// ── Comments ─────────────────────────────────────────────────────────

#[test]
fn comment_line_does_not_emit_semicolon() {
    let src = ".foo\n  // comment\n  color: red\n";
    check_significant(
        src,
        expect![[r#"
        DOT "."
        IDENT "foo"
        LBRACE(virtual)
        IDENT "color"
        COLON ":"
        IDENT "red"
        SEMICOLON(virtual)
        RBRACE(virtual)
    "#]],
    );
}

// ── Blank lines ──────────────────────────────────────────────────────

#[test]
fn blank_lines_are_ignored() {
    let src = ".foo\n  color: red\n\n  font-size: 14px\n";
    check_significant(
        src,
        expect![[r#"
        DOT "."
        IDENT "foo"
        LBRACE(virtual)
        IDENT "color"
        COLON ":"
        IDENT "red"
        SEMICOLON(virtual)
        IDENT "font-size"
        COLON ":"
        NUMBER "14"
        IDENT "px"
        SEMICOLON(virtual)
        RBRACE(virtual)
    "#]],
    );
}

// ── Variables ────────────────────────────────────────────────────────

#[test]
fn top_level_variable() {
    let src = "$color: red\n.foo\n  color: $color\n";
    check_significant(
        src,
        expect![[r#"
        DOLLAR "$"
        IDENT "color"
        COLON ":"
        IDENT "red"
        SEMICOLON(virtual)
        DOT "."
        IDENT "foo"
        LBRACE(virtual)
        IDENT "color"
        COLON ":"
        DOLLAR "$"
        IDENT "color"
        SEMICOLON(virtual)
        RBRACE(virtual)
    "#]],
    );
}

// ── No trailing newline ──────────────────────────────────────────────

#[test]
fn no_trailing_newline() {
    let src = ".foo\n  color: red";
    check_significant(
        src,
        expect![[r#"
        DOT "."
        IDENT "foo"
        LBRACE(virtual)
        IDENT "color"
        COLON ":"
        IDENT "red"
        SEMICOLON(virtual)
        RBRACE(virtual)
    "#]],
    );
}

// ── Interpolation ────────────────────────────────────────────────────

#[test]
fn interpolation_in_selector() {
    let src = ".foo-#{$bar}\n  color: red\n";
    let toks = significant(src);
    let kinds: Vec<_> = toks.iter().map(|(k, _)| *k).collect();
    // SCSS lexer treats `.foo-` as a single selector fragment before `#{`
    // so we get DOT IDENT HASH_LBRACE (no separate MINUS)
    assert_eq!(
        kinds,
        vec![
            DOT,
            IDENT,
            HASH_LBRACE,
            DOLLAR,
            IDENT,
            RBRACE,
            LBRACE,
            IDENT,
            COLON,
            IDENT,
            SEMICOLON,
            RBRACE,
        ]
    );
    // Verify HASH_LBRACE has real text, LBRACE is virtual
    assert_eq!(toks[2], (HASH_LBRACE, "#{"));
    assert_eq!(toks[6], (LBRACE, ""));
}

// ── Tab indentation ──────────────────────────────────────────────────

#[test]
fn tab_indentation() {
    let src = ".foo\n\tcolor: red\n";
    check_significant(
        src,
        expect![[r#"
        DOT "."
        IDENT "foo"
        LBRACE(virtual)
        IDENT "color"
        COLON ":"
        IDENT "red"
        SEMICOLON(virtual)
        RBRACE(virtual)
    "#]],
    );
}

// ── Windows line endings ─────────────────────────────────────────────

#[test]
fn crlf_line_endings() {
    let src = ".foo\r\n  color: red\r\n";
    check_significant(
        src,
        expect![[r#"
        DOT "."
        IDENT "foo"
        LBRACE(virtual)
        IDENT "color"
        COLON ":"
        IDENT "red"
        SEMICOLON(virtual)
        RBRACE(virtual)
    "#]],
    );
}

// ── Deep nesting ─────────────────────────────────────────────────────

#[test]
fn three_level_nesting() {
    let src = ".a\n  .b\n    .c\n      color: red\n";
    check_significant(
        src,
        expect![[r#"
        DOT "."
        IDENT "a"
        LBRACE(virtual)
        DOT "."
        IDENT "b"
        LBRACE(virtual)
        DOT "."
        IDENT "c"
        LBRACE(virtual)
        IDENT "color"
        COLON ":"
        IDENT "red"
        SEMICOLON(virtual)
        RBRACE(virtual)
        RBRACE(virtual)
        RBRACE(virtual)
    "#]],
    );
}

#[test]
fn dedent_multiple_levels_at_once() {
    let src = ".a\n  .b\n    .c\n      color: red\n.d\n  margin: 0\n";
    check_significant(
        src,
        expect![[r#"
        DOT "."
        IDENT "a"
        LBRACE(virtual)
        DOT "."
        IDENT "b"
        LBRACE(virtual)
        DOT "."
        IDENT "c"
        LBRACE(virtual)
        IDENT "color"
        COLON ":"
        IDENT "red"
        SEMICOLON(virtual)
        RBRACE(virtual)
        RBRACE(virtual)
        RBRACE(virtual)
        DOT "."
        IDENT "d"
        LBRACE(virtual)
        IDENT "margin"
        COLON ":"
        NUMBER "0"
        SEMICOLON(virtual)
        RBRACE(virtual)
    "#]],
    );
}

// ── Only comments ────────────────────────────────────────────────────

#[test]
fn only_comments() {
    let src = "// just a comment\n";
    assert_eq!(significant(src), vec![]);
}

#[test]
fn only_block_comment() {
    let src = "/* block\n   comment */\n";
    assert_eq!(significant(src), vec![]);
}

// ── Consecutive blank lines ──────────────────────────────────────────

#[test]
fn consecutive_blank_lines() {
    let src = ".foo\n  color: red\n\n\n\n  font-size: 14px\n";
    check_significant(
        src,
        expect![[r#"
        DOT "."
        IDENT "foo"
        LBRACE(virtual)
        IDENT "color"
        COLON ":"
        IDENT "red"
        SEMICOLON(virtual)
        IDENT "font-size"
        COLON ":"
        NUMBER "14"
        IDENT "px"
        SEMICOLON(virtual)
        RBRACE(virtual)
    "#]],
    );
}

// ── Comma edge cases ─────────────────────────────────────────────────

#[test]
fn comma_continuation_with_blank_line() {
    // Comma continuation spans blank lines (matches Dart Sass behavior)
    let src = ".a,\n\n.b\n  color: red\n";
    check_significant(
        src,
        expect![[r#"
        DOT "."
        IDENT "a"
        COMMA ","
        DOT "."
        IDENT "b"
        LBRACE(virtual)
        IDENT "color"
        COLON ":"
        IDENT "red"
        SEMICOLON(virtual)
        RBRACE(virtual)
    "#]],
    );
}

#[test]
fn comma_as_last_token_in_file() {
    let src = "$list: 1,";
    let toks = significant(src);
    let kinds: Vec<_> = toks.iter().map(|(k, _)| *k).collect();
    assert_eq!(kinds, vec![DOLLAR, IDENT, COLON, NUMBER, COMMA, SEMICOLON]);
}

// ── Bracket nesting ──────────────────────────────────────────────────

#[test]
fn bracket_in_selector() {
    let src = "a[href]\n  color: red\n";
    check_significant(
        src,
        expect![[r#"
        IDENT "a"
        LBRACKET "["
        IDENT "href"
        RBRACKET "]"
        LBRACE(virtual)
        IDENT "color"
        COLON ":"
        IDENT "red"
        SEMICOLON(virtual)
        RBRACE(virtual)
    "#]],
    );
}

// ── @use / @forward / @import ────────────────────────────────────────

#[test]
fn use_rule() {
    let src = "@use \"sass:math\"\n.foo\n  width: math.ceil(1.5)\n";
    let toks = significant(src);
    let kinds: Vec<_> = toks.iter().map(|(k, _)| *k).collect();
    // @use "sass:math" ; .foo { ... }
    assert!(kinds.starts_with(&[AT, IDENT, QUOTED_STRING, SEMICOLON]));
}

// ── Multi-line block comment ─────────────────────────────────────────

#[test]
fn multiline_block_comment_in_rule() {
    let src = ".foo\n  /* multi\n     line */\n  color: red\n";
    check_significant(
        src,
        expect![[r#"
        DOT "."
        IDENT "foo"
        LBRACE(virtual)
        IDENT "color"
        COLON ":"
        IDENT "red"
        SEMICOLON(virtual)
        RBRACE(virtual)
    "#]],
    );
}

// ── Round-trip (parse_sass) ──────────────────────────────────────────

#[test]
fn parse_sass_round_trip_simple() {
    let src = ".foo\n  color: red\n";
    let (green, _errors) = sass_parser::parse_sass(src);
    let tree = rowan::SyntaxNode::<SassLanguage>::new_root(green);
    assert_eq!(tree.text().to_string(), src);
}

#[test]
fn parse_sass_round_trip_nested() {
    let src = ".foo\n  .bar\n    color: red\n  margin: 0\n";
    let (green, _errors) = sass_parser::parse_sass(src);
    let tree = rowan::SyntaxNode::<SassLanguage>::new_root(green);
    assert_eq!(tree.text().to_string(), src);
}

#[test]
fn parse_sass_tree_simple() {
    check_tree(
        ".foo\n  color: red\n",
        expect![[r#"
        SOURCE_FILE@0..18
          RULE_SET@0..17
            SELECTOR_LIST@0..4
              SELECTOR@0..4
                SIMPLE_SELECTOR@0..4
                  DOT@0..1 "."
                  IDENT@1..4 "foo"
            BLOCK@4..17
              LBRACE@4..4 ""
              DECLARATION@4..17
                PROPERTY@4..12
                  WHITESPACE@4..7 "\n  "
                  IDENT@7..12 "color"
                COLON@12..13 ":"
                VALUE@13..17
                  VALUE@13..17
                    WHITESPACE@13..14 " "
                    IDENT@14..17 "red"
                SEMICOLON@17..17 ""
              RBRACE@17..17 ""
          WHITESPACE@17..18 "\n"
    "#]],
    );
}

#[test]
fn parse_sass_tree_variable() {
    check_tree(
        "$x: 1\n",
        expect![[r#"
        SOURCE_FILE@0..6
          VARIABLE_DECL@0..5
            DOLLAR@0..1 "$"
            IDENT@1..2 "x"
            COLON@2..3 ":"
            NUMBER_LITERAL@3..5
              WHITESPACE@3..4 " "
              NUMBER@4..5 "1"
            SEMICOLON@5..5 ""
          WHITESPACE@5..6 "\n"
    "#]],
    );
}
