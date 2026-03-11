use super::{AstChildren, AstNode};
use crate::syntax::{SyntaxNode, SyntaxToken};
use crate::syntax_kind::SyntaxKind;

/// Find the first child node of type `N`.
pub(crate) fn child<N: AstNode>(parent: &SyntaxNode) -> Option<N> {
    parent.children().find_map(N::cast)
}

/// Iterate over all child nodes of type `N`.
pub(crate) fn children<N: AstNode>(parent: &SyntaxNode) -> AstChildren<N> {
    AstChildren::new(parent)
}

/// Find the first child token of the given kind.
///
/// Part of the standard AST support layer (cf. rust-analyzer).
/// Will be used by typed AST accessors as they are added.
#[expect(dead_code, reason = "reserved for AST layer expansion")]
pub(crate) fn token(parent: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    parent
        .children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .find(|it| it.kind() == kind)
}
