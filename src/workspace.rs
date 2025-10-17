use crate::file_system_storage::FileSystemStorage;
use crate::managers::{IdentityManager, LogManager, PlanManager, TimesheetManager};
use crate::models::Config;
use crate::plugins::PluginManager;
use crate::storage::Storage;
use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use std::sync::{Arc, Mutex};

/// Workspace provides coordinated access to faff functionality
pub struct Workspace {
    storage: Arc<dyn Storage>,
    config: Config,
    plan_manager: Arc<PlanManager>,
    log_manager: Arc<LogManager>,
    timesheet_manager: Arc<TimesheetManager>,
    identity_manager: Arc<IdentityManager>,
    plugin_manager: Arc<Mutex<PluginManager>>,
}

impl Workspace {
    /// Create a new Workspace with the default FileSystemStorage
    ///
    /// This searches for a .faff directory starting from the current working directory.
    pub fn new() -> anyhow::Result<Self> {
        let storage = Arc::new(FileSystemStorage::new()?);
        Self::with_storage(storage)
    }

    /// Create a new Workspace with a custom storage implementation
    pub fn with_storage(storage: Arc<dyn Storage>) -> anyhow::Result<Self> {
        // Load config from storage
        let config_path = storage.config_file();
        let config_str = storage.read_string(&config_path)?;
        let config = Config::from_toml(&config_str)
            .map_err(|e| anyhow::anyhow!("Failed to parse config: {}", e))?;

        // Create managers
        let plan_manager = Arc::new(PlanManager::new(storage.clone()));
        let log_manager = Arc::new(LogManager::new(storage.clone(), config.timezone));
        let timesheet_manager = Arc::new(TimesheetManager::new(storage.clone()));
        let identity_manager = Arc::new(IdentityManager::new(storage.clone()));
        let plugin_manager = Arc::new(Mutex::new(PluginManager::new(storage.clone(), config.clone())));

        Ok(Self {
            storage,
            config,
            plan_manager,
            log_manager,
            timesheet_manager,
            identity_manager,
            plugin_manager,
        })
    }

    /// Get the current time in the configured timezone
    pub fn now(&self) -> DateTime<Tz> {
        Utc::now().with_timezone(&self.config.timezone)
    }

    /// Get today's date in the configured timezone
    pub fn today(&self) -> NaiveDate {
        self.now().date_naive()
    }

    /// Get the configured timezone
    pub fn timezone(&self) -> Tz {
        self.config.timezone
    }

    /// Get a reference to the config
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get a reference to the storage
    pub fn storage(&self) -> &Arc<dyn Storage> {
        &self.storage
    }

    /// Get the PlanManager
    pub fn plans(&self) -> Arc<PlanManager> {
        self.plan_manager.clone()
    }

    /// Get the LogManager
    pub fn logs(&self) -> Arc<LogManager> {
        self.log_manager.clone()
    }

    /// Get the TimesheetManager
    pub fn timesheets(&self) -> Arc<TimesheetManager> {
        self.timesheet_manager.clone()
    }

    /// Get the IdentityManager
    pub fn identities(&self) -> Arc<IdentityManager> {
        self.identity_manager.clone()
    }

    /// Get the PluginManager
    pub fn plugins(&self) -> Arc<Mutex<PluginManager>> {
        self.plugin_manager.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Storage;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Mutex;

    struct MockStorage {
        files: Mutex<HashMap<PathBuf, String>>,
    }

    impl MockStorage {
        fn new() -> Self {
            let mut files = HashMap::new();
            files.insert(
                PathBuf::from("/config.toml"),
                r#"timezone = "America/New_York""#.to_string(),
            );
            Self {
                files: Mutex::new(files),
            }
        }
    }

    impl Storage for MockStorage {
        fn root_dir(&self) -> PathBuf {
            PathBuf::from("/")
        }
        fn log_dir(&self) -> PathBuf {
            PathBuf::from("/logs")
        }
        fn plan_dir(&self) -> PathBuf {
            PathBuf::from("/plans")
        }
        fn identity_dir(&self) -> PathBuf {
            PathBuf::from("/identities")
        }
        fn timesheet_dir(&self) -> PathBuf {
            PathBuf::from("/timesheets")
        }
        fn config_file(&self) -> PathBuf {
            PathBuf::from("/config.toml")
        }
        fn read_string(&self, path: &PathBuf) -> anyhow::Result<String> {
            self.files
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("File not found"))
        }
        fn read_bytes(&self, _path: &PathBuf) -> anyhow::Result<Vec<u8>> {
            unimplemented!()
        }
        fn write_string(&self, _path: &PathBuf, _data: &str) -> anyhow::Result<()> {
            unimplemented!()
        }
        fn write_bytes(&self, _path: &PathBuf, _data: &[u8]) -> anyhow::Result<()> {
            unimplemented!()
        }
        fn exists(&self, path: &PathBuf) -> bool {
            self.files.lock().unwrap().contains_key(path)
        }
        fn create_dir_all(&self, _path: &PathBuf) -> anyhow::Result<()> {
            Ok(())
        }
        fn list_files(&self, _dir: &PathBuf, _pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
            Ok(vec![])
        }
    }

    #[test]
    fn test_workspace_creation() {
        let storage = Arc::new(MockStorage::new());
        let ws = Workspace::with_storage(storage).unwrap();
        assert_eq!(ws.timezone().name(), "America/New_York");
    }

    #[test]
    fn test_workspace_now_and_today() {
        let storage = Arc::new(MockStorage::new());
        let ws = Workspace::with_storage(storage).unwrap();

        let now = ws.now();
        let today = ws.today();

        // Just verify they execute without error and have the right timezone
        assert_eq!(now.timezone().name(), "America/New_York");
        assert_eq!(now.date_naive(), today);
    }
}
