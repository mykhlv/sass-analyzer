use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use dashmap::DashMap;
use sass_parser::imports::{self, ImportKind, ImportRef};
use sass_parser::resolver::{ModuleResolver, ResolvedModule};
use sass_parser::syntax::SyntaxNode;
use sass_parser::syntax_kind::SyntaxKind;
use sass_parser::text_range::TextRange;
use sass_parser::vfs::OsFileSystem;
use tower_lsp_server::ls_types::Uri;

use crate::builtins;
use crate::symbols::{self, FileSymbols};

/// Namespace binding for an `@use` rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Namespace {
    /// `@use "x" as name` or default (last path segment).
    Named(String),
    /// `@use "x" as *` — members merge into current scope.
    Star,
}

/// Visibility constraints from `@forward` show/hide/prefix clauses.
#[derive(Debug, Clone, Default)]
pub struct ForwardVisibility {
    pub show: Option<Vec<String>>,
    pub hide: Option<Vec<String>>,
    pub prefix: Option<String>,
}

/// A resolved import edge in the module graph.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ImportEdge {
    pub target: Uri,
    pub namespace: Namespace,
    pub kind: ImportKind,
    pub visibility: ForwardVisibility,
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
    resolver: RwLock<Arc<ModuleResolver<OsFileSystem>>>,
    builtin_symbols: DashMap<String, Vec<symbols::Symbol>>,
    prepend_imports: RwLock<Vec<String>>,
}

impl ModuleGraph {
    pub fn new() -> Self {
        Self {
            files: DashMap::new(),
            edges: DashMap::new(),
            resolver: RwLock::new(Arc::new(ModuleResolver::new())),
            builtin_symbols: DashMap::new(),
            prepend_imports: RwLock::new(Vec::new()),
        }
    }

    /// Set prepend imports (called once from `initialize`).
    pub fn set_prepend_imports(&self, imports: Vec<String>) {
        let mut guard = self
            .prepend_imports
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = imports;
    }

    /// Replace the resolver with a configured one (called once from `initialize`).
    pub fn set_resolver(&self, resolver: ModuleResolver<OsFileSystem>) {
        let mut guard = self
            .resolver
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = Arc::new(resolver);
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
            let resolver = self
                .resolver
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let resolved = resolver.resolve(&import_ref.path, base);

            if let Ok(ResolvedModule::File(target_path)) = resolved {
                let target_uri = path_to_uri(&target_path);
                let namespace = extract_namespace(&root, import_ref);
                let visibility = if import_ref.kind == ImportKind::Forward {
                    extract_forward_visibility(&root, import_ref)
                } else {
                    ForwardVisibility::default()
                };

                // Eagerly index the dependency if not already known
                if !self.files.contains_key(&target_uri) {
                    self.index_dependency(&target_uri, &target_path);
                }

                resolved_edges.push(ImportEdge {
                    target: target_uri,
                    namespace,
                    kind: import_ref.kind,
                    visibility,
                });
            } else if let Ok(ResolvedModule::Builtin(builtin)) = resolved {
                let name = builtin.name();
                if let Some(syms) = builtins::builtin_symbols(name) {
                    self.builtin_symbols.entry(name.to_owned()).or_insert(syms);
                    let target_uri: Uri = builtins::builtin_uri(name).parse().unwrap();
                    let namespace = extract_namespace(&root, import_ref);
                    resolved_edges.push(ImportEdge {
                        target: target_uri,
                        namespace,
                        kind: import_ref.kind,
                        visibility: ForwardVisibility::default(),
                    });
                }
            }
        }

        // Inject synthetic edges for prepend imports (@use ... as *)
        let prepend = self
            .prepend_imports
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        for spec in &prepend {
            if let Some(base) = &base_path {
                let resolver = self
                    .resolver
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let resolved = resolver.resolve(spec, base);
                if let Ok(ResolvedModule::File(target_path)) = resolved {
                    let target_uri = path_to_uri(&target_path);
                    if !self.files.contains_key(&target_uri) {
                        drop(resolver);
                        self.index_dependency(&target_uri, &target_path);
                    }
                    resolved_edges.push(ImportEdge {
                        target: target_uri,
                        namespace: Namespace::Star,
                        kind: ImportKind::Use,
                        visibility: ForwardVisibility::default(),
                    });
                }
            }
        }

