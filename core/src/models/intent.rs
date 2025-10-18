use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Intent {
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
    pub fn new(
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

        Self {
            alias,
            role,
            objective,
            action,
            subject,
            trackers: deduped,
        }
    }
}
