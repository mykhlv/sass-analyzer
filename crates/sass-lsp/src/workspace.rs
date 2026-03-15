use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use dashmap::DashMap;
use sass_parser::imports::{self, ImportKind, ImportRef};
use sass_parser::resolver::{ModuleResolver, ResolvedModule};
use sass_parser::syntax::SyntaxNode;
use sass_parser::syntax_kind::SyntaxKind;
use sass_parser::text_range::TextRange;
use sass_parser::vfs::OsFileSystem;
use tower_lsp_server::ls_types::Uri;

use crate::builtins;
use crate::config::RuntimeConfig;
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
#[derive(Debug, Clone)]
pub struct ImportEdge {
    pub target: Uri,
    pub namespace: Namespace,
    pub kind: ImportKind,
    pub visibility: ForwardVisibility,
}

/// Parsed + analyzed info for a single file.
/// Symbols and line index are always retained. Both the green tree and
/// source text may be evicted by their respective LRU caches to cap memory.
/// The green tree is re-parsed on demand from `source_text`; source text
/// is reconstructed on demand from the green tree via `green.text()`.
/// Invariant: at least one of `green` or `source_text` is always `Some`.
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub symbols: Arc<FileSymbols>,
    pub green: Option<rowan::GreenNode>,
    pub line_index: sass_parser::line_index::LineIndex,
    pub source_text: Option<String>,
    /// True if the file uses `@import` or `meta.load-css()`, which merge scopes
    /// in ways the module graph cannot fully track.
    pub has_legacy_import: bool,
    /// True if the file has `@use`/`@forward` imports that could not be resolved.
    pub has_unresolved_use: bool,
}

/// Cross-file dependency graph for the SCSS workspace.
pub struct ModuleGraph {
    files: DashMap<Uri, ModuleInfo>,
    edges: DashMap<Uri, Vec<ImportEdge>>,
    resolver: RwLock<Arc<ModuleResolver<OsFileSystem>>>,
    builtin_symbols: DashMap<String, Vec<symbols::Symbol>>,
    prepend_imports: RwLock<Vec<String>>,
    /// Canonicalized roots that bound filesystem reads (workspace + `load_paths` + alias targets).
    allowed_roots: RwLock<Vec<PathBuf>>,
    /// LRU order for green tree eviction (front = most recently used).
    tree_lru: Mutex<VecDeque<Uri>>,
    /// LRU order for source text eviction (front = most recently used).
    source_lru: Mutex<VecDeque<Uri>>,
    runtime_config: Arc<RuntimeConfig>,
}

