use sass_parser::lexer::{Lexer, tokenize};
use sass_parser::syntax_kind::*;

fn lex(input: &str) -> Vec<(SyntaxKind, &str)> {
    tokenize(input)
}

#[test]
fn empty_input() {
    let tokens = lex("");
    assert!(tokens.is_empty());

    let mut lexer = Lexer::new("");
    assert_eq!(lexer.next_token(), (EOF, ""));
}

#[test]
fn unknown_char_is_error() {
    let tokens = lex("\x01");
    assert_eq!(tokens, vec![(ERROR, "\x01")]);
}

#[test]
fn multiple_unknown_chars() {
    let tokens = lex("\x01\x02\x03");
    assert_eq!(
        tokens,
        vec![(ERROR, "\x01"), (ERROR, "\x02"), (ERROR, "\x03")]
    );
}

#[test]
fn multibyte_unknown_char() {
    let tokens = lex("🦀");
    assert_eq!(tokens, vec![(ERROR, "🦀")]);
}

#[test]
fn round_trip() {
    let inputs = ["", "\x01\x02", "🦀", "\x01🦀\x02"];
    for input in inputs {
        let tokens = lex(input);
        let reconstructed: String = tokens.iter().map(|(_, text)| *text).collect();
        assert_eq!(reconstructed, input, "round-trip failed for {input:?}");
    }
}

#[test]
fn eof_repeats() {
    let mut lexer = Lexer::new("");
    assert_eq!(lexer.next_token(), (EOF, ""));
    assert_eq!(lexer.next_token(), (EOF, ""));
    assert_eq!(lexer.next_token(), (EOF, ""));
}
