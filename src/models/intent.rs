use serde::{Serialize, Deserialize};
use std::collections::HashSet;

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
}