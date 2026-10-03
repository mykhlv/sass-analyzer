use crate::event::Event;
use crate::input::Input;
use crate::syntax_kind::SyntaxKind;

/// Maximum `SyntaxKind` value eligible for fixed-text token caching.
/// Kinds `0..=MAX_CACHED_KIND` are punctuation/operators whose text is fully
/// determined by kind, so one cached Arc serves all occurrences.
const MAX_CACHED_KIND: u16 = SyntaxKind::STAR_EQ as u16;

const WHITESPACE_KIND: u16 = SyntaxKind::WHITESPACE as u16;

/// Stack-based green tree builder with selective token caching.
///
/// rowan's `GreenNodeBuilder` hashes every token (`FxHash` of kind + full text)
/// and does a `HashMap` lookup for deduplication. For SCSS, most tokens are
/// unique identifiers/values, so the cache hit rate is low and the hash
/// overhead is pure waste.
///
/// This builder uses two caching strategies:
/// 1. Fixed-text tokens (punctuation/operators, kinds 0..=36): indexed by kind,
///    one Arc per kind.
/// 2. Whitespace tokens: linear-scan cache by text content. Formatted SCSS
///    typically has only 5-15 unique whitespace patterns (`" "`, `"\n"`,
///    `"\n    "`, etc.) but thousands of occurrences, so deduplication saves
///    significant memory.
///
/// Variable-text tokens (IDENT, NUMBER, STRING, etc.) are created directly
/// without any hash or cache overhead.
struct DirectBuilder {
    children: Vec<rowan::NodeOrToken<rowan::GreenNode, rowan::GreenToken>>,
    parents: Vec<(rowan::SyntaxKind, usize)>,
    token_cache: Vec<Option<rowan::GreenToken>>,
    whitespace_cache: Vec<(Box<str>, rowan::GreenToken)>,
}

impl DirectBuilder {
    fn new() -> Self {
        Self {
            children: Vec::new(),
            parents: Vec::new(),
            token_cache: vec![None; MAX_CACHED_KIND as usize + 1],
            whitespace_cache: Vec::new(),
        }
    }

    #[inline]
    fn token(&mut self, kind: rowan::SyntaxKind, text: &str) {
        let token = if kind.0 <= MAX_CACHED_KIND && !text.is_empty() {
            let idx = kind.0 as usize;
            if let Some(cached) = &self.token_cache[idx] {
                cached.clone()
            } else {
                let t = rowan::GreenToken::new(kind, text);
                self.token_cache[idx] = Some(t.clone());
                t
            }
        } else if kind.0 == WHITESPACE_KIND {
            self.cached_whitespace(kind, text)
        } else {
            rowan::GreenToken::new(kind, text)
        };
        self.children.push(rowan::NodeOrToken::Token(token));
    }

    #[inline]
    fn cached_whitespace(&mut self, kind: rowan::SyntaxKind, text: &str) -> rowan::GreenToken {
        for (cached_text, cached_token) in &self.whitespace_cache {
            if **cached_text == *text {
                return cached_token.clone();
            }
        }
        let t = rowan::GreenToken::new(kind, text);
        self.whitespace_cache.push((text.into(), t.clone()));
        t
    }

    #[inline]
    fn start_node(&mut self, kind: rowan::SyntaxKind) {
        self.parents.push((kind, self.children.len()));
    }

    #[inline]
    fn finish_node(&mut self) {
        let (kind, first_child) = self
            .parents
            .pop()
            .expect("finish_node called without matching start_node");
        let node = rowan::GreenNode::new(kind, self.children.drain(first_child..));
        self.children.push(rowan::NodeOrToken::Node(node));
    }

    fn finish(mut self) -> rowan::GreenNode {
        debug_assert_eq!(self.children.len(), 1);
        match self.children.pop().expect("finish called on empty builder") {
            rowan::NodeOrToken::Node(node) => node,
            rowan::NodeOrToken::Token(_) => panic!("expected node, got token"),
        }
    }
}

