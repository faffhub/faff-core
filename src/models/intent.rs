use serde::{Serialize, Deserialize, Deserializer};
use serde::de::{self, Visitor};
use std::collections::{HashSet, HashMap};
use crate::models::valuetype::ValueType;

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

    // FIXME: I really don't know whether it's good to have this here or in the python binding.
    // I guess what will answer that question is if/when I use the Rust Intent to interact
    // with the format on disk will I just use serde support or something else?
    pub fn from_dict(dict: HashMap<String, ValueType>) -> Result<Self, String> {
        let alias = dict.get("alias")
                        .and_then(|v| v.as_string())
                        .cloned();

        let role = dict.get("role")
                        .and_then(|v| v.as_string())
                        .cloned();

        let objective = dict.get("objective")
                        .and_then(|v| v.as_string())
                        .cloned();

        let action = dict.get("action")
                        .and_then(|v| v.as_string())
                        .cloned();

        let subject = dict.get("subject")
                        .and_then(|v| v.as_string())
                        .cloned();

        let trackers = dict.get("trackers")
                   .and_then(|v| v.as_list())
                   .cloned()
                   .unwrap_or_default();

        Ok(Self::new(alias, role, objective, action, subject, trackers))
    }

}
