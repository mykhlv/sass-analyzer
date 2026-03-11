use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use serde::Deserialize;

use sass_parser::resolver::ModuleResolver;

/// Default maximum file size the server will parse (2 MB).
const DEFAULT_MAX_FILE_SIZE: usize = 2_000_000;
/// Default debounce delay in milliseconds before re-parsing.
const DEFAULT_DEBOUNCE_MS: u64 = 50;
/// Default maximum number of green trees cached in the module graph.
const DEFAULT_MAX_CACHED_TREES: usize = 200;
/// Default maximum number of source texts cached in the module graph.
const DEFAULT_MAX_CACHED_SOURCES: usize = 200;

/// Settings sent via `initializationOptions` or `workspace/didChangeConfiguration`.
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SassAnalyzerConfig {
    pub load_paths: Vec<String>,
    pub import_aliases: HashMap<String, AliasTarget>,
    pub prepend_imports: Vec<String>,
    pub max_file_size: Option<usize>,
    pub debounce_ms: Option<u64>,
    pub max_cached_trees: Option<usize>,
    pub max_cached_sources: Option<usize>,
}

/// A single alias target: either one path or an array of paths.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum AliasTarget {
    Single(String),
    Multiple(Vec<String>),
}

impl AliasTarget {
    pub fn paths(&self) -> &[String] {
        match self {
            Self::Single(s) => std::slice::from_ref(s),
            Self::Multiple(v) => v,
        }
    }
}

/// Runtime-tunable parameters shared across the server via atomics.
///
/// Updated from `initializationOptions` at startup and from
/// `workspace/didChangeConfiguration` at runtime without restart.
pub struct RuntimeConfig {
    max_file_size: AtomicUsize,
    debounce_ms: AtomicU64,
    max_cached_trees: AtomicUsize,
    max_cached_sources: AtomicUsize,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_file_size: AtomicUsize::new(DEFAULT_MAX_FILE_SIZE),
            debounce_ms: AtomicU64::new(DEFAULT_DEBOUNCE_MS),
            max_cached_trees: AtomicUsize::new(DEFAULT_MAX_CACHED_TREES),
            max_cached_sources: AtomicUsize::new(DEFAULT_MAX_CACHED_SOURCES),
        }
    }
}

impl RuntimeConfig {
    pub fn max_file_size(&self) -> usize {
        self.max_file_size.load(Ordering::Relaxed)
    }

    pub fn debounce_ms(&self) -> u64 {
        self.debounce_ms.load(Ordering::Relaxed)
    }

    pub fn max_cached_trees(&self) -> usize {
        self.max_cached_trees.load(Ordering::Relaxed)
    }

    pub fn max_cached_sources(&self) -> usize {
        self.max_cached_sources.load(Ordering::Relaxed)
    }

    /// Apply values from a deserialized config, using defaults for unset fields.
    pub fn apply(&self, config: &SassAnalyzerConfig) {
        self.max_file_size.store(
            config.max_file_size.unwrap_or(DEFAULT_MAX_FILE_SIZE),
            Ordering::Relaxed,
        );
        self.debounce_ms.store(
            config.debounce_ms.unwrap_or(DEFAULT_DEBOUNCE_MS),
            Ordering::Relaxed,
        );
        self.max_cached_trees.store(
            config.max_cached_trees.unwrap_or(DEFAULT_MAX_CACHED_TREES),
            Ordering::Relaxed,
        );
        self.max_cached_sources.store(
            config.max_cached_sources.unwrap_or(DEFAULT_MAX_CACHED_SOURCES),
            Ordering::Relaxed,
        );
    }
}

/// Build a configured `ModuleResolver` from LSP initialization options.
pub fn build_resolver(
    config: &SassAnalyzerConfig,
    workspace_root: Option<&Path>,
) -> ModuleResolver {
    let mut resolver = ModuleResolver::new();

    if let Some(root) = workspace_root {
        for lp in &config.load_paths {
            resolver.add_load_path(root.join(lp));
        }

        for (prefix, target) in &config.import_aliases {
            let abs_targets: Vec<PathBuf> = target.paths().iter().map(|p| root.join(p)).collect();
            resolver.add_import_alias(prefix.clone(), abs_targets);
        }
    }

    // node_modules resolution is always enabled (zero-config).
    resolver.enable_node_modules();

    resolver
}
