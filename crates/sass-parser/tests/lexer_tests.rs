use sass_parser::lexer::{Lexer, tokenize};
use sass_parser::syntax_kind::*;

fn lex(input: &str) -> Vec<(SyntaxKind, &str)> {
    tokenize(input)
}

// ── EOF ────────────────────────────────────────────────────────────────

#[test]
fn empty_input() {
    let tokens = lex("");
    assert!(tokens.is_empty());

    let mut lexer = Lexer::new("");
    assert_eq!(lexer.next_token(), (EOF, ""));
}

#[test]
fn eof_repeats() {
    let mut lexer = Lexer::new("");
    assert_eq!(lexer.next_token(), (EOF, ""));
    assert_eq!(lexer.next_token(), (EOF, ""));
    assert_eq!(lexer.next_token(), (EOF, ""));
}

// ── Whitespace (1.3) ──────────────────────────────────────────────────

#[test]
fn whitespace_spaces() {
    assert_eq!(lex("   "), vec![(WHITESPACE, "   ")]);
}

#[test]
fn whitespace_tabs_and_newlines() {
    assert_eq!(lex("\t\n\r\n"), vec![(WHITESPACE, "\t\n\r\n")]);
}

#[test]
fn whitespace_form_feed() {
    assert_eq!(lex("\x0C"), vec![(WHITESPACE, "\x0C")]);
}

#[test]
fn whitespace_between_tokens() {
    assert_eq!(
        lex("a b"),
        vec![(IDENT, "a"), (WHITESPACE, " "), (IDENT, "b")]
    );
}

// ── Comments (1.4) ────────────────────────────────────────────────────

#[test]
fn single_line_comment() {
    assert_eq!(lex("// hello"), vec![(SINGLE_LINE_COMMENT, "// hello")]);
}

#[test]
fn single_line_comment_stops_at_newline() {
    assert_eq!(
        lex("// hi\nfoo"),
        vec![
            (SINGLE_LINE_COMMENT, "// hi"),
            (WHITESPACE, "\n"),
            (IDENT, "foo"),
        ]
    );
}

#[test]
fn multi_line_comment() {
    assert_eq!(
        lex("/* hello */"),
        vec![(MULTI_LINE_COMMENT, "/* hello */")]
    );
}

#[test]
fn multi_line_comment_with_stars() {
    assert_eq!(
        lex("/*** star ***/"),
        vec![(MULTI_LINE_COMMENT, "/*** star ***/")]
    );
}

#[test]
fn multi_line_comment_spans_lines() {
    assert_eq!(
        lex("/* a\n * b\n */"),
        vec![(MULTI_LINE_COMMENT, "/* a\n * b\n */")]
    );
}

#[test]
fn unterminated_block_comment() {
    assert_eq!(lex("/* oops"), vec![(ERROR, "/* oops")]);
}

#[test]
fn slash_alone() {
    assert_eq!(lex("/"), vec![(SLASH, "/")]);
}

#[test]
fn empty_single_line_comment() {
    assert_eq!(lex("//"), vec![(SINGLE_LINE_COMMENT, "//")]);
}

// ── Identifiers (1.5) ─────────────────────────────────────────────────

#[test]
fn simple_ident() {
    assert_eq!(lex("foo"), vec![(IDENT, "foo")]);
}

#[test]
fn ident_with_hyphens() {
    assert_eq!(lex("-webkit-foo"), vec![(IDENT, "-webkit-foo")]);
}

#[test]
fn custom_property_ident() {
    assert_eq!(lex("--custom"), vec![(IDENT, "--custom")]);
}

#[test]
fn ident_with_underscore_and_digits() {
    assert_eq!(lex("_foo123"), vec![(IDENT, "_foo123")]);
}

#[test]
fn unicode_ident() {
    assert_eq!(lex("café"), vec![(IDENT, "café")]);
}

#[test]
fn ident_starts_with_unicode() {
    assert_eq!(lex("über"), vec![(IDENT, "über")]);
}

#[test]
fn bare_hyphen_is_minus() {
    assert_eq!(lex("-"), vec![(MINUS, "-")]);
}

#[test]
fn hyphen_before_non_ident_is_minus() {
    assert_eq!(lex("- "), vec![(MINUS, "-"), (WHITESPACE, " ")]);
}

#[test]
fn hyphen_digit_is_minus_number() {
    assert_eq!(lex("-3"), vec![(MINUS, "-"), (NUMBER, "3")]);
}

#[test]
fn hyphen_digit_unit() {
    assert_eq!(
        lex("-10px"),
        vec![(MINUS, "-"), (NUMBER, "10"), (IDENT, "px")]
    );
}

#[test]
fn hyphen_dot_digit() {
    assert_eq!(lex("-.5"), vec![(MINUS, "-"), (NUMBER, ".5")]);
}

// ── Numbers (1.6) ─────────────────────────────────────────────────────

#[test]
fn integer() {
    assert_eq!(lex("42"), vec![(NUMBER, "42")]);
}