/// Converts parser events into a rowan `GreenNode`.
///
/// Handles forward-parent resolution and trivia re-insertion.
/// Leading trivia attaches to the NEXT significant token, or is split around
/// the zero-width tokens of `Event::Virtual` at the index each one names.
/// Trailing trivia after all tokens attaches to `SOURCE_FILE`.
pub fn build_tree(
    mut events: Vec<Event>,
    error_messages: &[String],
    input: &Input,
    source: &str,
) -> (
    rowan::GreenNode,
    Vec<(String, crate::text_range::TextRange)>,
) {
    let mut builder = DirectBuilder::new();
    let mut errors = Vec::new();
    let mut token_idx: usize = 0;
    let mut forward_parents = Vec::new();
    let mut depth: u32 = 0;

    // Linear trivia walk: track position in the flat trivia array instead of
    // calling trivia_before(token_idx) per token (saves 2 index lookups per token).
    let all_trivia = input.all_trivia();
    let mut trivia_pos: usize = 0;

    #[allow(clippy::indexing_slicing)]
    for i in 0..events.len() {
        match events[i] {
            // Fast path: Enter without forward_parent (>95% of Enter events).
            // Read directly without mem::replace — no tombstone write needed.
            Event::Enter {
                kind,
                forward_parent: None,
            } => {
                builder.start_node(rowan::SyntaxKind(kind as u16));
                depth += 1;
            }
            // Slow path: Enter with forward_parent chain. Needs mem::replace
            // to prevent double-processing of chained events.
            Event::Enter {
                kind,
                forward_parent: Some(next),
            } => {
                forward_parents.push(kind);
                let mut fp = Some(next);
                while let Some(next) = fp {
                    let idx = next as usize;
                    match std::mem::replace(&mut events[idx], Event::tombstone()) {
                        Event::Enter {
                            kind,
                            forward_parent,
                        } => {
                            forward_parents.push(kind);
                            fp = forward_parent;
                        }
                        _ => unreachable!(),
                    }
                }

                for kind in forward_parents.drain(..).rev() {
                    builder.start_node(rowan::SyntaxKind(kind as u16));
                    depth += 1;
                }
            }
            Event::Token { kind, range } => {
                // Emit trivia linearly: walk from trivia_pos to this token's trivia end.
                if token_idx < input.len() {
                    let trivia_end = input.trivia_start_index(token_idx + 1);
                    for &(tk, tr) in &all_trivia[trivia_pos..trivia_end] {
                        let text = &source[usize::from(tr.start())..usize::from(tr.end())];
                        builder.token(rowan::SyntaxKind(tk as u16), text);
                    }
                    trivia_pos = trivia_end;
                }
                let text = &source[usize::from(range.start())..usize::from(range.end())];
                builder.token(rowan::SyntaxKind(kind as u16), text);
                token_idx += 1;
            }
            Event::Virtual { kind, trivia } => {
                let trivia = trivia as usize;
                debug_assert!(
                    trivia >= trivia_pos && trivia <= input.trivia_end_index(token_idx),
                    "virtual token placed outside the trivia before the next token",
                );
                for &(tk, tr) in &all_trivia[trivia_pos..trivia] {
                    let text = &source[usize::from(tr.start())..usize::from(tr.end())];
                    builder.token(rowan::SyntaxKind(tk as u16), text);
                }
                trivia_pos = trivia;
                builder.token(rowan::SyntaxKind(kind as u16), "");
            }
            Event::Exit => {
                debug_assert!(depth > 0, "unbalanced Exit event");
                depth -= 1;
                // Before closing the root node, emit trailing trivia
                if depth == 0 {
                    for &(tk, tr) in &all_trivia[trivia_pos..] {
                        let text = &source[usize::from(tr.start())..usize::from(tr.end())];
                        builder.token(rowan::SyntaxKind(tk as u16), text);
                    }
                }
                builder.finish_node();
            }
            Event::Error { msg_index, range } => {
                let msg = error_messages[msg_index as usize].clone();
                errors.push((msg, range));
            }
            Event::Tombstone => {}
        }
    }

    debug_assert!(
        token_idx <= input.len(),
        "bridge consumed more tokens ({token_idx}) than input has ({})",
        input.len(),
    );

    (builder.finish(), errors)
}

#[cfg(test)]
mod tests {
    use expect_test::{Expect, expect};

    use super::build_tree;
    use crate::event::Event;
    use crate::input::Input;
    use crate::syntax::{SyntaxNode, debug_tree};
    use crate::syntax_kind::SyntaxKind::{self, AT, BLOCK, LBRACE, RBRACE, SEMICOLON, SOURCE_FILE};

    /// Where a virtual token goes among the trivia before the next token (or before EOF).
    #[derive(Clone, Copy)]
    enum Place {
        /// In front of the `n`th line break of that trivia, counting from 0.
        BeforeBreak(usize),
        /// After all of it.
        AfterTrivia,
    }

    #[derive(Clone, Copy)]
    enum Step {
        Enter(SyntaxKind),
        Exit,
        Token,
        Virtual(SyntaxKind, Place),
    }
    use Place::{AfterTrivia, BeforeBreak};
    use Step::{Enter, Exit, Token, Virtual};

