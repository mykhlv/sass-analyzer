use expect_test::{Expect, expect};
use sass_parser::ast::AstNode;
use sass_parser::input::Input;
use sass_parser::parser::Parser;
use sass_parser::syntax::SassLanguage;
use sass_parser::syntax_kind::*;

fn make_input(tokens: &[(SyntaxKind, &str)]) -> Input {
    Input::from_tokens(tokens)
}

fn check(
    tokens: &[(SyntaxKind, &str)],
    source: &str,
    parse_fn: impl FnOnce(&mut Parser<'_>),
    expect: Expect,
) {
    let input = make_input(tokens);
    let mut parser = Parser::new(input, source);
    parse_fn(&mut parser);
    let (events, errors, input, _) = parser.finish();
    let (green, errs) = sass_parser::build_tree(events, &errors, &input, source);
    let tree = rowan::SyntaxNode::<SassLanguage>::new_root(green);

    // Every test verifies lossless round-trip
    assert_eq!(
        tree.text().to_string(),
        source,
        "lossless round-trip failed"
    );

    let mut buf = sass_parser::syntax::debug_tree(&tree);
    if !errs.is_empty() {
        buf.push_str("errors:\n");
        for (msg, range) in &errs {
            buf.push_str(&format!("  {range:?}: {msg}\n"));
        }
    }
    expect.assert_eq(&buf);
}

#[test]
fn empty_input() {
    check(
        &[],
        "",
        |p| {
            let m = p.start();
            let _ = m.complete(p, SOURCE_FILE);
        },
        expect![[r#"
            SOURCE_FILE@0..0
        "#]],
    );
}

#[test]
fn single_token() {
    let source = "hello";
    let tokens = [(IDENT, "hello")];
    check(
        &tokens,
        source,
        |p| {
            let m = p.start();
            p.bump();
            let _ = m.complete(p, SOURCE_FILE);
        },
        expect![[r#"
            SOURCE_FILE@0..5
              IDENT@0..5 "hello"
        "#]],
    );
}

#[test]
fn trivia_preserved() {
    let source = "  hello  ";
    let tokens = [(WHITESPACE, "  "), (IDENT, "hello"), (WHITESPACE, "  ")];
    check(
        &tokens,
        source,
        |p| {
            let m = p.start();
            p.bump();
            let _ = m.complete(p, SOURCE_FILE);
        },
        expect![[r#"
            SOURCE_FILE@0..9
              WHITESPACE@0..2 "  "
              IDENT@2..7 "hello"
              WHITESPACE@7..9 "  "
        "#]],
    );
}

#[test]
fn forward_parent_precede() {
    let source = "ab";
    let tokens = [(IDENT, "a"), (IDENT, "b")];
    check(
        &tokens,
        source,
        |p| {
            let file = p.start();
            let m = p.start();
            p.bump(); // a
            let cm = m.complete(p, PROPERTY);
            let outer = cm.precede(p);
            p.bump(); // b
            let _ = outer.complete(p, DECLARATION);
            let _ = file.complete(p, SOURCE_FILE);
        },
        expect![[r#"
            SOURCE_FILE@0..2
              DECLARATION@0..2
                PROPERTY@0..1
                  IDENT@0..1 "a"
                IDENT@1..2 "b"
        "#]],
    );
}

#[test]
fn forward_parent_chain_of_three() {
    let source = "abc";
    let tokens = [(IDENT, "a"), (IDENT, "b"), (IDENT, "c")];
    check(
        &tokens,
        source,
        |p| {
            let file = p.start();
            // inner → middle → outer via two precede() calls
            let m = p.start();
            p.bump(); // a
            let cm1 = m.complete(p, PROPERTY);
            let m2 = cm1.precede(p);
            p.bump(); // b
            let cm2 = m2.complete(p, DECLARATION);
            let m3 = cm2.precede(p);
            p.bump(); // c
            let _ = m3.complete(p, BLOCK);
            let _ = file.complete(p, SOURCE_FILE);
        },
        expect![[r#"
            SOURCE_FILE@0..3
              BLOCK@0..3
                DECLARATION@0..2
                  PROPERTY@0..1
                    IDENT@0..1 "a"
                  IDENT@1..2 "b"
                IDENT@2..3 "c"
        "#]],
    );
}

#[test]
fn depth_limit_triggers_error() {
    check(
        &[],
        "",
        |p| {
            let m = p.start();
            fn recurse(p: &mut Parser<'_>, remaining: u32) {
                if remaining == 0 {
                    return;
                }
                match p.depth_guard() {
                    Ok(mut g) => recurse(&mut g, remaining - 1),
                    Err(()) => {}
                }
            }
            recurse(p, 257); // 256 succeed, 257th fails
            let _ = m.complete(p, SOURCE_FILE);
        },
        expect![[r#"
            SOURCE_FILE@0..0
            errors:
              0..0: nesting too deep
        "#]],
    );
}

#[test]
#[should_panic(expected = "marker must be completed or abandoned")]
fn drop_bomb_panics_on_abandoned_marker() {
    let input = make_input(&[]);
    let mut parser = Parser::new(input, "");
    let _m = parser.start(); // never completed or abandoned → panic on drop
}

#[test]
fn source_file_ast_cast() {
    let input = make_input(&[]);
    let mut parser = Parser::new(input, "");
    let m = parser.start();
    let _ = m.complete(&mut parser, SOURCE_FILE);
    let (events, errors, input, source) = parser.finish();
    let (green, _) = sass_parser::build_tree(events, &errors, &input, source);
    let tree = rowan::SyntaxNode::<SassLanguage>::new_root(green);

    let sf = sass_parser::ast::SourceFile::cast(tree);
    assert!(sf.is_some());
    assert_eq!(sf.unwrap().syntax().kind(), SOURCE_FILE);
}

#[test]
fn marker_abandon_tombstone() {
    let source = "ab";
    let tokens = [(IDENT, "a"), (IDENT, "b")];
    check(
        &tokens,
        source,
        |p| {
            let file = p.start();
            let m = p.start();
            p.bump(); // a — pushed after m, so abandon takes Tombstone path
            m.abandon(p);
            p.bump(); // b
            let _ = file.complete(p, SOURCE_FILE);
        },
        expect![[r#"
            SOURCE_FILE@0..2
              IDENT@0..1 "a"
              IDENT@1..2 "b"
        "#]],
    );
}

// ── Integration tests: lexer → Input → Parser → bridge → rowan (1.16) ──

fn lex_parse_check(source: &str, expect: Expect) {
    let tokens = sass_parser::lexer::tokenize(source);
    let input = Input::from_tokens(&tokens);
    let mut parser = Parser::new(input, source);

    // Trivial parse: wrap all tokens in SOURCE_FILE
    let m = parser.start();
    while !parser.at_end() {
        parser.bump();
    }
    let _ = m.complete(&mut parser, SOURCE_FILE);

    let (events, errors, input, src) = parser.finish();
    let (green, errs) = sass_parser::build_tree(events, &errors, &input, src);
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
            buf.push_str(&format!("  {range:?}: {msg}\n"));
        }
    }
    expect.assert_eq(&buf);
}

#[test]
fn integration_empty() {
    lex_parse_check(
        "",
        expect![[r#"
            SOURCE_FILE@0..0
        "#]],
    );
}

#[test]
fn integration_single_ident() {
    lex_parse_check(
        "hello",
        expect![[r#"
            SOURCE_FILE@0..5
              IDENT@0..5 "hello"
        "#]],
    );
}

#[test]
fn integration_variable_declaration() {
    lex_parse_check(
        "$color: red;",
        expect![[r#"
            SOURCE_FILE@0..12
              DOLLAR@0..1 "$"
              IDENT@1..6 "color"
              COLON@6..7 ":"
              WHITESPACE@7..8 " "
              IDENT@8..11 "red"
              SEMICOLON@11..12 ";"
        "#]],
    );
}

#[test]
fn integration_trivia_preserved() {
    lex_parse_check(
        "  /* comment */ $color: red;  ",
        expect![[r#"
            SOURCE_FILE@0..30
              WHITESPACE@0..2 "  "
              MULTI_LINE_COMMENT@2..15 "/* comment */"
              WHITESPACE@15..16 " "
              DOLLAR@16..17 "$"
              IDENT@17..22 "color"
              COLON@22..23 ":"
              WHITESPACE@23..24 " "
              IDENT@24..27 "red"
              SEMICOLON@27..28 ";"
              WHITESPACE@28..30 "  "
        "#]],
    );
}

#[test]
fn integration_string_interpolation() {
    lex_parse_check(
        "\"hello #{$name}\"",
        expect![[r##"
            SOURCE_FILE@0..16
              STRING_START@0..7 "\"hello "
              HASH_LBRACE@7..9 "#{"
              DOLLAR@9..10 "$"
              IDENT@10..14 "name"
              RBRACE@14..15 "}"
              STRING_END@15..16 "\""
        "##]],
    );
}

#[test]
fn integration_url_unquoted() {
    lex_parse_check(
        "background: url(img.png);",
        expect![[r#"
            SOURCE_FILE@0..25
              IDENT@0..10 "background"
              COLON@10..11 ":"
              WHITESPACE@11..12 " "
              IDENT@12..15 "url"
              LPAREN@15..16 "("
              URL_CONTENTS@16..23 "img.png"
              RPAREN@23..24 ")"
              SEMICOLON@24..25 ";"
        "#]],
    );
}

#[test]
fn integration_url_quoted() {
    lex_parse_check(
        "background: url(\"img.png\");",
        expect![[r#"
            SOURCE_FILE@0..27
              IDENT@0..10 "background"
              COLON@10..11 ":"
              WHITESPACE@11..12 " "
              IDENT@12..15 "url"
              LPAREN@15..16 "("
              QUOTED_STRING@16..25 "\"img.png\""
              RPAREN@25..26 ")"
              SEMICOLON@26..27 ";"
        "#]],
    );
}

#[test]
fn integration_whitespace_only() {
    lex_parse_check(
        "   \n\t  ",
        expect![[r#"
            SOURCE_FILE@0..7
              WHITESPACE@0..7 "   \n\t  "
        "#]],
    );
}

#[test]
fn integration_single_line_comment() {
    lex_parse_check(
        "// comment\n$x: 1;",
        expect![[r#"
            SOURCE_FILE@0..17
              SINGLE_LINE_COMMENT@0..10 "// comment"
              WHITESPACE@10..11 "\n"
              DOLLAR@11..12 "$"
              IDENT@12..13 "x"
              COLON@13..14 ":"
              WHITESPACE@14..15 " "
              NUMBER@15..16 "1"
              SEMICOLON@16..17 ";"
        "#]],
    );
}

#[test]
fn integration_bom_preserved() {
    lex_parse_check(
        "\u{FEFF}$x: 1;",
        expect![[r#"
            SOURCE_FILE@0..9
              WHITESPACE@0..3 "\u{feff}"
              DOLLAR@3..4 "$"
              IDENT@4..5 "x"
              COLON@5..6 ":"
              WHITESPACE@6..7 " "
              NUMBER@7..8 "1"
              SEMICOLON@8..9 ";"
        "#]],
    );
}

#[test]
fn integration_has_whitespace_before() {
    let source = "$a :b";
    let tokens = sass_parser::lexer::tokenize(source);
    let input = Input::from_tokens(&tokens);

    // Significant tokens: $, a, :, b (indices 0..3)
    // Whitespace between "a" and ":"
    assert!(!input.has_whitespace_before(0)); // $ — no trivia before
    assert!(!input.has_whitespace_before(1)); // a — no trivia before
    assert!(input.has_whitespace_before(2)); // : — whitespace before
    assert!(!input.has_whitespace_before(3)); // b — no trivia before
}
