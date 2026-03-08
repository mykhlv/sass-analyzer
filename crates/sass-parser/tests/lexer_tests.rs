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

// ── Strings (1.7) ────────────────────────────────────────────────────

#[test]
fn double_quoted_string() {
    assert_eq!(lex("\"hello\""), vec![(QUOTED_STRING, "\"hello\"")]);
}

#[test]
fn single_quoted_string() {
    assert_eq!(lex("'hello'"), vec![(QUOTED_STRING, "'hello'")]);
}

#[test]
fn empty_double_string() {
    assert_eq!(lex("\"\""), vec![(QUOTED_STRING, "\"\"")]);
}

#[test]
fn empty_single_string() {
    assert_eq!(lex("''"), vec![(QUOTED_STRING, "''")]);
}

#[test]
fn string_with_escaped_quote() {
    assert_eq!(lex("\"he\\\"llo\""), vec![(QUOTED_STRING, "\"he\\\"llo\"")]);
}

#[test]
fn string_with_escaped_backslash() {
    assert_eq!(lex("\"a\\\\b\""), vec![(QUOTED_STRING, "\"a\\\\b\"")]);
}

#[test]
fn unterminated_double_string() {
    assert_eq!(lex("\"hello"), vec![(ERROR, "\"hello")]);
}

#[test]
fn unterminated_single_string() {
    assert_eq!(lex("'hello"), vec![(ERROR, "'hello")]);
}

#[test]
fn string_in_declaration() {
    assert_eq!(
        lex("content: \"red\";"),
        vec![
            (IDENT, "content"),
            (COLON, ":"),
            (WHITESPACE, " "),
            (QUOTED_STRING, "\"red\""),
            (SEMICOLON, ";"),
        ]
    );
}

#[test]
fn string_with_hash_no_brace() {
    assert_eq!(lex("\"#fff\""), vec![(QUOTED_STRING, "\"#fff\"")]);
}

#[test]
fn escaped_hash_prevents_interpolation() {
    assert_eq!(lex("\"\\#{$x}\""), vec![(QUOTED_STRING, "\"\\#{$x}\"")]);
}

#[test]
fn escaped_hash_non_brace() {
    assert_eq!(lex("\"\\#fff\""), vec![(QUOTED_STRING, "\"\\#fff\"")]);
}

#[test]
fn unterminated_string_trailing_backslash() {
    assert_eq!(lex("\"hello\\"), vec![(ERROR, "\"hello\\")]);
}

#[test]
fn string_with_escaped_multibyte() {
    assert_eq!(lex("\"\\é\""), vec![(QUOTED_STRING, "\"\\é\"")]);
}

#[test]
fn double_backslash_before_interpolation() {
    // \\#{ — first \ escapes second \, so #{ triggers interpolation
    assert_eq!(
        lex("\"\\\\#{$x}\""),
        vec![
            (STRING_START, "\"\\\\"),
            (HASH_LBRACE, "#{"),
            (DOLLAR, "$"),
            (IDENT, "x"),
            (RBRACE, "}"),
            (STRING_END, "\""),
        ]
    );
}

// ── String interpolation (1.8) ───────────────────────────────────────

#[test]
fn string_with_interpolation() {
    assert_eq!(
        lex("\"hello #{$x}\""),
        vec![
            (STRING_START, "\"hello "),
            (HASH_LBRACE, "#{"),
            (DOLLAR, "$"),
            (IDENT, "x"),
            (RBRACE, "}"),
            (STRING_END, "\""),
        ]
    );
}

#[test]
fn string_interpolation_at_start() {
    assert_eq!(
        lex("\"#{$x} world\""),
        vec![
            (STRING_START, "\""),
            (HASH_LBRACE, "#{"),
            (DOLLAR, "$"),
            (IDENT, "x"),
            (RBRACE, "}"),
            (STRING_END, " world\""),
        ]
    );
}

#[test]
fn string_interpolation_only() {
    assert_eq!(
        lex("\"#{$x}\""),
        vec![
            (STRING_START, "\""),
            (HASH_LBRACE, "#{"),
            (DOLLAR, "$"),
            (IDENT, "x"),
            (RBRACE, "}"),
            (STRING_END, "\""),
        ]
    );
}

