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
    use std::fmt::Write;

    let mut buf = String::new();
    for (kind, text) in toks {
        if text.is_empty() {
            let _ = writeln!(buf, "{kind:?}(virtual)");
        } else {
            let escaped = text.replace('\n', "\\n").replace('\t', "\\t");
            let _ = writeln!(buf, "{kind:?} \"{escaped}\"");
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

// Nested @if/@else inside @function — body starts with `$variable` declaration
#[test]
fn nested_if_else_in_function_tree() {
    let src = "@function foo($bg)\n  $x: 100\n  @if $x > 128\n    @return $dark\n  @else\n    @return $light\n";
    check_tree(
        src,
        expect![[r#"
            SOURCE_FILE@0..89
              FUNCTION_RULE@0..88
                AT@0..1 "@"
                IDENT@1..9 "function"
                WHITESPACE@9..10 " "
                IDENT@10..13 "foo"
                PARAM_LIST@13..18
                  LPAREN@13..14 "("
                  PARAM@14..17
                    DOLLAR@14..15 "$"
                    IDENT@15..17 "bg"
                  RPAREN@17..18 ")"
                BLOCK@18..88
                  WHITESPACE@18..21 "\n  "
                  LBRACE@21..21 ""
                  VARIABLE_DECL@21..28
                    DOLLAR@21..22 "$"
                    IDENT@22..23 "x"
                    COLON@23..24 ":"
                    NUMBER_LITERAL@24..28
                      WHITESPACE@24..25 " "
                      NUMBER@25..28 "100"
                    SEMICOLON@28..28 ""
                  IF_RULE@28..88
                    WHITESPACE@28..31 "\n  "
                    AT@31..32 "@"
                    IDENT@32..34 "if"
                    BINARY_EXPR@34..43
                      VARIABLE_REF@34..37
                        WHITESPACE@34..35 " "
                        DOLLAR@35..36 "$"
                        IDENT@36..37 "x"
                      WHITESPACE@37..38 " "
                      GT@38..39 ">"
                      NUMBER_LITERAL@39..43
                        WHITESPACE@39..40 " "
                        NUMBER@40..43 "128"
                    BLOCK@43..61
                      WHITESPACE@43..48 "\n    "
                      LBRACE@48..48 ""
                      RETURN_RULE@48..61
                        AT@48..49 "@"
                        IDENT@49..55 "return"
                        VARIABLE_REF@55..61
                          WHITESPACE@55..56 " "
                          DOLLAR@56..57 "$"
                          IDENT@57..61 "dark"
                        SEMICOLON@61..61 ""
                      RBRACE@61..61 ""
                    ELSE_CLAUSE@61..88
                      WHITESPACE@61..64 "\n  "
                      AT@64..65 "@"
                      IDENT@65..69 "else"
                      BLOCK@69..88
                        WHITESPACE@69..74 "\n    "
                        LBRACE@74..74 ""
                        RETURN_RULE@74..88
                          AT@74..75 "@"
                          IDENT@75..81 "return"
                          VARIABLE_REF@81..88
                            WHITESPACE@81..82 " "
                            DOLLAR@82..83 "$"
                            IDENT@83..88 "light"
                          SEMICOLON@88..88 ""
                        RBRACE@88..88 ""
                  RBRACE@88..88 ""
              WHITESPACE@88..89 "\n"
        "#]],
    );
}

// Nested @if (no @else) inside @function — body starts with `$variable` declaration
#[test]
fn nested_if_no_else_in_function_tree() {
    let src = "@function foo($s)\n  $i: 1\n  @if $i\n    @return $s\n  @return $s\n";
    check_tree(
        src,
        expect![[r#"
            SOURCE_FILE@0..63
              FUNCTION_RULE@0..62
                AT@0..1 "@"
                IDENT@1..9 "function"
                WHITESPACE@9..10 " "
                IDENT@10..13 "foo"
                PARAM_LIST@13..17
                  LPAREN@13..14 "("
                  PARAM@14..16
                    DOLLAR@14..15 "$"
                    IDENT@15..16 "s"
                  RPAREN@16..17 ")"
                BLOCK@17..62
                  WHITESPACE@17..20 "\n  "
                  LBRACE@20..20 ""
                  VARIABLE_DECL@20..25
                    DOLLAR@20..21 "$"
                    IDENT@21..22 "i"
                    COLON@22..23 ":"
                    NUMBER_LITERAL@23..25
                      WHITESPACE@23..24 " "
                      NUMBER@24..25 "1"
                    SEMICOLON@25..25 ""
                  IF_RULE@25..49
                    WHITESPACE@25..28 "\n  "
                    AT@28..29 "@"
                    IDENT@29..31 "if"
                    VARIABLE_REF@31..34
                      WHITESPACE@31..32 " "
                      DOLLAR@32..33 "$"
                      IDENT@33..34 "i"
                    BLOCK@34..49
                      WHITESPACE@34..39 "\n    "
                      LBRACE@39..39 ""
                      RETURN_RULE@39..49
                        AT@39..40 "@"
                        IDENT@40..46 "return"
                        VARIABLE_REF@46..49
                          WHITESPACE@46..47 " "
                          DOLLAR@47..48 "$"
                          IDENT@48..49 "s"
                        SEMICOLON@49..49 ""
                      RBRACE@49..49 ""
                  RETURN_RULE@49..62
                    WHITESPACE@49..52 "\n  "
                    AT@52..53 "@"
                    IDENT@53..59 "return"
                    VARIABLE_REF@59..62
                      WHITESPACE@59..60 " "
                      DOLLAR@60..61 "$"
                      IDENT@61..62 "s"
                    SEMICOLON@62..62 ""
                  RBRACE@62..62 ""
              WHITESPACE@62..63 "\n"
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

// ── Blank line breaks continuation ───────────────────────────────────

#[test]
fn colon_continuation_broken_by_blank_line() {
    // COLON at EOL does NOT continue across blank lines.
    let src = "$var:\n\n  value\n";
    check_significant(
        src,
        expect![[r#"
            DOLLAR "$"
            IDENT "var"
            COLON ":"
            LBRACE(virtual)
            IDENT "value"
            SEMICOLON(virtual)
            RBRACE(virtual)
        "#]],
    );
}

#[test]
fn percent_as_unit_not_continuation() {
    // `50%` at EOL is a unit suffix, not modulo operator — no continuation.
    let src = ".foo\n  width: 50%\n  color: red\n";
    check_significant(
        src,
        expect![[r#"
        DOT "."
        IDENT "foo"
        LBRACE(virtual)
        IDENT "width"
        COLON ":"
        NUMBER "50"
        PERCENT "%"
        SEMICOLON(virtual)
        IDENT "color"
        COLON ":"
        IDENT "red"
        SEMICOLON(virtual)
        RBRACE(virtual)
    "#]],
    );
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

// ── At-rule header continuation after arguments (regression: 30ba7f4) ───

#[test]
fn for_header_continues_with_from_keyword() {
    check_tree(
        "@for $i\n  from 1 through 10\n",
        expect![[r#"
        SOURCE_FILE@0..28
          FOR_RULE@0..27
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
            NUMBER_LITERAL@24..27
              WHITESPACE@24..25 " "
              NUMBER@25..27 "10"
          SEMICOLON@27..27 ""
          WHITESPACE@27..28 "\n"
    "#]],
    );
}

#[test]
fn for_header_continues_with_through_keyword() {
    check_tree(
        "@for $i from 1\n  through 10\n",
        expect![[r#"
        SOURCE_FILE@0..28
          FOR_RULE@0..27
            AT@0..1 "@"
            IDENT@1..4 "for"
            WHITESPACE@4..5 " "
            DOLLAR@5..6 "$"
            IDENT@6..7 "i"
            WHITESPACE@7..8 " "
            IDENT@8..12 "from"
            NUMBER_LITERAL@12..14
              WHITESPACE@12..13 " "
              NUMBER@13..14 "1"
            WHITESPACE@14..17 "\n  "
            IDENT@17..24 "through"
            NUMBER_LITERAL@24..27
              WHITESPACE@24..25 " "
              NUMBER@25..27 "10"
          SEMICOLON@27..27 ""
          WHITESPACE@27..28 "\n"
    "#]],
    );
}

#[test]
fn each_header_continues_with_in_keyword() {
    check_tree(
        "@each $a\n  in b, c\n",
        expect![[r#"
        SOURCE_FILE@0..19
          EACH_RULE@0..18
            AT@0..1 "@"
            IDENT@1..5 "each"
            WHITESPACE@5..6 " "
            DOLLAR@6..7 "$"
            IDENT@7..8 "a"
            WHITESPACE@8..11 "\n  "
            IDENT@11..13 "in"
            VALUE@13..15
              WHITESPACE@13..14 " "
              IDENT@14..15 "b"
            COMMA@15..16 ","
            VALUE@16..18
              WHITESPACE@16..17 " "
              IDENT@17..18 "c"
          SEMICOLON@18..18 ""
          WHITESPACE@18..19 "\n"
    "#]],
    );
}

#[test]
fn each_header_continues_with_comma() {
    check_tree(
        "@each $a\n  , $b in (c: d)\n",
        expect![[r#"
        SOURCE_FILE@0..26
          EACH_RULE@0..25
            AT@0..1 "@"
            IDENT@1..5 "each"
            WHITESPACE@5..6 " "
            DOLLAR@6..7 "$"
            IDENT@7..8 "a"
            WHITESPACE@8..11 "\n  "
            COMMA@11..12 ","
            WHITESPACE@12..13 " "
            DOLLAR@13..14 "$"
            IDENT@14..15 "b"
            WHITESPACE@15..16 " "
            IDENT@16..18 "in"
            MAP_EXPR@18..25
              WHITESPACE@18..19 " "
              LPAREN@19..20 "("
              MAP_ENTRY@20..24
                VALUE@20..21
                  IDENT@20..21 "c"
                COLON@21..22 ":"
                VALUE@22..24
                  WHITESPACE@22..23 " "
                  IDENT@23..24 "d"
              RPAREN@24..25 ")"
          SEMICOLON@25..25 ""
          WHITESPACE@25..26 "\n"
    "#]],
    );
}

#[test]
fn function_header_continues_with_paren() {
    check_tree(
        "@function a\n  ()\n",
        expect![[r#"
        SOURCE_FILE@0..17
          FUNCTION_RULE@0..16
            AT@0..1 "@"
            IDENT@1..9 "function"
            WHITESPACE@9..10 " "
            IDENT@10..11 "a"
            PARAM_LIST@11..16
              WHITESPACE@11..14 "\n  "
              LPAREN@14..15 "("
              RPAREN@15..16 ")"
          SEMICOLON@16..16 ""
          WHITESPACE@16..17 "\n"
    "#]],
    );
}

#[test]
fn function_body_starts_after_params() {
    // The behaviour 30ba7f4 introduced — must not regress.
    check_tree(
        "@function foo($bg)\n  $x: 100\n",
        expect![[r#"
        SOURCE_FILE@0..29
          FUNCTION_RULE@0..28
            AT@0..1 "@"
            IDENT@1..9 "function"
            WHITESPACE@9..10 " "
            IDENT@10..13 "foo"
            PARAM_LIST@13..18
              LPAREN@13..14 "("
              PARAM@14..17
                DOLLAR@14..15 "$"
                IDENT@15..17 "bg"
              RPAREN@17..18 ")"
            BLOCK@18..28
              WHITESPACE@18..21 "\n  "
              LBRACE@21..21 ""
              VARIABLE_DECL@21..28
                DOLLAR@21..22 "$"
                IDENT@22..23 "x"
                COLON@23..24 ":"
                NUMBER_LITERAL@24..28
                  WHITESPACE@24..25 " "
                  NUMBER@25..28 "100"
                SEMICOLON@28..28 ""
              RBRACE@28..28 ""
          WHITESPACE@28..29 "\n"
    "#]],
    );
}

#[test]
fn for_header_continues_with_dollar_after_directive_name() {
    check_tree(
        "@for\n  $i from 1 through 3\n",
        expect![[r#"
        SOURCE_FILE@0..27
          FOR_RULE@0..26
            AT@0..1 "@"
            IDENT@1..4 "for"
            WHITESPACE@4..7 "\n  "
            DOLLAR@7..8 "$"
            IDENT@8..9 "i"
            WHITESPACE@9..10 " "
            IDENT@10..14 "from"
            NUMBER_LITERAL@14..16
              WHITESPACE@14..15 " "
              NUMBER@15..16 "1"
            WHITESPACE@16..17 " "
            IDENT@17..24 "through"
            NUMBER_LITERAL@24..26
              WHITESPACE@24..25 " "
              NUMBER@25..26 "3"
          SEMICOLON@26..26 ""
          WHITESPACE@26..27 "\n"
    "#]],
    );
}

// ── Header continuation is directive-dependent ───────────────────────

#[test]
fn keyframes_from_to_open_the_body() {
    check_tree(
        "@keyframes slide\n  from\n    opacity: 0\n  to\n    opacity: 1\n",
        expect![[r#"
            SOURCE_FILE@0..59
              KEYFRAMES_RULE@0..58
                AT@0..1 "@"
                IDENT@1..10 "keyframes"
                WHITESPACE@10..11 " "
                IDENT@11..16 "slide"
                WHITESPACE@16..19 "\n  "
                LBRACE@19..19 ""
                KEYFRAME_SELECTOR@19..23
                  IDENT@19..23 "from"
                DECLARATION@23..38
                  PROPERTY@23..35
                    WHITESPACE@23..28 "\n    "
                    IDENT@28..35 "opacity"
                  COLON@35..36 ":"
                  VALUE@36..38
                    NUMBER_LITERAL@36..38
                      WHITESPACE@36..37 " "
                      NUMBER@37..38 "0"
                  SEMICOLON@38..38 ""
                KEYFRAME_SELECTOR@38..43
                  WHITESPACE@38..41 "\n  "
                  IDENT@41..43 "to"
                DECLARATION@43..58
                  PROPERTY@43..55
                    WHITESPACE@43..48 "\n    "
                    IDENT@48..55 "opacity"
                  COLON@55..56 ":"
                  VALUE@56..58
                    NUMBER_LITERAL@56..58
                      WHITESPACE@56..57 " "
                      NUMBER@57..58 "1"
                  SEMICOLON@58..58 ""
                RBRACE@58..58 ""
              WHITESPACE@58..59 "\n"
        "#]],
    );
}

#[test]
fn keyframes_percent_selectors_open_the_body() {
    check_tree(
        "@keyframes slide\n  0%\n    opacity: 0\n  100%\n    opacity: 1\n",
        expect![[r#"
            SOURCE_FILE@0..59
              KEYFRAMES_RULE@0..58
                AT@0..1 "@"
                IDENT@1..10 "keyframes"
                WHITESPACE@10..11 " "
                IDENT@11..16 "slide"
                WHITESPACE@16..19 "\n  "
                LBRACE@19..19 ""
                KEYFRAME_SELECTOR@19..36
                  NUMBER@19..20 "0"
                  PERCENT@20..21 "%"
                  BLOCK@21..36
                    LBRACE@21..21 ""
                    DECLARATION@21..36
                      PROPERTY@21..33
                        WHITESPACE@21..26 "\n    "
                        IDENT@26..33 "opacity"
                      COLON@33..34 ":"
                      VALUE@34..36
                        NUMBER_LITERAL@34..36
                          WHITESPACE@34..35 " "
                          NUMBER@35..36 "0"
                      SEMICOLON@36..36 ""
                    RBRACE@36..36 ""
                KEYFRAME_SELECTOR@36..58
                  WHITESPACE@36..39 "\n  "
                  NUMBER@39..42 "100"
                  PERCENT@42..43 "%"
                  BLOCK@43..58
                    LBRACE@43..43 ""
                    DECLARATION@43..58
                      PROPERTY@43..55
                        WHITESPACE@43..48 "\n    "
                        IDENT@48..55 "opacity"
                      COLON@55..56 ":"
                      VALUE@56..58
                        NUMBER_LITERAL@56..58
                          WHITESPACE@56..57 " "
                          NUMBER@57..58 "1"
                      SEMICOLON@58..58 ""
                    RBRACE@58..58 ""
                RBRACE@58..58 ""
              WHITESPACE@58..59 "\n"
        "#]],
    );
}

#[test]
fn mixin_body_starting_with_pseudo_selector() {
    check_tree(
        "@mixin m($x)\n  :hover\n    color: red\n",
        expect![[r#"
        SOURCE_FILE@0..37
          MIXIN_RULE@0..36
            AT@0..1 "@"
            IDENT@1..6 "mixin"
            WHITESPACE@6..7 " "
            IDENT@7..8 "m"
            PARAM_LIST@8..12
              LPAREN@8..9 "("
              PARAM@9..11
                DOLLAR@9..10 "$"
                IDENT@10..11 "x"
              RPAREN@11..12 ")"
            BLOCK@12..36
              WHITESPACE@12..15 "\n  "
              LBRACE@15..15 ""
              RULE_SET@15..36
                SELECTOR_LIST@15..21
                  SELECTOR@15..21
                    PSEUDO_SELECTOR@15..21
                      COLON@15..16 ":"
                      IDENT@16..21 "hover"
                BLOCK@21..36
                  LBRACE@21..21 ""
                  DECLARATION@21..36
                    PROPERTY@21..31
                      WHITESPACE@21..26 "\n    "
                      IDENT@26..31 "color"
                    COLON@31..32 ":"
                    VALUE@32..36
                      VALUE@32..36
                        WHITESPACE@32..33 " "
                        IDENT@33..36 "red"
                    SEMICOLON@36..36 ""
                  RBRACE@36..36 ""
              RBRACE@36..36 ""
          WHITESPACE@36..37 "\n"
    "#]],
    );
}

// `to` continues a header only under `@for`, so here it opens the mixin body.
// The selector merging inside that body comes from the separate end-of-line
// continuation path (`is_end_of_line_continuation`), which is not directive-aware.
#[test]
fn mixin_body_starting_with_continuation_keyword() {
    check_tree(
        "@mixin m\n  to\n    color: red\n",
        expect![[r#"
        SOURCE_FILE@0..29
          MIXIN_RULE@0..28
            AT@0..1 "@"
            IDENT@1..6 "mixin"
            WHITESPACE@6..7 " "
            IDENT@7..8 "m"
            BLOCK@8..28
              WHITESPACE@8..11 "\n  "
              LBRACE@11..11 ""
              RULE_SET@11..28
                SELECTOR_LIST@11..28
                  SELECTOR@11..28
                    SIMPLE_SELECTOR@11..13
                      IDENT@11..13 "to"
                    SIMPLE_SELECTOR@13..23
                      WHITESPACE@13..18 "\n    "
                      IDENT@18..23 "color"
                    PSEUDO_SELECTOR@23..28
                      COLON@23..24 ":"
                      WHITESPACE@24..25 " "
                      IDENT@25..28 "red"
              SEMICOLON@28..28 ""
              RBRACE@28..28 ""
          WHITESPACE@28..29 "\n"
    "#]],
    );
}

// ── `as *` must not continue the line (real-world corpus regression) ────

#[test]
fn use_as_star_followed_by_rule() {
    check_tree(
        "@use 'a' as *\n.x\n  color: red\n",
        expect![[r#"
        SOURCE_FILE@0..30
          USE_RULE@0..13
            AT@0..1 "@"
            IDENT@1..4 "use"
            WHITESPACE@4..5 " "
            QUOTED_STRING@5..8 "'a'"
            WHITESPACE@8..9 " "
            IDENT@9..11 "as"
            WHITESPACE@11..12 " "
            STAR@12..13 "*"
            SEMICOLON@13..13 ""
          RULE_SET@13..29
            SELECTOR_LIST@13..16
              SELECTOR@13..16
                SIMPLE_SELECTOR@13..16
                  WHITESPACE@13..14 "\n"
                  DOT@14..15 "."
                  IDENT@15..16 "x"
            BLOCK@16..29
              LBRACE@16..16 ""
              DECLARATION@16..29
                PROPERTY@16..24
                  WHITESPACE@16..19 "\n  "
                  IDENT@19..24 "color"
                COLON@24..25 ":"
                VALUE@25..29
                  VALUE@25..29
                    WHITESPACE@25..26 " "
                    IDENT@26..29 "red"
                SEMICOLON@29..29 ""
              RBRACE@29..29 ""
          WHITESPACE@29..30 "\n"
    "#]],
    );
}

#[test]
fn use_as_star_is_case_insensitive() {
    // `@USE` is dispatched to GENERIC_AT_RULE rather than USE_RULE — the parser's
    // directive-name dispatch (grammar/at_rules.rs) is a separate, case-sensitive
    // mechanism from the sass_lexer.rs continuation check fixed here. The point of
    // this test is that STAR still ends the header instead of swallowing `.x`.
    check_tree(
        "@USE 'a' as *\n.x\n  color: red\n",
        expect![[r#"
        SOURCE_FILE@0..30
          GENERIC_AT_RULE@0..13
            AT@0..1 "@"
            IDENT@1..4 "USE"
            WHITESPACE@4..5 " "
            QUOTED_STRING@5..8 "'a'"
            WHITESPACE@8..9 " "
            IDENT@9..11 "as"
            WHITESPACE@11..12 " "
            STAR@12..13 "*"
            SEMICOLON@13..13 ""
          RULE_SET@13..29
            SELECTOR_LIST@13..16
              SELECTOR@13..16
                SIMPLE_SELECTOR@13..16
                  WHITESPACE@13..14 "\n"
                  DOT@14..15 "."
                  IDENT@15..16 "x"
            BLOCK@16..29
              LBRACE@16..16 ""
              DECLARATION@16..29
                PROPERTY@16..24
                  WHITESPACE@16..19 "\n  "
                  IDENT@19..24 "color"
                COLON@24..25 ":"
                VALUE@25..29
                  VALUE@25..29
                    WHITESPACE@25..26 " "
                    IDENT@26..29 "red"
                SEMICOLON@29..29 ""
              RBRACE@29..29 ""
          WHITESPACE@29..30 "\n"
    "#]],
    );
}

#[test]
fn use_as_star_followed_by_another_use() {
    check_tree(
        "@use 'a' as *\n@use 'b' as *\n",
        expect![[r#"
        SOURCE_FILE@0..28
          USE_RULE@0..13
            AT@0..1 "@"
            IDENT@1..4 "use"
            WHITESPACE@4..5 " "
            QUOTED_STRING@5..8 "'a'"
            WHITESPACE@8..9 " "
            IDENT@9..11 "as"
            WHITESPACE@11..12 " "
            STAR@12..13 "*"
            SEMICOLON@13..13 ""
          USE_RULE@13..27
            WHITESPACE@13..14 "\n"
            AT@14..15 "@"
            IDENT@15..18 "use"
            WHITESPACE@18..19 " "
            QUOTED_STRING@19..22 "'b'"
            WHITESPACE@22..23 " "
            IDENT@23..25 "as"
            WHITESPACE@25..26 " "
            STAR@26..27 "*"
            SEMICOLON@27..27 ""
          WHITESPACE@27..28 "\n"
    "#]],
    );
}

#[test]
fn forward_as_star_followed_by_rule() {
    check_tree(
        "@forward 'a' as *\n.x\n  color: red\n",
        expect![[r#"
        SOURCE_FILE@0..34
          FORWARD_RULE@0..17
            AT@0..1 "@"
            IDENT@1..8 "forward"
            WHITESPACE@8..9 " "
            QUOTED_STRING@9..12 "'a'"
            WHITESPACE@12..13 " "
            IDENT@13..15 "as"
            WHITESPACE@15..16 " "
            STAR@16..17 "*"
            SEMICOLON@17..17 ""
          RULE_SET@17..33
            SELECTOR_LIST@17..20
              SELECTOR@17..20
                SIMPLE_SELECTOR@17..20
                  WHITESPACE@17..18 "\n"
                  DOT@18..19 "."
                  IDENT@19..20 "x"
            BLOCK@20..33
              LBRACE@20..20 ""
              DECLARATION@20..33
                PROPERTY@20..28
                  WHITESPACE@20..23 "\n  "
                  IDENT@23..28 "color"
                COLON@28..29 ":"
                VALUE@29..33
                  VALUE@29..33
                    WHITESPACE@29..30 " "
                    IDENT@30..33 "red"
                SEMICOLON@33..33 ""
              RBRACE@33..33 ""
          WHITESPACE@33..34 "\n"
    "#]],
    );
}

#[test]
fn star_after_number_still_continues_the_expression() {
    // `*` as a binary operator at end of line is a real continuation.
    check_tree(
        "$a: 1 *\n  2\n",
        expect![[r#"
        SOURCE_FILE@0..12
          VARIABLE_DECL@0..11
            DOLLAR@0..1 "$"
            IDENT@1..2 "a"
            COLON@2..3 ":"
            BINARY_EXPR@3..11
              NUMBER_LITERAL@3..5
                WHITESPACE@3..4 " "
                NUMBER@4..5 "1"
              WHITESPACE@5..6 " "
              STAR@6..7 "*"
              NUMBER_LITERAL@7..11
                WHITESPACE@7..10 "\n  "
                NUMBER@10..11 "2"
            SEMICOLON@11..11 ""
          WHITESPACE@11..12 "\n"
    "#]],
    );
}

#[test]
fn bare_else_body_can_start_with_a_variable_declaration() {
    // A bare `@else` is a complete header, so an indented `$x: …` under it is the body.
    // It used to be pulled into the header, swallowing the first declaration silently and
    // then erroring on the second.
    check_tree(
        "@if $a\n  p: 1\n@else\n  $lum: 2\n  $pct: 3\n",
        expect![[r#"
            SOURCE_FILE@0..40
              IF_RULE@0..39
                AT@0..1 "@"
                IDENT@1..3 "if"
                VARIABLE_REF@3..6
                  WHITESPACE@3..4 " "
                  DOLLAR@4..5 "$"
                  IDENT@5..6 "a"
                BLOCK@6..13
                  WHITESPACE@6..9 "\n  "
                  LBRACE@9..9 ""
                  DECLARATION@9..13
                    PROPERTY@9..10
                      IDENT@9..10 "p"
                    COLON@10..11 ":"
                    VALUE@11..13
                      NUMBER_LITERAL@11..13
                        WHITESPACE@11..12 " "
                        NUMBER@12..13 "1"
                    SEMICOLON@13..13 ""
                  RBRACE@13..13 ""
                ELSE_CLAUSE@13..39
                  WHITESPACE@13..14 "\n"
                  AT@14..15 "@"
                  IDENT@15..19 "else"
                  BLOCK@19..39
                    WHITESPACE@19..22 "\n  "
                    LBRACE@22..22 ""
                    VARIABLE_DECL@22..29
                      DOLLAR@22..23 "$"
                      IDENT@23..26 "lum"
                      COLON@26..27 ":"
                      NUMBER_LITERAL@27..29
                        WHITESPACE@27..28 " "
                        NUMBER@28..29 "2"
                      SEMICOLON@29..29 ""
                    VARIABLE_DECL@29..39
                      WHITESPACE@29..32 "\n  "
                      DOLLAR@32..33 "$"
                      IDENT@33..36 "pct"
                      COLON@36..37 ":"
                      NUMBER_LITERAL@37..39
                        WHITESPACE@37..38 " "
                        NUMBER@38..39 "3"
                      SEMICOLON@39..39 ""
                    RBRACE@39..39 ""
              WHITESPACE@39..40 "\n"
        "#]],
    );
}

#[test]
fn else_if_on_the_next_line_still_continues_the_header() {
    // `@else`'s one real header continuation. Goes through the IDENT arm, not the DOLLAR
    // arm, so it is unaffected by the rule above.
    check_tree(
        "@if $a\n  p: 1\n@else\n  if $b\n    q: 2\n",
        expect![[r#"
        SOURCE_FILE@0..37
          IF_RULE@0..36
            AT@0..1 "@"
            IDENT@1..3 "if"
            VARIABLE_REF@3..6
              WHITESPACE@3..4 " "
              DOLLAR@4..5 "$"
              IDENT@5..6 "a"
            BLOCK@6..13
              WHITESPACE@6..9 "\n  "
              LBRACE@9..9 ""
              DECLARATION@9..13
                PROPERTY@9..10
                  IDENT@9..10 "p"
                COLON@10..11 ":"
                VALUE@11..13
                  NUMBER_LITERAL@11..13
                    WHITESPACE@11..12 " "
                    NUMBER@12..13 "1"
                SEMICOLON@13..13 ""
              RBRACE@13..13 ""
            ELSE_CLAUSE@13..36
              WHITESPACE@13..14 "\n"
              AT@14..15 "@"
              IDENT@15..19 "else"
              WHITESPACE@19..22 "\n  "
              IDENT@22..24 "if"
              VARIABLE_REF@24..27
                WHITESPACE@24..25 " "
                DOLLAR@25..26 "$"
                IDENT@26..27 "b"
              BLOCK@27..36
                WHITESPACE@27..32 "\n    "
                LBRACE@32..32 ""
                DECLARATION@32..36
                  PROPERTY@32..33
                    IDENT@32..33 "q"
                  COLON@33..34 ":"
                  VALUE@34..36
                    NUMBER_LITERAL@34..36
                      WHITESPACE@34..35 " "
                      NUMBER@35..36 "2"
                  SEMICOLON@36..36 ""
                RBRACE@36..36 ""
          WHITESPACE@36..37 "\n"
    "#]],
    );
}

#[test]
fn if_header_still_continues_with_a_variable_on_the_next_line() {
    // The other expression directives are incomplete after their bare name, so a `$` on
    // the next line still continues *their* headers. `@else` is the only exclusion.
    check_tree(
        "@if\n  $a\n  p: 1\n",
        expect![[r#"
        SOURCE_FILE@0..16
          IF_RULE@0..15
            AT@0..1 "@"
            IDENT@1..3 "if"
            VARIABLE_REF@3..8
              WHITESPACE@3..6 "\n  "
              DOLLAR@6..7 "$"
              IDENT@7..8 "a"
            BLOCK@8..15
              WHITESPACE@8..11 "\n  "
              LBRACE@11..11 ""
              DECLARATION@11..15
                PROPERTY@11..12
                  IDENT@11..12 "p"
                COLON@12..13 ":"
                VALUE@13..15
                  NUMBER_LITERAL@13..15
                    WHITESPACE@13..14 " "
                    NUMBER@14..15 "1"
                SEMICOLON@15..15 ""
              RBRACE@15..15 ""
          WHITESPACE@15..16 "\n"
    "#]],
    );
}

#[test]
fn at_root_opens_a_body_for_an_indented_include() {
    // A bare `@at-root` takes a block, so an indented `@include` under it is the body.
    // It used to be forced into the header, producing `@at-root @include …` and an
    // "expected selector" error — Vuetify's density-variant pattern, 19 corpus files.
    check_tree(
        ".foo\n  @at-root\n    @include bar()\n      baz: 1\n",
        expect![[r#"
            SOURCE_FILE@0..48
              RULE_SET@0..47
                SELECTOR_LIST@0..4
                  SELECTOR@0..4
                    SIMPLE_SELECTOR@0..4
                      DOT@0..1 "."
                      IDENT@1..4 "foo"
                BLOCK@4..47
                  LBRACE@4..4 ""
                  AT_ROOT_RULE@4..47
                    WHITESPACE@4..7 "\n  "
                    AT@7..8 "@"
                    IDENT@8..15 "at-root"
                    BLOCK@15..47
                      WHITESPACE@15..20 "\n    "
                      LBRACE@20..20 ""
                      INCLUDE_RULE@20..47
                        AT@20..21 "@"
                        IDENT@21..28 "include"
                        WHITESPACE@28..29 " "
                        IDENT@29..32 "bar"
                        ARG_LIST@32..34
                          LPAREN@32..33 "("
                          RPAREN@33..34 ")"
                        BLOCK@34..47
                          WHITESPACE@34..41 "\n      "
                          LBRACE@41..41 ""
                          DECLARATION@41..47
                            PROPERTY@41..44
                              IDENT@41..44 "baz"
                            COLON@44..45 ":"
                            VALUE@45..47
                              NUMBER_LITERAL@45..47
                                WHITESPACE@45..46 " "
                                NUMBER@46..47 "1"
                            SEMICOLON@47..47 ""
                          RBRACE@47..47 ""
                      RBRACE@47..47 ""
                  RBRACE@47..47 ""
              WHITESPACE@47..48 "\n"
        "#]],
    );
}

#[test]
fn at_root_opens_a_body_for_an_indented_selector() {
    // Behaviour change worth pinning: this used to parse as the selector form
    // `@at-root .bar`, with `.bar` inside AT_ROOT_RULE's own SELECTOR_LIST. In the
    // indented syntax the header ends at the newline, so it is the block form — the
    // same CSS either way, which is why the wrong shape went unnoticed.
    check_tree(
        ".foo\n  @at-root\n    .bar\n      baz: 1\n",
        expect![[r#"
        SOURCE_FILE@0..38
          RULE_SET@0..37
            SELECTOR_LIST@0..4
              SELECTOR@0..4
                SIMPLE_SELECTOR@0..4
                  DOT@0..1 "."
                  IDENT@1..4 "foo"
            BLOCK@4..37
              LBRACE@4..4 ""
              AT_ROOT_RULE@4..37
                WHITESPACE@4..7 "\n  "
                AT@7..8 "@"
                IDENT@8..15 "at-root"
                BLOCK@15..37
                  WHITESPACE@15..20 "\n    "
                  LBRACE@20..20 ""
                  RULE_SET@20..37
                    SELECTOR_LIST@20..24
                      SELECTOR@20..24
                        SIMPLE_SELECTOR@20..24
                          DOT@20..21 "."
                          IDENT@21..24 "bar"
                    BLOCK@24..37
                      LBRACE@24..24 ""
                      DECLARATION@24..37
                        PROPERTY@24..34
                          WHITESPACE@24..31 "\n      "
                          IDENT@31..34 "baz"
                        COLON@34..35 ":"
                        VALUE@35..37
                          NUMBER_LITERAL@35..37
                            WHITESPACE@35..36 " "
                            NUMBER@36..37 "1"
                        SEMICOLON@37..37 ""
                      RBRACE@37..37 ""
                  RBRACE@37..37 ""
              RBRACE@37..37 ""
          WHITESPACE@37..38 "\n"
    "#]],
    );
}

#[test]
fn at_root_header_still_continues_for_a_parenthesised_query() {
    // The one real header continuation `@at-root` has. It survives the removal above
    // because `continues_header` accepts LPAREN under `at-root` on the start-of-line
    // path, which is directive-aware where the end-of-line path is not.
    check_tree(
        ".foo\n  @at-root\n    (without: media)\n    .bar\n      x: 1\n",
        expect![[r#"
            SOURCE_FILE@0..57
              RULE_SET@0..56
                SELECTOR_LIST@0..4
                  SELECTOR@0..4
                    SIMPLE_SELECTOR@0..4
                      DOT@0..1 "."
                      IDENT@1..4 "foo"
                BLOCK@4..56
                  LBRACE@4..4 ""
                  AT_ROOT_RULE@4..56
                    WHITESPACE@4..7 "\n  "
                    AT@7..8 "@"
                    IDENT@8..15 "at-root"
                    AT_ROOT_QUERY@15..36
                      WHITESPACE@15..20 "\n    "
                      LPAREN@20..21 "("
                      IDENT@21..28 "without"
                      COLON@28..29 ":"
                      WHITESPACE@29..30 " "
                      IDENT@30..35 "media"
                      RPAREN@35..36 ")"
                    BLOCK@36..56
                      WHITESPACE@36..41 "\n    "
                      LBRACE@41..41 ""
                      RULE_SET@41..56
                        SELECTOR_LIST@41..45
                          SELECTOR@41..45
                            SIMPLE_SELECTOR@41..45
                              DOT@41..42 "."
                              IDENT@42..45 "bar"
                        BLOCK@45..56
                          LBRACE@45..45 ""
                          DECLARATION@45..56
                            PROPERTY@45..53
                              WHITESPACE@45..52 "\n      "
                              IDENT@52..53 "x"
                            COLON@53..54 ":"
                            VALUE@54..56
                              NUMBER_LITERAL@54..56
                                WHITESPACE@54..55 " "
                                NUMBER@55..56 "1"
                            SEMICOLON@56..56 ""
                          RBRACE@56..56 ""
                      RBRACE@56..56 ""
                  RBRACE@56..56 ""
              WHITESPACE@56..57 "\n"
        "#]],
    );
}

#[test]
fn combinator_and_universal_selector_open_a_block() {
    // `> *` is a selector, not a trailing `*` operator — the next line is its body,
    // not a continuation of the selector. Common in Vuetify/Quasar.
    check_tree(
        ".foo\n  > *\n    opacity: 0\n",
        expect![[r#"
        SOURCE_FILE@0..26
          RULE_SET@0..25
            SELECTOR_LIST@0..4
              SELECTOR@0..4
                SIMPLE_SELECTOR@0..4
                  DOT@0..1 "."
                  IDENT@1..4 "foo"
            BLOCK@4..25
              LBRACE@4..4 ""
              RULE_SET@4..25
                SELECTOR_LIST@4..10
                  SELECTOR@4..10
                    COMBINATOR@4..8
                      WHITESPACE@4..7 "\n  "
                      GT@7..8 ">"
                    SIMPLE_SELECTOR@8..10
                      WHITESPACE@8..9 " "
                      STAR@9..10 "*"
                BLOCK@10..25
                  LBRACE@10..10 ""
                  DECLARATION@10..25
                    PROPERTY@10..22
                      WHITESPACE@10..15 "\n    "
                      IDENT@15..22 "opacity"
                    COLON@22..23 ":"
                    VALUE@23..25
                      NUMBER_LITERAL@23..25
                        WHITESPACE@23..24 " "
                        NUMBER@24..25 "0"
                    SEMICOLON@25..25 ""
                  RBRACE@25..25 ""
              RBRACE@25..25 ""
          WHITESPACE@25..26 "\n"
    "#]],
    );
}

#[test]
fn descendant_universal_selector_opens_a_block() {
    check_tree(
        ".foo *\n  opacity: 0\n",
        expect![[r#"
        SOURCE_FILE@0..20
          RULE_SET@0..19
            SELECTOR_LIST@0..6
              SELECTOR@0..6
                SIMPLE_SELECTOR@0..4
                  DOT@0..1 "."
                  IDENT@1..4 "foo"
                SIMPLE_SELECTOR@4..6
                  WHITESPACE@4..5 " "
                  STAR@5..6 "*"
            BLOCK@6..19
              LBRACE@6..6 ""
              DECLARATION@6..19
                PROPERTY@6..16
                  WHITESPACE@6..9 "\n  "
                  IDENT@9..16 "opacity"
                COLON@16..17 ":"
                VALUE@17..19
                  NUMBER_LITERAL@17..19
                    WHITESPACE@17..18 " "
                    NUMBER@18..19 "0"
                SEMICOLON@19..19 ""
              RBRACE@19..19 ""
          WHITESPACE@19..20 "\n"
    "#]],
    );
}

#[test]
fn star_in_property_value_still_continues_the_expression() {
    // A top-level colon puts us in a value, where `*` is arithmetic again.
    check_tree(
        "a\n  width: 2px *\n    2\n",
        expect![[r#"
        SOURCE_FILE@0..23
          RULE_SET@0..22
            SELECTOR_LIST@0..1
              SELECTOR@0..1
                SIMPLE_SELECTOR@0..1
                  IDENT@0..1 "a"
            BLOCK@1..22
              LBRACE@1..1 ""
              DECLARATION@1..22
                PROPERTY@1..9
                  WHITESPACE@1..4 "\n  "
                  IDENT@4..9 "width"
                COLON@9..10 ":"
                VALUE@10..22
                  BINARY_EXPR@10..22
                    DIMENSION@10..14
                      WHITESPACE@10..11 " "
                      NUMBER@11..12 "2"
                      IDENT@12..14 "px"
                    WHITESPACE@14..15 " "
                    STAR@15..16 "*"
                    NUMBER_LITERAL@16..22
                      WHITESPACE@16..21 "\n    "
                      NUMBER@21..22 "2"
                SEMICOLON@22..22 ""
              RBRACE@22..22 ""
          WHITESPACE@22..23 "\n"
    "#]],
    );
}

#[test]
fn star_in_directive_expression_still_continues() {
    // `@return` takes an expression header, so a trailing `*` is arithmetic there too.
    check_tree(
        "@function f($x)\n  @return $x *\n    2\n",
        expect![[r#"
        SOURCE_FILE@0..37
          FUNCTION_RULE@0..36
            AT@0..1 "@"
            IDENT@1..9 "function"
            WHITESPACE@9..10 " "
            IDENT@10..11 "f"
            PARAM_LIST@11..15
              LPAREN@11..12 "("
              PARAM@12..14
                DOLLAR@12..13 "$"
                IDENT@13..14 "x"
              RPAREN@14..15 ")"
            BLOCK@15..36
              WHITESPACE@15..18 "\n  "
              LBRACE@18..18 ""
              RETURN_RULE@18..36
                AT@18..19 "@"
                IDENT@19..25 "return"
                BINARY_EXPR@25..36
                  VARIABLE_REF@25..28
                    WHITESPACE@25..26 " "
                    DOLLAR@26..27 "$"
                    IDENT@27..28 "x"
                  WHITESPACE@28..29 " "
                  STAR@29..30 "*"
                  NUMBER_LITERAL@30..36
                    WHITESPACE@30..35 "\n    "
                    NUMBER@35..36 "2"
                SEMICOLON@36..36 ""
              RBRACE@36..36 ""
          WHITESPACE@36..37 "\n"
    "#]],
    );
}

#[test]
fn universal_selector_after_tilde_combinator_opens_a_block() {
    check_tree(
        ".foo\n  & ~ *\n    x: 1\n",
        expect![[r#"
        SOURCE_FILE@0..22
          RULE_SET@0..21
            SELECTOR_LIST@0..4
              SELECTOR@0..4
                SIMPLE_SELECTOR@0..4
                  DOT@0..1 "."
                  IDENT@1..4 "foo"
            BLOCK@4..21
              LBRACE@4..4 ""
              RULE_SET@4..21
                SELECTOR_LIST@4..12
                  SELECTOR@4..12
                    SIMPLE_SELECTOR@4..8
                      WHITESPACE@4..7 "\n  "
                      AMP@7..8 "&"
                    COMBINATOR@8..10
                      WHITESPACE@8..9 " "
                      TILDE@9..10 "~"
                    SIMPLE_SELECTOR@10..12
                      WHITESPACE@10..11 " "
                      STAR@11..12 "*"
                BLOCK@12..21
                  LBRACE@12..12 ""
                  DECLARATION@12..21
                    PROPERTY@12..18
                      WHITESPACE@12..17 "\n    "
                      IDENT@17..18 "x"
                    COLON@18..19 ":"
                    VALUE@19..21
                      NUMBER_LITERAL@19..21
                        WHITESPACE@19..20 " "
                        NUMBER@20..21 "1"
                    SEMICOLON@21..21 ""
                  RBRACE@21..21 ""
              RBRACE@21..21 ""
          WHITESPACE@21..22 "\n"
    "#]],
    );
}

#[test]
fn owl_selector_opens_a_block() {
    // `* + *` — the line both starts and ends with `*`.
    check_tree(
        ".foo\n  * + *\n    x: 1\n",
        expect![[r#"
        SOURCE_FILE@0..22
          RULE_SET@0..21
            SELECTOR_LIST@0..4
              SELECTOR@0..4
                SIMPLE_SELECTOR@0..4
                  DOT@0..1 "."
                  IDENT@1..4 "foo"
            BLOCK@4..21
              LBRACE@4..4 ""
              RULE_SET@4..21
                SELECTOR_LIST@4..12
                  SELECTOR@4..12
                    SIMPLE_SELECTOR@4..8
                      WHITESPACE@4..7 "\n  "
                      STAR@7..8 "*"
                    COMBINATOR@8..10
                      WHITESPACE@8..9 " "
                      PLUS@9..10 "+"
                    SIMPLE_SELECTOR@10..12
                      WHITESPACE@10..11 " "
                      STAR@11..12 "*"
                BLOCK@12..21
                  LBRACE@12..12 ""
                  DECLARATION@12..21
                    PROPERTY@12..18
                      WHITESPACE@12..17 "\n    "
                      IDENT@17..18 "x"
                    COLON@18..19 ":"
                    VALUE@19..21
                      NUMBER_LITERAL@19..21
                        WHITESPACE@19..20 " "
                        NUMBER@20..21 "1"
                    SEMICOLON@21..21 ""
                  RBRACE@21..21 ""
              RBRACE@21..21 ""
          WHITESPACE@21..22 "\n"
    "#]],
    );
}

#[test]
fn pseudo_element_before_universal_selector_is_unaffected() {
    // `::` lexes as a single COLON_COLON, so a pseudo-element never sets the
    // value-context flag. Splitting it into two COLON tokens would silently widen
    // the pseudo-class limitation below to every pseudo-element.
    check_tree(
        ".a::before > *\n  color: red\n",
        expect![[r#"
        SOURCE_FILE@0..28
          RULE_SET@0..27
            SELECTOR_LIST@0..14
              SELECTOR@0..14
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
                PSEUDO_SELECTOR@2..10
                  COLON_COLON@2..4 "::"
                  IDENT@4..10 "before"
                COMBINATOR@10..12
                  WHITESPACE@10..11 " "
                  GT@11..12 ">"
                SIMPLE_SELECTOR@12..14
                  WHITESPACE@12..13 " "
                  STAR@13..14 "*"
            BLOCK@14..27
              LBRACE@14..14 ""
              DECLARATION@14..27
                PROPERTY@14..22
                  WHITESPACE@14..17 "\n  "
                  IDENT@17..22 "color"
                COLON@22..23 ":"
                VALUE@23..27
                  VALUE@23..27
                    WHITESPACE@23..24 " "
                    IDENT@24..27 "red"
                SEMICOLON@27..27 ""
              RBRACE@27..27 ""
          WHITESPACE@27..28 "\n"
    "#]],
    );
}

#[test]
fn value_context_is_cleared_when_a_statement_ends() {
    // The declaration's colon must not leak into the sibling rule's selector, or its
    // trailing `*` is read as multiplication again. Nothing else in the suite covers
    // the reset on the virtual-SEMICOLON path.
    check_tree(
        "a\n  color: red\n  .b > *\n    width: 1px\n",
        expect![[r#"
        SOURCE_FILE@0..39
          RULE_SET@0..38
            SELECTOR_LIST@0..1
              SELECTOR@0..1
                SIMPLE_SELECTOR@0..1
                  IDENT@0..1 "a"
            BLOCK@1..38
              LBRACE@1..1 ""
              DECLARATION@1..14
                PROPERTY@1..9
                  WHITESPACE@1..4 "\n  "
                  IDENT@4..9 "color"
                COLON@9..10 ":"
                VALUE@10..14
                  VALUE@10..14
                    WHITESPACE@10..11 " "
                    IDENT@11..14 "red"
                SEMICOLON@14..14 ""
              RULE_SET@14..38
                SELECTOR_LIST@14..23
                  SELECTOR@14..23
                    SIMPLE_SELECTOR@14..19
                      WHITESPACE@14..17 "\n  "
                      DOT@17..18 "."
                      IDENT@18..19 "b"
                    COMBINATOR@19..21
                      WHITESPACE@19..20 " "
                      GT@20..21 ">"
                    SIMPLE_SELECTOR@21..23
                      WHITESPACE@21..22 " "
                      STAR@22..23 "*"
                BLOCK@23..38
                  LBRACE@23..23 ""
                  DECLARATION@23..38
                    PROPERTY@23..33
                      WHITESPACE@23..28 "\n    "
                      IDENT@28..33 "width"
                    COLON@33..34 ":"
                    VALUE@34..38
                      DIMENSION@34..38
                        WHITESPACE@34..35 " "
                        NUMBER@35..36 "1"
                        IDENT@36..38 "px"
                    SEMICOLON@38..38 ""
                  RBRACE@38..38 ""
              RBRACE@38..38 ""
          WHITESPACE@38..39 "\n"
    "#]],
    );
}

#[test]
fn value_context_is_cleared_when_a_block_opens() {
    // A pseudo-class colon in the *parent* selector must not leak into the nested
    // rule. Without the reset on the virtual-LBRACE path this misparses silently —
    // no diagnostic, just a declaration swallowed into a selector.
    check_tree(
        "a:hover\n  .b > *\n    color: red\n",
        expect![[r#"
        SOURCE_FILE@0..32
          RULE_SET@0..31
            SELECTOR_LIST@0..7
              SELECTOR@0..7
                SIMPLE_SELECTOR@0..1
                  IDENT@0..1 "a"
                PSEUDO_SELECTOR@1..7
                  COLON@1..2 ":"
                  IDENT@2..7 "hover"
            BLOCK@7..31
              LBRACE@7..7 ""
              RULE_SET@7..31
                SELECTOR_LIST@7..16
                  SELECTOR@7..16
                    SIMPLE_SELECTOR@7..12
                      WHITESPACE@7..10 "\n  "
                      DOT@10..11 "."
                      IDENT@11..12 "b"
                    COMBINATOR@12..14
                      WHITESPACE@12..13 " "
                      GT@13..14 ">"
                    SIMPLE_SELECTOR@14..16
                      WHITESPACE@14..15 " "
                      STAR@15..16 "*"
                BLOCK@16..31
                  LBRACE@16..16 ""
                  DECLARATION@16..31
                    PROPERTY@16..26
                      WHITESPACE@16..21 "\n    "
                      IDENT@21..26 "color"
                    COLON@26..27 ":"
                    VALUE@27..31
                      VALUE@27..31
                        WHITESPACE@27..28 " "
                        IDENT@28..31 "red"
                    SEMICOLON@31..31 ""
                  RBRACE@31..31 ""
              RBRACE@31..31 ""
          WHITESPACE@31..32 "\n"
    "#]],
    );
}

#[test]
fn value_context_is_cleared_when_a_deferred_block_opens() {
    // Same reset, third path: an at-rule header defers its LBRACE, so the flush site
    // inside the main loop clears the flag rather than `process_newline`.
    check_tree(
        "@page :first\n  .b > *\n    color: red\n",
        expect![[r#"
        SOURCE_FILE@0..37
          PAGE_RULE@0..36
            AT@0..1 "@"
            IDENT@1..5 "page"
            WHITESPACE@5..6 " "
            COLON@6..7 ":"
            IDENT@7..12 "first"
            BLOCK@12..36
              WHITESPACE@12..15 "\n  "
              LBRACE@15..15 ""
              RULE_SET@15..36
                SELECTOR_LIST@15..21
                  SELECTOR@15..21
                    SIMPLE_SELECTOR@15..17
                      DOT@15..16 "."
                      IDENT@16..17 "b"
                    COMBINATOR@17..19
                      WHITESPACE@17..18 " "
                      GT@18..19 ">"
                    SIMPLE_SELECTOR@19..21
                      WHITESPACE@19..20 " "
                      STAR@20..21 "*"
                BLOCK@21..36
                  LBRACE@21..21 ""
                  DECLARATION@21..36
                    PROPERTY@21..31
                      WHITESPACE@21..26 "\n    "
                      IDENT@26..31 "color"
                    COLON@31..32 ":"
                    VALUE@32..36
                      VALUE@32..36
                        WHITESPACE@32..33 " "
                        IDENT@33..36 "red"
                    SEMICOLON@36..36 ""
                  RBRACE@36..36 ""
              RBRACE@36..36 ""
          WHITESPACE@36..37 "\n"
    "#]],
    );
}

#[test]
fn pseudo_class_before_universal_selector_is_a_known_limitation() {
    // A pseudo-class colon is indistinguishable from a declaration colon at the lexer
    // level, so it sets the same value-context flag and a trailing `*` on the same
    // statement is read as multiplication. The block indented under it is then absorbed
    // into the selector.
    //
    // Note this snapshot reports NO errors: the misparse is silent whenever the
    // swallowed value is an identifier (`color: red` reads as a pseudo-class), and only
    // becomes loud when it cannot be one (`opacity: 0`). Whether a diagnostic appears is
    // decided by the value's token type, not by the bug — so a partial fix that only
    // addresses the loud path would leave this test green and the defect alive.
    //
    // This form occurs zero times across the Bulma/Vuetify/Quasar corpus. See the
    // known-limitations entry in CHANGELOG.md.
    check_tree(
        ".a:hover > *\n  color: red\n",
        expect![[r#"
        SOURCE_FILE@0..26
          RULE_SET@0..25
            SELECTOR_LIST@0..25
              SELECTOR@0..25
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
                PSEUDO_SELECTOR@2..8
                  COLON@2..3 ":"
                  IDENT@3..8 "hover"
                COMBINATOR@8..10
                  WHITESPACE@8..9 " "
                  GT@9..10 ">"
                SIMPLE_SELECTOR@10..12
                  WHITESPACE@10..11 " "
                  STAR@11..12 "*"
                SIMPLE_SELECTOR@12..20
                  WHITESPACE@12..15 "\n  "
                  IDENT@15..20 "color"
                PSEUDO_SELECTOR@20..25
                  COLON@20..21 ":"
                  WHITESPACE@21..22 " "
                  IDENT@22..25 "red"
          SEMICOLON@25..25 ""
          WHITESPACE@25..26 "\n"
    "#]],
    );
}

#[test]
fn pseudo_class_poisons_a_trailing_star_on_another_line_of_the_selector_list() {
    // The flag is statement-scoped, and a comma-continued selector list is one
    // statement — so the pseudo-class and the trailing `*` need not share a line.
    // Also silent. Same limitation as above, wider blast radius.
    check_tree(
        ".a:focus,\n.b *\n  outline: none\n",
        expect![[r#"
        SOURCE_FILE@0..31
          RULE_SET@0..30
            SELECTOR_LIST@0..30
              SELECTOR@0..8
                SIMPLE_SELECTOR@0..2
                  DOT@0..1 "."
                  IDENT@1..2 "a"
                PSEUDO_SELECTOR@2..8
                  COLON@2..3 ":"
                  IDENT@3..8 "focus"
              COMMA@8..9 ","
              SELECTOR@9..30
                SIMPLE_SELECTOR@9..12
                  WHITESPACE@9..10 "\n"
                  DOT@10..11 "."
                  IDENT@11..12 "b"
                SIMPLE_SELECTOR@12..14
                  WHITESPACE@12..13 " "
                  STAR@13..14 "*"
                SIMPLE_SELECTOR@14..24
                  WHITESPACE@14..17 "\n  "
                  IDENT@17..24 "outline"
                PSEUDO_SELECTOR@24..30
                  COLON@24..25 ":"
                  WHITESPACE@25..26 " "
                  IDENT@26..30 "none"
          SEMICOLON@30..30 ""
          WHITESPACE@30..31 "\n"
    "#]],
    );
}

#[test]
fn forward_as_prefix_star_followed_by_rule() {
    check_tree(
        "@forward 'lib' as btn-*\n.x\n  color: red\n",
        expect![[r#"
        SOURCE_FILE@0..40
          FORWARD_RULE@0..23
            AT@0..1 "@"
            IDENT@1..8 "forward"
            WHITESPACE@8..9 " "
            QUOTED_STRING@9..14 "'lib'"
            WHITESPACE@14..15 " "
            IDENT@15..17 "as"
            WHITESPACE@17..18 " "
            IDENT@18..22 "btn-"
            STAR@22..23 "*"
            SEMICOLON@23..23 ""
          RULE_SET@23..39
            SELECTOR_LIST@23..26
              SELECTOR@23..26
                SIMPLE_SELECTOR@23..26
                  WHITESPACE@23..24 "\n"
                  DOT@24..25 "."
                  IDENT@25..26 "x"
            BLOCK@26..39
              LBRACE@26..26 ""
              DECLARATION@26..39
                PROPERTY@26..34
                  WHITESPACE@26..29 "\n  "
                  IDENT@29..34 "color"
                COLON@34..35 ":"
                VALUE@35..39
                  VALUE@35..39
                    WHITESPACE@35..36 " "
                    IDENT@36..39 "red"
                SEMICOLON@39..39 ""
              RBRACE@39..39 ""
          WHITESPACE@39..40 "\n"
    "#]],
    );
}

#[test]
fn use_with_multiplication_in_config_still_parses() {
    check_tree(
        "@use 'a' with ($w: 2 * 3)\n.x\n  color: red\n",
        expect![[r#"
        SOURCE_FILE@0..42
          USE_RULE@0..25
            AT@0..1 "@"
            IDENT@1..4 "use"
            WHITESPACE@4..5 " "
            QUOTED_STRING@5..8 "'a'"
            WHITESPACE@8..9 " "
            IDENT@9..13 "with"
            WHITESPACE@13..14 " "
            LPAREN@14..15 "("
            DOLLAR@15..16 "$"
            IDENT@16..17 "w"
            COLON@17..18 ":"
            BINARY_EXPR@18..24
              NUMBER_LITERAL@18..20
                WHITESPACE@18..19 " "
                NUMBER@19..20 "2"
              WHITESPACE@20..21 " "
              STAR@21..22 "*"
              NUMBER_LITERAL@22..24
                WHITESPACE@22..23 " "
                NUMBER@23..24 "3"
            RPAREN@24..25 ")"
            SEMICOLON@25..25 ""
          RULE_SET@25..41
            SELECTOR_LIST@25..28
              SELECTOR@25..28
                SIMPLE_SELECTOR@25..28
                  WHITESPACE@25..26 "\n"
                  DOT@26..27 "."
                  IDENT@27..28 "x"
            BLOCK@28..41
              LBRACE@28..28 ""
              DECLARATION@28..41
                PROPERTY@28..36
                  WHITESPACE@28..31 "\n  "
                  IDENT@31..36 "color"
                COLON@36..37 ":"
                VALUE@37..41
                  VALUE@37..41
                    WHITESPACE@37..38 " "
                    IDENT@38..41 "red"
                SEMICOLON@41..41 ""
              RBRACE@41..41 ""
          WHITESPACE@41..42 "\n"
    "#]],
    );
}
