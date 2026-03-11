use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

/// Abstract filesystem access for testability.
pub trait Vfs {
    /// Check whether a file exists at `path`.
    fn file_exists(&self, path: &Path) -> bool;
    /// Read the contents of a file at `path`.
    fn read_file(&self, path: &Path) -> io::Result<String>;
}

/// Real filesystem implementation.
pub struct OsFileSystem;

impl Vfs for OsFileSystem {
    fn file_exists(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn read_file(&self, path: &Path) -> io::Result<String> {
        std::fs::read_to_string(path)
    }
}

/// In-memory filesystem for tests.
pub struct MemoryFs {
    files: HashMap<PathBuf, String>,
}

impl MemoryFs {
    /// Create an empty in-memory filesystem.
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    /// Insert a file into the virtual filesystem.
    pub fn add(&mut self, path: impl Into<PathBuf>, content: impl Into<String>) {
        self.files.insert(path.into(), content.into());
    }
}

impl Default for MemoryFs {
    fn default() -> Self {
        Self::new()
    }
}

impl Vfs for MemoryFs {
    fn file_exists(&self, path: &Path) -> bool {
        self.files.contains_key(path)
    }

    fn read_file(&self, path: &Path) -> io::Result<String> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("{}", path.display())))
    }
}