impl ModuleGraph {
    pub fn new(runtime_config: Arc<RuntimeConfig>) -> Self {
        Self {
            files: DashMap::new(),
            edges: DashMap::new(),
            resolver: RwLock::new(Arc::new(ModuleResolver::new())),
            builtin_symbols: DashMap::new(),
            prepend_imports: RwLock::new(Vec::new()),
            allowed_roots: RwLock::new(Vec::new()),
            tree_lru: Mutex::new(VecDeque::new()),
            source_lru: Mutex::new(VecDeque::new()),
            runtime_config,
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

    /// Set allowed filesystem roots. Resolved paths outside these roots are rejected.
    pub fn set_allowed_roots(&self, roots: Vec<PathBuf>) {
        let canonical: Vec<PathBuf> = roots
            .into_iter()
            .filter_map(|r| std::fs::canonicalize(&r).ok())
            .collect();
        let mut guard = self
            .allowed_roots
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = canonical;
    }

    /// Check if a path is under one of the allowed roots.
    /// Returns `true` if no roots are configured (permissive fallback).
    fn is_path_allowed(&self, path: &Path) -> bool {
        let roots = self
            .allowed_roots
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if roots.is_empty() {
            return true;
        }
        // Try canonicalizing the full path first; if the file doesn't exist yet,
        // canonicalize the parent directory and append the file name.
        let canonical = std::fs::canonicalize(path).or_else(|_| {
            path.parent()
                .and_then(|p| std::fs::canonicalize(p).ok())
                .map(|p| p.join(path.file_name().unwrap_or_default()))
                .ok_or(std::io::ErrorKind::NotFound)
        });
        let Ok(canonical) = canonical else {
            return false;
        };
        roots.iter().any(|root| canonical.starts_with(root))
    }

    /// Replace the resolver with a configured one (called once from `initialize`).
    pub fn set_resolver(&self, resolver: ModuleResolver<OsFileSystem>) {
        let mut guard = self
            .resolver
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = Arc::new(resolver);
    }

    /// Move a URI to the front of the tree LRU list (most recently used).
    fn touch_tree_lru(&self, uri: &Uri) {
        let mut lru = self
            .tree_lru
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(pos) = lru.iter().position(|u| u == uri) {
            lru.remove(pos);
        }
        lru.push_front(uri.clone());
    }

    /// Move a URI to the front of the source LRU list (most recently used).
    fn touch_source_lru(&self, uri: &Uri) {
        let mut lru = self
            .source_lru
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(pos) = lru.iter().position(|u| u == uri) {
            lru.remove(pos);
        }
        lru.push_front(uri.clone());
    }

    /// Evict green trees from files beyond the LRU limit.
    /// Never evicts a green tree if the file's `source_text` is also evicted.
    fn evict_trees(&self) {
        let mut lru = self
            .tree_lru
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut kept = Vec::new();
        while lru.len() > self.runtime_config.max_cached_trees() {
            let Some(evicted_uri) = lru.pop_back() else {
                break;
            };
            if let Some(mut info) = self.files.get_mut(&evicted_uri) {
                if info.source_text.is_some() {
                    info.green = None;
                } else {
                    kept.push(evicted_uri);
                }
            }
        }
        for uri in kept {
            lru.push_back(uri);
        }
    }

    /// Evict source text from files beyond the LRU limit.
    /// Never evicts source text if the file's green tree is also evicted.
    fn evict_sources(&self) {
        let mut lru = self
            .source_lru
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut kept = Vec::new();
        while lru.len() > self.runtime_config.max_cached_sources() {
            let Some(evicted_uri) = lru.pop_back() else {
                break;
            };
            if let Some(mut info) = self.files.get_mut(&evicted_uri) {
                if info.green.is_some() {
                    info.source_text = None;
                } else {
                    kept.push(evicted_uri);
                }
            }
        }
        for uri in kept {
            lru.push_back(uri);
        }
    }

    /// Get the green tree for a URI, re-parsing from stored source if evicted.
    fn get_or_reparse_green(&self, uri: &Uri) -> Option<rowan::GreenNode> {
        let info = self.files.get(uri)?;
        if let Some(green) = &info.green {
            return Some(green.clone());
        }
        // Re-parse from stored text (invariant: source_text is Some when green is None).
        let source = info.source_text.clone()?;
        drop(info);
        let (green, _) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            sass_parser::parse_scss(&source)
        }))
        .ok()?;
        if let Some(mut info) = self.files.get_mut(uri) {
            info.green = Some(green.clone());
        }
        self.touch_tree_lru(uri);
        self.evict_trees();
        Some(green)
    }

    /// Total number of indexed files.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Number of files currently holding a green tree in memory.
    pub fn cached_tree_count(&self) -> usize {
        self.files.iter().filter(|e| e.green.is_some()).count()
    }

    /// Index a file: parse if needed, collect symbols and resolve imports.
    /// `open_docs` provides in-memory text for open documents (takes priority over disk).
    pub fn index_file(
        &self,
        uri: &Uri,
        green: rowan::GreenNode,
        symbols: Arc<FileSymbols>,
        line_index: sass_parser::line_index::LineIndex,
        source_text: String,
    ) {
        let root = SyntaxNode::new_root(green.clone());
        let import_refs = imports::collect_imports(&root);
        let has_legacy_import = import_refs
            .iter()
            .any(|r| r.kind == ImportKind::Import || r.kind == ImportKind::LoadCss);

        self.files.insert(
            uri.clone(),
            ModuleInfo {
                symbols,
                green: Some(green),
                line_index,
                source_text: Some(source_text),
                has_legacy_import,
                has_unresolved_use: false, // updated after resolution loop below
            },
        );
        self.touch_tree_lru(uri);
        self.touch_source_lru(uri);
        self.evict_trees();
        self.evict_sources();
        let base_path = uri_to_path(uri);

        let mut resolved_edges = Vec::new();
        let mut has_unresolved_use = false;

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

                self.index_dependency(&target_uri, &target_path);

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
            } else if import_ref.kind == ImportKind::Use || import_ref.kind == ImportKind::Forward {
                has_unresolved_use = true;
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
                    drop(resolver);
                    self.index_dependency(&target_uri, &target_path);
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

        if has_unresolved_use && let Some(mut info) = self.files.get_mut(uri) {
            info.has_unresolved_use = true;
        }
    }

    /// Check if a file has `@import` or `meta.load-css()` — these can provide
    /// definitions from unindexed sources, so undefined-reference warnings
    /// should be suppressed.
    pub fn has_unresolved_imports(&self, uri: &Uri) -> bool {
        self.files
            .get(uri)
            .is_some_and(|info| info.has_legacy_import || info.has_unresolved_use)
    }

    /// Find all files that directly import the given URI.
    pub fn dependents_of(&self, uri: &Uri) -> Vec<Uri> {
        let mut result = Vec::new();
        for entry in &self.edges {
            if entry.value().iter().any(|edge| &edge.target == uri) {
                result.push(entry.key().clone());
            }
        }
        result
    }

    /// Remove a file from the graph (used by file watcher for deleted files).
    pub fn remove_file(&self, uri: &Uri) {
        self.files.remove(uri);
        self.edges.remove(uri);
        let mut tree_lru = self
            .tree_lru
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(pos) = tree_lru.iter().position(|u| u == uri) {
            tree_lru.remove(pos);
        }
        drop(tree_lru);
        let mut src_lru = self
            .source_lru
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(pos) = src_lru.iter().position(|u| u == uri) {
            src_lru.remove(pos);
        }
    }

    /// Resolve a qualified name (`namespace.$name` or `namespace.func()`)
    /// from a given source file. Returns the target URI and matching Symbol.
    /// Resolve a reference — dispatches to `resolve_qualified` or `resolve_unqualified`
    /// depending on whether a namespace is present.
    pub fn resolve_reference(
        &self,
        from: &Uri,
        namespace: Option<&str>,
        name: &str,
        kind: symbols::SymbolKind,
    ) -> Option<(Uri, symbols::Symbol)> {
        if let Some(ns) = namespace {
            self.resolve_qualified(from, ns, name, kind)
        } else {
            self.resolve_unqualified(from, name, kind)
        }
    }

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
        if let Some(info) = self.files.get(from)
            && let Some(sym) = info
                .symbols
                .definitions
                .iter()
                .find(|s| s.name == name && s.kind == kind)
        {
            return Some((from.clone(), sym.clone()));
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
            let mut visited = HashSet::new();
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

    /// Get the source text for a URI. Reconstructs from the green tree if evicted,
    /// and caches the result to avoid repeated reconstruction.
    pub fn source_text(&self, uri: &Uri) -> Option<String> {
        {
            let info = self.files.get(uri)?;
            if let Some(src) = &info.source_text {
                return Some(src.clone());
            }
        }
        // Reconstruct from green tree and cache back.
        let mut info = self.files.get_mut(uri)?;
        // Double-check after re-acquiring (another thread may have filled it).
        if let Some(src) = &info.source_text {
            return Some(src.clone());
        }
        let green = info.green.as_ref()?;
        let root = SyntaxNode::new_root(green.clone());
        let text = root.text().to_string();
        info.source_text = Some(text.clone());
        Some(text)
    }

    pub fn get_symbols(&self, uri: &Uri) -> Option<std::sync::Arc<symbols::FileSymbols>> {
        self.files.get(uri).map(|info| info.symbols.clone())
    }

    pub fn get_green(&self, uri: &Uri) -> Option<rowan::GreenNode> {
        self.get_or_reparse_green(uri)
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

    /// Provide completion items for `@use "` / `@forward "` import paths.
    /// Returns built-in module names and relative SCSS files from the same directory.
    pub fn complete_use_paths(
        &self,
        from: &Uri,
        partial: &str,
    ) -> Vec<tower_lsp_server::ls_types::CompletionItem> {
        use tower_lsp_server::ls_types::{CompletionItem, CompletionItemKind};

        let mut items = Vec::new();

        // Built-in modules: sass:math, sass:color, etc.
        let builtins = ["math", "color", "list", "map", "selector", "string", "meta"];
        for name in &builtins {
            let label = format!("sass:{name}");
            if partial.is_empty() || label.starts_with(partial) {
                items.push(CompletionItem {
                    label,
                    kind: Some(CompletionItemKind::MODULE),
                    sort_text: Some(format!("1_{name}")),
                    ..CompletionItem::default()
                });
            }
        }

        // Relative SCSS files from the same directory
        let Some(base_path) = uri_to_path(from) else {
            return items;
        };
        let Some(dir) = base_path.parent() else {
            return items;
        };

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path == base_path {
                    continue;
                }
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };

                if path.is_dir() {
                    // Directory — could be an index import
                    if partial.is_empty() || name.starts_with(partial) {
                        items.push(CompletionItem {
                            label: name.to_owned(),
                            kind: Some(CompletionItemKind::FOLDER),
                            sort_text: Some(format!("0_{name}")),
                            ..CompletionItem::default()
                        });
                    }
                } else if is_scss_or_css(name) {
                    // Normalize: strip leading _, strip .scss extension
                    let stem = name.strip_prefix('_').unwrap_or(name);
                    let stem = stem
                        .strip_suffix(".scss")
                        .or_else(|| stem.strip_suffix(".css"))
                        .unwrap_or(stem);
                    if partial.is_empty() || stem.starts_with(partial) {
                        items.push(CompletionItem {
                            label: stem.to_owned(),
                            kind: Some(CompletionItemKind::FILE),
                            sort_text: Some(format!("0_{stem}")),
                            ..CompletionItem::default()
                        });
                    }
                }
            }
        }

        // Also scan load_paths
        let resolver = self
            .resolver
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for load_path in resolver.load_paths() {
            if let Ok(entries) = std::fs::read_dir(load_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                        continue;
                    };

                    if path.is_dir() {
                        if partial.is_empty() || name.starts_with(partial) {
                            items.push(CompletionItem {
                                label: name.to_owned(),
                                kind: Some(CompletionItemKind::FOLDER),
                                sort_text: Some(format!("0_{name}")),
                                ..CompletionItem::default()
                            });
                        }
                    } else if is_scss_or_css(name) {
                        let stem = name.strip_prefix('_').unwrap_or(name);
                        let stem = stem
                            .strip_suffix(".scss")
                            .or_else(|| stem.strip_suffix(".css"))
                            .unwrap_or(stem);
                        if partial.is_empty() || stem.starts_with(partial) {
                            items.push(CompletionItem {
                                label: stem.to_owned(),
                                kind: Some(CompletionItemKind::FILE),
                                sort_text: Some(format!("0_{stem}")),
                                ..CompletionItem::default()
                            });
                        }
                    }
                }
            }
        }

        // Deduplicate by label
        items.sort_by(|a, b| a.label.cmp(&b.label));
        items.dedup_by(|a, b| a.label == b.label);
        items
    }

    /// Find all references to a symbol across the entire workspace.
    ///
    /// O(files × refs) — iterates all indexed files and checks each reference.
    /// No reverse index is maintained. Acceptable for typical SCSS workspaces
    /// (hundreds of files); consider a reverse index if profiling shows this as
    /// a bottleneck in large monorepos.
    pub fn find_all_references(
        &self,
        target_uri: &Uri,
        target_name: &str,
        target_kind: symbols::SymbolKind,
        include_declaration: bool,
    ) -> Vec<(Uri, TextRange)> {
        let mut results = Vec::new();
        let ref_kind = symbol_to_ref_kind(target_kind);

        // Collect URIs + symbol-level matches first (releases DashMap shard locks).
        let file_uris: Vec<Uri> = self.files.iter().map(|e| e.key().clone()).collect();

        for file_uri in &file_uris {
            if let Some(info) = self.files.get(file_uri) {
                // Include declaration if requested
                if include_declaration
                    && file_uri == target_uri
                    && let Some(sym) = info
                        .symbols
                        .definitions
                        .iter()
                        .find(|s| s.name == target_name && s.kind == target_kind)
                {
                    results.push((target_uri.clone(), sym.selection_range));
                }

                // Check unqualified references from SymbolRef
                for sym_ref in &info.symbols.references {
                    if sym_ref.name != target_name || sym_ref.kind != ref_kind {
                        continue;
                    }
                    if let Some((resolved_uri, _)) =
                        self.resolve_unqualified(file_uri, &sym_ref.name, target_kind)
                        && &resolved_uri == target_uri
                    {
                        results.push((file_uri.clone(), sym_ref.selection_range));
                    }
                }
            }

            // Check namespace-qualified references via CST walk
            if let Some(green) = self.get_or_reparse_green(file_uri) {
                let root = SyntaxNode::new_root(green);
                for node in root.descendants() {
                    if node.kind() != SyntaxKind::NAMESPACE_REF {
                        continue;
                    }
                    if let Some((ns, name, kind, range)) = extract_ns_ref_info(&node)
                        && name == target_name
                        && kind == target_kind
                        && let Some((resolved_uri, _)) =
                            self.resolve_qualified(file_uri, &ns, &name, target_kind)
                        && &resolved_uri == target_uri
                    {
                        results.push((file_uri.clone(), range));
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

                let has_name = edge
                    .visibility
                    .show
                    .as_ref()
                    .is_some_and(|list| list.iter().any(|n| n == old_name))
                    || edge
                        .visibility
                        .hide
                        .as_ref()
                        .is_some_and(|list| list.iter().any(|n| n == old_name));

                if !has_name {
                    continue;
                }

                // Walk the FORWARD_RULE CST to find the exact token range
                if let Some(green) = self.get_or_reparse_green(file_uri) {
                    let root = SyntaxNode::new_root(green);
                    let ranges = find_name_in_forward_clauses(&root, old_name, kind);
                    for range in ranges {
                        results.push((file_uri.clone(), range));
                    }
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
        const MAX_DEPENDENCY_FILES: usize = 10_000;

        if self.files.contains_key(uri) {
            return;
        }
        if !self.is_path_allowed(path) {
            tracing::warn!(?path, "blocked path traversal outside allowed roots");
            return;
        }
        if self.files.len() >= MAX_DEPENDENCY_FILES {
            tracing::warn!(
                limit = MAX_DEPENDENCY_FILES,
                "dependency file limit reached, skipping further indexing"
            );
            return;
        }
        let Ok(source) = std::fs::read_to_string(path) else {
            return;
        };
        let parse_fn = if path.extension().is_some_and(|ext| ext == "sass") {
            sass_parser::parse_sass
        } else {
            sass_parser::parse_scss
        };
        let Some((green, _errors)) =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| parse_fn(&source))).ok()
        else {
            return;
        };
        let line_index = sass_parser::line_index::LineIndex::new(&source);
        let file_symbols = {
            let root = SyntaxNode::new_root(green.clone());
            Arc::new(symbols::collect_symbols(&root))
        };
        // Full indexing: also resolves imports so @forward chains are tracked.
        self.index_file(uri, green, file_symbols, line_index, source);
    }
}

// ── Namespace extraction ────────────────────────────────────────────

pub(crate) fn extract_namespace(root: &SyntaxNode, import_ref: &ImportRef) -> Namespace {
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
        if token.kind() == SyntaxKind::IDENT
            && token.text() == "as"
            && let Some(next) = tokens.get(i + 1)
        {
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

    default_namespace(&import_ref.path)
}

/// Default namespace: last segment of the path without extension/underscore.
/// `@use "src/colors"` → `colors`
/// `@use "sass:math"` → `math`
pub(crate) fn default_namespace(path: &str) -> Namespace {
    if let Some(name) = path.strip_prefix("sass:") {
        return Namespace::Named(name.to_owned());
    }
    let segment = path.rsplit('/').next().unwrap_or(path);
    let stem = segment.strip_prefix('_').unwrap_or(segment);
    let stem = stem
        .strip_suffix(".scss")
        .or_else(|| stem.strip_suffix(".sass"))
        .or_else(|| stem.strip_suffix(".css"))
        .unwrap_or(stem);
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
            if tok.kind() == SyntaxKind::IDENT && (tok.text() == "as" || tok.text() == "with") {
                break;
            }

            // Variable: $name
            if kind == symbols::SymbolKind::Variable
                && tok.kind() == SyntaxKind::DOLLAR
                && let Some(next) = tokens.get(i + 1)
                && next.kind() == SyntaxKind::IDENT
                && next.text() == name
            {
                results.push(next.text_range());
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

fn is_scss_or_css(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("scss") || ext.eq_ignore_ascii_case("css"))
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

pub(crate) fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    uri.to_file_path().map(std::borrow::Cow::into_owned)
}

fn path_to_uri(path: &Path) -> Uri {
    Uri::from_file_path(path).unwrap_or_else(|| {
        // Percent-encode the path for URI safety (spaces, #, %, etc.)
        let path_str = path.to_string_lossy();
        let encoded: String = path_str
            .bytes()
            .flat_map(|b| match b {
                b'/' | b'.' | b'-' | b'_' | b'~' | b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' => {
                    vec![b as char]
                }
                _ => format!("%{b:02X}").chars().collect(),
            })
            .collect();
        let encoded = encoded.strip_prefix('/').unwrap_or(&encoded);
        let s = format!("file:///{encoded}");
        match s.parse() {
            Ok(uri) => uri,
            Err(e) => {
                tracing::error!(?path, %e, "failed to construct URI from path");
                // Last-resort: return a syntactically valid but unresolvable URI
                "file:///invalid-path".parse().unwrap()
            }
        }
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
    fn default_namespace_sass_extension() {
        assert_eq!(
            default_namespace("base.sass"),
            Namespace::Named("base".into())
        );
    }

    #[test]
    fn default_namespace_css_extension() {
        assert_eq!(
            default_namespace("vendor.css"),
            Namespace::Named("vendor".into())
        );
    }

    #[test]
    fn extract_namespace_as_alias() {
        let (green, _) = sass_parser::parse_scss("@use \"colors\" as c;");
        let root = SyntaxNode::new_root(green);
        let imports = imports::collect_imports(&root);
        assert_eq!(imports.len(), 1);
        let ns = extract_namespace(&root, &imports[0]);
        assert_eq!(ns, Namespace::Named("c".into()));
    }

    #[test]
    fn extract_namespace_as_star() {
        let (green, _) = sass_parser::parse_scss("@use \"colors\" as *;");
        let root = SyntaxNode::new_root(green);
        let imports = imports::collect_imports(&root);
        let ns = extract_namespace(&root, &imports[0]);
        assert_eq!(ns, Namespace::Star);
    }

    #[test]
    fn extract_namespace_default() {
        let (green, _) = sass_parser::parse_scss("@use \"sass:math\";");
        let root = SyntaxNode::new_root(green);
        let imports = imports::collect_imports(&root);
        let ns = extract_namespace(&root, &imports[0]);
        assert_eq!(ns, Namespace::Named("math".into()));
    }

    #[test]
    fn extract_namespace_default_path() {
        let (green, _) = sass_parser::parse_scss("@use \"src/utils\";");
        let root = SyntaxNode::new_root(green);
        let imports = imports::collect_imports(&root);
        let ns = extract_namespace(&root, &imports[0]);
        assert_eq!(ns, Namespace::Named("utils".into()));
    }

    fn make_info(source: &str) -> ModuleInfo {
        let (green, _) = sass_parser::parse_scss(source);
        let root = SyntaxNode::new_root(green.clone());
        let syms = symbols::collect_symbols(&root);
        let li = sass_parser::line_index::LineIndex::new(source);
        ModuleInfo {
            symbols: Arc::new(syms),
            green: Some(green),
            line_index: li,
            source_text: Some(source.to_owned()),
            has_legacy_import: false,
            has_unresolved_use: false,
        }
    }

    #[test]
    fn module_graph_local_resolution() {
        let graph = ModuleGraph::new(Arc::new(RuntimeConfig::default()));
        let uri: Uri = "file:///test.scss".parse().unwrap();
        graph
            .files
            .insert(uri.clone(), make_info("$color: red;\n@mixin btn { }"));

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
        let graph = ModuleGraph::new(Arc::new(RuntimeConfig::default()));
        let uri: Uri = "file:///main.scss".parse().unwrap();
        let dep_uri: Uri = "file:///colors.scss".parse().unwrap();

        // Index dependency
        graph
            .files
            .insert(dep_uri.clone(), make_info("$primary: blue;"));

        // Index main with manual edge
        graph.files.insert(uri.clone(), make_info("$local: red;"));
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
        let graph = ModuleGraph::new(Arc::new(RuntimeConfig::default()));
        let uri: Uri = "file:///main.scss".parse().unwrap();
        let dep_uri: Uri = "file:///colors.scss".parse().unwrap();

        graph.files.insert(
            dep_uri.clone(),
            make_info("$primary: blue;\n$secondary: green;"),
        );

        graph.files.insert(uri.clone(), make_info(""));
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
        let graph = ModuleGraph::new(Arc::new(RuntimeConfig::default()));
        let consumer_uri: Uri = "file:///consumer.scss".parse().unwrap();
        let mid_uri: Uri = "file:///mid.scss".parse().unwrap();
        let lib_uri: Uri = "file:///lib.scss".parse().unwrap();

        graph.files.insert(
            lib_uri.clone(),
            make_info("$primary: blue;\n$secondary: green;\n@mixin btn { }\n@function size() { @return 1; }"),
        );

        graph.files.insert(mid_uri.clone(), make_info(""));
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

        graph.files.insert(consumer_uri.clone(), make_info(""));
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
        let graph = ModuleGraph::new(Arc::new(RuntimeConfig::default()));
        let consumer_uri: Uri = "file:///consumer.scss".parse().unwrap();
        let top_uri: Uri = "file:///top.scss".parse().unwrap();
        let mid_uri: Uri = "file:///mid.scss".parse().unwrap();
        let lib_uri: Uri = "file:///lib.scss".parse().unwrap();

        graph.files.insert(
            lib_uri.clone(),
            make_info("$primary: blue;\n$secondary: green;\n@mixin btn { }"),
        );

        graph.files.insert(mid_uri.clone(), make_info(""));
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

        graph.files.insert(top_uri.clone(), make_info(""));
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

        graph.files.insert(consumer_uri.clone(), make_info(""));
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
        let (green, _) = sass_parser::parse_scss("@forward \"lib\" as btn-*;");
        let root = SyntaxNode::new_root(green);
        let imports = imports::collect_imports(&root);
        assert_eq!(imports.len(), 1);
        let ns = extract_namespace(&root, &imports[0]);
        assert_eq!(ns, Namespace::Star);
    }

    // ── Builtin module tests ────────────────────────────────────────

    fn setup_builtin_graph(module: &str, namespace: Namespace) -> (ModuleGraph, Uri) {
        let graph = ModuleGraph::new(Arc::new(RuntimeConfig::default()));
        let uri: Uri = "file:///main.scss".parse().unwrap();

        graph.files.insert(uri.clone(), make_info(""));

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
        let graph = ModuleGraph::new(Arc::new(RuntimeConfig::default()));
        let uri: Uri = "file:///main.scss".parse().unwrap();
        let globals_uri: Uri = "file:///globals.scss".parse().unwrap();

        graph.files.insert(
            globals_uri.clone(),
            make_info("$brand: red;\n@mixin container { }"),
        );

        graph.files.insert(uri.clone(), make_info("$local: blue;"));

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
        let graph = ModuleGraph::new(Arc::new(RuntimeConfig::default()));
        let uri: Uri = "file:///main.scss".parse().unwrap();
        let globals_uri: Uri = "file:///globals.scss".parse().unwrap();

        graph
            .files
            .insert(globals_uri.clone(), make_info("$brand: red;"));

        graph.files.insert(uri.clone(), make_info(""));

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
        let graph = ModuleGraph::new(Arc::new(RuntimeConfig::default()));
        let uri: Uri = "file:///test.scss".parse().unwrap();
        graph
            .files
            .insert(uri.clone(), make_info("$color: red;\n$primary: blue;"));

        assert!(graph.check_name_conflict(&uri, "primary", symbols::SymbolKind::Variable));
        assert!(!graph.check_name_conflict(&uri, "shade", symbols::SymbolKind::Variable));
    }

    #[test]
    fn check_name_conflict_different_kind_no_conflict() {
        let graph = ModuleGraph::new(Arc::new(RuntimeConfig::default()));
        let uri: Uri = "file:///test.scss".parse().unwrap();
        graph.files.insert(
            uri.clone(),
            make_info("$color: red;\n@function color() { @return red; }"),
        );

        // "color" exists as function, but we're checking for variable → no conflict
        assert!(!graph.check_name_conflict(&uri, "color", symbols::SymbolKind::Mixin));
    }

    #[test]
    fn find_forward_show_hide_variable() {
        let source = "@forward \"lib\" show $primary, $secondary;";
        let (green, _) = sass_parser::parse_scss(source);
        let root = SyntaxNode::new_root(green);

        let ranges = find_name_in_forward_clauses(&root, "primary", symbols::SymbolKind::Variable);
        assert_eq!(ranges.len(), 1);
        let text = &source[usize::from(ranges[0].start())..usize::from(ranges[0].end())];
        assert_eq!(text, "primary");
    }

    #[test]
    fn find_forward_show_hide_mixin() {
        let source = "@forward \"lib\" show btn, card;";
        let (green, _) = sass_parser::parse_scss(source);
        let root = SyntaxNode::new_root(green);

        let ranges = find_name_in_forward_clauses(&root, "btn", symbols::SymbolKind::Mixin);
        assert_eq!(ranges.len(), 1);
        let text = &source[usize::from(ranges[0].start())..usize::from(ranges[0].end())];
        assert_eq!(text, "btn");
    }

    #[test]
    fn find_forward_hide_clause() {
        let source = "@forward \"lib\" hide $internal;";
        let (green, _) = sass_parser::parse_scss(source);
        let root = SyntaxNode::new_root(green);

        let ranges = find_name_in_forward_clauses(&root, "internal", symbols::SymbolKind::Variable);
        assert_eq!(ranges.len(), 1);
    }

    #[test]
    fn find_forward_clause_no_match() {
        let source = "@forward \"lib\" show $primary;";
        let (green, _) = sass_parser::parse_scss(source);
        let root = SyntaxNode::new_root(green);

        let ranges =
            find_name_in_forward_clauses(&root, "secondary", symbols::SymbolKind::Variable);
        assert!(ranges.is_empty());
    }

    #[test]
    fn forward_reaches_direct() {
        let graph = ModuleGraph::new(Arc::new(RuntimeConfig::default()));
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
        let graph = ModuleGraph::new(Arc::new(RuntimeConfig::default()));
        let main_uri: Uri = "file:///main.scss".parse().unwrap();
        let lib_uri: Uri = "file:///lib.scss".parse().unwrap();

        graph
            .files
            .insert(lib_uri.clone(), make_info("$primary: blue;"));

        // main: @forward "lib" show $primary;
        let main_source = "@forward \"lib\" show $primary;";
        graph.files.insert(main_uri.clone(), make_info(main_source));
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
        let text = &main_source[usize::from(refs[0].1.start())..usize::from(refs[0].1.end())];
        assert_eq!(text, "primary");
    }

    #[test]
    fn path_traversal_blocked_outside_roots() {
        let graph = ModuleGraph::new(Arc::new(RuntimeConfig::default()));
        // Use a known directory as allowed root
        let tmp = std::env::temp_dir();
        graph.set_allowed_roots(vec![tmp.clone()]);

        // Path inside the root is allowed
        assert!(graph.is_path_allowed(&tmp.join("some_file.scss")));

        // Path outside the root is blocked
        assert!(!graph.is_path_allowed(Path::new("/etc/passwd")));
    }

    #[test]
    fn path_traversal_permissive_without_roots() {
        let graph = ModuleGraph::new(Arc::new(RuntimeConfig::default()));
        // No roots configured → permissive fallback
        assert!(graph.is_path_allowed(Path::new("/any/path")));
    }

    #[test]
    fn path_traversal_dotdot_resolved() {
        let graph = ModuleGraph::new(Arc::new(RuntimeConfig::default()));
        let tmp = std::env::temp_dir();
        graph.set_allowed_roots(vec![tmp.clone()]);

        // A path that uses `..` to escape should be blocked
        let escaped = tmp.join("subdir").join("..").join("..").join("etc");
        assert!(!graph.is_path_allowed(&escaped));
    }

    // ── Circular import handling ──────────────────────────────────────

    #[test]
    fn circular_use_does_not_loop() {
        let graph = ModuleGraph::new(Arc::new(RuntimeConfig::default()));
        let a: Uri = "file:///a.scss".parse().unwrap();
        let b: Uri = "file:///b.scss".parse().unwrap();

        graph.files.insert(a.clone(), make_info("$from_a: red;"));
        graph.files.insert(b.clone(), make_info("$from_b: blue;"));

        // a → b (star import)
        graph.edges.insert(
            a.clone(),
            vec![ImportEdge {
                target: b.clone(),
                namespace: Namespace::Star,
                kind: ImportKind::Use,
                visibility: ForwardVisibility::default(),
            }],
        );
        // b → a (star import — circular)
        graph.edges.insert(
            b.clone(),
            vec![ImportEdge {
                target: a.clone(),
                namespace: Namespace::Star,
                kind: ImportKind::Use,
                visibility: ForwardVisibility::default(),
            }],
        );

        // Should terminate, not infinite loop
        let r = graph.resolve_unqualified(&a, "from_b", symbols::SymbolKind::Variable);
        assert!(r.is_some());
        assert_eq!(r.unwrap().1.name, "from_b");

        let r = graph.resolve_unqualified(&b, "from_a", symbols::SymbolKind::Variable);
        assert!(r.is_some());
        assert_eq!(r.unwrap().1.name, "from_a");
    }

    #[test]
    fn circular_forward_does_not_loop() {
        let graph = ModuleGraph::new(Arc::new(RuntimeConfig::default()));
        let a: Uri = "file:///a.scss".parse().unwrap();
        let b: Uri = "file:///b.scss".parse().unwrap();
        let c: Uri = "file:///c.scss".parse().unwrap();

        graph.files.insert(a.clone(), make_info("$a_var: 1;"));
        graph.files.insert(b.clone(), make_info(""));
        graph.files.insert(c.clone(), make_info("$c_var: 3;"));

        // a → b (forward), b → c (forward), c → a (forward — cycle)
        graph.edges.insert(
            a.clone(),
            vec![ImportEdge {
                target: b.clone(),
                namespace: Namespace::Star,
                kind: ImportKind::Forward,
                visibility: ForwardVisibility::default(),
            }],
        );
        graph.edges.insert(
            b.clone(),
            vec![ImportEdge {
                target: c.clone(),
                namespace: Namespace::Star,
                kind: ImportKind::Forward,
                visibility: ForwardVisibility::default(),
            }],
        );
        graph.edges.insert(
            c.clone(),
            vec![ImportEdge {
                target: a.clone(),
                namespace: Namespace::Star,
                kind: ImportKind::Forward,
                visibility: ForwardVisibility::default(),
            }],
        );

        // Consumer uses a as *
        let consumer: Uri = "file:///consumer.scss".parse().unwrap();
        graph.files.insert(consumer.clone(), make_info(""));
        graph.edges.insert(
            consumer.clone(),
            vec![ImportEdge {
                target: a.clone(),
                namespace: Namespace::Star,
                kind: ImportKind::Use,
                visibility: ForwardVisibility::default(),
            }],
        );

        // Should resolve through forward chain without infinite loop
        let r = graph.resolve_unqualified(&consumer, "c_var", symbols::SymbolKind::Variable);
        assert!(r.is_some());
        assert_eq!(r.unwrap().1.name, "c_var");

        // visible_symbols should also terminate
        let visible = graph.visible_symbols(&consumer);
        let names: Vec<&str> = visible.iter().map(|(_, _, s)| s.name.as_str()).collect();
        assert!(names.contains(&"a_var"));
        assert!(names.contains(&"c_var"));
    }

    #[test]
    fn self_import_does_not_loop() {
        let graph = ModuleGraph::new(Arc::new(RuntimeConfig::default()));
        let a: Uri = "file:///a.scss".parse().unwrap();

        graph.files.insert(a.clone(), make_info("$x: 1;"));

        // a imports itself
        graph.edges.insert(
            a.clone(),
            vec![ImportEdge {
                target: a.clone(),
                namespace: Namespace::Star,
                kind: ImportKind::Use,
                visibility: ForwardVisibility::default(),
            }],
        );

        let r = graph.resolve_unqualified(&a, "x", symbols::SymbolKind::Variable);
        assert!(r.is_some());
    }
}
