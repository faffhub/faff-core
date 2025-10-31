use anyhow::Result;
use chrono::NaiveDate;
use std::path::{Path, PathBuf};

use crate::models::Config;

/// Storage abstraction for Faffage data persistence.
///
/// This trait defines the interface for reading and writing Faffage data.
/// Implementations may use different backing stores:
/// - Real filesystem (CLI)
/// - Obsidian Vault API (plugin)
/// - In-memory (testing)
///
/// The Storage trait owns the faff repository structure (directory names, etc.)
/// Implementations only need to provide:
/// 1. The base directory where .faff content lives
/// 2. I/O primitives for their storage backend
pub trait Storage: Send + Sync {
    // ============================================================================
    // Required: Base directory - each implementation provides this
    // ============================================================================

    /// Returns the base directory for faff content
    ///
    /// For FileSystemStorage: /path/to/project/.faff
    /// For ObsidianStorage: vault/.faff
    /// etc.
    fn base_dir(&self) -> PathBuf;

    // ============================================================================
    // Required: I/O primitives - each implementation provides these
    // ============================================================================

    fn read_bytes(&self, path: &Path) -> Result<Vec<u8>>;
    fn read_string(&self, path: &Path) -> Result<String>;
    fn write_bytes(&self, path: &Path, data: &[u8]) -> Result<()>;
    fn write_string(&self, path: &Path, data: &str) -> Result<()>;
    fn delete(&self, path: &Path) -> Result<()>;
    fn exists(&self, path: &Path) -> bool;
    fn create_dir_all(&self, path: &Path) -> Result<()>;
    fn list_files(&self, dir: &Path, pattern: &str) -> Result<Vec<PathBuf>>;

    // ============================================================================
    // Default implementations: Repository structure
    // ============================================================================

    /// Returns the parent of base_dir (the project root)
    fn root_dir(&self) -> PathBuf {
        self.base_dir()
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| self.base_dir())
    }

    fn log_dir(&self) -> PathBuf {
        self.base_dir().join("logs")
    }

    fn plan_dir(&self) -> PathBuf {
        self.base_dir().join("plans")
    }

    fn identity_dir(&self) -> PathBuf {
        self.base_dir().join("keys")
    }

    fn timesheet_dir(&self) -> PathBuf {
        self.base_dir().join("timesheets")
    }

    fn config_file(&self) -> PathBuf {
        self.base_dir().join("config.toml")
    }

    // ============================================================================
    // Default implementations: Path construction helpers
    // ============================================================================

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

    // ============================================================================
    // Default implementation: Repository initialization
    // ============================================================================

    /// Initialize this storage as a new faff repository
    ///
    /// Creates the standard directory structure and writes a default config.
    /// This is storage-agnostic - works for any Storage implementation.
    fn init(&self) -> Result<()> {
        // Create standard directory structure
        self.create_dir_all(&self.log_dir())?;
        self.create_dir_all(&self.plan_dir())?;
        self.create_dir_all(&self.timesheet_dir())?;
        self.create_dir_all(&self.identity_dir())?;
        self.create_dir_all(&self.base_dir().join("intents"))?;
        self.create_dir_all(&self.base_dir().join("plugins"))?;
        self.create_dir_all(&self.base_dir().join("plugin_state"))?;

        // Create default config with system timezone
        let config = Config::with_system_timezone();
        let config_toml = config
            .to_toml()
            .map_err(|e| anyhow::anyhow!("Failed to serialize default config: {}", e))?;
        self.write_string(&self.config_file(), &config_toml)?;

        Ok(())
    }
}
