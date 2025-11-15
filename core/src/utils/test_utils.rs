//! Shared test utilities for faff-core
//!
//! This module provides common testing infrastructure used across multiple test modules,
//! including a MockStorage implementation that can be used in place of real file system storage.

#[cfg(test)]
pub mod mock_storage {
    use anyhow::Result;
    use async_trait::async_trait;
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
        base_dir: PathBuf,
    }

    impl MockStorage {
        /// Create a new MockStorage with default paths
        pub fn new() -> Self {
            Self {
                files: RwLock::new(HashMap::new()),
                base_dir: PathBuf::from("/faff"),
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

    #[async_trait]
    impl Storage for MockStorage {
        fn base_dir(&self) -> PathBuf {
            self.base_dir.clone()
        }

        async fn read_bytes(&self, path: &Path) -> Result<Vec<u8>> {
            let files = self.files.read().unwrap();
            files
                .get(path)
                .map(|s| s.as_bytes().to_vec())
                .ok_or_else(|| anyhow::anyhow!("File not found: {:?}", path))
        }

        async fn read_string(&self, path: &Path) -> Result<String> {
            let files = self.files.read().unwrap();
            files
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("File not found: {:?}", path))
        }

        async fn write_bytes(&self, path: &Path, data: &[u8]) -> Result<()> {
            let content = String::from_utf8(data.to_vec())?;
            let mut files = self.files.write().unwrap();
            files.insert(path.to_path_buf(), content);
            Ok(())
        }

        async fn write_string(&self, path: &Path, data: &str) -> Result<()> {
            let mut files = self.files.write().unwrap();
            files.insert(path.to_path_buf(), data.to_string());
            Ok(())
        }

        async fn delete(&self, path: &Path) -> Result<()> {
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

        async fn create_dir_all(&self, _path: &Path) -> Result<()> {
            // No-op for in-memory storage
            Ok(())
        }

        async fn list_files(&self, dir: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
            let files = self.files.read().unwrap();

            // Use glob::Pattern for proper glob matching
            let glob_pattern = glob::Pattern::new(pattern)?;

            // Collect both files and directories
            let mut results: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

            for path in files.keys() {
                // Check if this file is a descendant of dir
                if let Ok(rel_path) = path.strip_prefix(dir) {
                    let mut components = rel_path.components();
                    if let Some(first_component) = components.next() {
                        // Get the first component (either file name or subdirectory)
                        if let Some(name_str) = first_component.as_os_str().to_str() {
                            if glob_pattern.matches(name_str) {
                                // If there are more components, this is a subdirectory
                                if components.next().is_some() {
                                    // This is a subdirectory - add it
                                    results.insert(dir.join(first_component.as_os_str()));
                                } else {
                                    // This is a direct file - add it
                                    results.insert(path.clone());
                                }
                            }
                        }
                    }
                }
            }

            Ok(results.into_iter().collect())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn test_mock_storage_read_write() {
            let storage = MockStorage::new();
            let path = PathBuf::from("/test/file.txt");
            let content = "Hello, world!";

            storage.write_string(&path, content).await.unwrap();
            assert!(storage.exists(&path));

            let retrieved = storage.read_string(&path).await.unwrap();
            assert_eq!(retrieved, content);
        }

        #[tokio::test]
        async fn test_mock_storage_list_files() {
            let storage = MockStorage::new();
            let dir = PathBuf::from("/test");

            storage.add_file(dir.join("file1.txt"), "content1".to_string());
            storage.add_file(dir.join("file2.txt"), "content2".to_string());
            storage.add_file(dir.join("file3.log"), "content3".to_string());

            let txt_files = storage.list_files(&dir, "*.txt").await.unwrap();
            assert_eq!(txt_files.len(), 2);

            let all_files = storage.list_files(&dir, "*").await.unwrap();
            assert_eq!(all_files.len(), 3);
        }

        #[tokio::test]
        async fn test_mock_storage_clear() {
            let storage = MockStorage::new();
            let path = PathBuf::from("/test/file.txt");

            storage.write_string(&path, "content").await.unwrap();
            assert!(storage.exists(&path));

            storage.clear();
            assert!(!storage.exists(&path));
        }

        #[tokio::test]
        async fn test_mock_storage_bytes() {
            let storage = MockStorage::new();
            let path = PathBuf::from("/test/data.txt");
            let data = b"Hello, world!";

            storage.write_bytes(&path, data).await.unwrap();
            let retrieved = storage.read_bytes(&path).await.unwrap();

            assert_eq!(retrieved, data);
            assert!(storage.exists(&path));
        }
    }
}