        self.edges.insert(uri.clone(), resolved_edges);
    }

    /// Remove a file from the graph (used by file watcher for deleted files).
    #[allow(dead_code)]
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
        kind: symbols::SymbolKind,
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
            let mut visited = HashSet::new();
            if let Some(result) = self.find_in_module(&edge.target, name, Some(kind), &mut visited)
            {
                return Some(result);
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
            let mut visited = HashSet::new();
            if let Some(result) = self.find_in_module(&edge.target, name, Some(kind), &mut visited)
            {
                return Some(result);
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
                let mut visited = HashSet::new();
                self.collect_module_symbols(
                    &edge.target,
                    prefix.as_deref(),
                    &mut result,
                    &mut visited,
                );
            }
        }

        result
    }

    fn find_in_module(
        &self,
        uri: &Uri,
        name: &str,
        kind: Option<symbols::SymbolKind>,
        visited: &mut HashSet<Uri>,
    ) -> Option<(Uri, symbols::Symbol)> {
        if !visited.insert(uri.clone()) {
            return None;
        }

        // Builtin modules
        if let Some(module) = builtins::builtin_name_from_uri(uri.as_str()) {
            if let Some(syms) = self.builtin_symbols.get(module) {
                let found = syms
                    .iter()
                    .find(|s| s.name == name && kind.is_none_or(|k| s.kind == k));
                if let Some(sym) = found {
                    return Some((uri.clone(), sym.clone()));
                }
            }
            return None;
        }

        // Direct definitions
        if let Some(info) = self.files.get(uri) {
            let found = info
                .symbols
                .definitions
                .iter()
                .find(|s| s.name == name && kind.is_none_or(|k| s.kind == k));
            if let Some(sym) = found {
                return Some((uri.clone(), sym.clone()));
            }
        }

        // Follow @forward edges with visibility filtering
        if let Some(edges) = self.edges.get(uri) {
            for edge in edges.value() {
                if edge.kind == ImportKind::Forward {
                    // Strip prefix if present
                    let search_name = if let Some(pfx) = &edge.visibility.prefix {
                        let Some(stripped) = name.strip_prefix(pfx.as_str()) else {
                            continue;
                        };
                        stripped
                    } else {
                        name
                    };

                    if !is_visible(&edge.visibility, search_name) {
                        continue;
                    }

                    if let Some(result) =
                        self.find_in_module(&edge.target, search_name, kind, visited)
                    {
                        return Some(result);
                    }
                }
            }
        }

        None
    }

    fn collect_module_symbols(
        &self,
        uri: &Uri,
        ns_prefix: Option<&str>,
        result: &mut Vec<(Option<String>, Uri, symbols::Symbol)>,
        visited: &mut HashSet<Uri>,
    ) {
        self.collect_module_symbols_filtered(
            uri,
            ns_prefix,
            &ForwardVisibility::default(),
            result,
            visited,
        );
    }

    fn collect_module_symbols_filtered(
        &self,
        uri: &Uri,
        ns_prefix: Option<&str>,
        vis: &ForwardVisibility,
        result: &mut Vec<(Option<String>, Uri, symbols::Symbol)>,
        visited: &mut HashSet<Uri>,
    ) {
        if !visited.insert(uri.clone()) {
            return;
        }

        // Builtin modules
        if let Some(module) = builtins::builtin_name_from_uri(uri.as_str()) {
            if let Some(syms) = self.builtin_symbols.get(module) {
                for sym in syms.value() {
                    if !is_visible(vis, &sym.name) {
                        continue;
                    }
                    let mut sym_clone = sym.clone();
                    if let Some(pfx) = &vis.prefix {
                        sym_clone.name = format!("{pfx}{}", sym.name);
                    }
                    result.push((ns_prefix.map(String::from), uri.clone(), sym_clone));
                }
            }
            return;
        }

        if let Some(info) = self.files.get(uri) {
            for sym in &info.symbols.definitions {
                if !is_visible(vis, &sym.name) {
                    continue;
                }
                let mut sym_clone = sym.clone();
                if let Some(pfx) = &vis.prefix {
                    sym_clone.name = format!("{pfx}{}", sym.name);
                }
                result.push((ns_prefix.map(String::from), uri.clone(), sym_clone));
            }
        }

        if let Some(edges) = self.edges.get(uri) {
            for edge in edges.value() {
                if edge.kind == ImportKind::Forward {
                    self.collect_module_symbols_filtered(
                        &edge.target,
                        ns_prefix,
                        &edge.visibility,
                        result,
                        visited,
                    );
                }
            }
        }
    }

    pub fn line_index(&self, uri: &Uri) -> Option<sass_parser::line_index::LineIndex> {
        self.files.get(uri).map(|info| info.line_index.clone())
    }

    pub fn all_symbols(&self) -> Vec<(Uri, symbols::Symbol)> {
        let mut result = Vec::new();
        for entry in &self.files {
            for sym in &entry.value().symbols.definitions {
                result.push((entry.key().clone(), sym.clone()));
            }
        }
        result
    }

    /// Resolve an import specifier from a given file URI.
    /// Returns the target file URI if resolution succeeds.
    pub fn resolve_import(&self, from: &Uri, spec: &str) -> Option<Uri> {
        let base = uri_to_path(from)?;
        let resolver = self
            .resolver
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match resolver.resolve(spec, &base) {
            Ok(ResolvedModule::File(path)) => Some(path_to_uri(&path)),
            _ => None,
        }
    }

    /// Find all references to a symbol across the entire workspace.
    /// Walks each file's CST to correctly resolve both unqualified and namespace-qualified refs.
    pub fn find_all_references(
        &self,
        target_uri: &Uri,
        target_name: &str,
        target_kind: symbols::SymbolKind,
        include_declaration: bool,
    ) -> Vec<(Uri, TextRange)> {
        let mut results = Vec::new();
        let ref_kind = symbol_to_ref_kind(target_kind);

        for entry in &self.files {
            let file_uri = entry.key();
            let info = entry.value();

            // Include declaration if requested
            if include_declaration && file_uri == target_uri {
                if let Some(sym) = info
                    .symbols
                    .definitions
                    .iter()
                    .find(|s| s.name == target_name && s.kind == target_kind)
                {
                    results.push((target_uri.clone(), sym.selection_range));
                }
            }

            // Check unqualified references from SymbolRef
            for sym_ref in &info.symbols.references {
                if sym_ref.name != target_name || sym_ref.kind != ref_kind {
                    continue;
                }
                if let Some((resolved_uri, _)) =
                    self.resolve_unqualified(file_uri, &sym_ref.name, target_kind)
                {
                    if &resolved_uri == target_uri {
                        results.push((file_uri.clone(), sym_ref.selection_range));
                    }
                }
            }

            // Check namespace-qualified references via CST walk
            let root = SyntaxNode::new_root(info.green.clone());
            for node in root.descendants() {
                if node.kind() != SyntaxKind::NAMESPACE_REF {
                    continue;
                }
                if let Some((ns, name, kind, range)) = extract_ns_ref_info(&node) {
                    if name == target_name && kind == target_kind {
                        if let Some((resolved_uri, _)) =
                            self.resolve_qualified(file_uri, &ns, &name, target_kind)
                        {
                            if &resolved_uri == target_uri {
                                results.push((file_uri.clone(), range));
                            }
                        }
                    }
                }
            }
        }

        results
    }

    /// Check if renaming a symbol to `new_name` would conflict with an
    /// existing definition of the same kind in the target file.
    pub fn check_name_conflict(
        &self,
        target_uri: &Uri,
        new_name: &str,
        kind: symbols::SymbolKind,
    ) -> bool {
        if let Some(info) = self.files.get(target_uri) {
            return info
                .symbols
                .definitions
                .iter()
                .any(|s| s.name == new_name && s.kind == kind);
        }
        false
    }

    /// Find all `@forward ... show/hide` clauses across the workspace that
    /// mention `old_name` for the given symbol kind. Returns `(file_uri, text_range)`
    /// pairs pointing to the name token inside the show/hide list.
    pub fn find_forward_show_hide_references(
        &self,
        target_uri: &Uri,
        old_name: &str,
        kind: symbols::SymbolKind,
    ) -> Vec<(Uri, TextRange)> {
        let mut results = Vec::new();

        for entry in &self.edges {
            let file_uri = entry.key();
            let edges = entry.value();

            for edge in edges {
                if edge.kind != ImportKind::Forward {
                    continue;
                }
                // Only care about forwards that eventually reach target_uri
                if !self.forward_reaches(&edge.target, target_uri) {
                    continue;
                }

                let has_name = edge.visibility.show.as_ref().is_some_and(|list| {
                    list.iter().any(|n| n == old_name)
                }) || edge.visibility.hide.as_ref().is_some_and(|list| {
                    list.iter().any(|n| n == old_name)
                });

                if !has_name {
                    continue;
                }

                // Walk the FORWARD_RULE CST to find the exact token range
                let Some(info) = self.files.get(file_uri) else {
                    continue;
                };
                let root = SyntaxNode::new_root(info.green.clone());
                let ranges =
                    find_name_in_forward_clauses(&root, old_name, kind);
                for range in ranges {
                    results.push((file_uri.clone(), range));
                }
            }
        }

        results
    }

    /// Check if `from` URI can reach `target` through @forward chains.
    fn forward_reaches(&self, from: &Uri, target: &Uri) -> bool {
        let mut visited = HashSet::new();
        self.forward_reaches_inner(from, target, &mut visited)
    }

    fn forward_reaches_inner(
        &self,
        current: &Uri,
        target: &Uri,
        visited: &mut HashSet<Uri>,
    ) -> bool {
        if current == target {
            return true;
        }
        if !visited.insert(current.clone()) {
            return false;
        }
        // Check if this module directly defines the symbol (it IS the target)
        // Also follow further @forward chains
        if let Some(edges) = self.edges.get(current) {
            for edge in edges.value() {
                if edge.kind == ImportKind::Forward
                    && self.forward_reaches_inner(&edge.target, target, visited)
                {
                    return true;
                }
            }
        }
        false
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
        // Full indexing: also resolves imports so @forward chains are tracked.
        self.index_file(uri, green, file_symbols, line_index);
    }
}

