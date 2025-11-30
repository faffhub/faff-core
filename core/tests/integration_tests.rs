//! Integration tests for faff-core
//!
//! These tests verify that multiple managers work together correctly,
//! ensuring proper coordination and data flow between components.

use async_trait::async_trait;
use chrono::NaiveDate;
use faff_core::managers::{IdentityManager, PlanManager, TimesheetManager};
use faff_core::models::intent::Intent;
use faff_core::models::log::Log;
use faff_core::models::plan::Plan;
use faff_core::models::session::Session;
use faff_core::models::timesheet::{Timesheet, TimesheetMeta};
use faff_core::storage::Storage;
use faff_core::workspace::Workspace;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Shared in-memory storage for integration tests
struct IntegrationStorage {
    files: RwLock<HashMap<PathBuf, String>>,
}

impl IntegrationStorage {
    fn new() -> Self {
        Self {
            files: RwLock::new(HashMap::new()),
        }
    }

    fn add_file(&self, path: PathBuf, content: String) {
        let mut files = self.files.write().unwrap();
        files.insert(path, content);
    }
}

#[async_trait]
impl Storage for IntegrationStorage {
    fn base_dir(&self) -> PathBuf {
        PathBuf::from("/faff")
    }

    async fn read_bytes(&self, path: &Path) -> anyhow::Result<Vec<u8>> {
        let files = self.files.read().unwrap();
        files
            .get(path)
            .map(|s| s.as_bytes().to_vec())
            .ok_or_else(|| anyhow::anyhow!("File not found: {:?}", path))
    }

