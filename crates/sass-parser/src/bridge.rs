use crate::event::Event;
use crate::input::Input;
use rowan::GreenNodeBuilder;

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
    let mut builder = GreenNodeBuilder::new();
    let mut errors = Vec::new();
    let mut token_idx: usize = 0;
    let mut forward_parents = Vec::new();
    let mut depth: u32 = 0;

    for i in 0..events.len() {
        match std::mem::replace(&mut events[i], Event::tombstone()) {
            Event::Enter {
                kind,
                forward_parent,
            } => {
                forward_parents.push(kind);
                let mut fp = forward_parent;
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
                if token_idx < input.len() {
                    emit_trivia(&mut builder, input.trivia_before(token_idx), source);
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
                    emit_trivia(&mut builder, input.trailing_trivia(), source);
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

fn emit_trivia(
    builder: &mut GreenNodeBuilder<'static>,
    trivia: &[(crate::syntax_kind::SyntaxKind, crate::text_range::TextRange)],
    source: &str,
) {
    for &(kind, range) in trivia {
        let text = &source[usize::from(range.start())..usize::from(range.end())];
        builder.token(rowan::SyntaxKind(kind as u16), text);
    }
}