// ── Namespace extraction ────────────────────────────────────────────

fn extract_namespace(root: &SyntaxNode, import_ref: &ImportRef) -> Namespace {
    if import_ref.kind == ImportKind::Import {
        return Namespace::Star;
    }

    // @forward edges are transparent re-exports — the `as prefix-*` clause
    // sets a prefix (handled in ForwardVisibility), not a namespace.
    if import_ref.kind == ImportKind::Forward {
        return Namespace::Star;
    }

    // Find the USE_RULE node at the import's range
    let node = root
        .descendants()
        .find(|n| n.kind() == SyntaxKind::USE_RULE && n.text_range() == import_ref.range);

    let Some(node) = node else {
        return default_namespace(&import_ref.path);
    };

    let tokens: Vec<_> = node
        .children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .collect();

    for (i, token) in tokens.iter().enumerate() {
        if token.kind() == SyntaxKind::IDENT && token.text() == "as" {
            if let Some(next) = tokens.get(i + 1) {
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

// ── @forward visibility extraction ──────────────────────────────────

fn extract_forward_visibility(root: &SyntaxNode, import_ref: &ImportRef) -> ForwardVisibility {
    let node = root
        .descendants()
        .find(|n| n.kind() == SyntaxKind::FORWARD_RULE && n.text_range() == import_ref.range);
    let Some(node) = node else {
        return ForwardVisibility::default();
    };

    let tokens: Vec<_> = node
        .children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .collect();

    let mut vis = ForwardVisibility::default();
    let mut i = 0;

    while i < tokens.len() {
        if tokens[i].kind() == SyntaxKind::IDENT {
            match tokens[i].text() {
                "as" => {
                    // Look for IDENT + STAR (prefix-*) pattern
                    let j = skip_ws(&tokens, i + 1);
                    if j < tokens.len() && tokens[j].kind() == SyntaxKind::IDENT {
                        let k = j + 1; // no whitespace between prefix and *
                        if k < tokens.len() && tokens[k].kind() == SyntaxKind::STAR {
                            vis.prefix = Some(tokens[j].text().to_string());
                            i = k + 1;
                            continue;
                        }
                    }
                }
                "show" | "hide" => {
                    let is_show = tokens[i].text() == "show";
                    let members = parse_member_list(&tokens, i + 1);
                    if is_show {
                        vis.show = Some(members);
                    } else {
                        vis.hide = Some(members);
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }

    vis
}

fn skip_ws(tokens: &[sass_parser::syntax::SyntaxToken], start: usize) -> usize {
    let mut i = start;
    while i < tokens.len() && tokens[i].kind() == SyntaxKind::WHITESPACE {
        i += 1;
    }
    i
}

fn parse_member_list(tokens: &[sass_parser::syntax::SyntaxToken], start: usize) -> Vec<String> {
    let mut members = Vec::new();
    let mut i = start;
    while i < tokens.len() {
        let kind = tokens[i].kind();
        if kind == SyntaxKind::WHITESPACE || kind == SyntaxKind::COMMA {
            i += 1;
            continue;
        }
        if kind == SyntaxKind::SEMICOLON {
            break;
        }
        if kind == SyntaxKind::IDENT {
            let text = tokens[i].text();
            if text == "as" || text == "with" {
                break;
            }
            members.push(text.to_string());
        } else if kind == SyntaxKind::DOLLAR {
            // Variable: $name — take the next IDENT
            i += 1;
            if i < tokens.len() && tokens[i].kind() == SyntaxKind::IDENT {
                members.push(tokens[i].text().to_string());
            }
        } else {
            break;
        }
        i += 1;
    }
    members
}

/// Walk all `FORWARD_RULE` nodes in a file's CST and find token ranges where
/// `name` appears in a `show` or `hide` clause. For variables, matches `$name`.
fn find_name_in_forward_clauses(
    root: &SyntaxNode,
    name: &str,
    kind: symbols::SymbolKind,
) -> Vec<TextRange> {
    let mut results = Vec::new();

    for node in root.descendants() {
        if node.kind() != SyntaxKind::FORWARD_RULE {
            continue;
        }

        let tokens: Vec<_> = node
            .children_with_tokens()
            .filter_map(rowan::NodeOrToken::into_token)
            .collect();

        let mut i = 0;
        let mut in_show_hide = false;

        while i < tokens.len() {
            let tok = &tokens[i];
            if tok.kind() == SyntaxKind::IDENT && (tok.text() == "show" || tok.text() == "hide") {
                in_show_hide = true;
                i += 1;
                continue;
            }

            if !in_show_hide {
                i += 1;
                continue;
            }

            // End of show/hide list
            if tok.kind() == SyntaxKind::SEMICOLON {
                break;
            }
            if tok.kind() == SyntaxKind::IDENT
                && (tok.text() == "as" || tok.text() == "with")
            {
                break;
            }

            // Variable: $name
            if kind == symbols::SymbolKind::Variable && tok.kind() == SyntaxKind::DOLLAR {
                if let Some(next) = tokens.get(i + 1) {
                    if next.kind() == SyntaxKind::IDENT && next.text() == name {
                        // Range covers just the ident (without $), matching name_only_range
                        results.push(next.text_range());
                    }
                }
                i += 2;
                continue;
            }

            // Function/mixin/placeholder: plain IDENT
            if kind != symbols::SymbolKind::Variable
                && tok.kind() == SyntaxKind::IDENT
                && tok.text() == name
            {
                results.push(tok.text_range());
            }

            i += 1;
        }
    }

    results
}

fn is_visible(vis: &ForwardVisibility, name: &str) -> bool {
    if let Some(show) = &vis.show {
        return show.iter().any(|s| s == name);
    }
    if let Some(hide) = &vis.hide {
        return !hide.iter().any(|s| s == name);
    }
    true
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

// ── Find-references helpers ─────────────────────────────────────────

fn symbol_to_ref_kind(kind: symbols::SymbolKind) -> symbols::RefKind {
    match kind {
        symbols::SymbolKind::Variable => symbols::RefKind::Variable,
        symbols::SymbolKind::Function => symbols::RefKind::Function,
        symbols::SymbolKind::Mixin => symbols::RefKind::Mixin,
        symbols::SymbolKind::Placeholder => symbols::RefKind::Placeholder,
    }
}

/// Extract namespace, name, kind, and selection range from a `NAMESPACE_REF` node.
fn extract_ns_ref_info(
    node: &SyntaxNode,
) -> Option<(String, String, symbols::SymbolKind, TextRange)> {
    let tokens: Vec<_> = node
        .children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .collect();

    let namespace = tokens
        .iter()
        .find(|t| t.kind() == SyntaxKind::IDENT)?
        .text()
        .to_string();

    // ns.$var: IDENT DOT DOLLAR IDENT
    if let Some(dollar) = tokens.iter().find(|t| t.kind() == SyntaxKind::DOLLAR) {
        let ident = tokens
            .iter()
            .skip_while(|t| t.kind() != SyntaxKind::DOLLAR)
            .find(|t| t.kind() == SyntaxKind::IDENT)?;
        let range = TextRange::new(dollar.text_range().start(), ident.text_range().end());
        return Some((
            namespace,
            ident.text().to_string(),
            symbols::SymbolKind::Variable,
            range,
        ));
    }

    // ns.func(): has FUNCTION_CALL child
    if let Some(func_call) = node
        .children()
        .find(|c| c.kind() == SyntaxKind::FUNCTION_CALL)
    {
        let ident = func_call
            .children_with_tokens()
            .filter_map(rowan::NodeOrToken::into_token)
            .find(|t| t.kind() == SyntaxKind::IDENT)?;
        return Some((
            namespace,
            ident.text().to_string(),
            symbols::SymbolKind::Function,
            ident.text_range(),
        ));
    }

    // ns.mixin (inside @include): IDENT DOT IDENT
    let dot_pos = tokens.iter().position(|t| t.kind() == SyntaxKind::DOT)?;
    let ident = tokens[dot_pos + 1..]
        .iter()
        .find(|t| t.kind() == SyntaxKind::IDENT)?;

    let is_mixin = node
        .parent()
        .is_some_and(|p| p.kind() == SyntaxKind::INCLUDE_RULE);

    let kind = if is_mixin {
        symbols::SymbolKind::Mixin
    } else {
        symbols::SymbolKind::Function
    };

    Some((
        namespace,
        ident.text().to_string(),
        kind,
        ident.text_range(),
    ))
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
                visibility: ForwardVisibility::default(),
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
                visibility: ForwardVisibility::default(),
            }],
        );

        // c.$primary → should resolve
        let result = graph.resolve_qualified(&uri, "c", "primary", symbols::SymbolKind::Variable);
        assert!(result.is_some());
        assert_eq!(result.unwrap().1.name, "primary");

        // c.$nonexistent → None
        let result =
            graph.resolve_qualified(&uri, "c", "nonexistent", symbols::SymbolKind::Variable);
        assert!(result.is_none());

        // wrong namespace
        let result =
            graph.resolve_qualified(&uri, "colors", "primary", symbols::SymbolKind::Variable);
        assert!(result.is_none());
    }

    // ── @forward visibility tests ───────────────────────────────────

    /// 3-file chain: consumer @use "mid" as * → mid @forward "lib" with vis → lib defines symbols
    fn setup_forward_chain(vis: ForwardVisibility) -> (ModuleGraph, Uri, Uri, Uri) {
        let graph = ModuleGraph::new();
        let consumer_uri: Uri = "file:///consumer.scss".parse().unwrap();
        let mid_uri: Uri = "file:///mid.scss".parse().unwrap();
        let lib_uri: Uri = "file:///lib.scss".parse().unwrap();

        let (green, syms, li) = make_info(
            "$primary: blue;\n$secondary: green;\n@mixin btn { }\n@function size() { @return 1; }",
        );
        graph.files.insert(
            lib_uri.clone(),
            ModuleInfo {
                symbols: syms,
                green,
                line_index: li,
            },
        );

        let (green, _, li) = make_info("");
        graph.files.insert(
            mid_uri.clone(),
            ModuleInfo {
                symbols: symbols::FileSymbols::default(),
                green,
                line_index: li,
            },
        );
        // mid @forward "lib" with visibility
        graph.edges.insert(
            mid_uri.clone(),
            vec![ImportEdge {
                target: lib_uri.clone(),
                namespace: Namespace::Star,
                kind: ImportKind::Forward,
                visibility: vis,
            }],
        );

        let (green, _, li) = make_info("");
        graph.files.insert(
            consumer_uri.clone(),
            ModuleInfo {
                symbols: symbols::FileSymbols::default(),
                green,
                line_index: li,
            },
        );
        // consumer @use "mid" as *
        graph.edges.insert(
            consumer_uri.clone(),
            vec![ImportEdge {
                target: mid_uri.clone(),
                namespace: Namespace::Star,
                kind: ImportKind::Use,
                visibility: ForwardVisibility::default(),
            }],
        );

        (graph, consumer_uri, mid_uri, lib_uri)
    }

    #[test]
    fn forward_show_filter() {
        let vis = ForwardVisibility {
            show: Some(vec!["primary".into()]),
            ..Default::default()
        };
        let (graph, consumer, _, _) = setup_forward_chain(vis);

        // $primary is in show list → visible
        let r = graph.resolve_unqualified(&consumer, "primary", symbols::SymbolKind::Variable);
        assert!(r.is_some());

        // $secondary is NOT in show list → hidden
        let r = graph.resolve_unqualified(&consumer, "secondary", symbols::SymbolKind::Variable);
        assert!(r.is_none());

        // btn mixin is NOT in show list → hidden
        let r = graph.resolve_unqualified(&consumer, "btn", symbols::SymbolKind::Mixin);
        assert!(r.is_none());
    }

    #[test]
    fn forward_hide_filter() {
        let vis = ForwardVisibility {
            hide: Some(vec!["secondary".into(), "btn".into()]),
            ..Default::default()
        };
        let (graph, consumer, _, _) = setup_forward_chain(vis);

        // $primary not hidden → visible
        let r = graph.resolve_unqualified(&consumer, "primary", symbols::SymbolKind::Variable);
        assert!(r.is_some());

        // $secondary hidden → not visible
        let r = graph.resolve_unqualified(&consumer, "secondary", symbols::SymbolKind::Variable);
        assert!(r.is_none());

        // btn hidden → not visible
        let r = graph.resolve_unqualified(&consumer, "btn", symbols::SymbolKind::Mixin);
        assert!(r.is_none());

        // size not hidden → visible
        let r = graph.resolve_unqualified(&consumer, "size", symbols::SymbolKind::Function);
        assert!(r.is_some());
    }

    #[test]
    fn forward_prefix_rename() {
        let vis = ForwardVisibility {
            prefix: Some("lib-".into()),
            ..Default::default()
        };
        let (graph, consumer, _, _) = setup_forward_chain(vis);

        // visible_symbols from consumer: symbols get prefix lib-*
        let visible = graph.visible_symbols(&consumer);
        let names: Vec<&str> = visible.iter().map(|(_, _, s)| s.name.as_str()).collect();
        assert!(names.contains(&"lib-primary"));
        assert!(names.contains(&"lib-secondary"));
        assert!(names.contains(&"lib-btn"));
        assert!(names.contains(&"lib-size"));
        assert!(!names.contains(&"primary"));
    }

    #[test]
    fn forward_prefix_resolution() {
        let vis = ForwardVisibility {
            prefix: Some("lib-".into()),
            ..Default::default()
        };
        let (graph, consumer, _, _) = setup_forward_chain(vis);

        // resolve_unqualified("lib-primary") → strips prefix → finds "primary"
        let r = graph.resolve_unqualified(&consumer, "lib-primary", symbols::SymbolKind::Variable);
        assert!(r.is_some());
        assert_eq!(r.unwrap().1.name, "primary");

        // "primary" without prefix → not found (prefix required)
        let r = graph.resolve_unqualified(&consumer, "primary", symbols::SymbolKind::Variable);
        assert!(r.is_none());
    }

    #[test]
    fn forward_combined_prefix_hide() {
        let vis = ForwardVisibility {
            prefix: Some("lib-".into()),
            hide: Some(vec!["secondary".into()]),
            ..Default::default()
        };
        let (graph, consumer, _, _) = setup_forward_chain(vis);

        // lib-primary → strips prefix → "primary" not hidden → found
        let r = graph.resolve_unqualified(&consumer, "lib-primary", symbols::SymbolKind::Variable);
        assert!(r.is_some());

        // lib-secondary → strips prefix → "secondary" is hidden → not found
        let r =
            graph.resolve_unqualified(&consumer, "lib-secondary", symbols::SymbolKind::Variable);
        assert!(r.is_none());

        // visible_symbols should have lib-primary, lib-btn, lib-size but NOT lib-secondary
        let visible = graph.visible_symbols(&consumer);
        let names: Vec<&str> = visible.iter().map(|(_, _, s)| s.name.as_str()).collect();
        assert!(names.contains(&"lib-primary"));
        assert!(!names.contains(&"lib-secondary"));
        assert!(names.contains(&"lib-btn"));
    }

    #[test]
    fn nested_forward_visibility() {
        // lib defines $primary, $secondary, @mixin btn
        // mid @forward "lib" show primary, btn
        // top @forward "mid" as m-*
        // consumer @use "top" as *
        let graph = ModuleGraph::new();
        let consumer_uri: Uri = "file:///consumer.scss".parse().unwrap();
        let top_uri: Uri = "file:///top.scss".parse().unwrap();
        let mid_uri: Uri = "file:///mid.scss".parse().unwrap();
        let lib_uri: Uri = "file:///lib.scss".parse().unwrap();

        let (green, syms, li) = make_info("$primary: blue;\n$secondary: green;\n@mixin btn { }");
        graph.files.insert(
            lib_uri.clone(),
            ModuleInfo {
                symbols: syms,
                green,
                line_index: li,
            },
        );

        let (green, _, li) = make_info("");
        graph.files.insert(
            mid_uri.clone(),
            ModuleInfo {
                symbols: symbols::FileSymbols::default(),
                green,
                line_index: li,
            },
        );
        graph.edges.insert(
            mid_uri.clone(),
            vec![ImportEdge {
                target: lib_uri.clone(),
                namespace: Namespace::Star,
                kind: ImportKind::Forward,
                visibility: ForwardVisibility {
                    show: Some(vec!["primary".into(), "btn".into()]),
                    ..Default::default()
                },
            }],
        );

        let (green, _, li) = make_info("");
        graph.files.insert(
            top_uri.clone(),
            ModuleInfo {
                symbols: symbols::FileSymbols::default(),
                green,
                line_index: li,
            },
        );
        graph.edges.insert(
            top_uri.clone(),
            vec![ImportEdge {
                target: mid_uri.clone(),
                namespace: Namespace::Star,
                kind: ImportKind::Forward,
                visibility: ForwardVisibility {
                    prefix: Some("m-".into()),
                    ..Default::default()
                },
            }],
        );

        let (green, _, li) = make_info("");
        graph.files.insert(
            consumer_uri.clone(),
            ModuleInfo {
                symbols: symbols::FileSymbols::default(),
                green,
                line_index: li,
            },
        );
        graph.edges.insert(
            consumer_uri.clone(),
            vec![ImportEdge {
                target: top_uri.clone(),
                namespace: Namespace::Star,
                kind: ImportKind::Use,
                visibility: ForwardVisibility::default(),
            }],
        );

        // m-primary visible (prefix m- + show includes primary)
        let r =
            graph.resolve_unqualified(&consumer_uri, "m-primary", symbols::SymbolKind::Variable);
        assert!(r.is_some());

        // m-btn visible
        let r = graph.resolve_unqualified(&consumer_uri, "m-btn", symbols::SymbolKind::Mixin);
        assert!(r.is_some());

        // m-secondary filtered out by inner show list
        let r =
            graph.resolve_unqualified(&consumer_uri, "m-secondary", symbols::SymbolKind::Variable);
        assert!(r.is_none());

        // primary without prefix → not found
        let r = graph.resolve_unqualified(&consumer_uri, "primary", symbols::SymbolKind::Variable);
        assert!(r.is_none());
    }

    #[test]
    fn forward_as_does_not_create_namespace() {
        // @forward "lib" as btn-* should NOT create Named("btn-") namespace
        let (green, _) = sass_parser::parse("@forward \"lib\" as btn-*;");
        let root = SyntaxNode::new_root(green);
        let imports = imports::collect_imports(&root);
        assert_eq!(imports.len(), 1);
        let ns = extract_namespace(&root, &imports[0]);
        assert_eq!(ns, Namespace::Star);
    }

    // ── Builtin module tests ────────────────────────────────────────

    fn setup_builtin_graph(module: &str, namespace: Namespace) -> (ModuleGraph, Uri) {
        let graph = ModuleGraph::new();
        let uri: Uri = "file:///main.scss".parse().unwrap();

        let (green, _, li) = make_info("");
        graph.files.insert(
            uri.clone(),
            ModuleInfo {
                symbols: symbols::FileSymbols::default(),
                green,
                line_index: li,
            },
        );

        let syms = builtins::builtin_symbols(module).unwrap();
        graph.builtin_symbols.insert(module.to_owned(), syms);

        let target_uri: Uri = builtins::builtin_uri(module).parse().unwrap();
        graph.edges.insert(
            uri.clone(),
            vec![ImportEdge {
                target: target_uri,
                namespace,
                kind: ImportKind::Use,
                visibility: ForwardVisibility::default(),
            }],
        );

        (graph, uri)
    }

    #[test]
    fn builtin_qualified_resolution() {
        let (graph, uri) = setup_builtin_graph("math", Namespace::Named("math".into()));

        let r = graph.resolve_qualified(&uri, "math", "ceil", symbols::SymbolKind::Function);
        assert!(r.is_some());
        let (target_uri, sym) = r.unwrap();
        assert_eq!(sym.name, "ceil");
        assert!(builtins::is_builtin_uri(target_uri.as_str()));
    }

    #[test]
    fn builtin_alias_resolution() {
        let (graph, uri) = setup_builtin_graph("math", Namespace::Named("m".into()));

        let r = graph.resolve_qualified(&uri, "m", "ceil", symbols::SymbolKind::Function);
        assert!(r.is_some());
        assert_eq!(r.unwrap().1.name, "ceil");

        // Wrong namespace
        let r = graph.resolve_qualified(&uri, "math", "ceil", symbols::SymbolKind::Function);
        assert!(r.is_none());
    }

    #[test]
    fn builtin_star_import() {
        let (graph, uri) = setup_builtin_graph("math", Namespace::Star);

        let r = graph.resolve_unqualified(&uri, "ceil", symbols::SymbolKind::Function);
        assert!(r.is_some());
        assert_eq!(r.unwrap().1.name, "ceil");

        let r = graph.resolve_unqualified(&uri, "pi", symbols::SymbolKind::Variable);
        assert!(r.is_some());
    }

    #[test]
    fn builtin_visible_symbols() {
        let (graph, uri) = setup_builtin_graph("math", Namespace::Named("math".into()));

        let visible = graph.visible_symbols(&uri);
        let math_syms: Vec<&str> = visible
            .iter()
            .filter(|(ns, _, _)| ns.as_deref() == Some("math"))
            .map(|(_, _, s)| s.name.as_str())
            .collect();
        assert!(math_syms.contains(&"ceil"));
        assert!(math_syms.contains(&"pi"));
        assert!(math_syms.contains(&"floor"));
        assert!(math_syms.len() > 20);
    }

    #[test]
    fn builtin_has_docs() {
        let (graph, uri) = setup_builtin_graph("color", Namespace::Named("color".into()));

        let r = graph.resolve_qualified(&uri, "color", "mix", symbols::SymbolKind::Function);
        assert!(r.is_some());
        let sym = r.unwrap().1;
        assert!(sym.doc.is_some());
        assert!(sym.params.is_some());
    }

    #[test]
    fn builtin_unknown_module() {
        let syms = builtins::builtin_symbols("nope");
        assert!(syms.is_none());
    }

    // ── Prepend imports tests ───────────────────────────────────────

    #[test]
    fn prepend_import_visible_symbols() {
        // Simulate: prependImports = ["globals"] → every file gets @use "globals" as *
        let graph = ModuleGraph::new();
        let uri: Uri = "file:///main.scss".parse().unwrap();
        let globals_uri: Uri = "file:///globals.scss".parse().unwrap();

        let (green, syms, li) = make_info("$brand: red;\n@mixin container { }");
        graph.files.insert(
            globals_uri.clone(),
            ModuleInfo {
                symbols: syms,
                green,
                line_index: li,
            },
        );

        let (green, syms, li) = make_info("$local: blue;");
        graph.files.insert(
            uri.clone(),
            ModuleInfo {
                symbols: syms,
                green,
                line_index: li,
            },
        );

        // Synthetic star import from prepend
        graph.edges.insert(
            uri.clone(),
            vec![ImportEdge {
                target: globals_uri.clone(),
                namespace: Namespace::Star,
                kind: ImportKind::Use,
                visibility: ForwardVisibility::default(),
            }],
        );

        let visible = graph.visible_symbols(&uri);
        let names: Vec<&str> = visible.iter().map(|(_, _, s)| s.name.as_str()).collect();
        assert!(names.contains(&"local"));
        assert!(names.contains(&"brand"));
        assert!(names.contains(&"container"));
    }

    #[test]
    fn prepend_import_unqualified_resolution() {
        let graph = ModuleGraph::new();
        let uri: Uri = "file:///main.scss".parse().unwrap();
        let globals_uri: Uri = "file:///globals.scss".parse().unwrap();

        let (green, syms, li) = make_info("$brand: red;");
        graph.files.insert(
            globals_uri.clone(),
            ModuleInfo {
                symbols: syms,
                green,
                line_index: li,
            },
        );

        let (green, _, li) = make_info("");
        graph.files.insert(
            uri.clone(),
            ModuleInfo {
                symbols: symbols::FileSymbols::default(),
                green,
                line_index: li,
            },
        );

        graph.edges.insert(
            uri.clone(),
            vec![ImportEdge {
                target: globals_uri.clone(),
                namespace: Namespace::Star,
                kind: ImportKind::Use,
                visibility: ForwardVisibility::default(),
            }],
        );

        let r = graph.resolve_unqualified(&uri, "brand", symbols::SymbolKind::Variable);
        assert!(r.is_some());
        assert_eq!(r.unwrap().1.name, "brand");
    }

    #[test]
    fn config_deserializes_prepend_imports() {
        let json = serde_json::json!({
            "loadPaths": ["src/styles"],
            "prependImports": ["src/globals", "src/vars"],
            "importAliases": {}
        });
        let config: crate::config::SassAnalyzerConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config.prepend_imports, vec!["src/globals", "src/vars"]);
    }

    // ── Rename safety tests ─────────────────────────────────────────

    #[test]
    fn check_name_conflict_same_kind() {
        let graph = ModuleGraph::new();
        let uri: Uri = "file:///test.scss".parse().unwrap();
        let (green, syms, li) = make_info("$color: red;\n$primary: blue;");
        graph.files.insert(
            uri.clone(),
            ModuleInfo {
                symbols: syms,
                green,
                line_index: li,
            },
        );

        assert!(graph.check_name_conflict(&uri, "primary", symbols::SymbolKind::Variable));
        assert!(!graph.check_name_conflict(&uri, "shade", symbols::SymbolKind::Variable));
    }

    #[test]
    fn check_name_conflict_different_kind_no_conflict() {
        let graph = ModuleGraph::new();
        let uri: Uri = "file:///test.scss".parse().unwrap();
        let (green, syms, li) =
            make_info("$color: red;\n@function color() { @return red; }");
        graph.files.insert(
            uri.clone(),
            ModuleInfo {
                symbols: syms,
                green,
                line_index: li,
            },
        );

        // "color" exists as function, but we're checking for variable → no conflict
        assert!(!graph.check_name_conflict(&uri, "color", symbols::SymbolKind::Mixin));
    }

    #[test]
    fn find_forward_show_hide_variable() {
        let source = "@forward \"lib\" show $primary, $secondary;";
        let (green, _) = sass_parser::parse(source);
        let root = SyntaxNode::new_root(green);

        let ranges =
            find_name_in_forward_clauses(&root, "primary", symbols::SymbolKind::Variable);
        assert_eq!(ranges.len(), 1);
        let text = &source[usize::from(ranges[0].start())..usize::from(ranges[0].end())];
        assert_eq!(text, "primary");
    }

    #[test]
    fn find_forward_show_hide_mixin() {
        let source = "@forward \"lib\" show btn, card;";
        let (green, _) = sass_parser::parse(source);
        let root = SyntaxNode::new_root(green);

        let ranges = find_name_in_forward_clauses(&root, "btn", symbols::SymbolKind::Mixin);
        assert_eq!(ranges.len(), 1);
        let text = &source[usize::from(ranges[0].start())..usize::from(ranges[0].end())];
        assert_eq!(text, "btn");
    }

    #[test]
    fn find_forward_hide_clause() {
        let source = "@forward \"lib\" hide $internal;";
        let (green, _) = sass_parser::parse(source);
        let root = SyntaxNode::new_root(green);

        let ranges =
            find_name_in_forward_clauses(&root, "internal", symbols::SymbolKind::Variable);
        assert_eq!(ranges.len(), 1);
    }

    #[test]
    fn find_forward_clause_no_match() {
        let source = "@forward \"lib\" show $primary;";
        let (green, _) = sass_parser::parse(source);
        let root = SyntaxNode::new_root(green);

        let ranges =
            find_name_in_forward_clauses(&root, "secondary", symbols::SymbolKind::Variable);
        assert!(ranges.is_empty());
    }

    #[test]
    fn forward_reaches_direct() {
        let graph = ModuleGraph::new();
        let a: Uri = "file:///a.scss".parse().unwrap();
        let b: Uri = "file:///b.scss".parse().unwrap();

        graph.edges.insert(
            a.clone(),
            vec![ImportEdge {
                target: b.clone(),
                namespace: Namespace::Star,
                kind: ImportKind::Forward,
                visibility: ForwardVisibility::default(),
            }],
        );

        assert!(graph.forward_reaches(&b, &b));
        assert!(graph.forward_reaches(&a, &b));
        assert!(!graph.forward_reaches(&b, &a));
    }

    #[test]
    fn find_forward_show_hide_refs_integration() {
        let graph = ModuleGraph::new();
        let main_uri: Uri = "file:///main.scss".parse().unwrap();
        let lib_uri: Uri = "file:///lib.scss".parse().unwrap();

        let (green, syms, li) = make_info("$primary: blue;");
        graph.files.insert(
            lib_uri.clone(),
            ModuleInfo {
                symbols: syms,
                green,
                line_index: li,
            },
        );

        // main: @forward "lib" show $primary;
        let main_source = "@forward \"lib\" show $primary;";
        let (green, syms, li) = make_info(main_source);
        graph.files.insert(
            main_uri.clone(),
            ModuleInfo {
                symbols: syms,
                green,
                line_index: li,
            },
        );
        graph.edges.insert(
            main_uri.clone(),
            vec![ImportEdge {
                target: lib_uri.clone(),
                namespace: Namespace::Star,
                kind: ImportKind::Forward,
                visibility: ForwardVisibility {
                    show: Some(vec!["primary".into()]),
                    ..Default::default()
                },
            }],
        );

        let refs = graph.find_forward_show_hide_references(
            &lib_uri,
            "primary",
            symbols::SymbolKind::Variable,
        );
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].0, main_uri);
        let text =
            &main_source[usize::from(refs[0].1.start())..usize::from(refs[0].1.end())];
        assert_eq!(text, "primary");
    }
}
