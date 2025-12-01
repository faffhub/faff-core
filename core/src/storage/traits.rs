use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDate;
use std::path::{Path, PathBuf};

use crate::models::Config;

#[cfg(not(target_arch = "wasm32"))]
use super::events::EventStreamHandle;

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
///
/// ## Why two versions of this trait?
///
/// We define Storage twice with `cfg` attributes - once for native targets and once for WASM.
/// This is necessary because of different thread safety requirements:
///
/// **Native (Python/CLI):**
/// - Multi-threaded runtime (tokio with multiple threads)
/// - Storage might be accessed from different threads
/// - Requires `Send` (can be moved between threads) and `Sync` (can be shared between threads)
/// - `#[async_trait]` generates futures that are `Send`
///
/// **WASM (browser/Obsidian plugin):**
/// - Single-threaded (JavaScript has no threads)
/// - `JsValue` (wrapper for JavaScript objects) contains raw pointers to JS heap, which aren't Send/Sync
/// - No need for thread safety since there's only one thread
/// - `#[async_trait(?Send)]` generates futures without Send bound
///
/// The trait methods are identical - only the thread safety bounds differ.

// Non-WASM version: requires Send + Sync for thread safety
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
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

    async fn read_bytes(&self, path: &Path) -> Result<Vec<u8>>;
    async fn read_string(&self, path: &Path) -> Result<String>;
    async fn write_bytes(&self, path: &Path, data: &[u8]) -> Result<()>;
    async fn write_string(&self, path: &Path, data: &str) -> Result<()>;
    async fn delete(&self, path: &Path) -> Result<()>;
    fn exists(&self, path: &Path) -> bool; // Keep this sync - it's just a check
    async fn create_dir_all(&self, path: &Path) -> Result<()>;
    async fn list_files(&self, dir: &Path, pattern: &str) -> Result<Vec<PathBuf>>;

    // ============================================================================
    // Default implementations: Repository structure
    // ============================================================================

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

    fn remotes_dir(&self) -> PathBuf {
        self.base_dir().join("remotes")
    }

    fn plugin_state_dir(&self) -> PathBuf {
        self.base_dir().join("plugin_state")
    }

    fn plugins_dir(&self) -> PathBuf {
        self.base_dir().join("plugins")
    }

    fn intents_dir(&self) -> PathBuf {
        self.base_dir().join("intents")
    }

    fn config_file(&self) -> PathBuf {
        self.base_dir().join("config.toml")
    }

    // ============================================================================
    // Default implementations: Path construction helpers
    // ============================================================================

    fn log_file_path(&self, date: NaiveDate) -> PathBuf {
        self.log_dir().join(format!("{date}.toml"))
    }

    fn plan_file_path(&self, date: NaiveDate) -> PathBuf {
        self.plan_dir().join(format!("{date}.json"))
    }

    fn timesheet_file_path(&self, audience_id: &str, date: NaiveDate) -> PathBuf {
        self.timesheet_dir()
            .join(format!("{audience_id}.{date}.json"))
    }

    fn timesheet_meta_file_path(&self, audience_id: &str, date: NaiveDate) -> PathBuf {
        self.timesheet_dir()
            .join(format!("{audience_id}.{date}.meta.json"))
    }

    fn remote_file_path(&self, remote_id: &str) -> PathBuf {
        self.remotes_dir().join(format!("{remote_id}.toml"))
    }

    // ============================================================================
    // Optional: Event stream support
    // ============================================================================

    /// Returns true if this storage implementation supports event streams.
    ///
    /// Storage implementations that can detect file changes (like FileSystemStorage)
    /// can override this to return true and implement spawn_event_stream().
    fn supports_events(&self) -> bool {
        false
    }

    /// Spawn an event stream for watching file changes.
    ///
    /// Returns None if this storage implementation doesn't support events.
    /// Check supports_events() before calling this method.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use faff_core::storage::{Storage, FileSystemStorage};
    /// use std::path::PathBuf;
    ///
    /// let storage = FileSystemStorage::new(PathBuf::from("/path/to/.faff"));
    /// if storage.supports_events() {
    ///     if let Some(handle) = storage.spawn_event_stream() {
    ///         let mut rx = handle.subscribe();
    ///         // Process events...
    ///     }
    /// }
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    fn spawn_event_stream(&self) -> Option<EventStreamHandle> {
        None
    }

    // ============================================================================
    // Default implementation: Repository initialization
    // ============================================================================

    /// Initialize this storage as a new faff repository
    ///
    /// Creates the standard directory structure and writes a default config.
    /// This is storage-agnostic - works for any Storage implementation.
    async fn init(&self) -> Result<()> {
        // Create standard directory structure
        self.create_dir_all(&self.log_dir()).await?;
        self.create_dir_all(&self.plan_dir()).await?;
        self.create_dir_all(&self.timesheet_dir()).await?;
        self.create_dir_all(&self.remotes_dir()).await?;
        self.create_dir_all(&self.identity_dir()).await?;
        self.create_dir_all(&self.intents_dir()).await?;
        self.create_dir_all(&self.plugins_dir()).await?;
        self.create_dir_all(&self.plugin_state_dir()).await?;

        // Create default config with system timezone
        let config = Config::with_system_timezone();
        let config_toml = config
            .to_toml()
            .map_err(|e| anyhow::anyhow!("Failed to serialize default config: {}", e))?;
        self.write_string(&self.config_file(), &config_toml).await?;

        Ok(())
    }
}

