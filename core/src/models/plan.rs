use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use slug::slugify;
use std::collections::HashMap;

use crate::models::intent::Intent;

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub intents: Vec<Intent>,
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
        intents: Vec<Intent>,
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
            intents,
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

    /// Add an intent to the plan, deduplicating if it already exists
    ///
    /// If the intent doesn't have an ID, checks for an existing intent with matching ASTRO
    /// properties and reuses its ID to maintain continuity. Only generates a new ID if no
    /// matching intent exists.
    pub fn add_intent(&self, mut intent: Intent) -> Plan {
        let mut new_intents = self.intents.clone();

        // Check if there's an existing intent with matching ASTRO properties (ignoring ID)
        if let Some(existing) = new_intents.iter().find(|existing| {
            existing.alias == intent.alias
                && existing.role == intent.role
                && existing.objective == intent.objective
                && existing.action == intent.action
                && existing.subject == intent.subject
                && existing.trackers == intent.trackers
        }) {
            // Reuse the existing intent's ID to maintain continuity
            intent.intent_id = existing.intent_id.clone();
        } else if intent.intent_id.is_empty() {
            // Generate a new ID only if no matching intent exists
            intent.intent_id = Intent::generate_intent_id(&self.source, self.valid_from);
        }

        // Only add if not already present (by ID now)
        if !new_intents.iter().any(|i| i.intent_id == intent.intent_id) {
            new_intents.push(intent);
        }

        Plan {
            source: self.source.clone(),
            valid_from: self.valid_from,
            valid_until: self.valid_until,
            roles: self.roles.clone(),
            actions: self.actions.clone(),
            objectives: self.objectives.clone(),
            subjects: self.subjects.clone(),
            trackers: self.trackers.clone(),
            intents: new_intents,
        }
    }

    /// Update an existing intent in the plan
    ///
    /// Finds the intent by ID and replaces it with the updated version.
    /// Returns an error if the intent is not found.
    pub fn update_intent(&self, intent_id: &str, updated_intent: Intent) -> anyhow::Result<Plan> {
        let mut new_intents = self.intents.clone();

        // Find the intent and update it
        let found = new_intents.iter_mut().any(|intent| {
            if intent.intent_id == intent_id {
                *intent = updated_intent.clone();
                true
            } else {
                false
            }
        });

        if !found {
            anyhow::bail!("Intent with ID '{}' not found in plan", intent_id);
        }

        Ok(Plan {
            source: self.source.clone(),
            valid_from: self.valid_from,
            valid_until: self.valid_until,
            roles: self.roles.clone(),
            actions: self.actions.clone(),
            objectives: self.objectives.clone(),
            subjects: self.subjects.clone(),
            trackers: self.trackers.clone(),
            intents: new_intents,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_intent() -> Intent {
        Intent::new(
            Some("work".to_string()),
            Some("engineer".to_string()),
            Some("development".to_string()),
            Some("coding".to_string()),
            Some("features".to_string()),
            vec![],
        )
    }

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
            vec![],
        );

        assert_eq!(plan.source, "local");
        assert_eq!(
            plan.valid_from,
            NaiveDate::from_ymd_opt(2025, 3, 20).unwrap()
        );
        assert_eq!(plan.valid_until, None);
        assert!(plan.roles.is_empty());
        assert!(plan.intents.is_empty());
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
            vec![],
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
            vec![],
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
            vec![],
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
            vec![],
        );

        assert_eq!(plan.id(), "my-work-plan");
    }

    #[test]
    fn test_add_intent_to_empty_plan() {
        let plan = Plan::new(
            "local".to_string(),
            NaiveDate::from_ymd_opt(2025, 3, 20).unwrap(),
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            HashMap::new(),
            vec![],
        );

        let intent = sample_intent();
        let new_plan = plan.add_intent(intent.clone());

        assert_eq!(new_plan.intents.len(), 1);
        // The added intent should have the same ASTRO values but a generated ID
        assert_eq!(new_plan.intents[0].alias, intent.alias);
        assert_eq!(new_plan.intents[0].role, intent.role);
        assert_eq!(new_plan.intents[0].objective, intent.objective);
        assert_eq!(new_plan.intents[0].action, intent.action);
        assert_eq!(new_plan.intents[0].subject, intent.subject);
        assert!(
            !new_plan.intents[0].intent_id.is_empty(),
            "Intent should have a generated ID"
        );
        assert!(
            new_plan.intents[0].intent_id.starts_with("local:i-"),
            "Intent ID should have correct prefix"
        );
        // Original unchanged
        assert_eq!(plan.intents.len(), 0);
    }

    #[test]
    fn test_add_intent_to_plan_with_intents() {
        // Create intent1 with an ID so it can be compared
        let intent1 = Intent::new_with_id(
            Some(Intent::generate_intent_id(
                "local",
                NaiveDate::from_ymd_opt(2025, 3, 20).unwrap(),
            )),
            Some("work".to_string()),
            Some("engineer".to_string()),
            Some("development".to_string()),
            Some("coding".to_string()),
            Some("features".to_string()),
            vec![],
        );
        let plan = Plan::new(
            "local".to_string(),
            NaiveDate::from_ymd_opt(2025, 3, 20).unwrap(),
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            HashMap::new(),
            vec![intent1.clone()],
        );

        let intent2 = Intent::new(
            Some("review".to_string()),
            Some("manager".to_string()),
            Some("quality".to_string()),
            Some("reviewing".to_string()),
            Some("code".to_string()),
            vec![],
        );

        let new_plan = plan.add_intent(intent2.clone());

        assert_eq!(new_plan.intents.len(), 2);
        assert!(new_plan.intents.contains(&intent1));
        // Check that second intent exists with correct ASTRO values
        let added_intent2 = new_plan
            .intents
            .iter()
            .find(|i| i.alias.as_deref() == Some("review"))
            .unwrap();
        assert_eq!(added_intent2.role, intent2.role);
        assert_eq!(added_intent2.objective, intent2.objective);
        assert_eq!(added_intent2.action, intent2.action);
        assert_eq!(added_intent2.subject, intent2.subject);
        assert!(
            !added_intent2.intent_id.is_empty(),
            "Intent should have a generated ID"
        );
    }

    #[test]
    fn test_add_duplicate_intent_deduplicates() {
        // Create intent with an ID so deduplication can work
        let intent = Intent::new_with_id(
            Some(Intent::generate_intent_id(
                "local",
                NaiveDate::from_ymd_opt(2025, 3, 20).unwrap(),
            )),
            Some("work".to_string()),
            Some("engineer".to_string()),
            Some("development".to_string()),
            Some("coding".to_string()),
            Some("features".to_string()),
            vec![],
        );
        let plan = Plan::new(
            "local".to_string(),
            NaiveDate::from_ymd_opt(2025, 3, 20).unwrap(),
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            HashMap::new(),
            vec![intent.clone()],
        );

        let new_plan = plan.add_intent(intent);

        // Should still only have 1 intent (deduplicated)
        assert_eq!(new_plan.intents.len(), 1);
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
            vec![sample_intent()],
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

[[intents]]
alias = "work"
role = "engineer"
objective = "development"
action = "coding"
subject = "features"
trackers = []
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
        assert_eq!(plan.intents.len(), 1);
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
            vec![sample_intent()],
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
            vec![],
        );

        assert_eq!(
            plan.valid_until,
            Some(NaiveDate::from_ymd_opt(2025, 4, 1).unwrap())
        );
    }

    #[test]
    fn test_plan_immutability() {
        let plan = Plan::new(
            "local".to_string(),
            NaiveDate::from_ymd_opt(2025, 3, 20).unwrap(),
            None,
            vec![],
            vec![],
            vec![],
            vec![],
            HashMap::new(),
            vec![],
        );

        let intent = sample_intent();
        let new_plan = plan.add_intent(intent);

        // Original plan should be unchanged
        assert_eq!(plan.intents.len(), 0);
        // New plan should have the intent
        assert_eq!(new_plan.intents.len(), 1);
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
            vec![],
        );

        let toml_str = plan.to_toml().unwrap();

        // Empty collections should be omitted
        assert!(!toml_str.contains("roles = []"));
        assert!(!toml_str.contains("actions = []"));
        assert!(!toml_str.contains("objectives = []"));
        assert!(!toml_str.contains("subjects = []"));
        assert!(!toml_str.contains("intents = []"));
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
            vec![],
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
            vec![],
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
