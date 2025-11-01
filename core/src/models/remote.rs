use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

    /// Static ROAST vocabulary for this remote
    /// Used when the remote doesn't provide its own ROAST objects
    #[serde(default)]
    pub vocabulary: RemoteVocabulary,
}

/// Static ROAST vocabulary for a remote
///
/// These are source-scoped ROAST objects that don't come from the remote API
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
}