    /// Feeds `steps` to the bridge inside a `SOURCE_FILE` over the indented tokens of
    /// `source`, each `Token` taking the next one.
    #[allow(clippy::needless_pass_by_value)]
    #[track_caller]
    fn check(source: &str, steps: &[Step], expect: Expect) {
        let input = Input::from_source_indented(source);
        let mut events = vec![Event::Enter {
            kind: SOURCE_FILE,
            forward_parent: None,
        }];
        let mut pos = 0;
        for &step in steps {
            events.push(match step {
                Enter(kind) => Event::Enter {
                    kind,
                    forward_parent: None,
                },
                Exit => Event::Exit,
                Token => {
                    pos += 1;
                    Event::Token {
                        kind: input.kind(pos - 1),
                        range: input.range(pos - 1),
                    }
                }
                Virtual(kind, place) => Event::Virtual {
                    kind,
                    trivia: placement(&input, source, pos, place),
                },
            });
        }
        events.push(Event::Exit);
        assert_eq!(pos, input.len(), "every token must be consumed");

        let (green, errors) = build_tree(events, &[], &input, source);
        assert!(errors.is_empty());
        let tree = SyntaxNode::new_root(green);
        assert_eq!(
            tree.text().to_string(),
            source,
            "lossless round-trip failed"
        );
        assert_eq!(
            leaves(&tree),
            legacy_leaves(source),
            "leaves differ from the legacy lexer's tokens"
        );
        expect.assert_eq(&debug_tree(&tree));
    }

    /// Every token of the tree in order, zero-width ones included.
    fn leaves(tree: &SyntaxNode) -> Vec<(SyntaxKind, String)> {
        tree.descendants_with_tokens()
            .filter_map(rowan::NodeOrToken::into_token)
            .map(|t| (t.kind(), t.text().to_owned()))
            .collect()
    }

    /// The legacy lexer's tokens, with its `+` and `=` shorthands tagged as the plain
    /// tokens they are in `Input`.
    fn legacy_leaves(source: &str) -> Vec<(SyntaxKind, String)> {
        crate::sass_lexer::sass_tokenize(source)
            .into_iter()
            .map(|(kind, text)| match (kind, text) {
                (SyntaxKind::IDENT, "+") => (SyntaxKind::PLUS, text.to_owned()),
                (SyntaxKind::IDENT, "=") => (SyntaxKind::EQ, text.to_owned()),
                _ => (kind, text.to_owned()),
            })
            .collect()
    }

    #[allow(clippy::cast_possible_truncation)]
    fn placement(input: &Input, source: &str, pos: usize, place: Place) -> u32 {
        let start = if pos < input.len() {
            input.trivia_start_index(pos)
        } else {
            input.trivia_start_index(input.len())
        };
        let end = input.trivia_end_index(pos);
        let index = match place {
            AfterTrivia => end,
            BeforeBreak(n) => (start..end)
                .filter(|&i| {
                    let (kind, range) = input.all_trivia()[i];
                    kind == SyntaxKind::WHITESPACE && source[range].contains('\n')
                })
                .nth(n)
                .expect("no such line break"),
        };
        index as u32
    }

