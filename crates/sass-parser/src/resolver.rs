use std::fmt;
use std::path::{Path, PathBuf};

use crate::vfs::{OsFileSystem, Vfs};

/// A successfully resolved module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedModule {
    /// A file on disk (absolute path).
    File(PathBuf),
    /// A plain CSS import — no file lookup needed.
    Css(String),
    /// A built-in Sass module (`sass:math`, etc.).
    Builtin(BuiltinModule),
}

/// Built-in Sass modules available via `@use "sass:..."`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinModule {
    Math,
    Color,
    List,
    Map,
    Selector,
    SassString,
    Meta,
}

impl BuiltinModule {
    pub fn name(self) -> &'static str {
        match self {
            Self::Math => "math",
            Self::Color => "color",
            Self::List => "list",
            Self::Map => "map",
            Self::Selector => "selector",
            Self::SassString => "string",
            Self::Meta => "meta",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "math" => Some(Self::Math),
            "color" => Some(Self::Color),
            "list" => Some(Self::List),
            "map" => Some(Self::Map),
            "selector" => Some(Self::Selector),
            "string" => Some(Self::SassString),
            "meta" => Some(Self::Meta),
            _ => None,
        }
    }
}

/// Module resolution error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    /// No file found for the given import specifier.
    NotFound(String),
    /// Unknown built-in module (e.g. `sass:nope`).
    UnknownBuiltin(String),
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(spec) => write!(f, "module not found: {spec}"),
            Self::UnknownBuiltin(name) => write!(f, "unknown built-in module: sass:{name}"),
        }
    }
}

impl std::error::Error for ResolveError {}

/// Resolves `@use`/`@forward` specifiers to files or built-in modules.
pub struct ModuleResolver<V: Vfs = OsFileSystem> {
    vfs: V,
    load_paths: Vec<PathBuf>,
    /// `(prefix, absolute_targets)` sorted by prefix length descending (longest match first).
    import_aliases: Vec<(String, Vec<PathBuf>)>,
    node_modules_enabled: bool,
}

impl ModuleResolver {
    pub fn new() -> Self {
        Self {
            vfs: OsFileSystem,
            load_paths: Vec::new(),
            import_aliases: Vec::new(),
            node_modules_enabled: false,
        }
    }
}

impl Default for ModuleResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl<V: Vfs> ModuleResolver<V> {
    pub fn with_vfs(vfs: V) -> Self {
        Self {
            vfs,
            load_paths: Vec::new(),
            import_aliases: Vec::new(),
            node_modules_enabled: false,
        }
    }

    pub fn add_load_path(&mut self, path: impl Into<PathBuf>) {
        self.load_paths.push(path.into());
    }

    pub fn load_paths(&self) -> &[PathBuf] {
        &self.load_paths
    }

    /// Register an import alias. `targets` must be absolute paths.
    /// For monorepos, multiple targets can map to the same prefix;
    /// the resolver picks the target closest to the importing file.
    pub fn add_import_alias(&mut self, prefix: String, targets: Vec<PathBuf>) {
        self.import_aliases.push((prefix, targets));
        // Longest prefix first so `@sass/sub` matches before `@sass`.
        self.import_aliases
            .sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    }

    pub fn enable_node_modules(&mut self) {
        self.node_modules_enabled = true;
    }

    /// Resolve a module specifier relative to a base file.
    ///
    /// `spec` is the raw string from `@use "spec"` (without quotes).
    /// `base` is the absolute path of the file containing the `@use`.
    pub fn resolve(&self, spec: &str, base: &Path) -> Result<ResolvedModule, ResolveError> {
        // 1. Built-in modules
        if let Some(name) = spec.strip_prefix("sass:") {
            return BuiltinModule::from_name(name)
                .map(ResolvedModule::Builtin)
                .ok_or_else(|| ResolveError::UnknownBuiltin(name.to_owned()));
        }

        // 2. Plain CSS imports
        if Path::new(spec)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("css"))
        {
            return Ok(ResolvedModule::Css(spec.to_owned()));
        }

