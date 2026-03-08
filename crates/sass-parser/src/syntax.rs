use crate::syntax_kind::SyntaxKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SassLanguage {}

impl rowan::Language for SassLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> SyntaxKind {
        SyntaxKind::from(raw.0)
    }

    fn kind_to_raw(kind: SyntaxKind) -> rowan::SyntaxKind {
        rowan::SyntaxKind(kind as u16)
    }
}

pub type SyntaxNode = rowan::SyntaxNode<SassLanguage>;
pub type SyntaxToken = rowan::SyntaxToken<SassLanguage>;
pub type SyntaxElement = rowan::SyntaxElement<SassLanguage>;
