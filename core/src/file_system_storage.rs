use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::storage::Storage;

/// FileSystemStorage implements the Storage trait by finding and using
/// a .faff directory in the filesystem.
///
/// It searches upward from the current working directory (or a specified directory)
/// to find a .faff directory, then provides access to the standard faff directory structure.
#[derive(Clone)]
pub struct FileSystemStorage {
    faff_root: PathBuf,
    faff_dir: PathBuf,
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
        let faff_dir = faff_root.join(".faff");
        Ok(Self {
            faff_root,
            faff_dir,
        })
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
        self.faff_dir.join("logs")
    }

    fn plan_dir(&self) -> PathBuf {
        self.faff_dir.join("plans")
    }

    fn identity_dir(&self) -> PathBuf {
        self.faff_dir.join("keys")
    }

    fn timesheet_dir(&self) -> PathBuf {
        self.faff_dir.join("timesheets")
    }

    fn config_file(&self) -> PathBuf {
        self.faff_dir.join("config.toml")
    }

    fn read_bytes(&self, path: &Path) -> Result<Vec<u8>> {
        std::fs::read(path).with_context(|| format!("Failed to read file: {}", path.display()))
    }

    fn read_string(&self, path: &Path) -> Result<String> {
        std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))
    }

    fn write_bytes(&self, path: &Path, data: &[u8]) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }
        std::fs::write(path, data)
            .with_context(|| format!("Failed to write file: {}", path.display()))
    }

    fn write_string(&self, path: &Path, data: &str) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }
        std::fs::write(path, data)
            .with_context(|| format!("Failed to write file: {}", path.display()))
    }

    fn delete(&self, path: &Path) -> Result<()> {
        std::fs::remove_file(path)
            .with_context(|| format!("Failed to delete file: {}", path.display()))
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn create_dir_all(&self, path: &Path) -> Result<()> {
        std::fs::create_dir_all(path)
            .with_context(|| format!("Failed to create directory: {}", path.display()))
    }

    fn list_files(&self, dir: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
        if !dir.exists() {
            return Ok(vec![]);
        }

        let glob_pattern = dir.join(pattern);
        let pattern_str = glob_pattern.to_str().context("Invalid path pattern")?;

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

    #[test]
    fn test_read_write_bytes() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::from_path(temp.path().to_path_buf()).unwrap();

        let test_file = storage.log_dir().join("test.bin");
        let data = vec![0u8, 1, 2, 3, 4, 5];

        storage.write_bytes(&test_file, &data).unwrap();
        let retrieved = storage.read_bytes(&test_file).unwrap();

        assert_eq!(retrieved, data);
    }

    #[test]
    fn test_exists() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::from_path(temp.path().to_path_buf()).unwrap();

        let test_file = storage.log_dir().join("test.txt");
        assert!(!storage.exists(&test_file));

        storage.write_string(&test_file, "content").unwrap();
        assert!(storage.exists(&test_file));
    }

    #[test]
    fn test_create_dir_all() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::from_path(temp.path().to_path_buf()).unwrap();

        let nested_dir = storage.log_dir().join("nested").join("deep").join("dir");
        assert!(!nested_dir.exists());

        storage.create_dir_all(&nested_dir).unwrap();
        assert!(nested_dir.exists());
    }

    #[test]
    fn test_list_files() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::from_path(temp.path().to_path_buf()).unwrap();

        // Create some test files
        let log_dir = storage.log_dir();
        storage.create_dir_all(&log_dir).unwrap();

        storage
            .write_string(&log_dir.join("2025-03-15.toml"), "log1")
            .unwrap();
        storage
            .write_string(&log_dir.join("2025-03-16.toml"), "log2")
            .unwrap();
        storage
            .write_string(&log_dir.join("readme.txt"), "readme")
            .unwrap();

        let toml_files = storage.list_files(&log_dir, "*.toml").unwrap();
        assert_eq!(toml_files.len(), 2);

        let all_files = storage.list_files(&log_dir, "*").unwrap();
        assert_eq!(all_files.len(), 3);
    }

    #[test]
    fn test_list_files_empty_directory() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::from_path(temp.path().to_path_buf()).unwrap();

        let log_dir = storage.log_dir();
        storage.create_dir_all(&log_dir).unwrap();

        let files = storage.list_files(&log_dir, "*.toml").unwrap();
        assert_eq!(files.len(), 0);
    }

    #[test]
    fn test_list_files_nonexistent_directory() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::from_path(temp.path().to_path_buf()).unwrap();

        let nonexistent = temp.path().join("does_not_exist");
        let files = storage.list_files(&nonexistent, "*.toml").unwrap();
        assert_eq!(files.len(), 0);
    }

    #[test]
    fn test_write_creates_parent_directories() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::from_path(temp.path().to_path_buf()).unwrap();

        let nested_file = storage
            .log_dir()
            .join("nested")
            .join("deep")
            .join("file.txt");
        assert!(!nested_file.parent().unwrap().exists());

        storage.write_string(&nested_file, "content").unwrap();
        assert!(nested_file.exists());
        assert_eq!(storage.read_string(&nested_file).unwrap(), "content");
    }

    #[test]
    fn test_read_nonexistent_file() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::from_path(temp.path().to_path_buf()).unwrap();

        let nonexistent = storage.log_dir().join("nonexistent.txt");
        let result = storage.read_string(&nonexistent);

        assert!(result.is_err());
    }

    #[test]
    fn test_directory_paths() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::from_path(temp.path().to_path_buf()).unwrap();

        // All directories should be under .faff
        assert_eq!(storage.root_dir(), temp.path());
        assert_eq!(storage.log_dir(), temp.path().join(".faff").join("logs"));
        assert_eq!(storage.plan_dir(), temp.path().join(".faff").join("plans"));
        assert_eq!(
            storage.identity_dir(),
            temp.path().join(".faff").join("keys")
        );
        assert_eq!(
            storage.timesheet_dir(),
            temp.path().join(".faff").join("timesheets")
        );
        assert_eq!(
            storage.config_file(),
            temp.path().join(".faff").join("config.toml")
        );
    }

    #[test]
    fn test_clone() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::from_path(temp.path().to_path_buf()).unwrap();
        let cloned = storage.clone();

        assert_eq!(storage.root_dir(), cloned.root_dir());
        assert_eq!(storage.log_dir(), cloned.log_dir());
    }
}
