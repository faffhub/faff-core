use crate::models::Config;
use crate::storage::Storage;
use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use std::sync::Arc;

/// Workspace provides coordinated access to faff functionality
pub struct Workspace {
    storage: Arc<dyn Storage>,
    config: Config,
}

impl Workspace {
    pub fn new(storage: Arc<dyn Storage>) -> anyhow::Result<Self> {
        // Load config from storage
        let config_path = storage.config_file();
        let config_str = storage.read_string(&config_path)?;
        let config = Config::from_toml(&config_str)
            .map_err(|e| anyhow::anyhow!("Failed to parse config: {}", e))?;

        Ok(Self { storage, config })
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
        let ws = Workspace::new(storage).unwrap();
        assert_eq!(ws.timezone().name(), "America/New_York");
    }

    #[test]
    fn test_workspace_now_and_today() {
        let storage = Arc::new(MockStorage::new());
        let ws = Workspace::new(storage).unwrap();

        let now = ws.now();
        let today = ws.today();

        // Just verify they execute without error and have the right timezone
        assert_eq!(now.timezone().name(), "America/New_York");
        assert_eq!(now.date_naive(), today);
    }
}
