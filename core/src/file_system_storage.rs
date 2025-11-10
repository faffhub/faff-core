use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs as async_fs;

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

    /// Create a FileSystemStorage at a specific path (doesn't check if .faff exists)
    ///
    /// This is useful for initialization where .faff doesn't exist yet.
    pub fn at_path(path: PathBuf) -> Self {
        Self {
            faff_root: path.clone(),
            faff_dir: path.join(".faff"),
        }
    }

    /// Initialize a new faff repository at the given path
    ///
    /// Creates a FileSystemStorage at the path and initializes it with
    /// the standard faff structure and default config.
    ///
    /// Returns an error if:
    /// - .faff already exists at target (unless force=true)
    /// - Parent directory contains a .faff (unless force=true)
    pub async fn init_at(path: PathBuf, force: bool) -> Result<Self> {
        let faff_dir = path.join(".faff");

        // Check if .faff already exists at target
        if faff_dir.exists() && !force {
            anyhow::bail!(".faff directory already exists at {}", path.display());
        }

        // Check for parent .faff if not forcing
        if !force {
            match Self::find_faff_root(&path) {
                Ok(parent_root) => {
                    if parent_root != path {
                        anyhow::bail!(
                            "Cannot initialize inside existing faff repository at {}. Use --force to override.",
                            parent_root.display()
                        );
                    }
                }
                Err(_) => {
                    // No parent .faff found, this is good
                }
            }
        }

        // Create the storage and initialize it
        let storage = Self::at_path(path);
        storage.init().await?;
        Ok(storage)
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

#[async_trait]
impl Storage for FileSystemStorage {
    fn base_dir(&self) -> PathBuf {
        self.faff_dir.clone()
    }

    // Override root_dir() to use cached value instead of computing from parent
    fn root_dir(&self) -> PathBuf {
        self.faff_root.clone()
    }

    // Gets log_dir(), plan_dir(), etc. from trait defaults

    async fn read_bytes(&self, path: &Path) -> Result<Vec<u8>> {
        async_fs::read(path)
            .await
            .with_context(|| format!("Failed to read file: {}", path.display()))
    }

    async fn read_string(&self, path: &Path) -> Result<String> {
        async_fs::read_to_string(path)
            .await
            .with_context(|| format!("Failed to read file: {}", path.display()))
    }

    async fn write_bytes(&self, path: &Path, data: &[u8]) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            async_fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }
        async_fs::write(path, data)
            .await
            .with_context(|| format!("Failed to write file: {}", path.display()))
    }

    async fn write_string(&self, path: &Path, data: &str) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            async_fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }
        async_fs::write(path, data)
            .await
            .with_context(|| format!("Failed to write file: {}", path.display()))
    }

    async fn delete(&self, path: &Path) -> Result<()> {
        async_fs::remove_file(path)
            .await
            .with_context(|| format!("Failed to delete file: {}", path.display()))
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    async fn create_dir_all(&self, path: &Path) -> Result<()> {
        async_fs::create_dir_all(path)
            .await
            .with_context(|| format!("Failed to create directory: {}", path.display()))
    }

    async fn list_files(&self, dir: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
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

    #[tokio::test]
    async fn test_read_write_string() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::from_path(temp.path().to_path_buf()).unwrap();

        let test_file = storage.log_dir().join("test.txt");
        storage
            .write_string(&test_file, "hello world")
            .await
            .unwrap();

        let contents = storage.read_string(&test_file).await.unwrap();
        assert_eq!(contents, "hello world");
    }

    #[tokio::test]
    async fn test_read_write_bytes() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::from_path(temp.path().to_path_buf()).unwrap();

        let test_file = storage.log_dir().join("test.bin");
        let data = vec![0u8, 1, 2, 3, 4, 5];

        storage.write_bytes(&test_file, &data).await.unwrap();
        let retrieved = storage.read_bytes(&test_file).await.unwrap();

        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn test_exists() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::from_path(temp.path().to_path_buf()).unwrap();

        let test_file = storage.log_dir().join("test.txt");
        assert!(!storage.exists(&test_file));

        storage.write_string(&test_file, "content").await.unwrap();
        assert!(storage.exists(&test_file));
    }

    #[tokio::test]
    async fn test_create_dir_all() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::from_path(temp.path().to_path_buf()).unwrap();

        let nested_dir = storage.log_dir().join("nested").join("deep").join("dir");
        assert!(!nested_dir.exists());

        storage.create_dir_all(&nested_dir).await.unwrap();
        assert!(nested_dir.exists());
    }

    #[tokio::test]
    async fn test_list_files() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::from_path(temp.path().to_path_buf()).unwrap();

        // Create some test files
        let log_dir = storage.log_dir();
        storage.create_dir_all(&log_dir).await.unwrap();

        storage
            .write_string(&log_dir.join("2025-03-15.toml"), "log1")
            .await
            .unwrap();
        storage
            .write_string(&log_dir.join("2025-03-16.toml"), "log2")
            .await
            .unwrap();
        storage
            .write_string(&log_dir.join("readme.txt"), "readme")
            .await
            .unwrap();

        let toml_files = storage.list_files(&log_dir, "*.toml").await.unwrap();
        assert_eq!(toml_files.len(), 2);

        let all_files = storage.list_files(&log_dir, "*").await.unwrap();
        assert_eq!(all_files.len(), 3);
    }

    #[tokio::test]
    async fn test_list_files_empty_directory() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::from_path(temp.path().to_path_buf()).unwrap();

        let log_dir = storage.log_dir();
        storage.create_dir_all(&log_dir).await.unwrap();

        let files = storage.list_files(&log_dir, "*.toml").await.unwrap();
        assert_eq!(files.len(), 0);
    }

    #[tokio::test]
    async fn test_list_files_nonexistent_directory() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::from_path(temp.path().to_path_buf()).unwrap();

        let nonexistent = temp.path().join("does_not_exist");
        let files = storage.list_files(&nonexistent, "*.toml").await.unwrap();
        assert_eq!(files.len(), 0);
    }

    #[tokio::test]
    async fn test_write_creates_parent_directories() {
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

        storage.write_string(&nested_file, "content").await.unwrap();
        assert!(nested_file.exists());
        assert_eq!(storage.read_string(&nested_file).await.unwrap(), "content");
    }

    #[tokio::test]
    async fn test_read_nonexistent_file() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::from_path(temp.path().to_path_buf()).unwrap();

        let nonexistent = storage.log_dir().join("nonexistent.txt");
        let result = storage.read_string(&nonexistent).await;

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
