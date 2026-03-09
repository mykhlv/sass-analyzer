use std::path::{Path, PathBuf};
use std::sync::Arc;

use dashmap::DashMap;
use sass_parser::imports::{self, ImportKind, ImportRef};
use sass_parser::resolver::{ModuleResolver, ResolvedModule};
use sass_parser::syntax::SyntaxNode;
use sass_parser::syntax_kind::SyntaxKind;
use sass_parser::vfs::OsFileSystem;
use tower_lsp_server::ls_types::Uri;

use crate::symbols::{self, FileSymbols};

/// Namespace binding for an `@use` rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Namespace {
    /// `@use "x" as name` or default (last path segment).
    Named(String),
    /// `@use "x" as *` — members merge into current scope.
    Star,
}

/// A resolved import edge in the module graph.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ImportEdge {
    pub target: Uri,
    pub namespace: Namespace,
    pub kind: ImportKind,
}

/// Parsed + analyzed info for a single file.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub symbols: FileSymbols,
    pub green: rowan::GreenNode,
    pub line_index: sass_parser::line_index::LineIndex,
}

/// Cross-file dependency graph for the SCSS workspace.
pub struct ModuleGraph {
    files: DashMap<Uri, ModuleInfo>,
    edges: DashMap<Uri, Vec<ImportEdge>>,
    resolver: Arc<ModuleResolver<OsFileSystem>>,
}

impl ModuleGraph {
    pub fn new() -> Self {
        Self {
            files: DashMap::new(),
            edges: DashMap::new(),
            resolver: Arc::new(ModuleResolver::new()),
        }
    }

    /// Index a file: parse if needed, collect symbols and resolve imports.
    /// `open_docs` provides in-memory text for open documents (takes priority over disk).
    pub fn index_file(
        &self,
        uri: &Uri,
        green: rowan::GreenNode,
        symbols: FileSymbols,
        line_index: sass_parser::line_index::LineIndex,
    ) {
        self.files.insert(
            uri.clone(),
            ModuleInfo {
                symbols,
                green: green.clone(),
                line_index,
            },
        );

        let root = SyntaxNode::new_root(green);
        let import_refs = imports::collect_imports(&root);
        let base_path = uri_to_path(uri);

        let mut resolved_edges = Vec::new();

        for import_ref in &import_refs {
            if import_ref.kind == ImportKind::LoadCss {
                continue;
            }

            let Some(base) = &base_path else { continue };
            let resolved = self.resolver.resolve(&import_ref.path, base);

            if let Ok(ResolvedModule::File(target_path)) = resolved {
                let target_uri = path_to_uri(&target_path);
                let namespace = extract_namespace(&root, import_ref);

                // Eagerly index the dependency if not already known
                if !self.files.contains_key(&target_uri) {
                    self.index_dependency(&target_uri, &target_path);
                }

                resolved_edges.push(ImportEdge {
                    target: target_uri,
                    namespace,
                    kind: import_ref.kind,
                });
            } else if let Ok(ResolvedModule::Builtin(_)) = resolved {
                // Built-in modules don't have a file URI — skip graph edge
                // (built-in symbols handled separately in completion/hover)
            }
        }

        self.edges.insert(uri.clone(), resolved_edges);
    }

    /// Remove a file from the graph.
    pub fn remove_file(&self, uri: &Uri) {
        self.files.remove(uri);
        self.edges.remove(uri);
    }

    /// Resolve a qualified name (`namespace.$name` or `namespace.func()`)
    /// from a given source file. Returns the target URI and matching Symbol.
    pub fn resolve_qualified(
        &self,
        from: &Uri,
        namespace: &str,
        name: &str,
    ) -> Option<(Uri, symbols::Symbol)> {
        let edges = self.edges.get(from)?;
        for edge in edges.value() {
            let ns_match = match &edge.namespace {
                Namespace::Named(n) => n == namespace,
                Namespace::Star => false, // star imports don't use namespace prefix
            };
            if !ns_match {
                continue;
            }
            if let Some(info) = self.files.get(&edge.target) {
                if let Some(sym) = info.symbols.definitions.iter().find(|s| s.name == name) {
                    return Some((edge.target.clone(), sym.clone()));
                }
            }
        }
        None
    }

