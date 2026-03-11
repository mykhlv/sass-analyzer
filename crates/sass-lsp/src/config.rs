use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use sass_parser::resolver::ModuleResolver;

/// Settings sent via `initializationOptions` from the VS Code extension.
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SassAnalyzerConfig {
    pub load_paths: Vec<String>,
    pub import_aliases: HashMap<String, AliasTarget>,
    pub prepend_imports: Vec<String>,
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
