use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::storage::Storage;

/// FileSystemStorage implements the Storage trait by finding and using
/// a .faff directory in the filesystem.
///
/// It searches upward from the current working directory (or a specified directory)
/// to find a .faff directory, then provides access to the standard faff directory structure.
#[derive(Clone)]
pub struct FileSystemStorage {
    faff_root: PathBuf,
}

impl FileSystemStorage {
    /// Create a new FileSystemStorage by searching for .faff directory
    ///
    /// Starts from the current working directory and searches upward.
    pub fn new() -> Result<Self> {
        let cwd = std::env::current_dir().context("Failed to get current working directory")?;
        Self::from_path(cwd)
    }

    /// Create a new FileSystemStorage by searching for .faff directory starting from a specific path
    pub fn from_path(start_path: PathBuf) -> Result<Self> {
        let faff_root = Self::find_faff_root(&start_path)?;
        Ok(Self { faff_root })
    }

    /// Search upward from a given path for a `.faff` directory
    ///
    /// Returns the directory containing `.faff`, not the `.faff` directory itself.
    fn find_faff_root(start_path: &PathBuf) -> Result<PathBuf> {
        let mut current = start_path.clone();

        loop {
            let faff_dir = current.join(".faff");
            if faff_dir.is_dir() {
                return Ok(current);
            }

            // Try to go up one directory
            match current.parent() {
                Some(parent) => current = parent.to_path_buf(),
                None => {
                    anyhow::bail!(
                        "No .faff directory found from start path: {}",
                        start_path.display()
                    );
                }
            }
        }
    }
}

impl Storage for FileSystemStorage {
    fn root_dir(&self) -> PathBuf {
        self.faff_root.clone()
    }

    fn log_dir(&self) -> PathBuf {
        self.faff_root.join(".faff").join("logs")
    }

    fn plan_dir(&self) -> PathBuf {
        self.faff_root.join(".faff").join("plans")
    }

    fn identity_dir(&self) -> PathBuf {
        self.faff_root.join(".faff").join("keys")
    }

    fn timesheet_dir(&self) -> PathBuf {
        self.faff_root.join(".faff").join("timesheets")
    }

    fn config_file(&self) -> PathBuf {
        self.faff_root.join(".faff").join("config.toml")
    }

    fn read_bytes(&self, path: &PathBuf) -> Result<Vec<u8>> {
        std::fs::read(path).context(format!("Failed to read file: {}", path.display()))
    }

    fn read_string(&self, path: &PathBuf) -> Result<String> {
        std::fs::read_to_string(path).context(format!("Failed to read file: {}", path.display()))
    }

    fn write_bytes(&self, path: &PathBuf, data: &[u8]) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .context(format!("Failed to create directory: {}", parent.display()))?;
        }
        std::fs::write(path, data).context(format!("Failed to write file: {}", path.display()))
    }

    fn write_string(&self, path: &PathBuf, data: &str) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .context(format!("Failed to create directory: {}", parent.display()))?;
        }
        std::fs::write(path, data).context(format!("Failed to write file: {}", path.display()))
    }

    fn exists(&self, path: &PathBuf) -> bool {
        path.exists()
    }

    fn create_dir_all(&self, path: &PathBuf) -> Result<()> {
        std::fs::create_dir_all(path)
            .context(format!("Failed to create directory: {}", path.display()))
    }

    fn list_files(&self, dir: &PathBuf, pattern: &str) -> Result<Vec<PathBuf>> {
        if !dir.exists() {
            return Ok(vec![]);
        }

        let glob_pattern = dir.join(pattern);
        let pattern_str = glob_pattern
            .to_str()
            .context("Invalid path pattern")?;

        let paths: Result<Vec<PathBuf>, _> = glob::glob(pattern_str)
            .context("Failed to parse glob pattern")?
            .collect();

        paths.context("Failed to list files")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_faff_root() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::from_path(temp.path().to_path_buf()).unwrap();
        assert_eq!(storage.root_dir(), temp.path());
    }

    #[test]
    fn test_find_faff_root_in_subdirectory() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let subdir = temp.path().join("subdir").join("nested");
        fs::create_dir_all(&subdir).unwrap();

        let storage = FileSystemStorage::from_path(subdir).unwrap();
        assert_eq!(storage.root_dir(), temp.path());
    }

    #[test]
    fn test_find_faff_root_fails_when_not_found() {
        let temp = TempDir::new().unwrap();
        let result = FileSystemStorage::from_path(temp.path().to_path_buf());
        assert!(result.is_err());
    }

    #[test]
    fn test_storage_trait_methods() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::from_path(temp.path().to_path_buf()).unwrap();

        assert_eq!(storage.log_dir(), temp.path().join(".faff").join("logs"));
        assert_eq!(storage.plan_dir(), temp.path().join(".faff").join("plans"));
        assert_eq!(
            storage.config_file(),
            temp.path().join(".faff").join("config.toml")
        );
    }

    #[test]
    fn test_read_write_string() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::from_path(temp.path().to_path_buf()).unwrap();

        let test_file = storage.log_dir().join("test.txt");
        storage.write_string(&test_file, "hello world").unwrap();

        let contents = storage.read_string(&test_file).unwrap();
        assert_eq!(contents, "hello world");
    }
}
