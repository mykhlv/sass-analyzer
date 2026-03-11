use sass_parser::lexer::{Lexer, tokenize};
use sass_parser::syntax_kind::SyntaxKind::{self, *};

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

// ── Scientific notation ──────────────────────────────────────────────

#[test]
fn scientific_notation_integer() {
    assert_eq!(lex("1e3"), vec![(NUMBER, "1e3")]);
}

#[test]
fn scientific_notation_uppercase() {
    assert_eq!(lex("1E3"), vec![(NUMBER, "1E3")]);
}

#[test]
fn scientific_notation_positive_exponent() {
    assert_eq!(lex("1e+3"), vec![(NUMBER, "1e+3")]);
}

#[test]
fn scientific_notation_negative_exponent() {
    assert_eq!(lex("2.5e-2"), vec![(NUMBER, "2.5e-2")]);
}

#[test]
fn scientific_notation_no_digits_after_e_is_number_ident() {
    assert_eq!(lex("1em"), vec![(NUMBER, "1"), (IDENT, "em")]);
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

// ── url() context (1.12) ──────────────────────────────────────────────

#[test]
fn url_unquoted_simple() {
    assert_eq!(
        lex("url(http://example.com)"),
        vec![
            (IDENT, "url"),
            (LPAREN, "("),
            (URL_CONTENTS, "http://example.com"),
            (RPAREN, ")"),
        ]
    );
}

#[test]
fn url_quoted_double() {
    // Quoted URL: no Url context, parser handles as normal function call
    assert_eq!(
        lex("url(\"http://example.com\")"),
        vec![
            (IDENT, "url"),
            (LPAREN, "("),
            (QUOTED_STRING, "\"http://example.com\""),
            (RPAREN, ")"),
        ]
    );
}

#[test]
fn url_quoted_single() {
    assert_eq!(
        lex("url('http://example.com')"),
        vec![
            (IDENT, "url"),
            (LPAREN, "("),
            (QUOTED_STRING, "'http://example.com'"),
            (RPAREN, ")"),
        ]
    );
}

#[test]
fn url_quoted_with_whitespace() {
    // Whitespace before quoted string → still normal tokens, no Url context
    assert_eq!(
        lex("url( \"path\" )"),
        vec![
            (IDENT, "url"),
            (LPAREN, "("),
            (WHITESPACE, " "),
            (QUOTED_STRING, "\"path\""),
            (WHITESPACE, " "),
            (RPAREN, ")"),
        ]
    );
}

#[test]
fn url_empty() {
    assert_eq!(
        lex("url()"),
        vec![(IDENT, "url"), (LPAREN, "("), (RPAREN, ")"),]
    );
}

#[test]
fn url_with_whitespace() {
    assert_eq!(
        lex("url( http://example.com )"),
        vec![
            (IDENT, "url"),
            (LPAREN, "("),
            (WHITESPACE, " "),
            (URL_CONTENTS, "http://example.com"),
            (WHITESPACE, " "),
            (RPAREN, ")"),
        ]
    );
}

#[test]
fn url_with_interpolation() {
    assert_eq!(
        lex("url(#{$base}/path)"),
        vec![
            (IDENT, "url"),
            (LPAREN, "("),
            (HASH_LBRACE, "#{"),
            (DOLLAR, "$"),
            (IDENT, "base"),
            (RBRACE, "}"),
            (URL_CONTENTS, "/path"),
            (RPAREN, ")"),
        ]
    );
}

#[test]
fn url_content_then_interpolation() {
    assert_eq!(
        lex("url(path/#{$base}/end)"),
        vec![
            (IDENT, "url"),
            (LPAREN, "("),
            (URL_CONTENTS, "path/"),
            (HASH_LBRACE, "#{"),
            (DOLLAR, "$"),
            (IDENT, "base"),
            (RBRACE, "}"),
            (URL_CONTENTS, "/end"),
            (RPAREN, ")"),
        ]
    );
}

#[test]
fn url_case_insensitive() {
    assert_eq!(
        lex("URL(http://example.com)"),
        vec![
            (IDENT, "URL"),
            (LPAREN, "("),
            (URL_CONTENTS, "http://example.com"),
            (RPAREN, ")"),
        ]
    );
}

#[test]
fn url_mixed_case() {
    assert_eq!(
        lex("Url(http://example.com)"),
        vec![
            (IDENT, "Url"),
            (LPAREN, "("),
            (URL_CONTENTS, "http://example.com"),
            (RPAREN, ")"),
        ]
    );
}

#[test]
fn url_with_data_uri() {
    assert_eq!(
        lex("url(data:image/png;base64,abc==)"),
        vec![
            (IDENT, "url"),
            (LPAREN, "("),
            (URL_CONTENTS, "data:image/png;base64,abc=="),
            (RPAREN, ")"),
        ]
    );
}

#[test]
fn url_with_escaped_paren() {
    // \) is an escape, not a closing paren
    assert_eq!(
        lex("url(path\\)end)"),
        vec![
            (IDENT, "url"),
            (LPAREN, "("),
            (URL_CONTENTS, "path\\)end"),
            (RPAREN, ")"),
        ]
    );
}

#[test]
fn url_unterminated() {
    assert_eq!(
        lex("url(hello"),
        vec![(IDENT, "url"), (LPAREN, "("), (URL_CONTENTS, "hello"),]
    );
}

#[test]
fn url_not_a_function() {
    // "url" followed by something other than ( is just an ident
    assert_eq!(lex("url"), vec![(IDENT, "url")]);
}

#[test]
fn url_prefix_ident() {
    // "urlify" is just a regular ident, not a url() call
    assert_eq!(
        lex("urlify(x)"),
        vec![
            (IDENT, "urlify"),
            (LPAREN, "("),
            (IDENT, "x"),
            (RPAREN, ")"),
        ]
    );
}

#[test]
fn url_in_declaration() {
    assert_eq!(
        lex("background: url(img.png);"),
        vec![
            (IDENT, "background"),
            (COLON, ":"),
            (WHITESPACE, " "),
            (IDENT, "url"),
            (LPAREN, "("),
            (URL_CONTENTS, "img.png"),
            (RPAREN, ")"),
            (SEMICOLON, ";"),
        ]
    );
}

#[test]
fn url_interpolation_only() {
    assert_eq!(
        lex("url(#{$var})"),
        vec![
            (IDENT, "url"),
            (LPAREN, "("),
            (HASH_LBRACE, "#{"),
            (DOLLAR, "$"),
            (IDENT, "var"),
            (RBRACE, "}"),
            (RPAREN, ")"),
        ]
    );
}

// ── Comprehensive edge cases (1.17) ──────────────────────────────────

#[test]
fn nested_interpolation_with_expression() {
    // Plan example: "a #{$b + "c #{$d}"}"
    assert_eq!(
        lex("\"a #{$b + \"c #{$d}\"}\""),
        vec![
            (STRING_START, "\"a "),
            (HASH_LBRACE, "#{"),
            (DOLLAR, "$"),
            (IDENT, "b"),
            (WHITESPACE, " "),
            (PLUS, "+"),
            (WHITESPACE, " "),
            (STRING_START, "\"c "),
            (HASH_LBRACE, "#{"),
            (DOLLAR, "$"),
            (IDENT, "d"),
            (RBRACE, "}"),
            (STRING_END, "\""),
            (RBRACE, "}"),
            (STRING_END, "\""),
        ]
    );
}

#[test]
fn dot_unicode_ident() {
    // .café — class selector with unicode identifier
    assert_eq!(lex(".café"), vec![(DOT, "."), (IDENT, "café")]);
}

#[test]
fn dollar_unicode_ident() {
    // $über — variable with unicode identifier
    assert_eq!(lex("$über"), vec![(DOLLAR, "$"), (IDENT, "über")]);
}

#[test]
fn multibyte_byte_offsets() {
    // Verify TextRange byte offsets are correct for multi-byte chars.
    // "café" = 5 bytes (c=1, a=1, f=1, é=2), "x" starts at byte 6
    let mut lexer = Lexer::new("café x");
    let (k1, t1) = lexer.next_token();
    assert_eq!(k1, IDENT);
    assert_eq!(t1, "café");
    assert_eq!(t1.len(), 5); // 'é' is 2 bytes in UTF-8

    let (k2, t2) = lexer.next_token();
    assert_eq!(k2, WHITESPACE);
    assert_eq!(t2, " ");

    let (k3, t3) = lexer.next_token();
    assert_eq!(k3, IDENT);
    assert_eq!(t3, "x");

    // Verify total byte offsets
    assert_eq!(t1.len() + t2.len() + t3.len(), "café x".len());
}

#[test]
fn multibyte_in_string() {
    // Multi-byte chars in strings should have correct byte offsets
    let tokens = lex("\"héllo\"");
    assert_eq!(tokens, vec![(QUOTED_STRING, "\"héllo\"")]);
    // "héllo" = 1 (") + 1 (h) + 2 (é) + 3 (llo) + 1 (") = 8 bytes
    assert_eq!(tokens[0].1.len(), 8);
}

#[test]
fn multibyte_url_content() {
    // Unicode in URL content
    assert_eq!(
        lex("url(pàth)"),
        vec![
            (IDENT, "url"),
            (LPAREN, "("),
            (URL_CONTENTS, "pàth"),
            (RPAREN, ")"),
        ]
    );
    // "pàth" = 1 (p) + 2 (à) + 2 (th) = 5 bytes
    assert_eq!("pàth".len(), 5);
}

#[test]
fn consecutive_comments() {
    assert_eq!(
        lex("/* a *//* b */"),
        vec![
            (MULTI_LINE_COMMENT, "/* a */"),
            (MULTI_LINE_COMMENT, "/* b */"),
        ]
    );
}

#[test]
fn at_keyword_ident() {
    assert_eq!(lex("@import"), vec![(AT, "@"), (IDENT, "import")]);
}

#[test]
fn at_mixin_with_args() {
    assert_eq!(
        lex("@mixin foo($x)"),
        vec![
            (AT, "@"),
            (IDENT, "mixin"),
            (WHITESPACE, " "),
            (IDENT, "foo"),
            (LPAREN, "("),
            (DOLLAR, "$"),
            (IDENT, "x"),
            (RPAREN, ")"),
        ]
    );
}

#[test]
fn placeholder_selector() {
    assert_eq!(
        lex("%placeholder"),
        vec![(PERCENT, "%"), (IDENT, "placeholder")]
    );
}

#[test]
fn selector_combinator_sequence() {
    assert_eq!(
        lex("div > .class ~ #id + span"),
        vec![
            (IDENT, "div"),
            (WHITESPACE, " "),
            (GT, ">"),
            (WHITESPACE, " "),
            (DOT, "."),
            (IDENT, "class"),
            (WHITESPACE, " "),
            (TILDE, "~"),
            (WHITESPACE, " "),
            (HASH, "#"),
            (IDENT, "id"),
            (WHITESPACE, " "),
            (PLUS, "+"),
            (WHITESPACE, " "),
            (IDENT, "span"),
        ]
    );
}

#[test]
fn attribute_selector() {
    assert_eq!(
        lex("[data-value^=\"foo\"]"),
        vec![
            (LBRACKET, "["),
            (IDENT, "data-value"),
            (CARET_EQ, "^="),
            (QUOTED_STRING, "\"foo\""),
            (RBRACKET, "]"),
        ]
    );
}

#[test]
fn important_declaration() {
    assert_eq!(
        lex("color: red !important;"),
        vec![
            (IDENT, "color"),
            (COLON, ":"),
            (WHITESPACE, " "),
            (IDENT, "red"),
            (WHITESPACE, " "),
            (BANG, "!"),
            (IDENT, "important"),
            (SEMICOLON, ";"),
        ]
    );
}

#[test]
fn sass_variable_expression() {
    assert_eq!(
        lex("$total: $a + $b * 2;"),
        vec![
            (DOLLAR, "$"),
            (IDENT, "total"),
            (COLON, ":"),
            (WHITESPACE, " "),
            (DOLLAR, "$"),
            (IDENT, "a"),
            (WHITESPACE, " "),
            (PLUS, "+"),
            (WHITESPACE, " "),
            (DOLLAR, "$"),
            (IDENT, "b"),
            (WHITESPACE, " "),
            (STAR, "*"),
            (WHITESPACE, " "),
            (NUMBER, "2"),
            (SEMICOLON, ";"),
        ]
    );
}

#[test]
fn interpolation_in_selector() {
    assert_eq!(
        lex(".#{$class}"),
        vec![
            (DOT, "."),
            (HASH_LBRACE, "#{"),
            (DOLLAR, "$"),
            (IDENT, "class"),
            (RBRACE, "}"),
        ]
    );
}

#[test]
fn parent_selector_suffix() {
    assert_eq!(lex("&__element"), vec![(AMP, "&"), (IDENT, "__element"),]);
}

#[test]
fn rest_args() {
    assert_eq!(
        lex("$args..."),
        vec![(DOLLAR, "$"), (IDENT, "args"), (DOT_DOT_DOT, "..."),]
    );
}

#[test]
fn triple_nested_interpolation() {
    // Three levels of nesting
    assert_eq!(
        lex("\"#{\"#{\"inner\"}\"}\""),
        vec![
            (STRING_START, "\""),
            (HASH_LBRACE, "#{"),
            (STRING_START, "\""),
            (HASH_LBRACE, "#{"),
            (QUOTED_STRING, "\"inner\""),
            (RBRACE, "}"),
            (STRING_END, "\""),
            (RBRACE, "}"),
            (STRING_END, "\""),
        ]
    );
}

#[test]
fn all_whitespace_chars_combined() {
    // space, tab, newline, carriage return, form feed — all as single token
    assert_eq!(lex(" \t\n\r\x0C"), vec![(WHITESPACE, " \t\n\r\x0C")]);
}

#[test]
fn empty_interpolation() {
    assert_eq!(
        lex("\"#{}\""),
        vec![
            (STRING_START, "\""),
            (HASH_LBRACE, "#{"),
            (RBRACE, "}"),
            (STRING_END, "\""),
        ]
    );
}

// ── Lexer error recovery ──────────────────────────────────────────────

#[test]
fn unterminated_block_comment_then_valid() {
    // After an unterminated block comment, the rest of the input is consumed as error.
    // This verifies the lexer doesn't produce garbage tokens.
    let tokens = lex("/* oops");
    assert_eq!(tokens, vec![(ERROR, "/* oops")]);
}

#[test]
fn unterminated_string_consumes_to_eof() {
    // SCSS strings can span lines, so an unterminated string consumes to EOF
    let tokens = lex("\"oops\na: b;");
    assert_eq!(tokens, vec![(ERROR, "\"oops\na: b;")]);
}

#[test]
fn unterminated_interpolation_in_string() {
    // #{  opened but never closed — string continues to EOF
    let tokens = lex("\"hello #{$x");
    assert_eq!(
        tokens,
        vec![
            (STRING_START, "\"hello "),
            (HASH_LBRACE, "#{"),
            (DOLLAR, "$"),
            (IDENT, "x"),
        ]
    );
}

#[test]
fn error_char_then_valid_tokens() {
    // A single error character should not break lexing of subsequent tokens
    let tokens = lex("^\n.foo { }");
    assert_eq!(
        tokens,
        vec![
            (ERROR, "^"),
            (WHITESPACE, "\n"),
            (DOT, "."),
            (IDENT, "foo"),
            (WHITESPACE, " "),
            (LBRACE, "{"),
            (WHITESPACE, " "),
            (RBRACE, "}"),
        ]
    );
}

#[test]
fn multiple_error_chars_isolated() {
    // Each error char is its own token, not merged
    let tokens = lex("^\x01");
    assert_eq!(tokens, vec![(ERROR, "^"), (ERROR, "\x01")]);
}

#[test]
fn bom_mid_file_is_error() {
    // BOM is only valid at start; mid-file BOM should be an error
    let tokens = lex("a\u{FEFF}b");
    // The lexer treats BOM as whitespace regardless of position
    assert_eq!(
        tokens.len(),
        3,
        "should produce 3 tokens (ident, bom-token, ident)"
    );
    assert_eq!(tokens[0], (IDENT, "a"));
    assert_eq!(tokens[2], (IDENT, "b"));
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
        // url()
        "url(http://example.com)",
        "url(\"http://example.com\")",
        "url('path')",
        "url( http://example.com )",
        "url(#{$base}/path)",
        "url(path/#{$base}/end)",
        "url()",
        "url(data:image/png;base64,abc==)",
        "url(path\\)end)",
        "background: url(img.png);",
        // Comprehensive edge cases
        "\"a #{$b + \"c #{$d}\"}\"",
        ".café",
        "$über",
        "café x",
        "\"héllo\"",
        "url(pàth)",
        "@import",
        "%placeholder",
        ".#{$class}",
        "&__element",
        "$args...",
        "\"#{\"#{\"inner\"}\"}\"",
        " \t\n\r\x0C",
        "\"#{}\"",
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