    /// Resolve an unqualified name from a given source file.
    /// Searches: local definitions, then `@use ... as *` imports, then `@import` imports.
    pub fn resolve_unqualified(
        &self,
        from: &Uri,
        name: &str,
        kind: symbols::SymbolKind,
    ) -> Option<(Uri, symbols::Symbol)> {
        // 1. Local definitions
        if let Some(info) = self.files.get(from) {
            if let Some(sym) = info
                .symbols
                .definitions
                .iter()
                .find(|s| s.name == name && s.kind == kind)
            {
                return Some((from.clone(), sym.clone()));
            }
        }

        // 2. Star imports and @import (merged scope)
        let edges = self.edges.get(from)?;
        for edge in edges.value() {
            let is_merged =
                matches!(edge.namespace, Namespace::Star) || edge.kind == ImportKind::Import;
            if !is_merged {
                continue;
            }
            if let Some(info) = self.files.get(&edge.target) {
                if let Some(sym) = info
                    .symbols
                    .definitions
                    .iter()
                    .find(|s| s.name == name && s.kind == kind)
                {
                    return Some((edge.target.clone(), sym.clone()));
                }
            }
        }

        None
    }

    /// Collect all symbols visible from a given file (for completions).
    /// Returns `(namespace_prefix, uri, symbol)` triples.
    pub fn visible_symbols(&self, from: &Uri) -> Vec<(Option<String>, Uri, symbols::Symbol)> {
        let mut result = Vec::new();

        // Local definitions
        if let Some(info) = self.files.get(from) {
            for sym in &info.symbols.definitions {
                result.push((None, from.clone(), sym.clone()));
            }
        }

        // Imported symbols
        if let Some(edges) = self.edges.get(from) {
            for edge in edges.value() {
                if let Some(info) = self.files.get(&edge.target) {
                    let prefix = match &edge.namespace {
                        Namespace::Named(n) => Some(n.clone()),
                        Namespace::Star => None,
                    };
                    // @import merges into scope (no prefix)
                    let prefix = if edge.kind == ImportKind::Import {
                        None
                    } else {
                        prefix
                    };
                    for sym in &info.symbols.definitions {
                        result.push((prefix.clone(), edge.target.clone(), sym.clone()));
                    }
                }
            }
        }

        result
    }

    pub fn line_index(&self, uri: &Uri) -> Option<sass_parser::line_index::LineIndex> {
        self.files.get(uri).map(|info| info.line_index.clone())
    }

    fn index_dependency(&self, uri: &Uri, path: &Path) {
        let Ok(source) = std::fs::read_to_string(path) else {
            return;
        };
        let Some((green, _errors)) =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sass_parser::parse(&source)))
                .ok()
        else {
            return;
        };
        let line_index = sass_parser::line_index::LineIndex::new(&source);
        let file_symbols = {
            let root = SyntaxNode::new_root(green.clone());
            symbols::collect_symbols(&root)
        };
        self.files.insert(
            uri.clone(),
            ModuleInfo {
                symbols: file_symbols,
                green,
                line_index,
            },
        );
    }
}

// ── Namespace extraction ────────────────────────────────────────────

