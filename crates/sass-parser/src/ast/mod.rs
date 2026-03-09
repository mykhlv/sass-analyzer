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

// ── Hand-written accessors (not codegen) ────────────────────────────

fn extract_module_path(syntax: &SyntaxNode) -> Option<String> {
    let token = syntax
        .children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .find(|t| t.kind() == SyntaxKind::QUOTED_STRING)?;
    let text = token.text();
    Some(text[1..text.len() - 1].to_owned())
}

/// Extract the first IDENT token text from a syntax node.
fn first_ident_text(syntax: &SyntaxNode) -> Option<String> {
    syntax
        .children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .find(|t| t.kind() == SyntaxKind::IDENT)
        .map(|t| t.text().to_owned())
}

impl UseRule {
    pub fn module_path(&self) -> Option<String> {
        extract_module_path(&self.syntax)
    }
}

impl ForwardRule {
    pub fn module_path(&self) -> Option<String> {
        extract_module_path(&self.syntax)
    }
}

impl NamespaceRef {
    /// The namespace prefix (e.g. `meta` in `meta.load-css()`).
    pub fn namespace(&self) -> Option<String> {
        first_ident_text(&self.syntax)
    }
}

impl FunctionCall {
    /// The function name token text (e.g. `load-css` in `meta.load-css()`).
    pub fn name_text(&self) -> Option<String> {
        first_ident_text(&self.syntax)
    }

    /// The first positional argument as a quoted string value (unquoted).
    /// Returns `None` if no arguments or if the first arg is not a string literal.
    pub fn first_string_arg(&self) -> Option<String> {
        let args = self.args()?;
        let first_token = args
            .syntax()
            .descendants_with_tokens()
            .filter_map(rowan::NodeOrToken::into_token)
            .find(|t| t.kind() == SyntaxKind::QUOTED_STRING)?;
        let text = first_token.text();
        Some(text[1..text.len() - 1].to_owned())
    }
}
