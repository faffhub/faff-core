use serde::{Serialize, Deserialize};
use std::collections::{HashSet, HashMap};
use crate::models::valuetype::ValueType;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Intent {
    pub alias: Option<String>,
    pub role: Option<String>,
    pub objective: Option<String>,
    pub action: Option<String>,
    pub subject: Option<String>,
    pub trackers: Vec<String>,
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
