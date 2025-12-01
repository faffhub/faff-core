use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs as async_fs;

use crate::storage::Storage;

/// FileSystemStorage implements the Storage trait using a faff directory.
///
/// The faff directory location is determined by (in order):
/// 1. $FAFF_DIR if FAFF_DIR environment variable is set (used directly)
/// 2. ~/.faff (default - hidden directory in home)
///
/// This provides a single workspace location with portability via environment variable.
#[derive(Clone)]
pub struct FileSystemStorage {
    faff_dir: PathBuf,
}

impl FileSystemStorage {
    /// Create a new FileSystemStorage using FAFF_DIR or ~/.faff
    ///
    /// Returns an error if the faff directory doesn't exist.
    pub fn new() -> Result<Self> {
        let faff_dir = Self::get_faff_dir()?;

        if !faff_dir.exists() {
            anyhow::bail!(
                "No faff directory found at {}. Run 'faff init' to create one.",
                faff_dir.display()
            );
        }

        Ok(Self { faff_dir })
    }

    /// Get the faff directory from FAFF_DIR env var or ~/.faff
    ///
    /// - If FAFF_DIR is set: returns $FAFF_DIR (used directly)
    /// - Otherwise: returns ~/.faff (hidden directory in home)
    fn get_faff_dir() -> Result<PathBuf> {
        if let Ok(faff_dir) = std::env::var("FAFF_DIR") {
            Ok(PathBuf::from(faff_dir))
        } else {
            let home = dirs::home_dir().context("Could not determine home directory")?;
            Ok(home.join(".faff"))
        }
    }

    /// Create a FileSystemStorage at a specific path (doesn't check if directory exists)
    ///
    /// This is useful for initialization where the directory doesn't exist yet.
    /// The path is used directly as the faff directory.
    pub fn at_path(path: PathBuf) -> Self {
        Self { faff_dir: path }
    }

    /// Initialize a new faff repository at the given path
    ///
    /// Creates a FileSystemStorage at the path and initializes it with
    /// the standard faff structure and default config.
    ///
    /// If no path is provided, uses FAFF_DIR or ~/.faff
    ///
    /// Returns an error if directory already has faff content
    pub async fn init_at(path: Option<PathBuf>) -> Result<Self> {
        let faff_dir = match path {
            Some(p) => p,
            None => Self::get_faff_dir()?,
        };

        // Check if faff content already exists (config file is a good indicator)
        let config_path = faff_dir.join("config.toml");
        if config_path.exists() {
            anyhow::bail!("Faff ledger already initialized at {}", faff_dir.display());
        }

        // Create the storage and initialize it
        let storage = Self::at_path(faff_dir);
        storage.init().await?;
        Ok(storage)
    }
}

#[async_trait]
impl Storage for FileSystemStorage {
    fn base_dir(&self) -> PathBuf {
        self.faff_dir.clone()
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

    // Event support - FileSystemStorage supports watching file changes
    fn supports_events(&self) -> bool {
        true
    }

    fn spawn_event_stream(&self) -> Option<super::events::EventStreamHandle> {
        Some(super::events::spawn_filesystem_watcher(
            self.base_dir(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_at_path() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::at_path(faff_dir.clone());
        assert_eq!(storage.base_dir(), faff_dir);
    }

    #[test]
    fn test_storage_trait_methods() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::at_path(faff_dir.clone());

        assert_eq!(storage.log_dir(), faff_dir.join("logs"));
        assert_eq!(storage.plan_dir(), faff_dir.join("plans"));
        assert_eq!(storage.config_file(), faff_dir.join("config.toml"));
    }

    #[tokio::test]
    async fn test_read_write_string() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::at_path(faff_dir.clone());

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

        let storage = FileSystemStorage::at_path(faff_dir.clone());

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

        let storage = FileSystemStorage::at_path(faff_dir.clone());

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

        let storage = FileSystemStorage::at_path(faff_dir.clone());

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

        let storage = FileSystemStorage::at_path(faff_dir.clone());

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

        let storage = FileSystemStorage::at_path(faff_dir.clone());

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

        let storage = FileSystemStorage::at_path(faff_dir.clone());

        let nonexistent = temp.path().join("does_not_exist");
        let files = storage.list_files(&nonexistent, "*.toml").await.unwrap();
        assert_eq!(files.len(), 0);
    }

    #[tokio::test]
    async fn test_write_creates_parent_directories() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::at_path(faff_dir.clone());

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

        let storage = FileSystemStorage::at_path(faff_dir.clone());

        let nonexistent = storage.log_dir().join("nonexistent.txt");
        let result = storage.read_string(&nonexistent).await;

        assert!(result.is_err());
    }

    #[test]
    fn test_directory_paths() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::at_path(faff_dir.clone());

        // All directories should be under the faff_dir
        assert_eq!(storage.base_dir(), faff_dir);
        assert_eq!(storage.log_dir(), faff_dir.join("logs"));
        assert_eq!(storage.plan_dir(), faff_dir.join("plans"));
        assert_eq!(storage.identity_dir(), faff_dir.join("keys"));
        assert_eq!(storage.timesheet_dir(), faff_dir.join("timesheets"));
        assert_eq!(storage.config_file(), faff_dir.join("config.toml"));
    }

    #[test]
    fn test_clone() {
        let temp = TempDir::new().unwrap();
        let faff_dir = temp.path().join(".faff");
        fs::create_dir(&faff_dir).unwrap();

        let storage = FileSystemStorage::at_path(faff_dir.clone());
        let cloned = storage.clone();

        assert_eq!(storage.base_dir(), cloned.base_dir());
        assert_eq!(storage.log_dir(), cloned.log_dir());
    }
}
