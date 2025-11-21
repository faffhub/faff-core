use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use regex::{Captures, Regex};
use slug::slugify;

/// Configuration for a remote plugin instance
///
/// A Remote represents a configured instance of a plugin that can:
/// - Pull plans from a remote source
/// - Compile timesheets for a remote audience
/// - Push timesheets to a remote destination
///
/// Multiple remotes can use the same plugin with different configurations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Remote {
    /// Unique identifier for this remote (e.g., "mycompany", "personal")
    /// Used in plan and timesheet filenames
    pub id: String,

    /// Name of the plugin to use (e.g., "myhours", "toggl")
    pub plugin: String,

    /// Plugin-specific connection configuration (API keys, URLs, etc.)
    #[serde(default)]
    pub connection: HashMap<String, toml::Value>,

    /// Static ASTRO vocabulary for this remote
    /// Used when the remote doesn't provide its own ASTRO objects
    #[serde(default)]
    pub vocabulary: RemoteVocabulary,

    /// Vocabulary mapping rules for transforming source vocabulary to target vocabulary
    #[serde(default, rename = "vocabulary_mapping")]
    pub vocabulary_mappings: Vec<VocabularyMapping>,
}

/// Type of vocabulary being mapped from or to
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VocabularyType {
    Tracker,
    Role,
    Objective,
    Action,
    Subject,
    Intent,
}

/// Configuration for mapping vocabulary from one type to another
///
/// Uses regex patterns to match source vocabulary and templates to generate
/// target vocabulary. Supports mapping any vocabulary type to any other type.
///
/// Examples:
/// - tracker → subject: Extract customer name from tracker
/// - subject → intent: Create standard intents for customer meetings
/// - tracker → intent: Most common case, direct tracker to intent mapping
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VocabularyMapping {
    /// Type of vocabulary to match against
    pub source_type: VocabularyType,

    /// Type of vocabulary to generate
    pub target_type: VocabularyType,

    /// Regex pattern to match source values (with named capture groups)
    /// Example: "^POC-(?P<id>\\d+)\\s+(?P<description>.+)$"
    pub pattern: String,

    /// Template for intent alias (required if target_type is Intent)
    /// Example: "POC-{id}: {description}"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,

    /// Template for role (required if target_type is Role, optional for Intent)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,

    /// Template for objective (required if target_type is Objective, optional for Intent)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub objective: Option<String>,

    /// Template for action (required if target_type is Action, optional for Intent)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,

    /// Template for subject (required if target_type is Subject, optional for Intent)
    /// Supports filters: "customer/{customer|slugify}"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,

    /// Templates for trackers (optional for Intent)
    /// Example: ["{source_id}"]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trackers: Option<Vec<String>>,
}

impl VocabularyMapping {
    /// Create a new vocabulary mapping
    pub fn new(
        source_type: VocabularyType,
        target_type: VocabularyType,
        pattern: impl Into<String>,
    ) -> Self {
        Self {
            source_type,
            target_type,
            pattern: pattern.into(),
            alias: None,
            role: None,
            objective: None,
            action: None,
            subject: None,
            trackers: None,
        }
    }

    /// Compile the regex pattern
    pub fn regex(&self) -> anyhow::Result<Regex> {
        Regex::new(&self.pattern)
            .map_err(|e| anyhow::anyhow!("Invalid regex pattern '{}': {}", self.pattern, e))
    }

    /// Validate that required fields are present for the target type
    pub fn validate(&self) -> anyhow::Result<()> {
        match self.target_type {
            VocabularyType::Intent => {
                if self.alias.is_none() {
                    anyhow::bail!("Intent mapping requires 'alias' field");
                }
            }
            VocabularyType::Role => {
                if self.role.is_none() {
                    anyhow::bail!("Role mapping requires 'role' field");
                }
            }
            VocabularyType::Objective => {
                if self.objective.is_none() {
                    anyhow::bail!("Objective mapping requires 'objective' field");
                }
            }
            VocabularyType::Action => {
                if self.action.is_none() {
                    anyhow::bail!("Action mapping requires 'action' field");
                }
            }
            VocabularyType::Subject => {
                if self.subject.is_none() {
                    anyhow::bail!("Subject mapping requires 'subject' field");
                }
            }
            VocabularyType::Tracker => {
                anyhow::bail!("Cannot map to tracker (trackers are source data only)");
            }
        }
        Ok(())
    }

