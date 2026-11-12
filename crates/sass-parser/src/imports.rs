//! Extract import dependencies from a parsed SCSS file.
//!
//! Collects static imports (`@use`, `@forward`, `@import`) and dynamic imports
//! (`meta.load-css()`) for dependency graph construction in the LSP.

use crate::ast::{self, AstNode};
use crate::syntax::SyntaxNode;
use crate::syntax_kind::SyntaxKind;
use crate::text_range::TextRange;

/// A single import dependency extracted from the source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRef {
    /// Whether this is a `@use`, `@forward`, `@import`, or `meta.load-css()`.
    pub kind: ImportKind,
    /// The unquoted module path (e.g. `sass:meta`, `./colors`).
    pub path: String,
    /// Source range of the entire import construct.
    pub range: TextRange,
    /// Source range of the URL string, quotes included.
    pub path_range: TextRange,
    /// An `@import` that Dart Sass leaves to CSS rather than loading: its URL ends in
    /// `.css`, starts with `http://`, `https://` or `//`, is written as `url(...)`, or is
    /// followed by a media query or `supports(...)`. Always `false` for other kinds.
    pub plain_css: bool,
}

/// The kind of import statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportKind {
    /// `@use "..."`.
    Use,
    /// `@forward "..."`.
    Forward,
    /// `@import "..."` (legacy).
    Import,
    /// `meta.load-css()` — dynamic import, first string argument is the URL.
    LoadCss,
}

/// Collect all import references from a parsed syntax tree.
pub fn collect_imports(root: &SyntaxNode) -> Vec<ImportRef> {
    let mut imports = Vec::new();
    collect_rec(root, &mut imports);
    imports
}

fn collect_rec(node: &SyntaxNode, out: &mut Vec<ImportRef>) {
    match node.kind() {
        SyntaxKind::USE_RULE | SyntaxKind::FORWARD_RULE => {
            let path = if node.kind() == SyntaxKind::USE_RULE {
                ast::UseRule::cast(node.clone()).and_then(|rule| rule.module_path())
            } else {
                ast::ForwardRule::cast(node.clone()).and_then(|rule| rule.module_path())
            };
            let token = node
                .children_with_tokens()
                .filter_map(rowan::NodeOrToken::into_token)
                .find(|t| t.kind() == SyntaxKind::QUOTED_STRING);
            if let (Some(path), Some(token)) = (path, token) {
                out.push(ImportRef {
                    kind: if node.kind() == SyntaxKind::USE_RULE {
                        ImportKind::Use
                    } else {
                        ImportKind::Forward
                    },
                    path,
                    range: node.text_range(),
                    path_range: token.text_range(),
                    plain_css: false,
                });
            }
        }
        SyntaxKind::IMPORT_RULE => {
            // @import paths are bare QUOTED_STRING tokens (not wrapped in Expr nodes)
            let tokens: Vec<_> = node
                .children_with_tokens()
                .filter_map(rowan::NodeOrToken::into_token)
                .filter(|t| !t.kind().is_trivia())
                .collect();
            for (i, token) in tokens.iter().enumerate() {
                if token.kind() != SyntaxKind::QUOTED_STRING {
                    continue;
                }
                let path = crate::ast::unquote(token.text());
                let in_url = i > 0 && tokens[i - 1].kind() == SyntaxKind::LPAREN;
                let has_modifiers = tokens.get(i + 1).is_some_and(|next| {
                    !matches!(next.kind(), SyntaxKind::COMMA | SyntaxKind::SEMICOLON)
                });
                out.push(ImportRef {
                    kind: ImportKind::Import,
                    path: path.to_owned(),
                    range: node.text_range(),
                    path_range: token.text_range(),
                    plain_css: in_url || has_modifiers || is_plain_css_url(path),
                });
            }
        }
        SyntaxKind::NAMESPACE_REF => {
            if let Some(import) = try_load_css(node) {
                out.push(import);
            }
        }
        _ => {}
    }
    for child in node.children() {
        collect_rec(&child, out);
    }
}

/// Check if a `NAMESPACE_REF` node is a `meta.load-css()` call and extract its path.
fn try_load_css(node: &SyntaxNode) -> Option<ImportRef> {
    let ns_ref = ast::NamespaceRef::cast(node.clone())?;
    let ns_name = ns_ref.namespace()?;
    if ns_name != "meta" {
        return None;
    }
    let func = ns_ref.member()?;
    let name = func.name_text()?;
    if name != "load-css" {
        return None;
    }
    let path = func.first_string_arg()?;
    let token = func
        .args()?
        .syntax()
        .descendants_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .find(|t| t.kind() == SyntaxKind::QUOTED_STRING)?;
    Some(ImportRef {
        kind: ImportKind::LoadCss,
        path,
        range: node.text_range(),
        path_range: token.text_range(),
        plain_css: false,
    })
}

// Case-sensitive, as in Dart Sass: `@import "a.CSS"` loads a stylesheet.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn is_plain_css_url(url: &str) -> bool {
    url.ends_with(".css")
        || url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("//")
}
