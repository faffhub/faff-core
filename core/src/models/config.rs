use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub timezone: Tz,
    #[serde(default)]
    pub plan_remote: Vec<PlanRemote>,
    #[serde(default)]
    pub timesheet_audience: Vec<TimesheetAudience>,
    #[serde(default)]
    pub role: Vec<Role>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanRemote {
    pub name: String,
    pub plugin: String,
    #[serde(default)]
    pub config: HashMap<String, toml::Value>,
    #[serde(default)]
    pub defaults: PlanDefaults,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct PlanDefaults {
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub objectives: Vec<String>,
    #[serde(default)]
    pub actions: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimesheetAudience {
    pub name: String,
    pub plugin: String,
    #[serde(default)]
    pub config: HashMap<String, toml::Value>,
    #[serde(default)]
    pub signing_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Role {
    pub name: String,
    #[serde(default)]
    pub config: HashMap<String, toml::Value>,
}

impl Config {
    /// Create a default config using the system's detected timezone
    ///
    /// Attempts to detect the system timezone using IANA timezone database.
    /// Falls back to UTC if detection fails.
    pub fn with_system_timezone() -> Self {
        let timezone = match iana_time_zone::get_timezone() {
            Ok(tz_name) => match tz_name.parse::<Tz>() {
                Ok(tz) => tz,
                Err(_) => {
                    eprintln!(
                        "Warning: Could not parse detected timezone '{}', defaulting to UTC",
                        tz_name
                    );
                    chrono_tz::UTC
                }
            },
            Err(_) => {
                eprintln!("Warning: Could not detect system timezone, defaulting to UTC");
                chrono_tz::UTC
            }
        };

        Self {
            timezone,
            plan_remote: vec![PlanRemote {
                name: "local".to_string(),
                plugin: "local".to_string(),
                config: HashMap::new(),
                defaults: PlanDefaults::default(),
            }],
            timesheet_audience: vec![],
            role: vec![],
        }
    }

    /// Load config from TOML string
    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
    }

    /// Serialize config to TOML string
    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_config() {
        let toml_str = r#"
            timezone = "Europe/London"
        "#;

        let config = Config::from_toml(toml_str).unwrap();
        assert_eq!(config.timezone.name(), "Europe/London");
        assert_eq!(config.plan_remote.len(), 0);
        assert_eq!(config.timesheet_audience.len(), 0);
        assert_eq!(config.role.len(), 0);
    }

    #[test]
    fn test_full_config() {
        let toml_str = r#"
            timezone = "America/New_York"

            [[plan_remote]]
            name = "test"
            plugin = "myhours"
            config.email = "test@example.com"

            [plan_remote.defaults]
            roles = ["role1", "role2"]
            objectives = ["obj1"]
            actions = ["action1"]

            [[timesheet_audience]]
            name = "audience1"
            plugin = "myhours"
            signing_ids = ["id1", "id2"]

            [[role]]
            name = "developer"
        "#;

        let config = Config::from_toml(toml_str).unwrap();
        assert_eq!(config.timezone.name(), "America/New_York");
        assert_eq!(config.plan_remote.len(), 1);
        assert_eq!(config.plan_remote[0].name, "test");
        assert_eq!(config.plan_remote[0].plugin, "myhours");
        assert_eq!(config.plan_remote[0].defaults.roles, vec!["role1", "role2"]);
        assert_eq!(config.timesheet_audience.len(), 1);
        assert_eq!(config.role.len(), 1);
    }

    #[test]
    fn test_invalid_timezone() {
        let toml_str = r#"
            timezone = "Invalid/Timezone"
        "#;

        let result = Config::from_toml(toml_str);
        assert!(result.is_err());
    }
}
