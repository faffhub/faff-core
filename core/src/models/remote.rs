use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

use regex::{Captures, Regex};
use slug::slugify;

/// Compiled once; used in apply_template to find `{name}` / `{name|filter}` placeholders.
static PLACEHOLDER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{([^}|]+)(\|[^}]+)?\}").expect("PLACEHOLDER_REGEX pattern is valid")
});

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

    /// Static session field vocabulary for this remote
    /// Used when the remote doesn't provide its own session field objects
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
    #[serde(alias = "objective")]
    Impact,
    #[serde(alias = "action")]
    Mode,
    Subject,
}

/// Configuration for mapping vocabulary from one type to another
///
/// Uses regex patterns to match source vocabulary and templates to generate
/// target vocabulary. Supports mapping any vocabulary type to any other type.
///
/// Examples:
/// - tracker → subject: Extract customer name from tracker
/// - tracker → role: Derive role from tracker pattern
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VocabularyMapping {
    /// Type of vocabulary to match against
    pub source_type: VocabularyType,

    /// Type of vocabulary to generate
    pub target_type: VocabularyType,

    /// Regex pattern to match source values (with named capture groups)
    /// Example: "^POC-(?P<id>\\d+)\\s+(?P<description>.+)$"
    pub pattern: String,

    /// Template for role (required if target_type is Role)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,

    /// Template for impact (required if target_type is Impact)
    #[serde(skip_serializing_if = "Option::is_none", alias = "objective")]
    pub impact: Option<String>,

    /// Template for mode (required if target_type is Mode)
    #[serde(skip_serializing_if = "Option::is_none", alias = "action")]
    pub mode: Option<String>,

    /// Template for subject (required if target_type is Subject)
    /// Supports filters: "customer/{customer|slugify}"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,

    /// Template for the hint title (used in session start suggestions)
    /// Defaults to the raw source value (e.g. tracker description) if not set.
    /// Example: "Support {customer}" instead of "Support - Acme Corp"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Templates for trackers
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
            role: None,
            impact: None,
            mode: None,
            subject: None,
            title: None,
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
            VocabularyType::Role => {
                if self.role.is_none() {
                    anyhow::bail!("Role mapping requires 'role' field");
                }
            }
            VocabularyType::Impact => {
                if self.impact.is_none() {
                    anyhow::bail!("Impact mapping requires 'impact' field");
                }
            }
            VocabularyType::Mode => {
                if self.mode.is_none() {
                    anyhow::bail!("Mode mapping requires 'mode' field");
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
                role: None,
                impact: None,
                mode: None,
                subject: None,
                trackers: None,
            };

            // Apply templates to each field
            if let Some(template) = &self.role {
                result.role = Some(Self::apply_template(
                    template,
                    &captures,
                    source_value,
                    source_id,
                )?);
            }
            if let Some(template) = &self.impact {
                result.impact = Some(Self::apply_template(
                    template,
                    &captures,
                    source_value,
                    source_id,
                )?);
            }
            if let Some(template) = &self.mode {
                result.mode = Some(Self::apply_template(
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
    pub(super) fn apply_template(
        template: &str,
        captures: &Captures,
        original_value: &str,
        source_id: &str,
    ) -> anyhow::Result<String> {
        let mut result = String::new();
        let mut last_end = 0;

        for cap in PLACEHOLDER_REGEX.captures_iter(template) {
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

/// A resolved tracker mapping entry for reverse lookup
///
/// Maps specific session field values back to a tracker, enabling auto-derivation
/// of trackers from session fields at start time.
#[derive(Clone, Debug, PartialEq)]
pub struct TrackerMapping {
    /// Prefixed tracker ID (e.g., "element:1231232")
    pub tracker_id: String,
    /// Human-readable tracker name (raw tracker description)
    pub tracker_name: String,
    /// Hint title to use in session suggestions (may differ from tracker_name
    /// when a `title` template is configured on the vocabulary mapping)
    pub hint_title: String,
    /// Required role value for this mapping (if any)
    pub role: Option<String>,
    /// Required impact value for this mapping (if any)
    pub impact: Option<String>,
    /// Required mode value for this mapping (if any)
    pub mode: Option<String>,
    /// Required subject value for this mapping (if any)
    pub subject: Option<String>,
}

impl TrackerMapping {
    /// Check if this mapping matches the given session field values
    ///
    /// Returns true if every non-None field on the mapping matches the corresponding
    /// session value. None fields on the mapping are wildcards (match anything).
    pub fn matches_session(
        &self,
        role: Option<&str>,
        subject: Option<&str>,
        impact: Option<&str>,
        mode: Option<&str>,
    ) -> bool {
        if let Some(required) = &self.role {
            if role != Some(required.as_str()) {
                return false;
            }
        }
        if let Some(required) = &self.subject {
            if subject != Some(required.as_str()) {
                return false;
            }
        }
        if let Some(required) = &self.impact {
            if impact != Some(required.as_str()) {
                return false;
            }
        }
        if let Some(required) = &self.mode {
            if mode != Some(required.as_str()) {
                return false;
            }
        }
        true
    }
}

/// Result of applying a vocabulary mapping
#[derive(Clone, Debug, PartialEq)]
pub struct MappingResult {
    /// Type of vocabulary that was generated
    pub target_type: VocabularyType,

    /// Generated role
    pub role: Option<String>,

    /// Generated impact
    pub impact: Option<String>,

    /// Generated mode
    pub mode: Option<String>,

    /// Generated subject
    pub subject: Option<String>,

    /// Generated trackers
    pub trackers: Option<Vec<String>>,
}

/// Static session field vocabulary for a remote
///
/// These are source-scoped session field objects that don't come from the remote API
/// but should be associated with this remote's source ID.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct RemoteVocabulary {
    /// Role identifiers (e.g., ["mycompany:engineer", "mycompany:lead"])
    #[serde(default)]
    pub roles: Vec<String>,

    /// Impact identifiers (e.g., ["mycompany:feature-dev"])
    #[serde(default, alias = "objectives")]
    pub impacts: Vec<String>,

    /// Mode identifiers (e.g., ["mycompany:coding"])
    #[serde(default, alias = "actions")]
    pub modes: Vec<String>,

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

    /// Generate tracker mappings by running all tracker-source vocabulary mappings
    ///
    /// For each tracker in the plan, tries all vocabulary mappings where `source_type == Tracker`.
    /// For each match, collects all non-None output fields (role, impact, mode, subject) into a
    /// `TrackerMapping` entry. This builds a reverse lookup index from session field values to
    /// tracker IDs, used for auto-deriving trackers at session start time.
    pub fn generate_tracker_mappings(
        &self,
        plan: &crate::models::plan::Plan,
    ) -> anyhow::Result<Vec<TrackerMapping>> {
        let mut mappings = Vec::new();

        for mapping in &self.vocabulary_mappings {
            if mapping.source_type != VocabularyType::Tracker {
                continue;
            }

            // Compile regex once per mapping, not once per tracker
            let regex = mapping.regex()?;

            for (tracker_key, tracker_desc) in &plan.trackers {
                let tracker_id = format!("{}:{}", plan.source, tracker_key);

                let Some(captures) = regex.captures(tracker_desc) else {
                    continue;
                };

                // Build result from captures (inline try_match logic to reuse captures)
                let result = {
                    let mut r = MappingResult {
                        target_type: mapping.target_type.clone(),
                        role: None,
                        impact: None,
                        mode: None,
                        subject: None,
                        trackers: None,
                    };
                    if let Some(t) = &mapping.role {
                        r.role = Some(VocabularyMapping::apply_template(
                            t,
                            &captures,
                            tracker_desc,
                            &tracker_id,
                        )?);
                    }
                    if let Some(t) = &mapping.impact {
                        r.impact = Some(VocabularyMapping::apply_template(
                            t,
                            &captures,
                            tracker_desc,
                            &tracker_id,
                        )?);
                    }
                    if let Some(t) = &mapping.mode {
                        r.mode = Some(VocabularyMapping::apply_template(
                            t,
                            &captures,
                            tracker_desc,
                            &tracker_id,
                        )?);
                    }
                    if let Some(t) = &mapping.subject {
                        r.subject = Some(VocabularyMapping::apply_template(
                            t,
                            &captures,
                            tracker_desc,
                            &tracker_id,
                        )?);
                    }
                    r
                };

                // Only create a TrackerMapping if there's at least one field constraint
                if result.role.is_none()
                    && result.impact.is_none()
                    && result.mode.is_none()
                    && result.subject.is_none()
                {
                    continue;
                }

                let qualify = |v: String| -> String {
                    if v.contains(':') {
                        v
                    } else {
                        format!("{}:{}", plan.source, v)
                    }
                };

                // Reuse captures for hint title (no second regex.captures() call)
                let hint_title = if let Some(title_template) = &mapping.title {
                    VocabularyMapping::apply_template(
                        title_template,
                        &captures,
                        tracker_desc,
                        &tracker_id,
                    )?
                } else {
                    tracker_desc.clone()
                };

                mappings.push(TrackerMapping {
                    tracker_id,
                    tracker_name: tracker_desc.clone(),
                    hint_title,
                    role: result.role.map(&qualify),
                    impact: result.impact.map(&qualify),
                    mode: result.mode.map(&qualify),
                    subject: result.subject.map(&qualify),
                });
            }
        }

        Ok(mappings)
    }

    /// Apply vocabulary mappings to a plan, augmenting its vocabulary
    ///
    /// This method:
    /// - Iterates through configured vocabulary mappings
    /// - Matches source vocabulary (trackers, roles, etc.) against patterns
    /// - Generates new vocabulary (subjects, roles, etc.) from matches
    /// - Returns an augmented plan with additional vocabulary
    ///
    /// The original plan vocabulary is preserved; mappings only add new items.
    pub fn apply_vocabulary_mappings(
        &self,
        plan: &crate::models::plan::Plan,
        _previous_plan: Option<&crate::models::plan::Plan>,
    ) -> anyhow::Result<crate::models::plan::Plan> {
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
                VocabularyType::Impact => plan
                    .impacts
                    .iter()
                    .map(|o| (format!("{}:{}", plan.source, o), o.clone()))
                    .collect(),
                VocabularyType::Mode => plan
                    .modes
                    .iter()
                    .map(|a| (format!("{}:{}", plan.source, a), a.clone()))
                    .collect(),
                VocabularyType::Subject => plan
                    .subjects
                    .iter()
                    .map(|s| (format!("{}:{}", plan.source, s), s.clone()))
                    .collect(),
            };

            // Try to match each source value
            for (source_id, source_value) in source_values {
                if let Some(result) = mapping.try_match(&source_value, &source_id)? {
                    // For tracker-source mappings with multi-field results, generate a
                    // SessionHint so the CLI can pre-weight field suggestions and
                    // auto-derive the tracker. Fields are source-prefixed to match
                    // the qualified values that session prompts use.
                    if mapping.source_type == VocabularyType::Tracker {
                        let qualify = |v: String| -> String {
                            if v.contains(':') {
                                v
                            } else {
                                format!("{}:{}", plan.source, v)
                            }
                        };
                        let hint_role = result.role.clone().map(&qualify);
                        let hint_impact = result.impact.clone().map(&qualify);
                        let hint_mode = result.mode.clone().map(&qualify);
                        let hint_subject = result.subject.clone().map(&qualify);

                        if hint_role.is_some()
                            || hint_impact.is_some()
                            || hint_mode.is_some()
                            || hint_subject.is_some()
                        {
                            if !augmented_plan
                                .hints
                                .iter()
                                .any(|h| h.trackers.contains(&source_id))
                            {
                                // Use the title template if set, otherwise fall back to
                                // the raw tracker description.
                                let hint_title = if let Some(title_template) = &mapping.title {
                                    let regex = mapping.regex()?;
                                    regex
                                        .captures(&source_value)
                                        .map(|caps| {
                                            VocabularyMapping::apply_template(
                                                title_template,
                                                &caps,
                                                &source_value,
                                                &source_id,
                                            )
                                        })
                                        .transpose()?
                                        .unwrap_or_else(|| source_value.clone())
                                } else {
                                    source_value.clone()
                                };
                                augmented_plan.hints.push(crate::models::plan::SessionHint {
                                    title: hint_title,
                                    role: hint_role,
                                    impact: hint_impact,
                                    mode: hint_mode,
                                    subject: hint_subject,
                                    trackers: vec![source_id.clone()],
                                });
                            }
                        }
                    }

                    // Generate new vocabulary based on target_type
                    match result.target_type {
                        VocabularyType::Role => {
                            let role = result.role.ok_or_else(|| {
                                anyhow::anyhow!("Role mapping must produce a role")
                            })?;
                            if !augmented_plan.roles.contains(&role) {
                                augmented_plan.roles.push(role);
                            }
                        }
                        VocabularyType::Impact => {
                            let impact = result.impact.ok_or_else(|| {
                                anyhow::anyhow!("Impact mapping must produce an impact")
                            })?;
                            if !augmented_plan.impacts.contains(&impact) {
                                augmented_plan.impacts.push(impact);
                            }
                        }
                        VocabularyType::Mode => {
                            let mode = result.mode.ok_or_else(|| {
                                anyhow::anyhow!("Mode mapping must produce a mode")
                            })?;
                            if !augmented_plan.modes.contains(&mode) {
                                augmented_plan.modes.push(mode);
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
        assert_eq!(remote.vocabulary.impacts.len(), 2);
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
    fn test_template_substitution() {
        let mut mapping = VocabularyMapping::new(
            VocabularyType::Tracker,
            VocabularyType::Subject,
            r"^POC-(?P<id>\d+)\s+(?P<description>.+)$",
        );
        mapping.subject = Some("poc/{description|slugify}".to_string());

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
            VocabularyType::Subject,
            r"^POC-(?P<id>\d+).*$",
        );
        mapping.subject = Some("poc/{id}".to_string());

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
        );

        let augmented = remote.apply_vocabulary_mappings(&plan, None).unwrap();

        // No subjects should be generated since pattern doesn't match
        assert_eq!(augmented.subjects.len(), 0);
    }

    #[test]
    fn test_apply_vocabulary_mapping_tracker_to_role() {
        use crate::models::plan::Plan;
        use chrono::NaiveDate;

        let mut remote = Remote::new("element", "myhours");

        let mut mapping = VocabularyMapping::new(
            VocabularyType::Tracker,
            VocabularyType::Role,
            r"^POC-(?P<id>\d+)\s+(?P<description>.+)$",
        );
        mapping.role = Some("element.io:pre-sales-engineer".to_string());

        remote.vocabulary_mappings.push(mapping);

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
        );

        let augmented = remote.apply_vocabulary_mappings(&plan, None).unwrap();

        // Check that a role was generated
        assert_eq!(augmented.roles.len(), 1);
        assert_eq!(augmented.roles[0], "element.io:pre-sales-engineer");
    }

    #[test]
    fn test_real_world_poc_mapping() {
        use crate::models::plan::Plan;
        use chrono::NaiveDate;

        // Simulate an element.io remote configuration mapping trackers to subjects
        let mut remote = Remote::new("element", "myhours");

        let mut mapping = VocabularyMapping::new(
            VocabularyType::Tracker,
            VocabularyType::Subject,
            r"^POC-(?P<id>\d+)\s+(?P<description>.+)$",
        );
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
        );

        let augmented = remote.apply_vocabulary_mappings(&plan, None).unwrap();

        // Should generate 3 subjects from the 3 POC trackers
        assert_eq!(augmented.subjects.len(), 3);

        // Verify subjects were generated correctly
        assert!(augmented
            .subjects
            .contains(&"poc/european-commission-poc".to_string()));
        assert!(augmented
            .subjects
            .contains(&"poc/unicredit-poc".to_string()));
        assert!(augmented.subjects.contains(&"poc/eppo".to_string()));

        // Verify that non-POC trackers are not converted
        assert!(!augmented.subjects.iter().any(|s| s.contains("experiment")));
    }

    #[test]
    fn test_generate_tracker_mappings_multi_field() {
        use crate::models::plan::Plan;
        use chrono::NaiveDate;

        let mut remote = Remote::new("element", "myhours");

        // A multi-field mapping: tracker → subject, with extra role field
        let mut mapping = VocabularyMapping::new(
            VocabularyType::Tracker,
            VocabularyType::Subject,
            r"^POC-(?P<id>\d+)\s+(?P<description>.+)$",
        );
        mapping.subject = Some("poc/{description|slugify}".to_string());
        mapping.role = Some("element:pre-sales-engineer".to_string());

        remote.vocabulary_mappings.push(mapping);

        let mut trackers = std::collections::HashMap::new();
        trackers.insert("1231232".to_string(), "POC-123 Acme Corp".to_string());
        trackers.insert("9999".to_string(), "Support: Other".to_string());

        let plan = Plan::new(
            "element".to_string(),
            NaiveDate::from_ymd_opt(2025, 11, 4).unwrap(),
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            trackers,
        );

        let mappings = remote.generate_tracker_mappings(&plan).unwrap();

        // Only POC tracker matches
        assert_eq!(mappings.len(), 1);
        let m = &mappings[0];
        assert_eq!(m.tracker_id, "element:1231232");
        assert_eq!(m.tracker_name, "POC-123 Acme Corp");
        // Both fields are prefixed with plan.source ("element") to match session values
        assert_eq!(m.role, Some("element:pre-sales-engineer".to_string()));
        assert_eq!(m.subject, Some("element:poc/acme-corp".to_string()));
        assert!(m.impact.is_none());
        assert!(m.mode.is_none());
    }

    #[test]
    fn test_tracker_mapping_matches_session() {
        let mapping = TrackerMapping {
            tracker_id: "element:1231232".to_string(),
            tracker_name: "POC-123 Acme Corp".to_string(),
            hint_title: "POC-123 Acme Corp".to_string(),
            role: Some("element:pre-sales-engineer".to_string()),
            subject: Some("element:poc/acme-corp".to_string()),
            impact: None,
            mode: None,
        };

        // Exact match
        assert!(mapping.matches_session(
            Some("element:pre-sales-engineer"),
            Some("element:poc/acme-corp"),
            None,
            None,
        ));

        // Wrong role
        assert!(!mapping.matches_session(
            Some("element:engineer"),
            Some("element:poc/acme-corp"),
            None,
            None,
        ));

        // Wrong subject
        assert!(!mapping.matches_session(
            Some("element:pre-sales-engineer"),
            Some("element:poc/other"),
            None,
            None,
        ));

        // Role missing (session has no role)
        assert!(!mapping.matches_session(None, Some("element:poc/acme-corp"), None, None));

        // Impact/mode don't matter (they're None on the mapping)
        assert!(mapping.matches_session(
            Some("element:pre-sales-engineer"),
            Some("element:poc/acme-corp"),
            Some("element:revenue"),
            Some("element:meeting"),
        ));
    }

    #[test]
    fn test_generate_tracker_mappings_no_tracker_source() {
        use crate::models::plan::Plan;
        use chrono::NaiveDate;

        let mut remote = Remote::new("element", "myhours");

        // Non-tracker source mapping — should not generate tracker mappings
        let mut mapping =
            VocabularyMapping::new(VocabularyType::Role, VocabularyType::Subject, r"^engineer$");
        mapping.subject = Some("engineering".to_string());
        remote.vocabulary_mappings.push(mapping);

        let plan = Plan::new(
            "element".to_string(),
            NaiveDate::from_ymd_opt(2025, 11, 4).unwrap(),
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            std::collections::HashMap::new(),
        );

        let mappings = remote.generate_tracker_mappings(&plan).unwrap();
        assert!(mappings.is_empty());
    }
}
