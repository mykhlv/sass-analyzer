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
    Exit,
    Error {
        msg_index: u32,
        range: TextRange,
    },
    Tombstone,
}

impl Event {
    pub fn tombstone() -> Self {
        Self::Tombstone
    }
}
