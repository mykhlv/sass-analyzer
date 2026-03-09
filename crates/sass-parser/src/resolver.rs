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
}

impl ModuleResolver {
    pub fn new() -> Self {
        Self {
            vfs: OsFileSystem,
            load_paths: Vec::new(),
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
        }
    }

    pub fn add_load_path(&mut self, path: impl Into<PathBuf>) {
        self.load_paths.push(path.into());
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

        // 3. Resolve relative to the base file's directory
        let base_dir = base.parent().unwrap_or(Path::new("."));
        if let Some(path) = self.resolve_in_dir(base_dir, spec) {
            return Ok(ResolvedModule::File(path));
        }

        // 4. Try each load path
        for load_path in &self.load_paths {
            if let Some(path) = self.resolve_in_dir(load_path, spec) {
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
}
