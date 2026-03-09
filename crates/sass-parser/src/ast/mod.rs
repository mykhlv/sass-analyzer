mod generated;
mod support;

pub use generated::*;

use crate::syntax::SyntaxNode;
use crate::syntax_kind::SyntaxKind;

/// Trait for typed AST node wrappers over rowan's `SyntaxNode`.
pub trait AstNode: Sized {
    fn can_cast(kind: SyntaxKind) -> bool;
    fn cast(syntax: SyntaxNode) -> Option<Self>;
    fn syntax(&self) -> &SyntaxNode;
}

/// Iterator over typed child nodes of a given AST type.
pub struct AstChildren<N: AstNode> {
    inner: rowan::SyntaxNodeChildren<crate::syntax::SassLanguage>,
    _phantom: std::marker::PhantomData<N>,
}

impl<N: AstNode> AstChildren<N> {
    pub(crate) fn new(parent: &SyntaxNode) -> Self {
        Self {
            inner: parent.children(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<N: AstNode> Iterator for AstChildren<N> {
    type Item = N;

    fn next(&mut self) -> Option<N> {
        self.inner.by_ref().find_map(N::cast)
    }
}
