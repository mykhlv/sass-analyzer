use crate::event::Event;
use crate::input::Input;

/// Stack-based green tree builder that bypasses rowan's `NodeCache`.
///
/// rowan's `GreenNodeBuilder` hashes every token (`FxHash` of kind + full text)
/// and does a `HashMap` lookup for deduplication. For SCSS, most tokens are
/// unique identifiers/values, so the cache hit rate is low and the hash
/// overhead is pure waste. This builder constructs `GreenToken`/`GreenNode`
/// directly, eliminating ~40-80 ns per token of hash + lookup overhead.
struct DirectBuilder {
    children: Vec<rowan::NodeOrToken<rowan::GreenNode, rowan::GreenToken>>,
    parents: Vec<(rowan::SyntaxKind, usize)>,
}

impl DirectBuilder {
    fn new() -> Self {
        Self {
            children: Vec::new(),
            parents: Vec::new(),
        }
    }

    #[inline]
    fn token(&mut self, kind: rowan::SyntaxKind, text: &str) {
        self.children
            .push(rowan::NodeOrToken::Token(rowan::GreenToken::new(kind, text)));
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
                        let text =
                            &source[usize::from(tr.start())..usize::from(tr.end())];
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
                        let text =
                            &source[usize::from(tr.start())..usize::from(tr.end())];
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