    #[test]
    fn block_opens_before_the_line_break_after_a_same_line_comment() {
        check(
            ".a // c\n  b\n",
            &[
                Token,
                Token,
                Enter(BLOCK),
                Virtual(LBRACE, BeforeBreak(0)),
                Token,
                Virtual(SEMICOLON, BeforeBreak(0)),
                Virtual(RBRACE, BeforeBreak(0)),
                Exit,
            ],
            expect![[r#"
                SOURCE_FILE@0..12
                  DOT@0..1 "."
                  IDENT@1..2 "a"
                  BLOCK@2..11
                    WHITESPACE@2..3 " "
                    SINGLE_LINE_COMMENT@3..7 "// c"
                    LBRACE@7..7 ""
                    WHITESPACE@7..10 "\n  "
                    IDENT@10..11 "b"
                    SEMICOLON@11..11 ""
                    RBRACE@11..11 ""
                  WHITESPACE@11..12 "\n"
            "#]],
        );
    }

    #[test]
    fn comment_only_body_closes_after_its_comment() {
        check(
            "@mixin m\n  // c\n.a\n",
            &[
                Token,
                Token,
                Token,
                Virtual(LBRACE, BeforeBreak(0)),
                Virtual(RBRACE, BeforeBreak(1)),
                Token,
                Token,
                Virtual(SEMICOLON, BeforeBreak(0)),
            ],
            expect![[r#"
                SOURCE_FILE@0..19
                  AT@0..1 "@"
                  IDENT@1..6 "mixin"
                  WHITESPACE@6..7 " "
                  IDENT@7..8 "m"
                  LBRACE@8..8 ""
                  WHITESPACE@8..11 "\n  "
                  SINGLE_LINE_COMMENT@11..15 "// c"
                  RBRACE@15..15 ""
                  WHITESPACE@15..16 "\n"
                  DOT@16..17 "."
                  IDENT@17..18 "a"
                  SEMICOLON@18..18 ""
                  WHITESPACE@18..19 "\n"
            "#]],
        );
    }

    #[test]
    fn end_of_input_without_a_line_break_closes_after_the_trailing_comment() {
        check(
            ".a\n  b // c",
            &[
                Token,
                Token,
                Virtual(LBRACE, BeforeBreak(0)),
                Token,
                Virtual(SEMICOLON, AfterTrivia),
                Virtual(RBRACE, AfterTrivia),
            ],
            expect![[r#"
                SOURCE_FILE@0..11
                  DOT@0..1 "."
                  IDENT@1..2 "a"
                  LBRACE@2..2 ""
                  WHITESPACE@2..5 "\n  "
                  IDENT@5..6 "b"
                  WHITESPACE@6..7 " "
                  SINGLE_LINE_COMMENT@7..11 "// c"
                  SEMICOLON@11..11 ""
                  RBRACE@11..11 ""
            "#]],
        );
    }

    #[test]
    fn shorthand_at_goes_after_the_indentation() {
        check(
            ".a\n  +m\n",
            &[
                Token,
                Token,
                Virtual(LBRACE, BeforeBreak(0)),
                Virtual(AT, AfterTrivia),
                Token,
                Token,
                Virtual(SEMICOLON, BeforeBreak(0)),
                Virtual(RBRACE, BeforeBreak(0)),
            ],
            expect![[r#"
                SOURCE_FILE@0..8
                  DOT@0..1 "."
                  IDENT@1..2 "a"
                  LBRACE@2..2 ""
                  WHITESPACE@2..5 "\n  "
                  AT@5..5 ""
                  PLUS@5..6 "+"
                  IDENT@6..7 "m"
                  SEMICOLON@7..7 ""
                  RBRACE@7..7 ""
                  WHITESPACE@7..8 "\n"
            "#]],
        );
    }

    #[test]
    fn dedent_by_two_levels_closes_both_blocks_before_one_line_break() {
        check(
            ".a\n  .b\n    c\n.d\n",
            &[
                Token,
                Token,
                Virtual(LBRACE, BeforeBreak(0)),
                Token,
                Token,
                Virtual(LBRACE, BeforeBreak(0)),
                Token,
                Virtual(SEMICOLON, BeforeBreak(0)),
                Virtual(RBRACE, BeforeBreak(0)),
                Virtual(RBRACE, BeforeBreak(0)),
                Token,
                Token,
                Virtual(SEMICOLON, BeforeBreak(0)),
            ],
            expect![[r#"
                SOURCE_FILE@0..17
                  DOT@0..1 "."
                  IDENT@1..2 "a"
                  LBRACE@2..2 ""
                  WHITESPACE@2..5 "\n  "
                  DOT@5..6 "."
                  IDENT@6..7 "b"
                  LBRACE@7..7 ""
                  WHITESPACE@7..12 "\n    "
                  IDENT@12..13 "c"
                  SEMICOLON@13..13 ""
                  RBRACE@13..13 ""
                  RBRACE@13..13 ""
                  WHITESPACE@13..14 "\n"
                  DOT@14..15 "."
                  IDENT@15..16 "d"
                  SEMICOLON@16..16 ""
                  WHITESPACE@16..17 "\n"
            "#]],
        );
    }

    #[test]
    #[should_panic(expected = "virtual token placed outside the trivia before the next token")]
    #[cfg(debug_assertions)]
    fn a_virtual_token_cannot_reach_past_the_next_token() {
        let source = ".a b";
        let input = Input::from_source_indented(source);
        let events = vec![
            Event::Enter {
                kind: SOURCE_FILE,
                forward_parent: None,
            },
            Event::Token {
                kind: input.kind(0),
                range: input.range(0),
            },
            Event::Virtual {
                kind: SEMICOLON,
                trivia: 1,
            },
        ];
        let _ = build_tree(events, &[], &input, source);
    }
}