// ============================================================================
// WASM version of Storage trait - identical methods, no Send/Sync bounds
// ============================================================================

/// Storage trait for WASM targets (see main trait docs above for full explanation)
///
/// This is the same trait as above, but without Send/Sync bounds because:
/// - WASM is single-threaded (no threads in JavaScript)
/// - JsValue (JS object wrapper) uses raw pointers that can't be Send/Sync
/// - We use `#[async_trait(?Send)]` so futures don't need to be Send either
#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
pub trait Storage {
    fn base_dir(&self) -> PathBuf;

    async fn read_bytes(&self, path: &Path) -> Result<Vec<u8>>;
    async fn read_string(&self, path: &Path) -> Result<String>;
    async fn write_bytes(&self, path: &Path, data: &[u8]) -> Result<()>;
    async fn write_string(&self, path: &Path, data: &str) -> Result<()>;
    async fn delete(&self, path: &Path) -> Result<()>;
    fn exists(&self, path: &Path) -> bool;
    async fn create_dir_all(&self, path: &Path) -> Result<()>;
    async fn list_files(&self, dir: &Path, pattern: &str) -> Result<Vec<PathBuf>>;

    // Default implementations - identical to native version
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

    fn remotes_dir(&self) -> PathBuf {
        self.base_dir().join("remotes")
    }

    fn plugin_state_dir(&self) -> PathBuf {
        self.base_dir().join("plugin_state")
    }

    fn plugins_dir(&self) -> PathBuf {
        self.base_dir().join("plugins")
    }

    fn intents_dir(&self) -> PathBuf {
        self.base_dir().join("intents")
    }

    fn config_file(&self) -> PathBuf {
        self.base_dir().join("config.toml")
    }

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

    fn remote_file_path(&self, remote_id: &str) -> PathBuf {
        self.remotes_dir().join(format!("{}.toml", remote_id))
    }

    // Event support not available on WASM
    fn supports_events(&self) -> bool {
        false
    }

    async fn init(&self) -> Result<()> {
        self.create_dir_all(&self.log_dir()).await?;
        self.create_dir_all(&self.plan_dir()).await?;
        self.create_dir_all(&self.timesheet_dir()).await?;
        self.create_dir_all(&self.remotes_dir()).await?;
        self.create_dir_all(&self.identity_dir()).await?;
        self.create_dir_all(&self.intents_dir()).await?;
        self.create_dir_all(&self.plugins_dir()).await?;
        self.create_dir_all(&self.plugin_state_dir()).await?;

        let config = Config::with_system_timezone();
        let config_toml = config
            .to_toml()
            .map_err(|e| anyhow::anyhow!("Failed to serialize default config: {}", e))?;
        self.write_string(&self.config_file(), &config_toml).await?;

        Ok(())
    }
}
