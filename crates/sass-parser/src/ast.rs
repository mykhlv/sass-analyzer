use crate::syntax::SyntaxNode;
use crate::syntax_kind::SyntaxKind;

/// Typed AST wrapper for the root `SOURCE_FILE` node.
///
/// Establishes the cast/syntax pattern for all future AST wrappers.
pub struct SourceFile(SyntaxNode);

impl SourceFile {
    pub fn cast(node: SyntaxNode) -> Option<Self> {
        if node.kind() == SyntaxKind::SOURCE_FILE {
            Some(Self(node))
        } else {
            None
        }
    }

    pub fn syntax(&self) -> &SyntaxNode {
        &self.0
    }
}