#[test]
fn decimal_number() {
    assert_eq!(lex("3.14"), vec![(NUMBER, "3.14")]);
}

#[test]
fn leading_dot_number() {
    assert_eq!(lex(".5"), vec![(NUMBER, ".5")]);
}

#[test]
fn number_with_unit_is_two_tokens() {
    assert_eq!(lex("10px"), vec![(NUMBER, "10"), (IDENT, "px")]);
}

#[test]
fn dot_without_digit_is_dot() {
    assert_eq!(lex(".class"), vec![(DOT, "."), (IDENT, "class")]);
}

#[test]
fn number_dot_no_digit_is_number_dot() {
    assert_eq!(
        lex("10.class"),
        vec![(NUMBER, "10"), (DOT, "."), (IDENT, "class")]
    );
}

// ── Single-char operators (1.9) ───────────────────────────────────────

#[test]
fn all_single_char_punctuation() {
    let input = ";:,.(){}[]+-*%=><!&~|@$#";
    let tokens = lex(input);
    #[rustfmt::skip]
    let expected = vec![
        (SEMICOLON,  ";"),
        (COLON,      ":"),
        (COMMA,      ","),
        (DOT,        "."),
        (LPAREN,     "("),
        (RPAREN,     ")"),
        (LBRACE,     "{"),
        (RBRACE,     "}"),
        (LBRACKET,   "["),
        (RBRACKET,   "]"),
        (PLUS,       "+"),
        (MINUS,      "-"),
        (STAR,       "*"),
        (PERCENT,    "%"),
        (EQ,         "="),
        (GT,         ">"),
        (LT,         "<"),
        (BANG,       "!"),
        (AMP,        "&"),
        (TILDE,      "~"),
        (PIPE,       "|"),
        (AT,         "@"),
        (DOLLAR,     "$"),
        (HASH,       "#"),
    ];
    assert_eq!(tokens, expected);
}

// ── Multi-char operators (1.9) ────────────────────────────────────────

#[test]
fn hash_lbrace() {
    assert_eq!(lex("#{"), vec![(HASH_LBRACE, "#{")]);
}

#[test]
fn dot_dot_dot() {
    assert_eq!(lex("..."), vec![(DOT_DOT_DOT, "...")]);
}

#[test]
fn double_dot_is_two_dots() {
    assert_eq!(lex(".."), vec![(DOT, "."), (DOT, ".")]);
}

#[test]
fn comparison_operators() {
    assert_eq!(
        lex("== != <= >="),
        vec![
            (EQ_EQ, "=="),
            (WHITESPACE, " "),
            (BANG_EQ, "!="),
            (WHITESPACE, " "),
            (LT_EQ, "<="),
            (WHITESPACE, " "),
            (GT_EQ, ">="),
        ]
    );
}

#[test]
fn colon_colon() {
    assert_eq!(lex("::"), vec![(COLON_COLON, "::")]);
}

#[test]
fn attribute_selector_operators() {
    assert_eq!(
        lex("~= |= ^= $= *="),
        vec![
            (TILDE_EQ, "~="),
            (WHITESPACE, " "),
            (PIPE_EQ, "|="),
            (WHITESPACE, " "),
            (CARET_EQ, "^="),
            (WHITESPACE, " "),
            (DOLLAR_EQ, "$="),
            (WHITESPACE, " "),
            (STAR_EQ, "*="),
        ]
    );
}

// ── Error handling ────────────────────────────────────────────────────

#[test]
fn unknown_char_is_error() {
    assert_eq!(lex("\x01"), vec![(ERROR, "\x01")]);
}

#[test]
fn multibyte_unknown_char() {
    assert_eq!(lex("🦀"), vec![(ERROR, "🦀")]);
}

#[test]
fn standalone_caret_is_error() {
    assert_eq!(lex("^"), vec![(ERROR, "^")]);
}

// ── Round-trip ────────────────────────────────────────────────────────

#[test]
fn round_trip_basic() {
    let inputs = [
        "",
        "   ",
        "hello",
        "42",
        ".5",
        "// comment\n",
        "/* block */",
        "/* unterminated",
        ";:,.(){}[]+-*/%=><!&~|@$#",
        "#{",
        "...",
        "== != <= >=",
        ":: ~= |= ^= $= *=",
        "-webkit-foo",
        "--custom",
        "café",
        "🦀",
        "\x01",
    ];
    for input in inputs {
        let tokens = lex(input);
        let reconstructed: String = tokens.iter().map(|(_, text)| *text).collect();
        assert_eq!(reconstructed, input, "round-trip failed for {input:?}");
    }
}

#[test]
fn round_trip_real_css() {
    let input = "div.class > #id:hover {\n  color: red; /* comment */\n  font-size: 16px;\n}\n";
    let tokens = lex(input);
    let reconstructed: String = tokens.iter().map(|(_, text)| *text).collect();
    assert_eq!(reconstructed, input);
}
