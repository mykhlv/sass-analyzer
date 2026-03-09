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
    pub kind: ImportKind,
    /// The unquoted module path (e.g. `sass:meta`, `./colors`).
    pub path: String,
    /// Source range of the entire import construct.
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportKind {
    Use,
    Forward,
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
        SyntaxKind::USE_RULE => {
            if let Some(rule) = ast::UseRule::cast(node.clone()) {
                if let Some(path) = rule.module_path() {
                    out.push(ImportRef {
                        kind: ImportKind::Use,
                        path,
                        range: node.text_range(),
                    });
                }
            }
        }
        SyntaxKind::FORWARD_RULE => {
            if let Some(rule) = ast::ForwardRule::cast(node.clone()) {
                if let Some(path) = rule.module_path() {
                    out.push(ImportRef {
                        kind: ImportKind::Forward,
                        path,
                        range: node.text_range(),
                    });
                }
            }
        }
        SyntaxKind::IMPORT_RULE => {
            // @import paths are bare QUOTED_STRING tokens (not wrapped in Expr nodes)
            for token in node
                .children_with_tokens()
                .filter_map(rowan::NodeOrToken::into_token)
            {
                if token.kind() == SyntaxKind::QUOTED_STRING {
                    let text = token.text();
                    let path = text[1..text.len() - 1].to_owned();
                    out.push(ImportRef {
                        kind: ImportKind::Import,
                        path,
                        range: node.text_range(),
                    });
                }
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
    Some(ImportRef {
        kind: ImportKind::LoadCss,
        path,
        range: node.text_range(),
    })
}