    /// Try to match this mapping against a source value
    ///
    /// Returns Some(MappingResult) if the pattern matches, None otherwise
    pub fn try_match(
        &self,
        source_value: &str,
        source_id: &str,
    ) -> anyhow::Result<Option<MappingResult>> {
        let regex = self.regex()?;

        if let Some(captures) = regex.captures(source_value) {
            let mut result = MappingResult {
                target_type: self.target_type.clone(),
                alias: None,
                role: None,
                objective: None,
                action: None,
                subject: None,
                trackers: None,
            };

            // Apply templates to each field
            if let Some(template) = &self.alias {
                result.alias = Some(Self::apply_template(
                    template,
                    &captures,
                    source_value,
                    source_id,
                )?);
            }
            if let Some(template) = &self.role {
                result.role = Some(Self::apply_template(
                    template,
                    &captures,
                    source_value,
                    source_id,
                )?);
            }
            if let Some(template) = &self.objective {
                result.objective = Some(Self::apply_template(
                    template,
                    &captures,
                    source_value,
                    source_id,
                )?);
            }
            if let Some(template) = &self.action {
                result.action = Some(Self::apply_template(
                    template,
                    &captures,
                    source_value,
                    source_id,
                )?);
            }
            if let Some(template) = &self.subject {
                result.subject = Some(Self::apply_template(
                    template,
                    &captures,
                    source_value,
                    source_id,
                )?);
            }
            if let Some(tracker_templates) = &self.trackers {
                let mut processed_trackers = Vec::new();
                for template in tracker_templates {
                    processed_trackers.push(Self::apply_template(
                        template,
                        &captures,
                        source_value,
                        source_id,
                    )?);
                }
                result.trackers = Some(processed_trackers);
            }

            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    /// Apply a template string, substituting captures and applying filters
    ///
    /// Template syntax:
    /// - {name} - substitute named capture
    /// - {name|filter} - substitute and apply filter
    /// - {name|filter1|filter2} - chain multiple filters
    /// - {original} - the original source value
    /// - {source_id} - the source/remote ID
    fn apply_template(
        template: &str,
        captures: &Captures,
        original_value: &str,
        source_id: &str,
    ) -> anyhow::Result<String> {
        // Find all {xxx} or {xxx|filter} patterns
        let placeholder_regex =
            Regex::new(r"\{([^}|]+)(\|[^}]+)?\}").expect("Placeholder regex is valid");

        let mut result = String::new();
        let mut last_end = 0;

        for cap in placeholder_regex.captures_iter(template) {
            let full_match = cap.get(0).unwrap();
            let range = full_match.range();

            // Add the part before this match
            result.push_str(&template[last_end..range.start]);

            let var_name = cap.get(1).unwrap().as_str();
            let filters = cap.get(2).map(|m| m.as_str().trim_start_matches('|'));

            // Get the value to substitute
            let value = match var_name {
                "original" => original_value.to_string(),
                "source_id" => source_id.to_string(),
                name => captures
                    .name(name)
                    .map(|m| m.as_str().to_string())
                    .ok_or_else(|| anyhow::anyhow!("Capture group '{}' not found", name))?,
            };

            // Apply filters if any
            let filtered_value = if let Some(filter_chain) = filters {
                Self::apply_filters(&value, filter_chain)?
            } else {
                value
            };

            // Add the substituted value
            result.push_str(&filtered_value);
            last_end = range.end;
        }

        // Add any remaining part after the last match
        result.push_str(&template[last_end..]);

        Ok(result)
    }

    /// Apply a chain of filters to a value
    ///
    /// Supported filters: slugify, lowercase, uppercase, trim
    fn apply_filters(value: &str, filter_chain: &str) -> anyhow::Result<String> {
        let mut result = value.to_string();

        for filter in filter_chain.split('|') {
            result = match filter.trim() {
                "slugify" => slugify(&result),
                "lowercase" => result.to_lowercase(),
                "uppercase" => result.to_uppercase(),
                "trim" => result.trim().to_string(),
                unknown => anyhow::bail!("Unknown filter: {}", unknown),
            };
        }

        Ok(result)
    }
}

/// Result of applying a vocabulary mapping
#[derive(Clone, Debug, PartialEq)]
pub struct MappingResult {
    /// Type of vocabulary that was generated
    pub target_type: VocabularyType,

    /// Generated alias (for Intent targets)
    pub alias: Option<String>,

    /// Generated role
    pub role: Option<String>,

    /// Generated objective
    pub objective: Option<String>,

    /// Generated action
    pub action: Option<String>,

    /// Generated subject
    pub subject: Option<String>,

    /// Generated trackers
    pub trackers: Option<Vec<String>>,
}

/// Static ASTRO vocabulary for a remote
///
/// These are source-scoped ASTRO objects that don't come from the remote API
/// but should be associated with this remote's source ID.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct RemoteVocabulary {
    /// Role identifiers (e.g., ["mycompany:engineer", "mycompany:lead"])
    #[serde(default)]
    pub roles: Vec<String>,

    /// Objective identifiers (e.g., ["mycompany:feature-dev"])
    #[serde(default)]
    pub objectives: Vec<String>,

    /// Action identifiers (e.g., ["mycompany:coding"])
    #[serde(default)]
    pub actions: Vec<String>,

    /// Subject identifiers (e.g., ["mycompany:api"])
    #[serde(default)]
    pub subjects: Vec<String>,
}

impl Remote {
    /// Create a new remote configuration
    pub fn new(id: impl Into<String>, plugin: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            plugin: plugin.into(),
            connection: HashMap::new(),
            vocabulary: RemoteVocabulary::default(),
            vocabulary_mappings: Vec::new(),
        }
    }

