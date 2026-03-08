use expect_test::{Expect, expect};
use sass_parser::input::Input;
use sass_parser::parser::Parser;
use sass_parser::syntax::SassLanguage;
use sass_parser::syntax_kind::*;
use sass_parser::text_range::{TextRange, TextSize};

/// Build an `Input` from raw tokens for testing.
/// Real lexer comes in Phase 1.
fn make_input(tokens: &[(SyntaxKind, &str)]) -> Input {
    let mut kinds = Vec::new();
    let mut ranges = Vec::new();
    let mut all_trivia = Vec::new();
    let mut trivia_starts = Vec::new();
    let mut pending_trivia = Vec::new();
    let mut offset = 0u32;

    for &(kind, text) in tokens {
        let len = text.len() as u32;
        let range = TextRange::new(TextSize::new(offset), TextSize::new(offset + len));
        if kind.is_trivia() {
            pending_trivia.push((kind, range));
        } else {
            trivia_starts.push(all_trivia.len() as u32);
            all_trivia.extend_from_slice(&pending_trivia);
            pending_trivia.clear();
            kinds.push(kind);
            ranges.push(range);
        }
        offset += len;
    }

    // Sentinel
    trivia_starts.push(all_trivia.len() as u32);
    // Trailing trivia
    all_trivia.extend_from_slice(&pending_trivia);

    Input::new(kinds, ranges, all_trivia, trivia_starts)
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

    let mut buf = String::new();
    pretty_print(&tree, &mut buf, 0);
    if !errs.is_empty() {
        buf.push_str("errors:\n");
        for (msg, range) in &errs {
            buf.push_str(&format!("  {range:?}: {msg}\n"));
        }
    }
    expect.assert_eq(&buf);
}

fn pretty_print(node: &rowan::SyntaxNode<SassLanguage>, buf: &mut String, indent: usize) {
    let kind = node.kind();
    let range = node.text_range();
    buf.push_str(&format!(
        "{:indent$}{kind:?}@{range:?}\n",
        "",
        indent = indent,
    ));
    for child in node.children_with_tokens() {
        match child {
            rowan::NodeOrToken::Node(n) => pretty_print(&n, buf, indent + 2),
            rowan::NodeOrToken::Token(t) => {
                let kind = t.kind();
                let range = t.text_range();
                let text = t.text();
                buf.push_str(&format!(
                    "{:indent$}{kind:?}@{range:?} {text:?}\n",
                    "",
                    indent = indent + 2,
                ));
            }
        }
    }
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