/// Extract the namespace binding from a `@use` rule's CST.
fn extract_namespace(root: &SyntaxNode, import_ref: &ImportRef) -> Namespace {
    if import_ref.kind == ImportKind::Import {
        return Namespace::Star;
    }

    // Find the USE_RULE/FORWARD_RULE node at the import's range
    let node = root.descendants().find(|n| {
        (n.kind() == SyntaxKind::USE_RULE || n.kind() == SyntaxKind::FORWARD_RULE)
            && n.text_range() == import_ref.range
    });

    let Some(node) = node else {
        return default_namespace(&import_ref.path);
    };

    // Look for `as` IDENT followed by namespace IDENT or STAR
    let tokens: Vec<_> = node
        .children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .collect();

    for (i, token) in tokens.iter().enumerate() {
        if token.kind() == SyntaxKind::IDENT && token.text() == "as" {
            if let Some(next) = tokens.get(i + 1) {
                // Skip whitespace tokens to find the actual value
                let val = if next.kind() == SyntaxKind::WHITESPACE {
                    tokens.get(i + 2)
                } else {
                    Some(next)
                };
                if let Some(val) = val {
                    if val.kind() == SyntaxKind::STAR {
                        return Namespace::Star;
                    }
                    if val.kind() == SyntaxKind::IDENT {
                        return Namespace::Named(val.text().to_string());
                    }
                }
            }
        }
    }

    default_namespace(&import_ref.path)
}

/// Default namespace: last segment of the path without extension/underscore.
/// `@use "src/colors"` → `colors`
/// `@use "sass:math"` → `math`
fn default_namespace(path: &str) -> Namespace {
    if let Some(name) = path.strip_prefix("sass:") {
        return Namespace::Named(name.to_owned());
    }
    let segment = path.rsplit('/').next().unwrap_or(path);
    let stem = segment.strip_prefix('_').unwrap_or(segment);
    let stem = stem.strip_suffix(".scss").unwrap_or(stem);
    Namespace::Named(stem.to_owned())
}

// ── URI ↔ Path conversion ───────────────────────────────────────────

fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    uri.to_file_path().map(std::borrow::Cow::into_owned)
}

fn path_to_uri(path: &Path) -> Uri {
    Uri::from_file_path(path).unwrap_or_else(|| {
        let s = format!("file://{}", path.display());
        s.parse().expect("failed to parse fallback URI")
    })
}