    /// Load remote from TOML string
    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
    }

    /// Serialize remote to TOML string
    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string(self)
    }

    /// Apply vocabulary mappings to a plan, augmenting its vocabulary
    ///
    /// This method:
    /// - Iterates through configured vocabulary mappings
    /// - Matches source vocabulary (trackers, roles, etc.) against patterns
    /// - Generates new vocabulary (intents, subjects, etc.) from matches
    /// - Returns an augmented plan with additional vocabulary
    ///
    /// The original plan vocabulary is preserved; mappings only add new items.
    ///
    /// If `previous_plan` is provided, intent IDs will be preserved for intents
    /// that have matching ASTRO properties, maintaining continuity across updates.
    pub fn apply_vocabulary_mappings(
        &self,
        plan: &crate::models::plan::Plan,
        previous_plan: Option<&crate::models::plan::Plan>,
    ) -> anyhow::Result<crate::models::plan::Plan> {
        use crate::models::intent::Intent;

        let mut augmented_plan = plan.clone();

        // Validate all mappings first
        for mapping in &self.vocabulary_mappings {
            mapping.validate()?;
        }

        // Apply each mapping
        for mapping in &self.vocabulary_mappings {
            // Get source values based on source_type
            // source_id is prefixed with plan.source so {source_id} expands to prefixed form
            let source_values: Vec<(String, String)> = match mapping.source_type {
                VocabularyType::Tracker => {
                    // For trackers: (source:tracker_id, tracker_description)
                    plan.trackers
                        .iter()
                        .map(|(id, desc)| (format!("{}:{}", plan.source, id), desc.clone()))
                        .collect()
                }
                VocabularyType::Role => {
                    // For roles: (source:role, role) - prefix ID, use role itself as value
                    plan.roles
                        .iter()
                        .map(|r| (format!("{}:{}", plan.source, r), r.clone()))
                        .collect()
                }
                VocabularyType::Objective => plan
                    .objectives
                    .iter()
                    .map(|o| (format!("{}:{}", plan.source, o), o.clone()))
                    .collect(),
                VocabularyType::Action => plan
                    .actions
                    .iter()
                    .map(|a| (format!("{}:{}", plan.source, a), a.clone()))
                    .collect(),
                VocabularyType::Subject => plan
                    .subjects
                    .iter()
                    .map(|s| (format!("{}:{}", plan.source, s), s.clone()))
                    .collect(),
                VocabularyType::Intent => {
                    // For intents, use the alias as the value to match against
                    // Filter out intents without an alias
                    plan.intents
                        .iter()
                        .filter_map(|i| {
                            i.alias
                                .as_ref()
                                .map(|alias| (i.intent_id.clone(), alias.clone()))
                        })
                        .collect()
                }
            };

            // Try to match each source value
            for (source_id, source_value) in source_values {
                if let Some(result) = mapping.try_match(&source_value, &source_id)? {
                    // Generate new vocabulary based on target_type
                    match result.target_type {
                        VocabularyType::Intent => {
                            // Generate a new intent
                            let alias = result.alias.ok_or_else(|| {
                                anyhow::anyhow!("Intent mapping must produce an alias")
                            })?;

                            let trackers = result.trackers.unwrap_or_default();

                            // Check if there's a matching intent in the previous plan to reuse its ID
                            let existing_id = previous_plan.and_then(|prev| {
                                prev.intents
                                    .iter()
                                    .find(|existing| {
                                        existing.alias.as_deref() == Some(&alias)
                                            && existing.role == result.role
                                            && existing.objective == result.objective
                                            && existing.action == result.action
                                            && existing.subject == result.subject
                                            && existing.trackers == trackers
                                    })
                                    .map(|i| i.intent_id.clone())
                            });

                            let intent = if let Some(id) = existing_id {
                                // Reuse existing ID to maintain continuity
                                Intent::new_with_id(
                                    Some(id),
                                    Some(alias),
                                    result.role,
                                    result.objective,
                                    result.action,
                                    result.subject,
                                    trackers,
                                )
                            } else {
                                // Create new intent (ID will be generated by add_intent)
                                Intent::new(
                                    Some(alias),
                                    result.role,
                                    result.objective,
                                    result.action,
                                    result.subject,
                                    trackers,
                                )
                            };

                            // Add to plan (using add_intent to handle ID generation and deduplication)
                            augmented_plan = augmented_plan.add_intent(intent);
                        }
                        VocabularyType::Role => {
                            let role = result.role.ok_or_else(|| {
                                anyhow::anyhow!("Role mapping must produce a role")
                            })?;
                            if !augmented_plan.roles.contains(&role) {
                                augmented_plan.roles.push(role);
                            }
                        }
                        VocabularyType::Objective => {
                            let objective = result.objective.ok_or_else(|| {
                                anyhow::anyhow!("Objective mapping must produce an objective")
                            })?;
                            if !augmented_plan.objectives.contains(&objective) {
                                augmented_plan.objectives.push(objective);
                            }
                        }
                        VocabularyType::Action => {
                            let action = result.action.ok_or_else(|| {
                                anyhow::anyhow!("Action mapping must produce an action")
                            })?;
                            if !augmented_plan.actions.contains(&action) {
                                augmented_plan.actions.push(action);
                            }
                        }
                        VocabularyType::Subject => {
                            let subject = result.subject.ok_or_else(|| {
                                anyhow::anyhow!("Subject mapping must produce a subject")
                            })?;
                            if !augmented_plan.subjects.contains(&subject) {
                                augmented_plan.subjects.push(subject);
                            }
                        }
                        VocabularyType::Tracker => {
                            // This shouldn't happen due to validation, but handle it
                            anyhow::bail!("Cannot map to tracker type");
                        }
                    }
                }
            }
        }

        Ok(augmented_plan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_remote() {
        let toml_str = r#"
            id = "mycompany"
            plugin = "myhours"
        "#;

        let remote = Remote::from_toml(toml_str).unwrap();
        assert_eq!(remote.id, "mycompany");
        assert_eq!(remote.plugin, "myhours");
        assert!(remote.connection.is_empty());
        assert!(remote.vocabulary.roles.is_empty());
    }

    #[test]
    fn test_full_remote() {
        let toml_str = r#"
            id = "mycompany"
            plugin = "myhours"

            [connection]
            email = "user@company.com"
            api_key = "secret123"
            base_url = "https://api.myhours.com"

            [vocabulary]
            roles = ["mycompany:engineer", "mycompany:lead"]
            objectives = ["mycompany:feature-dev", "mycompany:maintenance"]
            actions = ["mycompany:coding", "mycompany:reviewing"]
            subjects = ["mycompany:api", "mycompany:infrastructure"]
        "#;

        let remote = Remote::from_toml(toml_str).unwrap();
        assert_eq!(remote.id, "mycompany");
        assert_eq!(remote.plugin, "myhours");
        assert_eq!(remote.connection.len(), 3);
        assert_eq!(
            remote.connection.get("email").unwrap().as_str().unwrap(),
            "user@company.com"
        );
        assert_eq!(remote.vocabulary.roles.len(), 2);
        assert_eq!(remote.vocabulary.roles[0], "mycompany:engineer");
        assert_eq!(remote.vocabulary.objectives.len(), 2);
    }

    #[test]
    fn test_remote_new() {
        let remote = Remote::new("test", "myhours");
        assert_eq!(remote.id, "test");
        assert_eq!(remote.plugin, "myhours");
    }

    #[test]
    fn test_remote_roundtrip() {
        let mut remote = Remote::new("test", "toggl");
        remote.connection.insert(
            "api_token".to_string(),
            toml::Value::String("token123".to_string()),
        );
        remote.vocabulary.roles.push("test:developer".to_string());

        let toml_str = remote.to_toml().unwrap();
        let parsed = Remote::from_toml(&toml_str).unwrap();
        assert_eq!(remote, parsed);
    }

    #[test]
    fn test_vocabulary_mapping_parse() {
        let toml_str = r#"
            id = "test"
            plugin = "myhours"

            [[vocabulary_mapping]]
            source_type = "tracker"
            target_type = "intent"
            pattern = "^POC-(?P<id>\\d+)\\s+(?P<description>.+)$"
            alias = "POC-{id}: {description}"
            role = "element.io:pre-sales-engineer"
            objective = "element.io:new-revenue-new-business"
            action = "element.io:support-poc"
        "#;

        let remote = Remote::from_toml(toml_str).unwrap();
        assert_eq!(remote.vocabulary_mappings.len(), 1);

        let mapping = &remote.vocabulary_mappings[0];
        assert!(matches!(mapping.source_type, VocabularyType::Tracker));
        assert!(matches!(mapping.target_type, VocabularyType::Intent));
        assert_eq!(mapping.pattern, "^POC-(?P<id>\\d+)\\s+(?P<description>.+)$");
        assert_eq!(mapping.alias, Some("POC-{id}: {description}".to_string()));
    }

    #[test]
    fn test_template_substitution() {
        let mapping = VocabularyMapping::new(
            VocabularyType::Tracker,
            VocabularyType::Intent,
            r"^POC-(?P<id>\d+)\s+(?P<description>.+)$",
        );

        let result = mapping.try_match("POC-123 Test customer", "456").unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_template_with_filters() {
        let mut mapping = VocabularyMapping::new(
            VocabularyType::Tracker,
            VocabularyType::Subject,
            r"^Customer:\s+(?P<name>.+)$",
        );
        mapping.subject = Some("customer/{name|slugify}".to_string());

        let result = mapping.try_match("Customer: Acme Corp!", "123").unwrap();
        assert!(result.is_some());

        let result = result.unwrap();
        assert_eq!(result.subject, Some("customer/acme-corp".to_string()));
    }

    #[test]
    fn test_apply_vocabulary_mapping_tracker_to_intent() {
        use crate::models::plan::Plan;
        use chrono::NaiveDate;

        let mut remote = Remote::new("element", "myhours");

        let mut mapping = VocabularyMapping::new(
            VocabularyType::Tracker,
            VocabularyType::Intent,
            r"^POC-(?P<id>\d+)\s+(?P<description>.+)$",
        );
        mapping.alias = Some("POC-{id}: {description}".to_string());
        mapping.role = Some("element.io:pre-sales-engineer".to_string());
        mapping.objective = Some("element.io:new-revenue-new-business".to_string());
        mapping.action = Some("element.io:support-poc".to_string());

        remote.vocabulary_mappings.push(mapping);

        // Create a plan with a matching tracker
        let mut trackers = std::collections::HashMap::new();
        trackers.insert("123".to_string(), "POC-456 Acme Corporation".to_string());

        let plan = Plan::new(
            "element".to_string(),
            NaiveDate::from_ymd_opt(2025, 11, 4).unwrap(),
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            trackers,
            vec![],
        );

        let augmented = remote.apply_vocabulary_mappings(&plan, None).unwrap();

        // Check that an intent was generated
        assert_eq!(augmented.intents.len(), 1);
        let intent = &augmented.intents[0];
        assert_eq!(intent.alias, Some("POC-456: Acme Corporation".to_string()));
        assert_eq!(
            intent.role,
            Some("element.io:pre-sales-engineer".to_string())
        );
        assert_eq!(
            intent.objective,
            Some("element.io:new-revenue-new-business".to_string())
        );
        assert_eq!(intent.action, Some("element.io:support-poc".to_string()));
    }

    #[test]
    fn test_apply_vocabulary_mapping_tracker_to_subject() {
        use crate::models::plan::Plan;
        use chrono::NaiveDate;

        let mut remote = Remote::new("test", "test");

        let mut mapping = VocabularyMapping::new(
            VocabularyType::Tracker,
            VocabularyType::Subject,
            r"^Customer:\s+(?P<name>.+)$",
        );
        mapping.subject = Some("customer/{name|slugify}".to_string());

        remote.vocabulary_mappings.push(mapping);

        let mut trackers = std::collections::HashMap::new();
        trackers.insert("1".to_string(), "Customer: Acme Corp".to_string());

        let plan = Plan::new(
            "test".to_string(),
            NaiveDate::from_ymd_opt(2025, 11, 4).unwrap(),
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            trackers,
            vec![],
        );

        let augmented = remote.apply_vocabulary_mappings(&plan, None).unwrap();

        // Check that a subject was generated
        assert_eq!(augmented.subjects.len(), 1);
        assert_eq!(augmented.subjects[0], "customer/acme-corp");
    }

    #[test]
    fn test_vocabulary_mapping_no_match() {
        use crate::models::plan::Plan;
        use chrono::NaiveDate;

        let mut remote = Remote::new("test", "test");

        let mut mapping = VocabularyMapping::new(
            VocabularyType::Tracker,
            VocabularyType::Intent,
            r"^POC-(?P<id>\d+).*$",
        );
        mapping.alias = Some("POC-{id}".to_string());

        remote.vocabulary_mappings.push(mapping);

        let mut trackers = std::collections::HashMap::new();
        trackers.insert(
            "1".to_string(),
            "Something completely different".to_string(),
        );

        let plan = Plan::new(
            "test".to_string(),
            NaiveDate::from_ymd_opt(2025, 11, 4).unwrap(),
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            trackers,
            vec![],
        );

        let augmented = remote.apply_vocabulary_mappings(&plan, None).unwrap();

        // No intents should be generated since pattern doesn't match
        assert_eq!(augmented.intents.len(), 0);
    }

    #[test]
    fn test_real_world_poc_mapping() {
        use crate::models::plan::Plan;
        use chrono::NaiveDate;

        // Simulate the element.io remote configuration
        let mut remote = Remote::new("element", "myhours");

        let mut mapping = VocabularyMapping::new(
            VocabularyType::Tracker,
            VocabularyType::Intent,
            r"^POC-(?P<id>\d+)\s+(?P<description>.+)$",
        );
        mapping.alias = Some("POC-{id}: {description}".to_string());
        mapping.role = Some("customer-success-manager".to_string());
        mapping.objective = Some("new-revenue-new-business".to_string());
        mapping.action = Some("drive-poc".to_string());
        mapping.subject = Some("poc/{description|slugify}".to_string());

        remote.vocabulary_mappings.push(mapping);

        // Create a plan with real POC trackers from element.io
        let mut trackers = std::collections::HashMap::new();
        trackers.insert(
            "2679845".to_string(),
            "POC-29 European Commission - PoC".to_string(),
        );
        trackers.insert("2821521".to_string(), "POC-62 Unicredit POC".to_string());
        trackers.insert("2844066".to_string(), "POC-66 EPPO".to_string());
        trackers.insert(
            "2783059".to_string(),
            "BIZ-205 Experiment: Transactional Mid-Market Sales Motion".to_string(),
        );

        let plan = Plan::new(
            "element".to_string(),
            NaiveDate::from_ymd_opt(2025, 11, 4).unwrap(),
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            trackers,
            vec![],
        );

        let augmented = remote.apply_vocabulary_mappings(&plan, None).unwrap();

        // Should generate 3 intents from the 3 POC trackers
        assert_eq!(augmented.intents.len(), 3);

        // Find the POC-29 intent
        let poc29 = augmented
            .intents
            .iter()
            .find(|i| {
                i.alias
                    .as_ref()
                    .map(|a| a.contains("POC-29"))
                    .unwrap_or(false)
            })
            .expect("Should find POC-29 intent");

        assert_eq!(
            poc29.alias,
            Some("POC-29: European Commission - PoC".to_string())
        );
        assert_eq!(poc29.role, Some("customer-success-manager".to_string()));
        assert_eq!(
            poc29.objective,
            Some("new-revenue-new-business".to_string())
        );
        assert_eq!(poc29.action, Some("drive-poc".to_string()));
        assert_eq!(
            poc29.subject,
            Some("poc/european-commission-poc".to_string())
        );

        // Verify POC-62
        let poc62 = augmented
            .intents
            .iter()
            .find(|i| {
                i.alias
                    .as_ref()
                    .map(|a| a.contains("POC-62"))
                    .unwrap_or(false)
            })
            .expect("Should find POC-62 intent");

        assert_eq!(poc62.alias, Some("POC-62: Unicredit POC".to_string()));
        assert_eq!(poc62.subject, Some("poc/unicredit-poc".to_string()));

        // Verify POC-66
        let poc66 = augmented
            .intents
            .iter()
            .find(|i| {
                i.alias
                    .as_ref()
                    .map(|a| a.contains("POC-66"))
                    .unwrap_or(false)
            })
            .expect("Should find POC-66 intent");

        assert_eq!(poc66.alias, Some("POC-66: EPPO".to_string()));
        assert_eq!(poc66.subject, Some("poc/eppo".to_string()));

        // Verify that non-POC trackers are not converted
        assert!(!augmented.intents.iter().any(|i| i
            .alias
            .as_ref()
            .map(|a| a.contains("BIZ-"))
            .unwrap_or(false)));
    }

    #[test]
    fn test_vocabulary_mapping_tracker_field() {
        use crate::models::plan::Plan;
        use chrono::NaiveDate;

        let mut remote = Remote::new("element", "myhours");

        // Create a mapping that includes the tracker field
        let mut mapping = VocabularyMapping::new(
            VocabularyType::Tracker,
            VocabularyType::Intent,
            r"^Support - (?P<customer>.+)$",
        );
        mapping.alias = Some("Support {customer}".to_string());
        mapping.role = Some("customer-success-manager".to_string());
        mapping.objective = Some("retain-customer".to_string());
        mapping.action = Some("support".to_string());
        mapping.subject = Some("customer/{customer|slugify}".to_string());
        mapping.trackers = Some(vec!["{source_id}".to_string()]);

        remote.vocabulary_mappings.push(mapping);

        // Create a plan with a matching tracker (keys are unprefixed in plan files)
        let mut trackers = std::collections::HashMap::new();
        trackers.insert("12345".to_string(), "Support - Acme Corp".to_string());

        let plan = Plan::new(
            "element".to_string(),
            NaiveDate::from_ymd_opt(2025, 11, 12).unwrap(),
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            trackers,
            vec![],
        );

        let augmented = remote.apply_vocabulary_mappings(&plan, None).unwrap();

        // Check that an intent was generated with the tracker
        assert_eq!(augmented.intents.len(), 1);
        let intent = &augmented.intents[0];
        assert_eq!(intent.alias, Some("Support Acme Corp".to_string()));
        assert_eq!(intent.role, Some("customer-success-manager".to_string()));
        assert_eq!(intent.objective, Some("retain-customer".to_string()));
        assert_eq!(intent.action, Some("support".to_string()));
        assert_eq!(intent.subject, Some("customer/acme-corp".to_string()));

        // Verify the tracker ID is included
        assert_eq!(intent.trackers, vec!["element:12345"]);
    }
}
