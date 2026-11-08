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
#[allow(missing_docs)]
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
    /// The canonical name of this module (e.g. `"math"`, `"color"`).
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
    /// Create a resolver backed by the real filesystem.
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
    /// Create a resolver backed by a custom virtual filesystem.
    pub fn with_vfs(vfs: V) -> Self {
        Self {
            vfs,
            load_paths: Vec::new(),
            import_aliases: Vec::new(),
            node_modules_enabled: false,
        }
    }

    /// Add a directory to the load path search list.
    pub fn add_load_path(&mut self, path: impl Into<PathBuf>) {
        self.load_paths.push(path.into());
    }

    /// The configured load paths.
    pub fn load_paths(&self) -> &[PathBuf] {
        &self.load_paths
    }

    /// The configured import aliases (prefix → target directories).
    pub fn import_aliases(&self) -> &[(String, Vec<PathBuf>)] {
        &self.import_aliases
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

    /// Enable `node_modules` resolution (walks up the directory tree).
    pub fn enable_node_modules(&mut self) {
        self.node_modules_enabled = true;
    }

    /// Resolve a module specifier relative to a base file.
    ///
    /// `spec` is the raw string from `@use "spec"` (without quotes).
    /// `base` is the absolute path of the file containing the `@use`.
    pub fn resolve(&self, spec: &str, base: &Path) -> Result<ResolvedModule, ResolveError> {
        self.resolve_in_context(spec, base, false)
    }

    /// Resolve the URL of an `@import`, which also finds import-only files
    /// (`name.import.scss`) and prefers them over the plain file, as Dart Sass does.
    pub fn resolve_for_import(
        &self,
        spec: &str,
        base: &Path,
    ) -> Result<ResolvedModule, ResolveError> {
        self.resolve_in_context(spec, base, true)
    }

    fn resolve_in_context(
        &self,
        spec: &str,
        base: &Path,
        for_import: bool,
    ) -> Result<ResolvedModule, ResolveError> {
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

        // The URL names a path the way a URL does: `%20` is a space.
        let decoded = percent_decode(spec);

        // 3. Import aliases
        if let Some(path) = self.resolve_alias(&decoded, base, for_import) {
            return Ok(ResolvedModule::File(path));
        }

        // 4. Resolve relative to the base file's directory
        let base_dir = base.parent().unwrap_or(Path::new("."));
        if let Some(path) = self.resolve_in_dir(base_dir, &decoded, for_import) {
            return Ok(ResolvedModule::File(path));
        }

        // 5. Try each load path
        for load_path in &self.load_paths {
            if let Some(path) = self.resolve_in_dir(load_path, &decoded, for_import) {
                return Ok(ResolvedModule::File(path));
            }
        }

        // 6. Walk up node_modules
        if self.node_modules_enabled
            && let Some(path) = self.resolve_node_modules(&decoded, base, for_import)
        {
            return Ok(ResolvedModule::File(path));
        }

        Err(ResolveError::NotFound(spec.to_owned()))
    }

    /// Try resolving `spec` within `dir` in Dart Sass's candidate order: with an
    /// explicit extension, that file; otherwise the file with each extension, `.scss`
    /// and `.sass` before `.css`; then the directory's `index` file. In an `@import`,
    /// each step tries the import-only file (`name.import.scss`) first. Where Dart
    /// Sass reports several candidates as ambiguous, the first is picked, a
    /// non-partial `.scss` file first, so the import does not look missing.
    fn resolve_in_dir(&self, dir: &Path, spec: &str, for_import: bool) -> Option<PathBuf> {
        let path = dir.join(spec);
        let name = path.file_name()?.to_str()?;

        if let Some((stem, ext)) = name.rsplit_once('.')
            && ["scss", "sass", "css"]
                .iter()
                .any(|known| ext.eq_ignore_ascii_case(known))
        {
            if for_import
                && let Some(found) =
                    self.existing_file(&path.with_file_name(format!("{stem}.import.{ext}")))
            {
                return Some(found);
            }
            return self.existing_file(&path);
        }

        if for_import
            && let Some(found) =
                self.with_extensions(&path.with_file_name(format!("{name}.import")))
        {
            return Some(found);
        }
        if let Some(found) = self.with_extensions(&path) {
            return Some(found);
        }
        if for_import && let Some(found) = self.with_extensions(&path.join("index.import")) {
            return Some(found);
        }
        self.with_extensions(&path.join("index"))
    }

    /// `path` itself or its partial (`_name`).
    fn existing_file(&self, path: &Path) -> Option<PathBuf> {
        let name = path.file_name()?.to_str()?;
        [path.to_path_buf(), path.with_file_name(format!("_{name}"))]
            .into_iter()
            .find(|candidate| self.vfs.file_exists(candidate))
    }

    /// `path` with a stylesheet extension, `.scss` before `.sass`, then with `.css`.
    fn with_extensions(&self, path: &Path) -> Option<PathBuf> {
        let name = path.file_name()?.to_str()?;
        ["scss", "sass", "css"]
            .iter()
            .find_map(|ext| self.existing_file(&path.with_file_name(format!("{name}.{ext}"))))
    }

    fn resolve_alias(&self, spec: &str, base: &Path, for_import: bool) -> Option<PathBuf> {
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
                return self.resolve_in_dir(&targets[0], rest, for_import);
            }

            // Multiple targets: pick the one closest to the importing file.
            let base_dir = base.parent().unwrap_or(Path::new("."));
            let best = targets
                .iter()
                .max_by_key(|t| common_prefix_len(t, base_dir));
            if let Some(target) = best
                && let Some(path) = self.resolve_in_dir(target, rest, for_import)
            {
                return Some(path);
            }
            // Fallback: try all targets in order.
            for target in targets {
                if let Some(path) = self.resolve_in_dir(target, rest, for_import) {
                    return Some(path);
                }
            }
        }
        None
    }

    fn resolve_node_modules(&self, spec: &str, base: &Path, for_import: bool) -> Option<PathBuf> {
        let mut dir = base.parent()?;
        loop {
            let nm = dir.join("node_modules");
            if let Some(path) = self.resolve_in_dir(&nm, spec, for_import) {
                return Some(path);
            }
            dir = dir.parent()?;
        }
    }
}

/// Decode `%XX` escapes. An invalid escape, or a result that is not UTF-8, leaves
/// the text as written, as Dart Sass reads `100%` as a file name.
fn percent_decode(spec: &str) -> std::borrow::Cow<'_, str> {
    if !spec.contains('%') {
        return std::borrow::Cow::Borrowed(spec);
    }
    let bytes = spec.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| char::from(b).to_digit(16);
        if bytes[i] == b'%'
            && let (Some(hi), Some(lo)) = (
                bytes.get(i + 1).copied().and_then(hex),
                bytes.get(i + 2).copied().and_then(hex),
            )
        {
            #[allow(clippy::cast_possible_truncation)]
            out.push((hi * 16 + lo) as u8);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_or(std::borrow::Cow::Borrowed(spec), std::borrow::Cow::Owned)
}

fn common_prefix_len(a: &Path, b: &Path) -> usize {
    a.components()
        .zip(b.components())
        .take_while(|(x, y)| x == y)
        .count()
}