// ── Helpers for external callers ────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_namespace_simple() {
        assert_eq!(
            default_namespace("colors"),
            Namespace::Named("colors".into())
        );
    }

    #[test]
    fn default_namespace_with_path() {
        assert_eq!(
            default_namespace("src/utils/colors"),
            Namespace::Named("colors".into())
        );
    }

    #[test]
    fn default_namespace_builtin() {
        assert_eq!(
            default_namespace("sass:math"),
            Namespace::Named("math".into())
        );
    }

    #[test]
    fn default_namespace_partial() {
        assert_eq!(
            default_namespace("_mixins"),
            Namespace::Named("mixins".into())
        );
    }

    #[test]
    fn extract_namespace_as_alias() {
        let (green, _) = sass_parser::parse("@use \"colors\" as c;");
        let root = SyntaxNode::new_root(green);
        let imports = imports::collect_imports(&root);
        assert_eq!(imports.len(), 1);
        let ns = extract_namespace(&root, &imports[0]);
        assert_eq!(ns, Namespace::Named("c".into()));
    }

    #[test]
    fn extract_namespace_as_star() {
        let (green, _) = sass_parser::parse("@use \"colors\" as *;");
        let root = SyntaxNode::new_root(green);
        let imports = imports::collect_imports(&root);
        let ns = extract_namespace(&root, &imports[0]);
        assert_eq!(ns, Namespace::Star);
    }

    #[test]
    fn extract_namespace_default() {
        let (green, _) = sass_parser::parse("@use \"sass:math\";");
        let root = SyntaxNode::new_root(green);
        let imports = imports::collect_imports(&root);
        let ns = extract_namespace(&root, &imports[0]);
        assert_eq!(ns, Namespace::Named("math".into()));
    }

    #[test]
    fn extract_namespace_default_path() {
        let (green, _) = sass_parser::parse("@use \"src/utils\";");
        let root = SyntaxNode::new_root(green);
        let imports = imports::collect_imports(&root);
        let ns = extract_namespace(&root, &imports[0]);
        assert_eq!(ns, Namespace::Named("utils".into()));
    }

    fn make_info(
        source: &str,
    ) -> (
        rowan::GreenNode,
        FileSymbols,
        sass_parser::line_index::LineIndex,
    ) {
        let (green, _) = sass_parser::parse(source);
        let root = SyntaxNode::new_root(green.clone());
        let syms = symbols::collect_symbols(&root);
        let li = sass_parser::line_index::LineIndex::new(source);
        (green, syms, li)
    }

    #[test]
    fn module_graph_local_resolution() {
        let graph = ModuleGraph::new();
        let uri: Uri = "file:///test.scss".parse().unwrap();
        let (green, syms, line_index) = make_info("$color: red;\n@mixin btn { }");
        graph.files.insert(
            uri.clone(),
            ModuleInfo {
                symbols: syms,
                green,
                line_index,
            },
        );

        let result = graph.resolve_unqualified(&uri, "color", symbols::SymbolKind::Variable);
        assert!(result.is_some());
        let (found_uri, sym) = result.unwrap();
        assert_eq!(found_uri, uri);
        assert_eq!(sym.name, "color");

        let result = graph.resolve_unqualified(&uri, "btn", symbols::SymbolKind::Mixin);
        assert!(result.is_some());
    }

    #[test]
    fn module_graph_visible_symbols() {
        let graph = ModuleGraph::new();
        let uri: Uri = "file:///main.scss".parse().unwrap();
        let dep_uri: Uri = "file:///colors.scss".parse().unwrap();

        // Index dependency
        let (green, syms, line_index) = make_info("$primary: blue;");
        graph.files.insert(
            dep_uri.clone(),
            ModuleInfo {
                symbols: syms,
                green,
                line_index,
            },
        );

        // Index main with manual edge
        let (green, syms, line_index) = make_info("$local: red;");
        graph.files.insert(
            uri.clone(),
            ModuleInfo {
                symbols: syms,
                green,
                line_index,
            },
        );
        graph.edges.insert(
            uri.clone(),
            vec![ImportEdge {
                target: dep_uri.clone(),
                namespace: Namespace::Named("colors".into()),
                kind: ImportKind::Use,
            }],
        );

        let visible = graph.visible_symbols(&uri);
        // local: $local (no prefix), imported: $primary (prefix "colors")
        assert_eq!(visible.len(), 2);
        let local = visible.iter().find(|(p, _, _)| p.is_none()).unwrap();
        assert_eq!(local.2.name, "local");
        let imported = visible.iter().find(|(p, _, _)| p.is_some()).unwrap();
        assert_eq!(imported.0.as_deref(), Some("colors"));
        assert_eq!(imported.2.name, "primary");
    }

    #[test]
    fn qualified_resolution() {
        let graph = ModuleGraph::new();
        let uri: Uri = "file:///main.scss".parse().unwrap();
        let dep_uri: Uri = "file:///colors.scss".parse().unwrap();

        let (green, syms, line_index) = make_info("$primary: blue;\n$secondary: green;");
        graph.files.insert(
            dep_uri.clone(),
            ModuleInfo {
                symbols: syms,
                green,
                line_index,
            },
        );

        let (green, _, line_index) = make_info("");
        graph.files.insert(
            uri.clone(),
            ModuleInfo {
                symbols: symbols::FileSymbols::default(),
                green,
                line_index,
            },
        );
        graph.edges.insert(
            uri.clone(),
            vec![ImportEdge {
                target: dep_uri,
                namespace: Namespace::Named("c".into()),
                kind: ImportKind::Use,
            }],
        );

        // c.$primary → should resolve
        let result = graph.resolve_qualified(&uri, "c", "primary");
        assert!(result.is_some());
        assert_eq!(result.unwrap().1.name, "primary");

        // c.$nonexistent → None
        let result = graph.resolve_qualified(&uri, "c", "nonexistent");
        assert!(result.is_none());

        // wrong namespace
        let result = graph.resolve_qualified(&uri, "colors", "primary");
        assert!(result.is_none());
    }
}
