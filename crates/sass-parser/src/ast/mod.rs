mod generated;
mod support;

pub use generated::*;

use crate::syntax::SyntaxNode;
use crate::syntax_kind::SyntaxKind;

/// Trait for typed AST node wrappers over rowan's `SyntaxNode`.
pub trait AstNode: Sized {
    /// Returns `true` if the given `SyntaxKind` can be wrapped by this type.
    fn can_cast(kind: SyntaxKind) -> bool;
    /// Try to wrap a raw `SyntaxNode` into this typed wrapper.
    fn cast(syntax: SyntaxNode) -> Option<Self>;
    /// Access the underlying raw `SyntaxNode`.
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

/// The contents of a `QUOTED_STRING` token's text: without its opening quote, and without its
/// closing one unless the string was left unclosed at the end of its line.
pub fn unquote(text: &str) -> &str {
    let Some(&quote) = text.as_bytes().first() else {
        return text;
    };
    let contents = &text[1..];
    if crate::lexer::closes_string(contents, quote) {
        &contents[..contents.len() - 1]
    } else {
        contents
    }
}

fn extract_module_path(syntax: &SyntaxNode) -> Option<String> {
    let token = syntax
        .children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .find(|t| t.kind() == SyntaxKind::QUOTED_STRING)?;
    Some(unquote(token.text()).to_owned())
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
    /// The quoted module path (e.g. `sass:math` from `@use "sass:math"`).
    pub fn module_path(&self) -> Option<String> {
        extract_module_path(&self.syntax)
    }
}

impl ForwardRule {
    /// The quoted module path (e.g. `variables` from `@forward "variables"`).
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
        Some(unquote(first_token.text()).to_owned())
    }
}
