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
    pub fn add_intent(&self, intent: Intent) -> Plan {
        let mut new_intents = self.intents.clone();

        // Only add if not already present (deduplication)
        if !new_intents.contains(&intent) {
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
        assert_eq!(new_plan.intents[0], intent);
        // Original unchanged
        assert_eq!(plan.intents.len(), 0);
    }

    #[test]
    fn test_add_intent_to_plan_with_intents() {
        let intent1 = sample_intent();
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
        assert!(new_plan.intents.contains(&intent2));
    }

    #[test]
    fn test_add_duplicate_intent_deduplicates() {
        let intent = sample_intent();
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
}
