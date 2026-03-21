use anyhow::{Context, Result};
use chrono::NaiveDate;
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use crate::models::plan::{Plan, SessionHint};
use crate::models::remote::TrackerMapping;

/// All plan-derived data needed at session start time, loaded in a single pass.
pub struct StartData {
    pub roles: Vec<String>,
    pub impacts: Vec<String>,
    pub modes: Vec<String>,
    pub subjects: Vec<String>,
    pub trackers: HashMap<String, String>,
    pub hints: Vec<SessionHint>,
    pub tracker_mappings: Vec<TrackerMapping>,
}
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

    /// Get all roles from plans valid for a given date
    ///
    /// Returns roles prefixed with their source (e.g., "element:engineer")
    pub async fn get_roles(&self, date: NaiveDate) -> Result<Vec<String>> {
        let plans = self.get_plans(date).await?;
        let mut roles = Vec::new();

        for plan in plans.values() {
            for role in &plan.roles {
                roles.push(format!("{}:{}", plan.source, role));
            }
        }

        // Deduplicate and sort
        roles.sort();
        roles.dedup();

        Ok(roles)
    }

    /// Get all impacts from plans valid for a given date
    pub async fn get_impacts(&self, date: NaiveDate) -> Result<Vec<String>> {
        let plans = self.get_plans(date).await?;
        let mut impacts = Vec::new();

        for plan in plans.values() {
            for impact in &plan.impacts {
                impacts.push(format!("{}:{}", plan.source, impact));
            }
        }

        // Deduplicate and sort
        impacts.sort();
        impacts.dedup();

        Ok(impacts)
    }

    /// Get all modes from plans valid for a given date
    pub async fn get_modes(&self, date: NaiveDate) -> Result<Vec<String>> {
        let plans = self.get_plans(date).await?;
        let mut modes = Vec::new();

        for plan in plans.values() {
            for mode in &plan.modes {
                modes.push(format!("{}:{}", plan.source, mode));
            }
        }

        // Deduplicate and sort
        modes.sort();
        modes.dedup();

        Ok(modes)
    }

    /// Get all subjects from plans valid for a given date
    pub async fn get_subjects(&self, date: NaiveDate) -> Result<Vec<String>> {
        let plans = self.get_plans(date).await?;
        let mut subjects = Vec::new();

        for plan in plans.values() {
            for subject in &plan.subjects {
                subjects.push(format!("{}:{}", plan.source, subject));
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
            ))
        }
    }

    /// Get all session hints from plans valid for a given date
    ///
    /// Returns hints generated by plugins (e.g. POC/Support tracker hints),
    /// collected across all plans for the date.
    pub async fn get_session_hints(&self, date: NaiveDate) -> Result<Vec<SessionHint>> {
        let plans = self.get_plans(date).await?;
        let mut all_hints = Vec::new();
        for plan in plans.values() {
            all_hints.extend(plan.hints.clone());
        }
        Ok(all_hints)
    }

    /// Get tracker mappings for all plans valid for a given date
    ///
    /// Loads each plan's remote config (if any) and runs all tracker-source vocabulary
    /// mappings to build a reverse lookup index from session field values to trackers.
    /// Used for auto-deriving trackers at session start time.
    pub async fn get_tracker_mappings(&self, date: NaiveDate) -> Result<Vec<TrackerMapping>> {
        use crate::models::remote::Remote;

        let plans = self.get_plans(date).await?;
        let mut all_mappings = Vec::new();

        for plan in plans.values() {
            let remote_file = self
                .storage
                .remotes_dir()
                .join(format!("{}.toml", plan.source));

            if !self.storage.exists(&remote_file) {
                continue;
            }

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

            let tracker_mappings = remote.generate_tracker_mappings(plan).with_context(|| {
                format!(
                    "Failed to generate tracker mappings for remote '{}'",
                    remote.id
                )
            })?;

            all_mappings.extend(tracker_mappings);
        }

        Ok(all_mappings)
    }

    /// Return all plan-derived data needed at session start time in a single plan load.
    ///
    /// Replaces calling get_roles/get_impacts/get_modes/get_subjects/get_trackers/
    /// get_session_hints/get_tracker_mappings individually (each re-reads plan files).
    pub async fn get_start_data(&self, date: NaiveDate) -> Result<StartData> {
        use crate::models::remote::Remote;

        let plans = self.get_plans(date).await?;

        let mut roles = Vec::new();
        let mut impacts = Vec::new();
        let mut modes = Vec::new();
        let mut subjects = Vec::new();
        let mut trackers = HashMap::new();
        let mut hints = Vec::new();
        let mut tracker_mappings = Vec::new();

        for plan in plans.values() {
            for role in &plan.roles {
                roles.push(format!("{}:{}", plan.source, role));
            }
            for impact in &plan.impacts {
                impacts.push(format!("{}:{}", plan.source, impact));
            }
            for mode in &plan.modes {
                modes.push(format!("{}:{}", plan.source, mode));
            }
            for subject in &plan.subjects {
                subjects.push(format!("{}:{}", plan.source, subject));
            }
            for (key, value) in &plan.trackers {
                trackers.insert(format!("{}:{}", plan.source, key), value.clone());
            }
            hints.extend(plan.hints.clone());

            let remote_file = self
                .storage
                .remotes_dir()
                .join(format!("{}.toml", plan.source));
            if self.storage.exists(&remote_file) {
                let remote_toml =
                    self.storage
                        .read_string(&remote_file)
                        .await
                        .with_context(|| {
                            format!("Failed to read remote config: {}", remote_file.display())
                        })?;
                let remote = Remote::from_toml(&remote_toml).with_context(|| {
                    format!("Failed to parse remote config: {}", remote_file.display())
                })?;
                let plan_mappings = remote.generate_tracker_mappings(plan).with_context(|| {
                    format!("Failed to generate tracker mappings for '{}'", remote.id)
                })?;
                tracker_mappings.extend(plan_mappings);
            }
        }

        roles.sort();
        roles.dedup();
        impacts.sort();
        impacts.dedup();
        modes.sort();
        modes.dedup();
        subjects.sort();
        subjects.dedup();

        Ok(StartData {
            roles,
            impacts,
            modes,
            subjects,
            trackers,
            hints,
            tracker_mappings,
        })
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
                // Try to load existing plan for this date to maintain continuity
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
    /// Updates plan-level ASTRO collections
    ///
    /// # Arguments
    /// * `field` - The field to update (role, impact, mode, subject)
    /// * `old_value` - The value to replace
    /// * `new_value` - The new value
    ///
    /// # Returns
    /// Number of plans updated
    pub async fn replace_field_in_all_plans(
        &self,
        field: &str,
        old_value: &str,
        new_value: &str,
    ) -> Result<usize> {
        let plan_dir = self.storage.plan_dir();
        let entries = std::fs::read_dir(&plan_dir)
            .with_context(|| format!("Failed to read plan directory: {}", plan_dir.display()))?;

        let mut plans_updated = 0;

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

            // Plans store vocabulary values WITHOUT source prefix (e.g. "engineer" not
            // "element:engineer"). Strip the plan's source prefix from old/new values so
            // that callers can pass prefixed values (as used in logs) and still match.
            let source_prefix = format!("{}:", plan.source);
            let plan_old = old_value.strip_prefix(&source_prefix).unwrap_or(old_value);
            let plan_new = new_value.strip_prefix(&source_prefix).unwrap_or(new_value);

            // Skip if the normalised old and new values are identical (no real change)
            if plan_old == plan_new {
                continue;
            }

            // Update plan-level ASTRO collection
            match field {
                "role" => {
                    if plan.roles.iter().any(|v| v == plan_old) {
                        plan.roles = plan
                            .roles
                            .into_iter()
                            .map(|v| {
                                if v == plan_old {
                                    plan_new.to_string()
                                } else {
                                    v
                                }
                            })
                            .collect();
                        plan_modified = true;
                    }
                }
                "impact" => {
                    if plan.impacts.iter().any(|v| v == plan_old) {
                        plan.impacts = plan
                            .impacts
                            .into_iter()
                            .map(|v| {
                                if v == plan_old {
                                    plan_new.to_string()
                                } else {
                                    v
                                }
                            })
                            .collect();
                        plan_modified = true;
                    }
                }
                "mode" => {
                    if plan.modes.iter().any(|v| v == plan_old) {
                        plan.modes = plan
                            .modes
                            .into_iter()
                            .map(|v| {
                                if v == plan_old {
                                    plan_new.to_string()
                                } else {
                                    v
                                }
                            })
                            .collect();
                        plan_modified = true;
                    }
                }
                "subject" => {
                    if plan.subjects.iter().any(|v| v == plan_old) {
                        plan.subjects = plan
                            .subjects
                            .into_iter()
                            .map(|v| {
                                if v == plan_old {
                                    plan_new.to_string()
                                } else {
                                    v
                                }
                            })
                            .collect();
                        plan_modified = true;
                    }
                }
                _ => return Err(anyhow::anyhow!("Unsupported field: {}", field)),
            };

            if plan_modified {
                self.write_plan(&plan).await?;
                plans_updated += 1;
            }
        }

        Ok(plans_updated)
    }

    /// Get usage statistics for a field across all plans
    ///
    /// Returns a HashMap of field value -> plan count
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

            // Count vocabulary usage in this plan
            let values: Vec<&String> = match field {
                "role" => plan.roles.iter().collect(),
                "impact" => plan.impacts.iter().collect(),
                "mode" => plan.modes.iter().collect(),
                "subject" => plan.subjects.iter().collect(),
                "tracker" => {
                    for tracker in plan.trackers.keys() {
                        *usage_stats.entry(tracker.clone()).or_insert(0) += 1;
                    }
                    continue;
                }
                _ => return Err(anyhow::anyhow!("Unsupported field: {}", field)),
            };

            for value in values {
                *usage_stats.entry(value.clone()).or_insert(0) += 1;
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
        assert!(plan.roles.is_empty());
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

        // Create a remote configuration with vocabulary mapping (tracker -> subject)
        let remote_toml = r#"
id = "test-remote"
plugin = "test"

[[vocabulary_mapping]]
source_type = "tracker"
target_type = "subject"
pattern = "^POC-(?P<id>\\d+)\\s+(?P<description>.+)$"
subject = "poc/{description|slugify}"
        "#;

        // Store the remote config
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
        );

        // Write the plan (should apply vocabulary mappings)
        manager
            .write_plan(&plan)
            .await
            .expect("Failed to write plan");

        // Read back the written plan
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

        // Verify that subjects were generated
        assert_eq!(
            written_plan.subjects.len(),
            2,
            "Should generate 2 subjects from 2 POC trackers"
        );

        // Check that subjects were created correctly
        assert!(written_plan
            .subjects
            .contains(&"poc/european-commission".to_string()));
        assert!(written_plan
            .subjects
            .contains(&"poc/unicredit-poc".to_string()));
    }
}