    async fn read_string(&self, path: &Path) -> anyhow::Result<String> {
        let files = self.files.read().unwrap();
        files
            .get(path)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("File not found: {:?}", path))
    }

    async fn write_bytes(&self, path: &Path, data: &[u8]) -> anyhow::Result<()> {
        let content = String::from_utf8(data.to_vec())?;
        let mut files = self.files.write().unwrap();
        files.insert(path.to_path_buf(), content);
        Ok(())
    }

    async fn write_string(&self, path: &Path, data: &str) -> anyhow::Result<()> {
        let mut files = self.files.write().unwrap();
        files.insert(path.to_path_buf(), data.to_string());
        Ok(())
    }

    async fn delete(&self, path: &Path) -> anyhow::Result<()> {
        let mut files = self.files.write().unwrap();
        files
            .remove(path)
            .ok_or_else(|| anyhow::anyhow!("File not found: {:?}", path))?;
        Ok(())
    }

    fn exists(&self, path: &Path) -> bool {
        let files = self.files.read().unwrap();
        files.contains_key(path)
    }

    async fn create_dir_all(&self, _path: &Path) -> anyhow::Result<()> {
        Ok(())
    }

    async fn list_files(&self, dir: &Path, pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
        let files = self.files.read().unwrap();
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

// Helper function to create a Workspace with integration storage for tests
async fn create_test_workspace(storage: Arc<IntegrationStorage>) -> Arc<Workspace> {
    // Add a config file to storage
    storage.add_file(
        PathBuf::from("/faff/config.toml"),
        r#"timezone = "UTC""#.to_string(),
    );
    Workspace::with_storage(storage).await.unwrap()
}

#[tokio::test]
async fn test_plan_and_log_integration() {
    // Create shared storage
    let storage = Arc::new(IntegrationStorage::new());

    // Add a plan with trackers
    storage.add_file(
        PathBuf::from("/faff/plans/local.20250315.toml"),
        r#"
source = "local"
valid_from = "2025-03-15"
roles = ["engineer"]
objectives = ["development"]
actions = ["coding"]
subjects = ["api"]

[trackers]
"PROJ-123" = "Implement user auth"
"PROJ-456" = "Add API endpoints"

[[intents]]
alias = "auth-work"
role = "engineer"
objective = "development"
action = "coding"
subject = "api"
trackers = ["PROJ-123"]
"#
        .to_string(),
    );

    // Create workspace
    let ws = create_test_workspace(storage.clone()).await;

    let date = NaiveDate::from_ymd_opt(2025, 3, 20).unwrap();

    // Load plan
    let plans = ws.plans().get_plans(date).await.unwrap();
    assert_eq!(plans.len(), 1);
    assert!(plans.contains_key("local"));

    // Get trackers from plan
    let trackers = ws.plans().get_trackers(date).await.unwrap();
    assert_eq!(trackers.len(), 2);
    assert_eq!(
        trackers.get("local:PROJ-123"),
        Some(&"Implement user auth".to_string())
    );

    // Create a log using intent from plan
    let intents = ws.plans().get_intents(date).await.unwrap();
    assert_eq!(intents.len(), 1);

    let intent = &intents[0];
    assert_eq!(intent.alias.as_ref().unwrap(), "auth-work");

    // Create session and log
    let start_time = chrono::Utc::now().with_timezone(&chrono_tz::UTC);
    let session = Session::new(intent.clone(), start_time, None, None);
    let log = Log::new(date, chrono_tz::UTC, vec![session]);

    // Write log
    ws.logs().write_log(&log, &trackers).await.unwrap();

    // Read log back
    let retrieved_log = ws.logs().get_log(date).await.unwrap();
    assert_eq!(retrieved_log.timeline.len(), 1);
    assert_eq!(
        retrieved_log.timeline[0].intent.alias.as_ref().unwrap(),
        "auth-work"
    );
}

#[tokio::test]
async fn test_log_and_timesheet_integration() {
    let storage = Arc::new(IntegrationStorage::new());
    let ws = create_test_workspace(storage.clone()).await;
    let timesheet_manager = TimesheetManager::new(storage.clone());

    let date = NaiveDate::from_ymd_opt(2025, 3, 20).unwrap();

    // Create a log with sessions
    let intent = Intent::new(
        Some("work".to_string()),
        Some("engineer".to_string()),
        Some("development".to_string()),
        Some("coding".to_string()),
        Some("features".to_string()),
        vec!["PROJ-123".to_string()],
    );

    let start_datetime = date
        .and_hms_opt(9, 0, 0)
        .unwrap()
        .and_utc()
        .with_timezone(&chrono_tz::UTC);
    let end_datetime = date
        .and_hms_opt(12, 30, 0)
        .unwrap()
        .and_utc()
        .with_timezone(&chrono_tz::UTC);

    let session = Session::new(
        intent.clone(),
        start_datetime,
        Some(end_datetime),
        Some("Morning work".to_string()),
    );
    let log = Log::new(date, chrono_tz::UTC, vec![session]);

    let trackers = HashMap::new();
    ws.logs().write_log(&log, &trackers).await.unwrap();

    // Create a timesheet from the log data
    let meta = TimesheetMeta::new("client1".to_string(), None, "test-hash".to_string());
    let compiled = chrono::Utc::now().with_timezone(&chrono_tz::UTC);

    let timesheet = Timesheet::new(
        HashMap::new(),
        date,
        compiled,
        chrono_tz::UTC,
        log.timeline.clone(),
        HashMap::new(),
        meta,
    );

    // Write timesheet
    timesheet_manager.write_timesheet(&timesheet).await.unwrap();

    // Read it back
    let retrieved = timesheet_manager
        .get_timesheet("client1", date)
        .await
        .unwrap()
        .expect("Timesheet should exist");

    assert_eq!(retrieved.date, date);
    assert_eq!(retrieved.timeline.len(), 1);
    assert_eq!(retrieved.timeline[0].intent.alias.as_ref().unwrap(), "work");
    assert_eq!(retrieved.timeline[0].note.as_ref().unwrap(), "Morning work");
}

#[tokio::test]
async fn test_identity_and_timesheet_integration() {
    let storage = Arc::new(IntegrationStorage::new());

    let identity_manager = IdentityManager::new(storage.clone());
    let timesheet_manager = TimesheetManager::new(storage.clone());

    // Create an identity
    let signing_key = identity_manager
        .create_identity("alice", false)
        .await
        .unwrap();

    // Verify we can retrieve it
    let retrieved_key = identity_manager
        .get_identity("alice")
        .await
        .unwrap()
        .expect("Identity should exist");

    assert_eq!(signing_key.to_bytes(), retrieved_key.to_bytes());

    // Create a timesheet
    let date = NaiveDate::from_ymd_opt(2025, 3, 20).unwrap();
    let compiled = chrono::Utc::now().with_timezone(&chrono_tz::UTC);
    let meta = TimesheetMeta::new("client1".to_string(), None, "test-hash".to_string());

    let timesheet = Timesheet::new(
        HashMap::new(),
        date,
        compiled,
        chrono_tz::UTC,
        vec![],
        HashMap::new(),
        meta,
    );

    // Sign the timesheet with the identity
    let signed_timesheet = timesheet.sign("alice", &signing_key.to_bytes()).unwrap();

    timesheet_manager
        .write_timesheet(&signed_timesheet)
        .await
        .unwrap();

    // Read it back and verify signature is preserved
    let retrieved = timesheet_manager
        .get_timesheet("client1", date)
        .await
        .unwrap()
        .expect("Timesheet should exist");

    assert_eq!(retrieved.signatures.len(), 1);
    assert!(retrieved.signatures.contains_key("alice"));
}

#[tokio::test]
async fn test_multiple_managers_share_storage() {
    let storage = Arc::new(IntegrationStorage::new());
    let ws = create_test_workspace(storage.clone()).await;

    // Write data with log manager
    let date = NaiveDate::from_ymd_opt(2025, 3, 20).unwrap();
    let log = Log::new(date, chrono_tz::UTC, vec![]);
    ws.logs().write_log(&log, &HashMap::new()).await.unwrap();

    // Verify log manager can see the storage was used
    assert!(ws.logs().log_exists(date));

    // Verify storage is shared by checking base dir
    assert_eq!(storage.base_dir(), PathBuf::from("/faff"));

    // Plan manager should be able to access plans (even if none exist yet)
    let plans = ws.plans().get_plans(date).await.unwrap();
    assert_eq!(plans.len(), 0); // No plans yet, but should not error
}

#[tokio::test]
async fn test_plan_caching_across_calls() {
    let storage = Arc::new(IntegrationStorage::new());

    storage.add_file(
        PathBuf::from("/faff/plans/local.20250315.toml"),
        r#"
source = "local"
valid_from = "2025-03-15"
roles = ["engineer"]
"#
        .to_string(),
    );

    let plan_manager = PlanManager::new(storage.clone());
    let date = NaiveDate::from_ymd_opt(2025, 3, 20).unwrap();

    // First call - loads from storage
    let plans1 = plan_manager.get_plans(date).await.unwrap();
    assert_eq!(plans1.len(), 1);

    // Second call - should use cache
    let plans2 = plan_manager.get_plans(date).await.unwrap();
    assert_eq!(plans2.len(), 1);

    // They should be identical
    assert_eq!(plans1.get("local"), plans2.get("local"));

    // Writing a new plan should clear the cache
    let new_plan = Plan::new(
        "local".to_string(),
        NaiveDate::from_ymd_opt(2025, 3, 21).unwrap(),
        None,
        vec!["manager".to_string()],
        vec![],
        vec![],
        vec![],
        HashMap::new(),
        vec![],
    );
    plan_manager.write_plan(&new_plan).await.unwrap();

    // Cache should be cleared, different date returns different results
    let plans3 = plan_manager
        .get_plans(NaiveDate::from_ymd_opt(2025, 3, 21).unwrap())
        .await
        .unwrap();
    assert_eq!(plans3.len(), 1);
    assert_eq!(plans3.get("local").unwrap().roles, vec!["manager"]);
}

#[tokio::test]
async fn test_log_list_and_read_integration() {
    let storage = Arc::new(IntegrationStorage::new());
    let ws = create_test_workspace(storage.clone()).await;

    // Create multiple logs
    let date1 = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
    let date2 = NaiveDate::from_ymd_opt(2025, 3, 16).unwrap();
    let date3 = NaiveDate::from_ymd_opt(2025, 3, 17).unwrap();

    let log1 = Log::new(date1, chrono_tz::UTC, vec![]);
    let log2 = Log::new(date2, chrono_tz::UTC, vec![]);
    let log3 = Log::new(date3, chrono_tz::UTC, vec![]);

    let trackers = HashMap::new();
    ws.logs().write_log(&log1, &trackers).await.unwrap();
    ws.logs().write_log(&log2, &trackers).await.unwrap();
    ws.logs().write_log(&log3, &trackers).await.unwrap();

    // List all logs
    let dates = ws.logs().list_logs().await.unwrap();
    assert_eq!(dates.len(), 3);
    assert_eq!(dates[0], date1);
    assert_eq!(dates[1], date2);
    assert_eq!(dates[2], date3);

    // Read each log back
    for date in dates {
        let log = ws.logs().get_log(date).await.unwrap();
        assert_eq!(log.date, date);
    }
}

#[tokio::test]
async fn test_timesheet_list_filtering() {
    let storage = Arc::new(IntegrationStorage::new());
    let timesheet_manager = TimesheetManager::new(storage.clone());

    let date1 = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
    let date2 = NaiveDate::from_ymd_opt(2025, 3, 16).unwrap();
    let compiled = chrono::Utc::now().with_timezone(&chrono_tz::UTC);

    // Create timesheets for different audiences and dates
    for (audience, date) in [("client1", date1), ("client2", date1), ("client1", date2)] {
        let meta = TimesheetMeta::new(audience.to_string(), None, "test-hash".to_string());
        let timesheet = Timesheet::new(
            HashMap::new(),
            date,
            compiled,
            chrono_tz::UTC,
            vec![],
            HashMap::new(),
            meta,
        );
        timesheet_manager.write_timesheet(&timesheet).await.unwrap();
    }

    // List all timesheets
    let all = timesheet_manager.list_timesheets(None).await.unwrap();
    assert_eq!(all.len(), 3);

    // List filtered by date
    let filtered = timesheet_manager
        .list_timesheets(Some(date1))
        .await
        .unwrap();
    assert_eq!(filtered.len(), 2);
    assert!(filtered.iter().all(|t| t.date == date1));

    let filtered2 = timesheet_manager
        .list_timesheets(Some(date2))
        .await
        .unwrap();
    assert_eq!(filtered2.len(), 1);
    assert_eq!(filtered2[0].date, date2);
}
