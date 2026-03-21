use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use slug::slugify;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub source: String,
    pub valid_from: NaiveDate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<NaiveDate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub objectives: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subjects: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub trackers: HashMap<String, String>,
}

impl Plan {
    /// Constructor mirrors struct fields, so many arguments are unavoidable
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source: String,
        valid_from: NaiveDate,
        valid_until: Option<NaiveDate>,
        roles: Vec<String>,
        actions: Vec<String>,
        objectives: Vec<String>,
        subjects: Vec<String>,
        trackers: HashMap<String, String>,
    ) -> Self {
        Self {
            source,
            valid_from,
            valid_until,
            roles,
            actions,
            objectives,
            subjects,
            trackers,
        }
    }

    /// Generate a slug ID from the source
    pub fn id(&self) -> String {
        slugify(&self.source)
    }

    /// Serialize this Plan to TOML format
    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_minimal_plan() {
        let plan = Plan::new(
            "local".to_string(),
            NaiveDate::from_ymd_opt(2025, 3, 20).unwrap(),
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            HashMap::new(),
        );

        assert_eq!(plan.source, "local");
        assert_eq!(
            plan.valid_from,
            NaiveDate::from_ymd_opt(2025, 3, 20).unwrap()
        );
        assert_eq!(plan.valid_until, None);
        assert!(plan.roles.is_empty());
    }

    #[test]
    fn test_create_full_plan() {
        let mut trackers = HashMap::new();
        trackers.insert("work".to_string(), "id123".to_string());

        let plan = Plan::new(
            "https://example.com/plan".to_string(),
            NaiveDate::from_ymd_opt(2025, 3, 20).unwrap(),
            Some(NaiveDate::from_ymd_opt(2025, 4, 1).unwrap()),
            vec!["engineer".to_string()],
            vec!["coding".to_string()],
            vec!["development".to_string()],
            vec!["features".to_string()],
            trackers.clone(),
        );

        assert_eq!(plan.source, "https://example.com/plan");
        assert_eq!(
            plan.valid_until,
            Some(NaiveDate::from_ymd_opt(2025, 4, 1).unwrap())
        );
        assert_eq!(plan.roles, vec!["engineer"]);
        assert_eq!(plan.trackers, trackers);
    }

    #[test]
    fn test_id_from_simple_source() {
        let plan = Plan::new(
            "local".to_string(),
            NaiveDate::from_ymd_opt(2025, 3, 20).unwrap(),
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            HashMap::new(),
        );

        assert_eq!(plan.id(), "local");
    }

    #[test]
    fn test_id_from_url_source() {
        let plan = Plan::new(
            "https://example.com/my-plan".to_string(),
            NaiveDate::from_ymd_opt(2025, 3, 20).unwrap(),
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            HashMap::new(),
        );

        assert_eq!(plan.id(), "https-example-com-my-plan");
    }

    #[test]
    fn test_id_with_spaces() {
        let plan = Plan::new(
            "My Work Plan".to_string(),
            NaiveDate::from_ymd_opt(2025, 3, 20).unwrap(),
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            HashMap::new(),
        );

        assert_eq!(plan.id(), "my-work-plan");
    }

    #[test]
    fn test_plan_serialization() {
        let mut trackers = HashMap::new();
        trackers.insert("ABC-123".to_string(), "Fix critical bug".to_string());

        let plan = Plan::new(
            "local".to_string(),
            NaiveDate::from_ymd_opt(2025, 3, 20).unwrap(),
            Some(NaiveDate::from_ymd_opt(2025, 4, 1).unwrap()),
            vec!["engineer".to_string()],
            vec!["coding".to_string()],
            vec!["development".to_string()],
            vec!["features".to_string()],
            trackers,
        );

        let toml_str = plan.to_toml().unwrap();

        // Verify it contains expected fields
        assert!(toml_str.contains("source = \"local\""));
        assert!(toml_str.contains("valid_from = \"2025-03-20\""));
        assert!(toml_str.contains("valid_until = \"2025-04-01\""));
        assert!(toml_str.contains("roles"));
        assert!(toml_str.contains("engineer"));
    }

    #[test]
    fn test_plan_deserialization() {
        let toml_str = r#"
source = "local"
valid_from = "2025-03-20"
valid_until = "2025-04-01"
roles = ["engineer"]
actions = ["coding"]
objectives = ["development"]
subjects = ["features"]

[trackers]
"ABC-123" = "Fix critical bug"
"#;

        let plan: Plan = toml::from_str(toml_str).unwrap();

        assert_eq!(plan.source, "local");
        assert_eq!(
            plan.valid_from,
            NaiveDate::from_ymd_opt(2025, 3, 20).unwrap()
        );
        assert_eq!(
            plan.valid_until,
            Some(NaiveDate::from_ymd_opt(2025, 4, 1).unwrap())
        );
        assert_eq!(plan.roles, vec!["engineer"]);
        assert_eq!(
            plan.trackers.get("ABC-123"),
            Some(&"Fix critical bug".to_string())
        );
    }

    #[test]
    fn test_plan_deserialization_ignores_intents() {
        // Old plan files with [[intents]] should silently be ignored
        let toml_str = r#"
source = "local"
valid_from = "2025-03-20"
roles = ["engineer"]

[[intents]]
alias = "work"
role = "engineer"
"#;

        let plan: Plan = toml::from_str(toml_str).unwrap();

        assert_eq!(plan.source, "local");
        assert_eq!(plan.roles, vec!["engineer"]);
        // intents are silently ignored
    }

    #[test]
    fn test_plan_roundtrip_serialization() {
        let original = Plan::new(
            "local".to_string(),
            NaiveDate::from_ymd_opt(2025, 3, 20).unwrap(),
            None,
            vec!["engineer".to_string()],
            vec!["coding".to_string()],
            vec!["development".to_string()],
            vec!["features".to_string()],
            HashMap::new(),
        );

        let toml_str = original.to_toml().unwrap();
        let deserialized: Plan = toml::from_str(&toml_str).unwrap();

        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_plan_with_valid_until() {
        let plan = Plan::new(
            "local".to_string(),
            NaiveDate::from_ymd_opt(2025, 3, 20).unwrap(),
            Some(NaiveDate::from_ymd_opt(2025, 4, 1).unwrap()),
            vec![],
            vec![],
            vec![],
            vec![],
            HashMap::new(),
        );

        assert_eq!(
            plan.valid_until,
            Some(NaiveDate::from_ymd_opt(2025, 4, 1).unwrap())
        );
    }

    #[test]
    fn test_plan_empty_collections_omitted_in_toml() {
        let plan = Plan::new(
            "local".to_string(),
            NaiveDate::from_ymd_opt(2025, 3, 20).unwrap(),
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            HashMap::new(),
        );

        let toml_str = plan.to_toml().unwrap();

        // Empty collections should be omitted
        assert!(!toml_str.contains("roles = []"));
        assert!(!toml_str.contains("actions = []"));
        assert!(!toml_str.contains("objectives = []"));
        assert!(!toml_str.contains("subjects = []"));
        // But source and valid_from should be present
        assert!(toml_str.contains("source"));
        assert!(toml_str.contains("valid_from"));
    }

    #[test]
    fn test_plan_clone() {
        let plan = Plan::new(
            "local".to_string(),
            NaiveDate::from_ymd_opt(2025, 3, 20).unwrap(),
            None,
            vec!["engineer".to_string()],
            vec![],
            vec![],
            vec![],
            HashMap::new(),
        );

        let cloned = plan.clone();

        assert_eq!(plan, cloned);
        assert_eq!(plan.source, cloned.source);
        assert_eq!(plan.roles, cloned.roles);
    }

    #[test]
    fn test_plan_with_multiple_trackers() {
        let mut trackers = HashMap::new();
        trackers.insert("ABC-123".to_string(), "Bug fix".to_string());
        trackers.insert("DEF-456".to_string(), "Feature request".to_string());
        trackers.insert("GHI-789".to_string(), "Refactoring".to_string());

        let plan = Plan::new(
            "project".to_string(),
            NaiveDate::from_ymd_opt(2025, 3, 20).unwrap(),
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            trackers.clone(),
        );

        assert_eq!(plan.trackers.len(), 3);
        assert_eq!(plan.trackers.get("ABC-123"), Some(&"Bug fix".to_string()));
        assert_eq!(
            plan.trackers.get("DEF-456"),
            Some(&"Feature request".to_string())
        );
        assert_eq!(
            plan.trackers.get("GHI-789"),
            Some(&"Refactoring".to_string())
        );
    }
}
