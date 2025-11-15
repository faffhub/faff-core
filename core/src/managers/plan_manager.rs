use anyhow::{Context, Result};
use chrono::NaiveDate;
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
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
/// Manages plan loading and querying
#[derive(Clone)]
pub struct PlanManager {
    storage: Arc<dyn Storage>,
}

impl PlanManager {
    const LOCAL_PLAN_SOURCE: &'static str = "local";

    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self { storage }
    }

    /// Get all plans valid for a given date
    ///
    /// A plan is valid if:
    /// - valid_from <= target_date
    /// - and (valid_until >= target_date or valid_until is None)
    pub async fn get_plans(&self, date: NaiveDate) -> Result<HashMap<String, Plan>> {
        self.load_plans_for_date(date).await
    }

    /// Load plans from storage for a given date
    async fn load_plans_for_date(&self, date: NaiveDate) -> Result<HashMap<String, Plan>> {
        let plan_dir = self.storage.plan_dir();
        let plan_files = self.find_plan_files_for_date(&plan_dir, date).await?;

        let mut plans: HashMap<String, Plan> = HashMap::new();

        for file_path in plan_files {
            let content = self
                .storage
                .read_string(&file_path)
                .await
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
    async fn find_plan_files_for_date(
        &self,
        plan_dir: &Path,
        date: NaiveDate,
    ) -> Result<Vec<PathBuf>> {
        let files = self
            .storage
            .list_files(plan_dir, "*.toml")
            .await
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
    pub async fn get_intents(&self, date: NaiveDate) -> Result<Vec<Intent>> {
        let plans = self.get_plans(date).await?;
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
    pub async fn get_roles(&self, date: NaiveDate) -> Result<Vec<String>> {
        let plans = self.get_plans(date).await?;
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
    pub async fn get_objectives(&self, date: NaiveDate) -> Result<Vec<String>> {
        let plans = self.get_plans(date).await?;
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
    pub async fn get_actions(&self, date: NaiveDate) -> Result<Vec<String>> {
        let plans = self.get_plans(date).await?;
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
    pub async fn get_subjects(&self, date: NaiveDate) -> Result<Vec<String>> {
        let plans = self.get_plans(date).await?;
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
    pub async fn get_trackers(&self, date: NaiveDate) -> Result<HashMap<String, String>> {
        let plans = self.get_plans(date).await?;
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
    ///
    /// Returns None if the tracker is not found in any plan for the given date
    pub async fn get_plan_by_tracker_id(
        &self,
        tracker_id: &str,
        date: NaiveDate,
    ) -> Result<Option<Plan>> {
        let plans = self.get_plans(date).await?;

        for plan in plans.values() {
            if plan.trackers.contains_key(tracker_id) {
                return Ok(Some(plan.clone()));
            }
        }

        Ok(None)
    }

    /// Get the local plan for a given date
    ///
    /// Returns None if the local plan doesn't exist
    pub async fn get_local_plan(&self, date: NaiveDate) -> Result<Option<Plan>> {
        let plans = self.get_plans(date).await?;
        Ok(plans.get(Self::LOCAL_PLAN_SOURCE).cloned())
    }

    /// Get the local plan for a given date, creating an empty one if it doesn't exist
    ///
    /// This is a convenience method for callers who always want a plan to work with
    pub async fn get_local_plan_or_create(&self, date: NaiveDate) -> Result<Plan> {
        if let Some(plan) = self.get_local_plan(date).await? {
            Ok(plan)
        } else {
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
    ///
    /// If a remote configuration exists for this plan's source, vocabulary mappings
    /// will be automatically applied before writing.
    ///
    /// Note: Remote files must be named `{source}.toml` where source matches the plan source
    /// (which is the slugified remote id).
    pub async fn write_plan(&self, plan: &Plan) -> Result<()> {
        use crate::models::remote::Remote;

        // Try to load remote configuration for this plan's source
        let remote_file = self
            .storage
            .remotes_dir()
            .join(format!("{}.toml", plan.source));
        let plan_to_write = if self.storage.exists(&remote_file) {
            // Load remote and apply vocabulary mappings if configured
            let remote_toml = self
                .storage
                .read_string(&remote_file)
                .await
                .with_context(|| {
                    format!("Failed to read remote config: {}", remote_file.display())
                })?;

            let remote = Remote::from_toml(&remote_toml).with_context(|| {
                format!("Failed to parse remote config: {}", remote_file.display())
            })?;

            if !remote.vocabulary_mappings.is_empty() {
                // Try to load existing plan for this date to maintain intent ID continuity
                let existing_plan = self
                    .get_plans(plan.valid_from)
                    .await
                    .ok()
                    .and_then(|plans| plans.get(&plan.source).cloned());

                // Apply vocabulary mappings and use the augmented plan
                remote
                    .apply_vocabulary_mappings(plan, existing_plan.as_ref())
                    .with_context(|| {
                        format!(
                            "Failed to apply vocabulary mappings for remote '{}'",
                            remote.id
                        )
                    })?
            } else {
                // No mappings, use original plan
                plan.clone()
            }
        } else {
            // No remote config, use original plan
            plan.clone()
        };

        let plan_dir = self.storage.plan_dir();
        self.storage.create_dir_all(&plan_dir).await?;

        let filename = format!(
            "{}.{}.toml",
            plan_to_write.source,
            plan_to_write.valid_from.format("%Y%m%d")
        );
        let file_path = plan_dir.join(filename);

        let toml_content =
            toml::to_string_pretty(&plan_to_write).context("Failed to serialize plan to TOML")?;

        self.storage
            .write_string(&file_path, &toml_content)
            .await
            .context("Failed to write plan file")?;

        Ok(())
    }

    /// List all plan files
    ///
    /// Returns a vector of (source, valid_from_date) tuples
    pub async fn list_plans(&self) -> Result<Vec<(String, NaiveDate)>> {
        let plan_dir = self.storage.plan_dir();
        let files = self
            .storage
            .list_files(&plan_dir, "*.toml")
            .await
            .context("Failed to list plan files")?;

        let mut plan_info = Vec::new();

        for file_path in files {
            let filename = file_path
                .file_name()
                .and_then(|n| n.to_str())
                .with_context(|| format!("Invalid filename in plan directory: {file_path:?}"))?;

            if let Some(captures) = PLAN_FILENAME_REGEX.captures(filename) {
                let source = captures.name("source").unwrap().as_str().to_string();
                let datestr = captures.name("datestr").unwrap().as_str();

                if let Ok(date) = NaiveDate::parse_from_str(datestr, "%Y%m%d") {
                    plan_info.push((source, date));
                }
            }
        }

        plan_info.sort();
        Ok(plan_info)
    }

    /// Check if a plan exists for a specific source and date
    pub fn plan_exists(&self, source: &str, date: NaiveDate) -> bool {
        let plan_dir = self.storage.plan_dir();
        let filename = format!("{}.{}.toml", source, date.format("%Y%m%d"));
        let file_path = plan_dir.join(filename);
        self.storage.exists(&file_path)
    }

    /// Delete a plan
    pub async fn delete_plan(&self, source: &str, date: NaiveDate) -> Result<()> {
        let plan_dir = self.storage.plan_dir();
        let filename = format!("{}.{}.toml", source, date.format("%Y%m%d"));
        let file_path = plan_dir.join(filename);

        if !self.storage.exists(&file_path) {
            anyhow::bail!(
                "Plan for source '{}' and date {} does not exist",
                source,
                date
            );
        }

        self.storage.delete(&file_path).await.with_context(|| {
            format!("Failed to delete plan for source '{source}' and date {date}")
        })?;

        Ok(())
    }

    /// Find an intent by ID across all plan files
    ///
    /// Searches all plan files for an intent with the given intent_id.
    /// Returns None if the intent is not found.
    ///
    /// # Returns
    /// - Ok(Some((source, intent, plan_file_path))) if found
    /// - Ok(None) if not found
    /// - Err if there's an error reading files
    pub async fn find_intent_by_id(
        &self,
        intent_id: &str,
    ) -> Result<Option<(String, Intent, PathBuf)>> {
        let plan_dir = self.storage.plan_dir();
        let plan_files = self
            .storage
            .list_files(&plan_dir, "*.toml")
            .await
            .context("Failed to list plan files")?;

        for file_path in plan_files {
            let content = self
                .storage
                .read_string(&file_path)
                .await
                .with_context(|| format!("Failed to read plan file: {}", file_path.display()))?;

            let plan: Plan = match toml::from_str(&content) {
                Ok(p) => p,
                Err(_) => continue, // Skip invalid plan files
            };

            // Search for the intent in this plan
            for intent in &plan.intents {
                if intent.intent_id == intent_id {
                    return Ok(Some((plan.source.clone(), intent.clone(), file_path)));
                }
            }
        }

        Ok(None)
    }

    /// Update an intent by ID across all plan files
    ///
    /// Searches all plan files for an intent with the given intent_id and updates it.
    /// Returns the updated plan if found and successfully updated.
    ///
    /// # Returns
    /// - Ok(Some(plan)) if the intent was found and updated
    /// - Ok(None) if the intent was not found
    /// - Err if there's an error reading/writing files or updating the intent
    pub async fn update_intent_by_id(
        &self,
        intent_id: &str,
        updated_intent: Intent,
    ) -> Result<Option<Plan>> {
        // First find the intent
        let found = self.find_intent_by_id(intent_id).await?;

        if let Some((_source, _original_intent, file_path)) = found {
            // Load the plan
            let content = self
                .storage
                .read_string(&file_path)
                .await
                .with_context(|| format!("Failed to read plan file: {}", file_path.display()))?;

            let plan: Plan = toml::from_str(&content)
                .with_context(|| format!("Failed to parse plan file: {}", file_path.display()))?;

            // Update the intent
            let updated_plan = plan.update_intent(intent_id, updated_intent)?;

            // Write it back
            self.write_plan(&updated_plan).await?;

            Ok(Some(updated_plan))
        } else {
            Ok(None)
        }
    }

    /// Get plan remote plugin instances
    ///
    /// This is a convenience method that delegates to the plugin manager.
    /// While plan remotes are implemented as plugins, they are conceptually
    /// associated with plans, so this provides a domain-focused access pattern.
    ///
    /// # Arguments
    /// * `plugin_manager` - Reference to the plugin manager
    ///
    /// # Returns
    /// Vector of plan remote plugin instances
    #[cfg(feature = "python")]
    pub async fn remotes(
        &self,
        plugin_manager: &tokio::sync::Mutex<crate::managers::PluginManager>,
    ) -> anyhow::Result<Vec<pyo3::Py<pyo3::PyAny>>> {
        let mut pm = plugin_manager.lock().await;
        pm.plan_remotes().await
    }

    /// Replace a field value across all plans
    ///
    /// Updates both plan-level ASTRO collections and intents
    ///
    /// # Arguments
    /// * `field` - The field to update (role, objective, action, subject)
    /// * `old_value` - The value to replace
    /// * `new_value` - The new value
    ///
    /// # Returns
    /// Tuple of (plans_updated, intents_updated)
    pub async fn replace_field_in_all_plans(
        &self,
        field: &str,
        old_value: &str,
        new_value: &str,
    ) -> Result<(usize, usize)> {
        let plan_dir = self.storage.plan_dir();
        let entries = std::fs::read_dir(&plan_dir)
            .with_context(|| format!("Failed to read plan directory: {}", plan_dir.display()))?;

        let mut plans_updated = 0;
        let mut intents_updated = 0;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Skip non-TOML files
            if path.extension().and_then(|s| s.to_str()) != Some("toml") {
                continue;
            }

            // Read and parse the plan
            let content = self.storage.read_string(&path).await?;
            let mut plan: Plan = toml::from_str(&content)?;

            let mut plan_modified = false;

            // Update plan-level ASTRO collection
            match field {
                "role" => {
                    let mut roles = plan.roles.clone();
                    if roles.iter().any(|v| v == old_value) {
                        roles = roles
                            .into_iter()
                            .map(|v| {
                                if v == old_value {
                                    new_value.to_string()
                                } else {
                                    v
                                }
                            })
                            .collect();
                        plan.roles = roles;
                        plan_modified = true;
                    }
                }
                "objective" => {
                    let mut objectives = plan.objectives.clone();
                    if objectives.iter().any(|v| v == old_value) {
                        objectives = objectives
                            .into_iter()
                            .map(|v| {
                                if v == old_value {
                                    new_value.to_string()
                                } else {
                                    v
                                }
                            })
                            .collect();
                        plan.objectives = objectives;
                        plan_modified = true;
                    }
                }
                "action" => {
                    let mut actions = plan.actions.clone();
                    if actions.iter().any(|v| v == old_value) {
                        actions = actions
                            .into_iter()
                            .map(|v| {
                                if v == old_value {
                                    new_value.to_string()
                                } else {
                                    v
                                }
                            })
                            .collect();
                        plan.actions = actions;
                        plan_modified = true;
                    }
                }
                "subject" => {
                    let mut subjects = plan.subjects.clone();
                    if subjects.iter().any(|v| v == old_value) {
                        subjects = subjects
                            .into_iter()
                            .map(|v| {
                                if v == old_value {
                                    new_value.to_string()
                                } else {
                                    v
                                }
                            })
                            .collect();
                        plan.subjects = subjects;
                        plan_modified = true;
                    }
                }
                _ => return Err(anyhow::anyhow!("Unsupported field: {}", field)),
            };

            // Update intents
            let mut updated_intents = Vec::new();
            for intent in &plan.intents {
                let intent_field_value = match field {
                    "role" => &intent.role,
                    "objective" => &intent.objective,
                    "action" => &intent.action,
                    "subject" => &intent.subject,
                    _ => unreachable!(),
                };

                if intent_field_value.as_ref().map(|s| s.as_str()) == Some(old_value) {
                    // Create updated intent
                    let updated_intent = Intent::new(
                        intent.alias.clone(),
                        if field == "role" {
                            Some(new_value.to_string())
                        } else {
                            intent.role.clone()
                        },
                        if field == "objective" {
                            Some(new_value.to_string())
                        } else {
                            intent.objective.clone()
                        },
                        if field == "action" {
                            Some(new_value.to_string())
                        } else {
                            intent.action.clone()
                        },
                        if field == "subject" {
                            Some(new_value.to_string())
                        } else {
                            intent.subject.clone()
                        },
                        intent.trackers.clone(),
                    );
                    updated_intents.push(updated_intent);
                    intents_updated += 1;
                    plan_modified = true;
                } else {
                    updated_intents.push(intent.clone());
                }
            }

            if plan_modified {
                plan.intents = updated_intents;
                self.write_plan(&plan).await?;
                plans_updated += 1;
            }
        }

        Ok((plans_updated, intents_updated))
    }

    /// Get usage statistics for a field across all plans
    ///
    /// Returns a HashMap of field value -> intent count
    pub async fn get_field_usage_stats(&self, field: &str) -> Result<HashMap<String, usize>> {
        let plan_dir = self.storage.plan_dir();
        let entries = std::fs::read_dir(&plan_dir)
            .with_context(|| format!("Failed to read plan directory: {}", plan_dir.display()))?;

        let mut usage_stats: HashMap<String, usize> = HashMap::new();

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Skip non-TOML files
            if path.extension().and_then(|s| s.to_str()) != Some("toml") {
                continue;
            }

            // Read and parse the plan
            let content = self.storage.read_string(&path).await?;
            let plan: Plan = toml::from_str(&content)?;

            // Count intents using this field value
            for intent in &plan.intents {
                let intent_field_value = match field {
                    "role" => &intent.role,
                    "objective" => &intent.objective,
                    "action" => &intent.action,
                    "subject" => &intent.subject,
                    "tracker" => {
                        // Trackers are a list, count each one
                        for tracker in &intent.trackers {
                            *usage_stats.entry(tracker.clone()).or_insert(0) += 1;
                        }
                        continue;
                    }
                    _ => return Err(anyhow::anyhow!("Unsupported field: {}", field)),
                };

                if let Some(value) = intent_field_value {
                    *usage_stats.entry(value.clone()).or_insert(0) += 1;
                }
            }
        }

        Ok(usage_stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::mock_storage::MockStorage;

    fn sample_plan_toml(source: &str, date: &str) -> String {
        format!(
            r#"
source = "{source}"
valid_from = "{date}"
roles = ["engineer"]
objectives = ["development"]
actions = ["coding"]
subjects = ["features"]

[trackers]
"123" = "Task 123"

[[intents]]
alias = "Work on feature"
role = "{source}:engineer"
objective = "{source}:development"
"#
        )
    }

    #[tokio::test]
    async fn test_load_single_plan() {
        let storage = Arc::new(MockStorage::new());
        storage.add_file(
            PathBuf::from("/faff/plans/local.20250101.toml"),
            sample_plan_toml("local", "2025-01-01"),
        );

        let manager = PlanManager::new(storage);
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();

        let plans = manager.get_plans(date).await.unwrap();
        assert_eq!(plans.len(), 1);
        assert!(plans.contains_key("local"));
    }

    #[tokio::test]
    async fn test_get_trackers() {
        let storage = Arc::new(MockStorage::new());
        storage.add_file(
            PathBuf::from("/faff/plans/local.20250101.toml"),
            sample_plan_toml("local", "2025-01-01"),
        );

        let manager = PlanManager::new(storage);
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();

        let trackers = manager.get_trackers(date).await.unwrap();
        assert_eq!(trackers.get("local:123"), Some(&"Task 123".to_string()));
    }

    #[tokio::test]
    async fn test_cache_works() {
        let storage = Arc::new(MockStorage::new());
        storage.add_file(
            PathBuf::from("/faff/plans/local.20250101.toml"),
            sample_plan_toml("local", "2025-01-01"),
        );

        let manager = PlanManager::new(storage);
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();

        // First call - loads from storage
        let plans1 = manager.get_plans(date).await.unwrap();
        // Second call - should use cache
        let plans2 = manager.get_plans(date).await.unwrap();

        assert_eq!(plans1.len(), plans2.len());
    }

    #[tokio::test]
    async fn test_get_local_plan_returns_none_when_missing() {
        let storage = Arc::new(MockStorage::new());
        let manager = PlanManager::new(storage);
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();

        let plan = manager.get_local_plan(date).await.unwrap();
        assert!(plan.is_none());
    }

    #[tokio::test]
    async fn test_get_local_plan_or_create() {
        let storage = Arc::new(MockStorage::new());
        let manager = PlanManager::new(storage);
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();

        let plan = manager.get_local_plan_or_create(date).await.unwrap();
        assert_eq!(plan.source, "local");
        assert_eq!(plan.valid_from, date);
        assert_eq!(plan.intents.len(), 0);
    }

    #[tokio::test]
    async fn test_get_plan_by_tracker_id_returns_none() {
        let storage = Arc::new(MockStorage::new());
        storage.add_file(
            PathBuf::from("/faff/plans/local.20250101.toml"),
            sample_plan_toml("local", "2025-01-01"),
        );

        let manager = PlanManager::new(storage);
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();

        let plan = manager.get_plan_by_tracker_id("999", date).await.unwrap();
        assert!(plan.is_none());
    }

    #[tokio::test]
    async fn test_list_plans() {
        let storage = Arc::new(MockStorage::new());
        storage.add_file(
            PathBuf::from("/faff/plans/local.20250101.toml"),
            sample_plan_toml("local", "2025-01-01"),
        );
        storage.add_file(
            PathBuf::from("/faff/plans/remote.20250115.toml"),
            sample_plan_toml("remote", "2025-01-15"),
        );

        let manager = PlanManager::new(storage);
        let plans = manager.list_plans().await.unwrap();

        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].0, "local");
        assert_eq!(plans[0].1, NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());
        assert_eq!(plans[1].0, "remote");
        assert_eq!(plans[1].1, NaiveDate::from_ymd_opt(2025, 1, 15).unwrap());
    }

    #[tokio::test]
    async fn test_plan_exists() {
        let storage = Arc::new(MockStorage::new());
        storage.add_file(
            PathBuf::from("/faff/plans/local.20250101.toml"),
            sample_plan_toml("local", "2025-01-01"),
        );

        let manager = PlanManager::new(storage);
        let date = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();

        assert!(manager.plan_exists("local", date));
        assert!(!manager.plan_exists("remote", date));
    }

    #[tokio::test]
    async fn test_delete_plan() {
        let storage = Arc::new(MockStorage::new());
        storage.add_file(
            PathBuf::from("/faff/plans/local.20250101.toml"),
            sample_plan_toml("local", "2025-01-01"),
        );

        let manager = PlanManager::new(storage.clone());
        let date = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();

        assert!(manager.plan_exists("local", date));

        manager.delete_plan("local", date).await.unwrap();

        assert!(!manager.plan_exists("local", date));
    }

    #[tokio::test]
    async fn test_delete_nonexistent_plan() {
        let storage = Arc::new(MockStorage::new());
        let manager = PlanManager::new(storage);
        let date = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();

        let result = manager.delete_plan("nonexistent", date).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[tokio::test]
    async fn test_write_plan_applies_vocabulary_mappings() {
        let storage = Arc::new(MockStorage::new());

        // Create a remote configuration with vocabulary mapping
        let remote_toml = r#"
id = "test-remote"
plugin = "test"

[[vocabulary_mapping]]
source_type = "tracker"
target_type = "intent"
pattern = "^POC-(?P<id>\\d+)\\s+(?P<description>.+)$"
alias = "POC-{id}: {description}"
role = "customer-success"
objective = "revenue"
action = "drive-poc"
subject = "poc/{description|slugify}"
        "#;

        // Store the remote config (MockStorage base_dir is /faff/.faff)
        storage.add_file(
            PathBuf::from("/faff/remotes/test-remote.toml"),
            remote_toml.to_string(),
        );

        let manager = PlanManager::new(storage.clone());

        // Create a plan with POC trackers
        let mut trackers = std::collections::HashMap::new();
        trackers.insert("1".to_string(), "POC-29 European Commission".to_string());
        trackers.insert("2".to_string(), "POC-62 Unicredit POC".to_string());
        trackers.insert("3".to_string(), "Not a POC".to_string());

        let plan = Plan::new(
            "test-remote".to_string(),
            NaiveDate::from_ymd_opt(2025, 11, 4).unwrap(),
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            trackers,
            vec![],
        );

        // Write the plan (should apply vocabulary mappings)
        manager
            .write_plan(&plan)
            .await
            .expect("Failed to write plan");

        // Read back the written plan (MockStorage base_dir is /faff/.faff)
        let written_plan_path = PathBuf::from("/faff/plans/test-remote.20251104.toml");
        assert!(
            storage.exists(&written_plan_path),
            "Plan file should exist after write_plan"
        );

        let written_content = storage
            .read_string(&written_plan_path)
            .await
            .expect("Failed to read written plan");
        let written_plan: Plan =
            toml::from_str(&written_content).expect("Failed to parse written plan");

        // Verify that intents were generated
        assert_eq!(
            written_plan.intents.len(),
            2,
            "Should generate 2 intents from 2 POC trackers"
        );

        // Check that POC-29 intent was created
        let poc29 = written_plan
            .intents
            .iter()
            .find(|i| {
                i.alias
                    .as_ref()
                    .map(|a| a.contains("POC-29"))
                    .unwrap_or(false)
            })
            .expect("Should find POC-29 intent");

        assert_eq!(poc29.alias, Some("POC-29: European Commission".to_string()));
        assert_eq!(poc29.role, Some("customer-success".to_string()));
        assert_eq!(poc29.objective, Some("revenue".to_string()));
        assert_eq!(poc29.action, Some("drive-poc".to_string()));
        assert_eq!(poc29.subject, Some("poc/european-commission".to_string()));
    }
}