#[test]
fn string_multiple_interpolations() {
    assert_eq!(
        lex("\"#{$a} and #{$b}\""),
        vec![
            (STRING_START, "\""),
            (HASH_LBRACE, "#{"),
            (DOLLAR, "$"),
            (IDENT, "a"),
            (RBRACE, "}"),
            (STRING_MID, " and "),
            (HASH_LBRACE, "#{"),
            (DOLLAR, "$"),
            (IDENT, "b"),
            (RBRACE, "}"),
            (STRING_END, "\""),
        ]
    );
}

#[test]
fn string_adjacent_interpolations() {
    assert_eq!(
        lex("\"#{$a}#{$b}\""),
        vec![
            (STRING_START, "\""),
            (HASH_LBRACE, "#{"),
            (DOLLAR, "$"),
            (IDENT, "a"),
            (RBRACE, "}"),
            (HASH_LBRACE, "#{"),
            (DOLLAR, "$"),
            (IDENT, "b"),
            (RBRACE, "}"),
            (STRING_END, "\""),
        ]
    );
}

#[test]
fn single_quoted_interpolation() {
    assert_eq!(
        lex("'hello #{$x}'"),
        vec![
            (STRING_START, "'hello "),
            (HASH_LBRACE, "#{"),
            (DOLLAR, "$"),
            (IDENT, "x"),
            (RBRACE, "}"),
            (STRING_END, "'"),
        ]
    );
}

#[test]
fn nested_string_in_interpolation() {
    assert_eq!(
        lex("\"a #{\"b\"} c\""),
        vec![
            (STRING_START, "\"a "),
            (HASH_LBRACE, "#{"),
            (QUOTED_STRING, "\"b\""),
            (RBRACE, "}"),
            (STRING_END, " c\""),
        ]
    );
}

#[test]
fn deeply_nested_interpolation() {
    assert_eq!(
        lex("\"a #{\"b #{$c} d\"} e\""),
        vec![
            (STRING_START, "\"a "),
            (HASH_LBRACE, "#{"),
            (STRING_START, "\"b "),
            (HASH_LBRACE, "#{"),
            (DOLLAR, "$"),
            (IDENT, "c"),
            (RBRACE, "}"),
            (STRING_END, " d\""),
            (RBRACE, "}"),
            (STRING_END, " e\""),
        ]
    );
}

#[test]
fn interpolation_with_expression() {
    assert_eq!(
        lex("\"#{1 + 2}\""),
        vec![
            (STRING_START, "\""),
            (HASH_LBRACE, "#{"),
            (NUMBER, "1"),
            (WHITESPACE, " "),
            (PLUS, "+"),
            (WHITESPACE, " "),
            (NUMBER, "2"),
            (RBRACE, "}"),
            (STRING_END, "\""),
        ]
    );
}

#[test]
fn interpolation_with_braces_inside() {
    // Braces inside interpolation are tracked for depth
    assert_eq!(
        lex("\"#{fn()} x\""),
        vec![
            (STRING_START, "\""),
            (HASH_LBRACE, "#{"),
            (IDENT, "fn"),
            (LPAREN, "("),
            (RPAREN, ")"),
            (RBRACE, "}"),
            (STRING_END, " x\""),
        ]
    );
}

#[test]
fn unterminated_string_after_interpolation() {
    assert_eq!(
        lex("\"#{$x} oops"),
        vec![
            (STRING_START, "\""),
            (HASH_LBRACE, "#{"),
            (DOLLAR, "$"),
            (IDENT, "x"),
            (RBRACE, "}"),
            (ERROR, " oops"),
        ]
    );
}

#[test]
fn escaped_hash_in_string_content() {
    // After interpolation, \# should NOT start a second interpolation
    assert_eq!(
        lex("\"#{$a}\\#{$b}\""),
        vec![
            (STRING_START, "\""),
            (HASH_LBRACE, "#{"),
            (DOLLAR, "$"),
            (IDENT, "a"),
            (RBRACE, "}"),
            (STRING_END, "\\#{$b}\""),
        ]
    );
}

#[test]
fn braces_inside_interpolation() {
    // { and } inside interpolation are depth-tracked
    assert_eq!(
        lex("\"#{ {a} }\""),
        vec![
            (STRING_START, "\""),
            (HASH_LBRACE, "#{"),
            (WHITESPACE, " "),
            (LBRACE, "{"),
            (IDENT, "a"),
            (RBRACE, "}"),
            (WHITESPACE, " "),
            (RBRACE, "}"),
            (STRING_END, "\""),
        ]
    );
}

// ── Unicode range (1.13) ─────────────────────────────────────────────

#[test]
fn unicode_range_single_value() {
    assert_eq!(lex("U+0041"), vec![(UNICODE_RANGE, "U+0041")]);
}

