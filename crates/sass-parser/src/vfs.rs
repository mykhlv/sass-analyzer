use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

/// Abstract filesystem access for testability.
pub trait Vfs {
    fn file_exists(&self, path: &Path) -> bool;
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
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

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