        // 3. Import aliases
        if let Some(path) = self.resolve_alias(spec, base) {
            return Ok(ResolvedModule::File(path));
        }

        // 4. Resolve relative to the base file's directory
        let base_dir = base.parent().unwrap_or(Path::new("."));
        if let Some(path) = self.resolve_in_dir(base_dir, spec) {
            return Ok(ResolvedModule::File(path));
        }

        // 5. Try each load path
        for load_path in &self.load_paths {
            if let Some(path) = self.resolve_in_dir(load_path, spec) {
                return Ok(ResolvedModule::File(path));
            }
        }

        // 6. Walk up node_modules
        if self.node_modules_enabled {
            if let Some(path) = self.resolve_node_modules(spec, base) {
                return Ok(ResolvedModule::File(path));
            }
        }

        Err(ResolveError::NotFound(spec.to_owned()))
    }

    /// Try resolving `spec` within `dir` using Sass candidate order.
    fn resolve_in_dir(&self, dir: &Path, spec: &str) -> Option<PathBuf> {
        // Split spec into directory part and file stem
        let spec_path = Path::new(spec);
        let (spec_dir, stem) = match spec_path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => {
                (Some(parent), spec_path.file_name()?.to_str()?)
            }
            _ => (None, spec),
        };

        let search_dir = match spec_dir {
            Some(rel) => dir.join(rel),
            None => dir.to_path_buf(),
        };

        // If spec already has .scss extension, try direct path + partial first.
        if Path::new(stem)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("scss"))
        {
            let direct = search_dir.join(stem);
            if self.vfs.file_exists(&direct) {
                return Some(direct);
            }
            let partial = search_dir.join(format!("_{stem}"));
            if self.vfs.file_exists(&partial) {
                return Some(partial);
            }
        }

        // Candidate order per Sass spec:
        // 1. {stem}.scss
        // 2. _{stem}.scss
        // 3. {stem}/index.scss
        // 4. {stem}/_index.scss
        let candidates = [
            search_dir.join(format!("{stem}.scss")),
            search_dir.join(format!("_{stem}.scss")),
            search_dir.join(stem).join("index.scss"),
            search_dir.join(stem).join("_index.scss"),
        ];

        candidates.into_iter().find(|c| self.vfs.file_exists(c))
    }

    fn resolve_alias(&self, spec: &str, base: &Path) -> Option<PathBuf> {
        for (prefix, targets) in &self.import_aliases {
            let Some(rest) = spec.strip_prefix(prefix.as_str()) else {
                continue;
            };
            // Guard: "@sass" must not match "@sass-utils"
            if !rest.is_empty() && !rest.starts_with('/') {
                continue;
            }
            let rest = rest.strip_prefix('/').unwrap_or(rest);

            if targets.len() == 1 {
                return self.resolve_in_dir(&targets[0], rest);
            }

            // Multiple targets: pick the one closest to the importing file.
            let base_dir = base.parent().unwrap_or(Path::new("."));
            let best = targets
                .iter()
                .max_by_key(|t| common_prefix_len(t, base_dir));
            if let Some(target) = best {
                if let Some(path) = self.resolve_in_dir(target, rest) {
                    return Some(path);
                }
            }
            // Fallback: try all targets in order.
            for target in targets {
                if let Some(path) = self.resolve_in_dir(target, rest) {
                    return Some(path);
                }
            }
        }
        None
    }

    fn resolve_node_modules(&self, spec: &str, base: &Path) -> Option<PathBuf> {
        let mut dir = base.parent()?;
        loop {
            let nm = dir.join("node_modules");
            if let Some(path) = self.resolve_in_dir(&nm, spec) {
                return Some(path);
            }
            dir = dir.parent()?;
        }
    }
}

fn common_prefix_len(a: &Path, b: &Path) -> usize {
    a.components()
        .zip(b.components())
        .take_while(|(x, y)| x == y)
        .count()
}
