use anyhow::{Context, Result};
use chrono::NaiveDate;
use regex::Regex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};

use crate::models::intent::Intent;
use crate::models::plan::Plan;
use crate::storage::Storage;

// Regex for parsing plan filenames - validated at compile time
static PLAN_FILENAME_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?P<source>.+?)\.(?P<datestr>\d{8})\.toml$")
        .expect("PLAN_FILENAME_REGEX pattern is valid")
});

/// Manages Plan loading, caching, and querying
///
/// FIXME: Currently takes just Storage, but may need access to other managers
/// (e.g., to coordinate with IdentityManager, TimesheetManager) in the future.
/// For now, coordination happens via method parameters (like get_trackers()).
/// Consider creating a Workspace wrapper or passing managers as needed.
pub struct PlanManager {
    storage: Arc<dyn Storage>,
    /// Cache of plans by date
    /// Key: (date) -> Value: HashMap<source, Plan>
    cache: std::sync::RwLock<HashMap<NaiveDate, HashMap<String, Plan>>>,
}

impl PlanManager {
    const LOCAL_PLAN_SOURCE: &'static str = "local";

    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self {
            storage,
            cache: std::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Get all plans valid for a given date
    ///
    /// A plan is valid if:
    /// - valid_from <= target_date
    /// - and (valid_until >= target_date or valid_until is None)
    pub fn get_plans(&self, date: NaiveDate) -> Result<HashMap<String, Plan>> {
        // Check cache first
        {
            let cache = self.cache.read().unwrap();
            if let Some(plans) = cache.get(&date) {
                return Ok(plans.clone());
            }
        }

        // Not in cache, load from storage
        let plans = self.load_plans_for_date(date)?;

        // Store in cache
        {
            let mut cache = self.cache.write().unwrap();
            cache.insert(date, plans.clone());
        }

        Ok(plans)
    }

    /// Load plans from storage for a given date
    fn load_plans_for_date(&self, date: NaiveDate) -> Result<HashMap<String, Plan>> {
        let plan_dir = self.storage.plan_dir();
        let plan_files = self.find_plan_files_for_date(&plan_dir, date)?;

        let mut plans: HashMap<String, Plan> = HashMap::new();

        for file_path in plan_files {
            let content = self
                .storage
                .read_string(&file_path)
                .with_context(|| format!("Failed to read plan file: {}", file_path.display()))?;

            let plan: Plan = toml::from_str(&content)
                .with_context(|| format!("Failed to parse plan file: {}", file_path.display()))?;

            // Validate date range
            if plan.valid_from > date {
                continue;
            }
            if let Some(valid_until) = plan.valid_until {
                if valid_until < date {
                    continue;
                }
            }

            // Keep the most recent plan for each source
            if let Some(existing) = plans.get(&plan.source) {
                if plan.valid_from > existing.valid_from {
                    plans.insert(plan.source.clone(), plan);
                }
            } else {
                plans.insert(plan.source.clone(), plan);
            }
        }

        Ok(plans)
    }

    /// Find plan files relevant for a given date
    ///
    /// Plan files follow the pattern: `<source>.<YYYYMMDD>.toml`
    /// For each source, we find the most recent file where file_date <= target_date
    fn find_plan_files_for_date(
        &self,
        plan_dir: &PathBuf,
        date: NaiveDate,
    ) -> Result<Vec<PathBuf>> {
        let files = self
            .storage
            .list_files(plan_dir, "*.toml")
            .context("Failed to list plan files")?;

        // Map of source -> (most recent date, file path)
        let mut candidates: HashMap<String, (NaiveDate, PathBuf)> = HashMap::new();

        for file_path in files {
            let filename = file_path
                .file_name()
                .and_then(|n| n.to_str())
                .context("Invalid filename")?;

            if let Some(captures) = PLAN_FILENAME_REGEX.captures(filename) {
                // These unwraps are safe because the regex guarantees named groups exist
                let source = captures.name("source").unwrap().as_str().to_string();
                let datestr = captures.name("datestr").unwrap().as_str();

                if let Ok(file_date) = NaiveDate::parse_from_str(datestr, "%Y%m%d") {
                    // Skip files with dates after our target date
                    if file_date > date {
                        continue;
                    }

                    // Keep the most recent file for this source
                    if let Some((existing_date, _)) = candidates.get(&source) {
                        if file_date > *existing_date {
                            candidates.insert(source, (file_date, file_path));
                        }
                    } else {
                        candidates.insert(source, (file_date, file_path));
                    }
                }
            }
        }

        Ok(candidates.into_values().map(|(_, path)| path).collect())
    }

    /// Get all intents from plans valid for a given date
    pub fn get_intents(&self, date: NaiveDate) -> Result<Vec<Intent>> {
        let plans = self.get_plans(date)?;
        let mut intents = std::collections::HashSet::new();

        for plan in plans.values() {
            for intent in &plan.intents {
                intents.insert(intent.clone());
            }
        }

        Ok(intents.into_iter().collect())
    }

    /// Get all roles from plans valid for a given date
    ///
    /// Returns roles prefixed with their source (e.g., "element:engineer")
    /// plus any roles from intents
    pub fn get_roles(&self, date: NaiveDate) -> Result<Vec<String>> {
        let plans = self.get_plans(date)?;
        let mut roles = Vec::new();

        for plan in plans.values() {
            // Roles from plan (prefixed with source)
            for role in &plan.roles {
                roles.push(format!("{}:{}", plan.source, role));
            }

            // Roles from intents
            for intent in &plan.intents {
                if let Some(role) = &intent.role {
                    roles.push(role.clone());
                }
            }
        }

        // Deduplicate and sort
        roles.sort();
        roles.dedup();

        Ok(roles)
    }

    /// Get all objectives from plans valid for a given date
    pub fn get_objectives(&self, date: NaiveDate) -> Result<Vec<String>> {
        let plans = self.get_plans(date)?;
        let mut objectives = Vec::new();

        for plan in plans.values() {
            // Objectives from plan (prefixed with source)
            for objective in &plan.objectives {
                objectives.push(format!("{}:{}", plan.source, objective));
            }

            // Objectives from intents
            for intent in &plan.intents {
                if let Some(objective) = &intent.objective {
                    objectives.push(objective.clone());
                }
            }
        }

        // Deduplicate and sort
        objectives.sort();
        objectives.dedup();

        Ok(objectives)
    }

    /// Get all actions from plans valid for a given date
    pub fn get_actions(&self, date: NaiveDate) -> Result<Vec<String>> {
        let plans = self.get_plans(date)?;
        let mut actions = Vec::new();

        for plan in plans.values() {
            // Actions from plan (prefixed with source)
            for action in &plan.actions {
                actions.push(format!("{}:{}", plan.source, action));
            }

            // Actions from intents
            for intent in &plan.intents {
                if let Some(action) = &intent.action {
                    actions.push(action.clone());
                }
            }
        }

        // Deduplicate and sort
        actions.sort();
        actions.dedup();

        Ok(actions)
    }

    /// Get all subjects from plans valid for a given date
    pub fn get_subjects(&self, date: NaiveDate) -> Result<Vec<String>> {
        let plans = self.get_plans(date)?;
        let mut subjects = Vec::new();

        for plan in plans.values() {
            // Subjects from plan (prefixed with source)
            for subject in &plan.subjects {
                subjects.push(format!("{}:{}", plan.source, subject));
            }

            // Subjects from intents
            for intent in &plan.intents {
                if let Some(subject) = &intent.subject {
                    subjects.push(subject.clone());
                }
            }
        }

        // Deduplicate and sort
        subjects.sort();
        subjects.dedup();

        Ok(subjects)
    }

    /// Get all trackers from plans valid for a given date
    ///
    /// Returns a map of tracker IDs (prefixed with source) to human-readable names
    /// Example: "element:12345" -> "Fix critical bug"
    pub fn get_trackers(&self, date: NaiveDate) -> Result<HashMap<String, String>> {
        let plans = self.get_plans(date)?;
        let mut trackers = HashMap::new();

        for plan in plans.values() {
            for (tracker_key, tracker_value) in &plan.trackers {
                let prefixed_key = format!("{}:{}", plan.source, tracker_key);
                trackers.insert(prefixed_key, tracker_value.clone());
            }
        }

        Ok(trackers)
    }

    /// Get the plan containing a specific tracker ID
    pub fn get_plan_by_tracker_id(&self, tracker_id: &str, date: NaiveDate) -> Result<Plan> {
        let plans = self.get_plans(date)?;

        for plan in plans.values() {
            if plan.trackers.contains_key(tracker_id) {
                return Ok(plan.clone());
            }
        }

        anyhow::bail!("Tracker ID {} not found in plans for {}", tracker_id, date)
    }

    /// Get the local plan for a given date, creating an empty one if it doesn't exist
    pub fn local_plan(&self, date: NaiveDate) -> Result<Plan> {
        let plans = self.get_plans(date)?;

        if let Some(plan) = plans.get(Self::LOCAL_PLAN_SOURCE) {
            Ok(plan.clone())
        } else {
            // Return an empty local plan
            Ok(Plan::new(
                Self::LOCAL_PLAN_SOURCE.to_string(),
                date,
                None,
                vec![],
                vec![],
                vec![],
                vec![],
                HashMap::new(),
                vec![],
            ))
        }
    }

    /// Write a plan to storage
    pub fn write_plan(&self, plan: &Plan) -> Result<()> {
        let plan_dir = self.storage.plan_dir();
        self.storage.create_dir_all(&plan_dir)?;

        let filename = format!("{}.{}.toml", plan.source, plan.valid_from.format("%Y%m%d"));
        let file_path = plan_dir.join(filename);

        let toml_content =
            toml::to_string_pretty(plan).context("Failed to serialize plan to TOML")?;

        self.storage
            .write_string(&file_path, &toml_content)
            .context("Failed to write plan file")?;

        // Clear cache to force reload on next access
        self.clear_cache();

        Ok(())
    }

    /// Clear the plan cache
    pub fn clear_cache(&self) {
        let mut cache = self.cache.write().unwrap();
        cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::mock_storage::MockStorage;

    fn sample_plan_toml(source: &str, date: &str) -> String {
        format!(
            r#"
source = "{}"
valid_from = "{}"
roles = ["engineer"]
objectives = ["development"]
actions = ["coding"]
subjects = ["features"]

[trackers]
"123" = "Task 123"

[[intents]]
alias = "Work on feature"
role = "{}:engineer"
objective = "{}:development"
"#,
            source, date, source, source
        )
    }

    #[test]
    fn test_load_single_plan() {
        let storage = Arc::new(MockStorage::new());
        storage.add_file(
            PathBuf::from("/faff/plans/local.20250101.toml"),
            sample_plan_toml("local", "2025-01-01"),
        );

        let manager = PlanManager::new(storage);
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();

        let plans = manager.get_plans(date).unwrap();
        assert_eq!(plans.len(), 1);
        assert!(plans.contains_key("local"));
    }

    #[test]
    fn test_get_trackers() {
        let storage = Arc::new(MockStorage::new());
        storage.add_file(
            PathBuf::from("/faff/plans/local.20250101.toml"),
            sample_plan_toml("local", "2025-01-01"),
        );

        let manager = PlanManager::new(storage);
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();

        let trackers = manager.get_trackers(date).unwrap();
        assert_eq!(trackers.get("local:123"), Some(&"Task 123".to_string()));
    }

    #[test]
    fn test_cache_works() {
        let storage = Arc::new(MockStorage::new());
        storage.add_file(
            PathBuf::from("/faff/plans/local.20250101.toml"),
            sample_plan_toml("local", "2025-01-01"),
        );

        let manager = PlanManager::new(storage);
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();

        // First call - loads from storage
        let plans1 = manager.get_plans(date).unwrap();
        // Second call - should use cache
        let plans2 = manager.get_plans(date).unwrap();

        assert_eq!(plans1.len(), plans2.len());
    }
}
