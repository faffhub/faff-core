//! Shared test utilities for faff-core
//!
//! This module provides common testing infrastructure used across multiple test modules,
//! including a MockStorage implementation that can be used in place of real file system storage.

#[cfg(test)]
pub mod mock_storage {
    use anyhow::Result;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::RwLock;

    use crate::storage::Storage;

    /// In-memory storage implementation for testing
    ///
    /// Uses RwLock for better concurrent test performance compared to Mutex.
    /// Provides a simple HashMap-based storage that implements the Storage trait.
    pub struct MockStorage {
        files: RwLock<HashMap<PathBuf, String>>,
        root_dir: PathBuf,
        log_dir: PathBuf,
        plan_dir: PathBuf,
        identity_dir: PathBuf,
        timesheet_dir: PathBuf,
        config_file: PathBuf,
    }

    impl MockStorage {
        /// Create a new MockStorage with default paths
        pub fn new() -> Self {
            Self {
                files: RwLock::new(HashMap::new()),
                root_dir: PathBuf::from("/faff"),
                log_dir: PathBuf::from("/faff/logs"),
                plan_dir: PathBuf::from("/faff/plans"),
                identity_dir: PathBuf::from("/faff/keys"),
                timesheet_dir: PathBuf::from("/faff/timesheets"),
                config_file: PathBuf::from("/faff/config.toml"),
            }
        }

        /// Add a file to storage (useful for setting up test fixtures)
        pub fn add_file(&self, path: PathBuf, content: String) {
            let mut files = self.files.write().unwrap();
            files.insert(path, content);
        }

        /// Get all files currently in storage (useful for test assertions)
        pub fn get_all_files(&self) -> HashMap<PathBuf, String> {
            let files = self.files.read().unwrap();
            files.clone()
        }

        /// Clear all files from storage
        pub fn clear(&self) {
            let mut files = self.files.write().unwrap();
            files.clear();
        }
    }

    impl Default for MockStorage {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Storage for MockStorage {
        fn root_dir(&self) -> PathBuf {
            self.root_dir.clone()
        }

        fn log_dir(&self) -> PathBuf {
            self.log_dir.clone()
        }

        fn plan_dir(&self) -> PathBuf {
            self.plan_dir.clone()
        }

        fn identity_dir(&self) -> PathBuf {
            self.identity_dir.clone()
        }

        fn timesheet_dir(&self) -> PathBuf {
            self.timesheet_dir.clone()
        }

        fn config_file(&self) -> PathBuf {
            self.config_file.clone()
        }

        fn read_bytes(&self, path: &Path) -> Result<Vec<u8>> {
            let files = self.files.read().unwrap();
            files
                .get(path)
                .map(|s| s.as_bytes().to_vec())
                .ok_or_else(|| anyhow::anyhow!("File not found: {:?}", path))
        }

        fn read_string(&self, path: &Path) -> Result<String> {
            let files = self.files.read().unwrap();
            files
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("File not found: {:?}", path))
        }

        fn write_bytes(&self, path: &Path, data: &[u8]) -> Result<()> {
            let content = String::from_utf8(data.to_vec())?;
            let mut files = self.files.write().unwrap();
            files.insert(path.to_path_buf(), content);
            Ok(())
        }

        fn write_string(&self, path: &Path, data: &str) -> Result<()> {
            let mut files = self.files.write().unwrap();
            files.insert(path.to_path_buf(), data.to_string());
            Ok(())
        }

        fn delete(&self, path: &Path) -> Result<()> {
            let mut files = self.files.write().unwrap();
            if files.remove(path).is_some() {
                Ok(())
            } else {
                anyhow::bail!("File not found: {:?}", path)
            }
        }

        fn exists(&self, path: &Path) -> bool {
            let files = self.files.read().unwrap();
            files.contains_key(path)
        }

        fn create_dir_all(&self, _path: &Path) -> Result<()> {
            // No-op for in-memory storage
            Ok(())
        }

        fn list_files(&self, dir: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
            let files = self.files.read().unwrap();

            // Use glob::Pattern for proper glob matching
            let glob_pattern = glob::Pattern::new(pattern)?;

            Ok(files
                .keys()
                .filter(|path| {
                    // Check if the file is in the specified directory
                    path.parent() == Some(dir)
                        && path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| glob_pattern.matches(n))
                            .unwrap_or(false)
                })
                .cloned()
                .collect())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_mock_storage_read_write() {
            let storage = MockStorage::new();
            let path = PathBuf::from("/test/file.txt");
            let content = "Hello, world!";

            storage.write_string(&path, content).unwrap();
            assert!(storage.exists(&path));

            let retrieved = storage.read_string(&path).unwrap();
            assert_eq!(retrieved, content);
        }

        #[test]
        fn test_mock_storage_list_files() {
            let storage = MockStorage::new();
            let dir = PathBuf::from("/test");

            storage.add_file(dir.join("file1.txt"), "content1".to_string());
            storage.add_file(dir.join("file2.txt"), "content2".to_string());
            storage.add_file(dir.join("file3.log"), "content3".to_string());

            let txt_files = storage.list_files(&dir, "*.txt").unwrap();
            assert_eq!(txt_files.len(), 2);

            let all_files = storage.list_files(&dir, "*").unwrap();
            assert_eq!(all_files.len(), 3);
        }

        #[test]
        fn test_mock_storage_clear() {
            let storage = MockStorage::new();
            let path = PathBuf::from("/test/file.txt");

            storage.write_string(&path, "content").unwrap();
            assert!(storage.exists(&path));

            storage.clear();
            assert!(!storage.exists(&path));
        }

        #[test]
        fn test_mock_storage_bytes() {
            let storage = MockStorage::new();
            let path = PathBuf::from("/test/data.txt");
            let data = b"Hello, world!";

            storage.write_bytes(&path, data).unwrap();
            let retrieved = storage.read_bytes(&path).unwrap();

            assert_eq!(retrieved, data);
            assert!(storage.exists(&path));
        }
    }
}
