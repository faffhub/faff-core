use anyhow::Result;
use chrono::NaiveDate;
use std::path::PathBuf;

/// Storage abstraction for Faffage data persistence.
///
/// This trait defines the interface for reading and writing Faffage data.
/// Implementations may use different backing stores:
/// - Real filesystem (CLI)
/// - Obsidian Vault API (plugin)
/// - In-memory (testing)
pub trait Storage: Send + Sync {
    // Directory and file paths
    fn root_dir(&self) -> PathBuf;
    fn log_dir(&self) -> PathBuf;
    fn plan_dir(&self) -> PathBuf;
    fn identity_dir(&self) -> PathBuf;
    fn timesheet_dir(&self) -> PathBuf;
    fn config_file(&self) -> PathBuf;

    // File operations
    fn read_bytes(&self, path: &PathBuf) -> Result<Vec<u8>>;
    fn read_string(&self, path: &PathBuf) -> Result<String>;
    fn write_bytes(&self, path: &PathBuf, data: &[u8]) -> Result<()>;
    fn write_string(&self, path: &PathBuf, data: &str) -> Result<()>;
    fn delete(&self, path: &PathBuf) -> Result<()>;

    // Directory operations
    fn exists(&self, path: &PathBuf) -> bool;
    fn create_dir_all(&self, path: &PathBuf) -> Result<()>;
    fn list_files(&self, dir: &PathBuf, pattern: &str) -> Result<Vec<PathBuf>>;

    // Faffage-specific path construction helpers
    fn log_file_path(&self, date: NaiveDate) -> PathBuf {
        self.log_dir().join(format!("{}.toml", date))
    }

    fn plan_file_path(&self, date: NaiveDate) -> PathBuf {
        self.plan_dir().join(format!("{}.json", date))
    }

    fn timesheet_file_path(&self, audience_id: &str, date: NaiveDate) -> PathBuf {
        self.timesheet_dir()
            .join(format!("{}.{}.json", audience_id, date))
    }

    fn timesheet_meta_file_path(&self, audience_id: &str, date: NaiveDate) -> PathBuf {
        self.timesheet_dir()
            .join(format!("{}.{}.meta.json", audience_id, date))
    }
}
