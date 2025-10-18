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
        let plugin_manager = Arc::new(Mutex::new(PluginManager::new(
            storage.clone(),
            config.clone(),
        )));

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
    use crate::test_utils::mock_storage::MockStorage;
    use std::path::PathBuf;

    fn create_test_workspace() -> Workspace {
        let storage = Arc::new(MockStorage::new());

        // Add a config file to storage
        storage.add_file(
            PathBuf::from("/faff/config.toml"),
            r#"timezone = "America/New_York""#.to_string(),
        );

        Workspace::with_storage(storage).unwrap()
    }

    #[test]
    fn test_workspace_creation() {
        let ws = create_test_workspace();
        assert_eq!(ws.timezone().name(), "America/New_York");
    }

    #[test]
    fn test_workspace_now_and_today() {
        let ws = create_test_workspace();

        let now = ws.now();
        let today = ws.today();

        // Just verify they execute without error and have the right timezone
        assert_eq!(now.timezone().name(), "America/New_York");
        assert_eq!(now.date_naive(), today);
    }

    #[test]
    fn test_workspace_config_access() {
        let ws = create_test_workspace();
        let config = ws.config();

        assert_eq!(config.timezone.name(), "America/New_York");
    }

    #[test]
    fn test_workspace_storage_access() {
        let ws = create_test_workspace();
        let storage = ws.storage();

        assert_eq!(storage.root_dir(), PathBuf::from("/faff"));
    }

    #[test]
    fn test_workspace_manager_access() {
        let ws = create_test_workspace();

        // Verify all managers are accessible
        let _plans = ws.plans();
        let _logs = ws.logs();
        let _timesheets = ws.timesheets();
        let _identities = ws.identities();
        let _plugins = ws.plugins();

        // Just verify they don't panic when accessed
        assert!(true);
    }

    #[test]
    fn test_workspace_with_utc_timezone() {
        let storage = Arc::new(MockStorage::new());
        storage.add_file(
            PathBuf::from("/faff/config.toml"),
            r#"timezone = "UTC""#.to_string(),
        );

        let ws = Workspace::with_storage(storage).unwrap();
        assert_eq!(ws.timezone().name(), "UTC");
    }

    #[test]
    fn test_workspace_with_london_timezone() {
        let storage = Arc::new(MockStorage::new());
        storage.add_file(
            PathBuf::from("/faff/config.toml"),
            r#"timezone = "Europe/London""#.to_string(),
        );

        let ws = Workspace::with_storage(storage).unwrap();
        assert_eq!(ws.timezone().name(), "Europe/London");
    }

    #[test]
    fn test_workspace_fails_without_config() {
        let storage = Arc::new(MockStorage::new());
        // Don't add a config file

        let result = Workspace::with_storage(storage);
        assert!(result.is_err());
    }

    #[test]
    fn test_workspace_fails_with_invalid_config() {
        let storage = Arc::new(MockStorage::new());
        storage.add_file(
            PathBuf::from("/faff/config.toml"),
            r#"invalid toml content {"#.to_string(),
        );

        let result = Workspace::with_storage(storage);
        assert!(result.is_err());
    }

    #[test]
    fn test_workspace_managers_share_storage() {
        let ws = create_test_workspace();

        // All managers should share the same storage instance
        let plans = ws.plans();
        let logs = ws.logs();

        // Verify they use the same root directory (indirectly testing shared storage)
        assert_eq!(ws.storage().root_dir(), PathBuf::from("/faff"));

        // Managers should be functional
        assert!(plans.get_plans(ws.today()).is_ok());
        assert!(logs.log_exists(ws.today()) == false);
    }
}
