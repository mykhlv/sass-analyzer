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
        let token = if kind.0 <= MAX_CACHED_KIND {
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
        let (kind, first_child) = self.parents.pop().unwrap();
        let node = rowan::GreenNode::new(kind, self.children.drain(first_child..));
        self.children.push(rowan::NodeOrToken::Node(node));
    }

    fn finish(mut self) -> rowan::GreenNode {
        debug_assert_eq!(self.children.len(), 1);
        match self.children.pop().unwrap() {
            rowan::NodeOrToken::Node(node) => node,
            rowan::NodeOrToken::Token(_) => panic!("expected node, got token"),
        }
    }
}

/// Converts parser events into a rowan `GreenNode`.
///
/// Handles forward-parent resolution and trivia re-insertion.
/// Leading trivia attaches to the NEXT significant token.
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

    #[allow(clippy::match_on_vec_items)]
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
