use crate::syntax_kind::SyntaxKind;
use crate::text_range::TextRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Enter {
        kind: SyntaxKind,
        forward_parent: Option<u32>,
    },
    Token {
        kind: SyntaxKind,
        range: TextRange,
    },
    /// A zero-width token that stands for no source text: the `{`, `}`, `;` or `@` that
    /// indentation implies. It consumes no `Input` token.
    ///
    /// `trivia` is an index into [`Input::all_trivia`](crate::input::Input::all_trivia):
    /// the token goes after every trivia token before that index, and before the rest.
    /// That makes its position among the trivia explicit, since it decides node ranges
    /// and what `has_whitespace_before` sees: a block opens before the line break, after
    /// a comment on the same line, and a comment-only body closes after the comment.
    /// The index must lie within the trivia between the previous `Token` and the next.
    Virtual {
        kind: SyntaxKind,
        trivia: u32,
    },
    Exit,
    Error {
        msg_index: u32,
        range: TextRange,
    },
    Tombstone,
}

// The parser pushes one or more events per token; growing them costs throughput.
const _: () = assert!(size_of::<Event>() == 16);

impl Event {
    pub fn tombstone() -> Self {
        Self::Tombstone
    }
}