#[test]
fn unicode_range_lowercase() {
    assert_eq!(lex("u+00ff"), vec![(UNICODE_RANGE, "u+00ff")]);
}

#[test]
fn unicode_range_with_hyphen() {
    assert_eq!(lex("U+0025-00FF"), vec![(UNICODE_RANGE, "U+0025-00FF")]);
}

#[test]
fn unicode_range_wildcard() {
    assert_eq!(lex("U+00??"), vec![(UNICODE_RANGE, "U+00??")]);
}

#[test]
fn unicode_range_all_wildcards() {
    assert_eq!(lex("U+????"), vec![(UNICODE_RANGE, "U+????")]);
}

#[test]
fn unicode_range_in_context() {
    assert_eq!(
        lex("U+0025-00FF,"),
        vec![(UNICODE_RANGE, "U+0025-00FF"), (COMMA, ",")]
    );
}

#[test]
fn u_without_plus_is_ident() {
    assert_eq!(lex("U"), vec![(IDENT, "U")]);
}

#[test]
fn u_plus_non_hex_is_ident_plus() {
    assert_eq!(lex("U+z"), vec![(IDENT, "U"), (PLUS, "+"), (IDENT, "z")]);
}

#[test]
fn ufoo_is_ident() {
    assert_eq!(lex("Ufoo"), vec![(IDENT, "Ufoo")]);
}

#[test]
fn unicode_range_hyphen_no_hex() {
    // U+0041- without hex after hyphen → range stops, -z is ident
    assert_eq!(
        lex("U+0041-z"),
        vec![(UNICODE_RANGE, "U+0041"), (IDENT, "-z")]
    );
}

// ── BOM and special bytes (1.14) ─────────────────────────────────────

#[test]
fn bom_at_start() {
    assert_eq!(
        lex("\u{FEFF}hello"),
        vec![(WHITESPACE, "\u{FEFF}"), (IDENT, "hello")]
    );
}

#[test]
fn bom_alone() {
    assert_eq!(lex("\u{FEFF}"), vec![(WHITESPACE, "\u{FEFF}")]);
}

#[test]
fn bom_followed_by_whitespace() {
    assert_eq!(
        lex("\u{FEFF} x"),
        vec![(WHITESPACE, "\u{FEFF}"), (WHITESPACE, " "), (IDENT, "x"),]
    );
}

#[test]
fn null_byte_is_error() {
    assert_eq!(lex("\x00"), vec![(ERROR, "\x00")]);
}

// ── CRLF handling (1.15) ────────────────────────────────────────────

#[test]
fn crlf_is_whitespace() {
    assert_eq!(
        lex("a\r\nb"),
        vec![(IDENT, "a"), (WHITESPACE, "\r\n"), (IDENT, "b")]
    );
}

#[test]
fn cr_alone_is_whitespace() {
    assert_eq!(
        lex("a\rb"),
        vec![(IDENT, "a"), (WHITESPACE, "\r"), (IDENT, "b")]
    );
}

#[test]
fn comment_with_crlf() {
    assert_eq!(
        lex("// hi\r\nx"),
        vec![
            (SINGLE_LINE_COMMENT, "// hi\r"),
            (WHITESPACE, "\n"),
            (IDENT, "x"),
        ]
    );
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
        // Strings
        "\"hello world\"",
        "'single'",
        "\"\"",
        "\"unterminated",
        "\"esc\\\"ape\"",
        // String interpolation
        "\"hello #{$x}\"",
        "\"#{$a} and #{$b}\"",
        "\"#{$a}#{$b}\"",
        "\"a #{\"b\"} c\"",
        "\"a #{\"b #{$c} d\"} e\"",
        "\"\\#{$x}\"",
        "\"\\\\#{$x}\"",
        "\"#{$a}\\#{$b}\"",
        "\"#{ {a} }\"",
        "\"hello\\",
        // Unicode range
        "U+0041",
        "u+00ff",
        "U+0025-00FF",
        "U+00??",
        // BOM
        "\u{FEFF}hello",
        // CRLF
        "a\r\nb",
        "// comment\r\nx",
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

#[test]
fn round_trip_scss_with_strings() {
    let input =
        "$name: 'world';\n.greeting {\n  content: \"hello #{$name}\";\n  font: \"Arial\";\n}\n";
    let tokens = lex(input);
    let reconstructed: String = tokens.iter().map(|(_, text)| *text).collect();
    assert_eq!(reconstructed, input);
}
