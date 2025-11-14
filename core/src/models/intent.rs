use chrono::NaiveDate;
use rand::Rng;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Intent {
    #[serde(default)]
    pub intent_id: String,
    pub alias: Option<String>,
    pub role: Option<String>,
    pub objective: Option<String>,
    pub action: Option<String>,
    pub subject: Option<String>,
    #[serde(default, deserialize_with = "deserialize_trackers")]
    pub trackers: Vec<String>,
}

/// Custom deserializer for trackers that handles both string and array formats
fn deserialize_trackers<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct TrackersVisitor;

    impl<'de> Visitor<'de> for TrackersVisitor {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or array of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Vec<String>, E>
        where
            E: de::Error,
        {
            Ok(vec![value.to_string()])
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Vec<String>, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let mut trackers = Vec::new();
            while let Some(value) = seq.next_element()? {
                trackers.push(value);
            }
            Ok(trackers)
        }
    }

    deserializer.deserialize_any(TrackersVisitor)
}

impl Intent {
    /// Generate a new intent ID in the format source:i-YYYYMMDD-{6 random chars}
    ///
    /// # Arguments
    /// * `source` - The source prefix (e.g., "local", "element", "jira")
    /// * `date` - The date to use in the ID (typically the current date)
    ///
    /// # Returns
    /// A string in the format "local:i-20250125-abc123"
    pub fn generate_intent_id(source: &str, date: NaiveDate) -> String {
        let date_str = date.format("%Y%m%d");
        let random_suffix: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(6)
            .map(|c| c.to_ascii_lowercase())
            .map(char::from)
            .collect();
        format!("{source}:i-{date_str}-{random_suffix}")
    }

    pub fn new(
        alias: Option<String>,
        role: Option<String>,
        objective: Option<String>,
        action: Option<String>,
        subject: Option<String>,
        trackers: Vec<String>,
    ) -> Self {
        Self::new_with_id(
            None, // Auto-generate ID with current date
            alias, role, objective, action, subject, trackers,
        )
    }

    pub fn new_with_id(
        intent_id: Option<String>,
        alias: Option<String>,
        role: Option<String>,
        objective: Option<String>,
        action: Option<String>,
        subject: Option<String>,
        trackers: Vec<String>,
    ) -> Self {
        let deduped: Vec<String> = HashSet::<_>::from_iter(trackers).into_iter().collect();

        let alias = alias.or_else(|| {
            Some(format!(
                "{}: {} to {} for {}",
                role.as_deref().unwrap_or(""),
                action.as_deref().unwrap_or(""),
                objective.as_deref().unwrap_or(""),
                subject.as_deref().unwrap_or("")
            ))
        });

        // Use provided ID or empty string
        // Note: Plan::add_intent() will generate a properly prefixed ID if empty
        let intent_id = intent_id.unwrap_or_default();

        Self {
            intent_id,
            alias,
            role,
            objective,
            action,
            subject,
            trackers: deduped,
        }
    }
}
