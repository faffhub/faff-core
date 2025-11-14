use anyhow::{bail, Result};
use chrono::{DateTime, NaiveDate};
use chrono_tz::Tz;
use ed25519_dalek::{Signature, Signer, SigningKey};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

use crate::models::valuetype::ValueType;
use crate::models::Session;

// Custom serializer for DateTime<Tz> that uses Z suffix for UTC
fn serialize_datetime<S>(dt: &DateTime<Tz>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    // Check if it's UTC (either UTC or Etc/UTC timezone)
    let tz_name = dt.timezone().name();
    if tz_name == "UTC" || tz_name == "Etc/UTC" {
        // Serialize with Z suffix
        if dt.timestamp_subsec_micros() > 0 {
            serializer.serialize_str(&dt.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string())
        } else {
            serializer.serialize_str(&dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        }
    } else {
        // Use RFC3339 with offset
        serializer.serialize_str(&dt.to_rfc3339())
    }
}

fn serialize_optional_datetime<S>(
    dt: &Option<DateTime<Tz>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match dt {
        Some(dt) => serialize_datetime(dt, serializer),
        None => serializer.serialize_none(),
    }
}

// Custom deserializer for DateTime<Tz> that parses RFC3339 strings (always converts to UTC)
fn deserialize_datetime<'de, D>(deserializer: D) -> Result<DateTime<Tz>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    DateTime::parse_from_rfc3339(&s)
        .map(|dt| dt.with_timezone(&chrono_tz::UTC))
        .map_err(serde::de::Error::custom)
}

fn deserialize_optional_datetime<'de, D>(deserializer: D) -> Result<Option<DateTime<Tz>>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        Some(s) => {
            let dt = DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&chrono_tz::UTC))
                .map_err(serde::de::Error::custom)?;
            Ok(Some(dt))
        }
        None => Ok(None),
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UnsignedTimesheet {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub actor: HashMap<String, String>,
    pub version: String,
    pub date: NaiveDate,
    #[serde(
        serialize_with = "serialize_datetime",
        deserialize_with = "deserialize_datetime"
    )]
    pub compiled: DateTime<Tz>,
    pub timezone: Tz,
    pub timeline: Vec<Session>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubmissionStatus {
    Success,
    Failed,
    Partial,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct TimesheetMeta {
    pub audience_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(
        serialize_with = "serialize_optional_datetime",
        deserialize_with = "deserialize_optional_datetime"
    )]
    pub submitted_at: Option<DateTime<Tz>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submission_status: Option<SubmissionStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submission_error: Option<String>,
}

impl TimesheetMeta {
    pub fn new(audience_id: String, submitted_at: Option<DateTime<Tz>>, log_hash: String) -> Self {
        Self {
            audience_id,
            submitted_at,
            log_hash: Some(log_hash),
            submission_status: None,
            submission_error: None,
        }
    }

    pub fn with_submission_result(
        mut self,
        status: SubmissionStatus,
        error: Option<String>,
        submitted_at: DateTime<Tz>,
    ) -> Self {
        self.submission_status = Some(status);
        self.submission_error = error;
        self.submitted_at = Some(submitted_at);
        self
    }

    pub fn from_dict(dict: HashMap<String, ValueType>) -> Result<Self> {
        let audience_id = dict
            .get("audience_id")
            .and_then(|v| v.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing 'audience_id' field"))?
            .clone();

        let submitted_at = dict
            .get("submitted_at")
            .and_then(|v| v.as_string())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono_tz::UTC));

        let log_hash = dict.get("log_hash").and_then(|v| v.as_string()).cloned();

        let submission_status = dict
            .get("submission_status")
            .and_then(|v| v.as_string())
            .and_then(|s| match s.as_str() {
                "success" => Some(SubmissionStatus::Success),
                "failed" => Some(SubmissionStatus::Failed),
                "partial" => Some(SubmissionStatus::Partial),
                _ => None,
            });

        let submission_error = dict
            .get("submission_error")
            .and_then(|v| v.as_string())
            .cloned();

        Ok(Self {
            audience_id,
            submitted_at,
            log_hash,
            submission_status,
            submission_error,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Timesheet {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub actor: HashMap<String, String>,
    pub version: String,
    pub date: NaiveDate,
    #[serde(
        serialize_with = "serialize_datetime",
        deserialize_with = "deserialize_datetime"
    )]
    pub compiled: DateTime<Tz>,
    pub timezone: Tz,
    pub timeline: Vec<Session>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub signatures: HashMap<String, HashMap<String, String>>,
    #[serde(skip)]
    pub meta: TimesheetMeta,
}

impl Timesheet {
    pub fn new(
        actor: HashMap<String, String>,
        date: NaiveDate,
        compiled: DateTime<Tz>,
        timezone: Tz,
        timeline: Vec<Session>,
        signatures: HashMap<String, HashMap<String, String>>,
        meta: TimesheetMeta,
    ) -> Self {
        Self {
            actor,
            version: "Faffage-generated timesheet v1.0 please see faffage.com for details"
                .to_string(),
            date,
            compiled,
            timezone,
            timeline,
            signatures,
            meta,
        }
    }

    fn unsigned(&self) -> UnsignedTimesheet {
        UnsignedTimesheet {
            actor: self.actor.clone(),
            version: self.version.clone(),
            date: self.date,
            compiled: self.compiled,
            timezone: self.timezone,
            timeline: self.timeline.clone(),
        }
    }

    pub fn sign(&self, id: &str, signing_key_bytes: &[u8]) -> Result<Self> {
        if signing_key_bytes.len() != 32 {
            bail!("Signing key must be exactly 32 bytes");
        }

        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(signing_key_bytes);
        let signing_key = SigningKey::from_bytes(&key_bytes);

        // Serialize the unsigned timesheet
        let unsigned = self.unsigned();
        let json = serde_json::to_vec(&unsigned)?;

        // Sign the serialized data
        let signature: Signature = signing_key.sign(&json);

        // Create key ID from public key hash
        let verify_key = signing_key.verifying_key();
        let mut hasher = Sha256::new();
        hasher.update(verify_key.to_bytes());
        let key_hash = hasher.finalize();
        let key_id = format!("ed25519:{}", hex::encode(key_hash));

        // Add the new signature
        let mut new_signatures = self.signatures.clone();
        let mut user_sigs = HashMap::new();
        user_sigs.insert(key_id, hex::encode(signature.to_bytes()));
        new_signatures.insert(id.to_string(), user_sigs);

        Ok(Self {
            actor: self.actor.clone(),
            version: self.version.clone(),
            date: self.date,
            compiled: self.compiled,
            timezone: self.timezone,
            timeline: self.timeline.clone(),
            signatures: new_signatures,
            meta: self.meta.clone(),
        })
    }

    pub fn update_meta(&self, audience_id: String, submitted_at: Option<DateTime<Tz>>) -> Self {
        let new_meta = TimesheetMeta {
            audience_id,
            submitted_at,
            log_hash: self.meta.log_hash.clone(),
            submission_status: self.meta.submission_status.clone(),
            submission_error: self.meta.submission_error.clone(),
        };

        Self {
            actor: self.actor.clone(),
            version: self.version.clone(),
            date: self.date,
            compiled: self.compiled,
            timezone: self.timezone,
            timeline: self.timeline.clone(),
            signatures: self.signatures.clone(),
            meta: new_meta,
        }
    }

    pub fn with_submission_result(
        &self,
        status: SubmissionStatus,
        error: Option<String>,
        submitted_at: DateTime<Tz>,
    ) -> Self {
        let new_meta = self
            .meta
            .clone()
            .with_submission_result(status, error, submitted_at);

        Self {
            actor: self.actor.clone(),
            version: self.version.clone(),
            date: self.date,
            compiled: self.compiled,
            timezone: self.timezone,
            timeline: self.timeline.clone(),
            signatures: self.signatures.clone(),
            meta: new_meta,
        }
    }

    pub fn submittable_timesheet(&self) -> SubmittableTimesheet {
        SubmittableTimesheet {
            actor: self.actor.clone(),
            version: self.version.clone(),
            date: self.date,
            compiled: self.compiled,
            timezone: self.timezone,
            timeline: self.timeline.clone(),
            signatures: self.signatures.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubmittableTimesheet {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub actor: HashMap<String, String>,
    pub version: String,
    pub date: NaiveDate,
    #[serde(
        serialize_with = "serialize_datetime",
        deserialize_with = "deserialize_datetime"
    )]
    pub compiled: DateTime<Tz>,
    pub timezone: Tz,
    pub timeline: Vec<Session>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub signatures: HashMap<String, HashMap<String, String>>,
}

impl SubmittableTimesheet {
    pub fn canonical_form(&self) -> Result<Vec<u8>> {
        // Use canonical JSON serialization (sorted keys, no whitespace)
        let mut buf = Vec::new();
        let mut serializer = serde_json::Serializer::with_formatter(
            &mut buf,
            serde_canonical_json::CanonicalFormatter::new(),
        );
        self.serialize(&mut serializer)?;
        Ok(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_create_timesheet() {
        let meta = TimesheetMeta::new("test-audience".to_string(), None, "test-hash".to_string());
        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let compiled = chrono_tz::UTC
            .with_ymd_and_hms(2025, 3, 15, 18, 30, 0)
            .unwrap();
        let timezone = chrono_tz::Europe::London;

        let timesheet = Timesheet::new(
            HashMap::new(),
            date,
            compiled,
            timezone,
            vec![],
            HashMap::new(),
            meta,
        );

        assert_eq!(timesheet.date, date);
        assert_eq!(timesheet.timezone, timezone);
        assert_eq!(timesheet.meta.audience_id, "test-audience");
    }

    #[test]
    fn test_update_meta() {
        let meta = TimesheetMeta::new("audience1".to_string(), None, "test-hash".to_string());
        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let compiled = chrono_tz::UTC
            .with_ymd_and_hms(2025, 3, 15, 18, 30, 0)
            .unwrap();
        let timezone = chrono_tz::Europe::London;

        let timesheet = Timesheet::new(
            HashMap::new(),
            date,
            compiled,
            timezone,
            vec![],
            HashMap::new(),
            meta,
        );

        let submitted_at = chrono_tz::UTC
            .with_ymd_and_hms(2025, 3, 15, 19, 0, 0)
            .unwrap();
        let updated = timesheet.update_meta("audience2".to_string(), Some(submitted_at));

        assert_eq!(updated.meta.audience_id, "audience2");
        assert_eq!(updated.meta.submitted_at, Some(submitted_at));

        // Original unchanged
        assert_eq!(timesheet.meta.audience_id, "audience1");
    }

    #[test]
    fn test_submittable_timesheet() {
        let meta = TimesheetMeta::new("test-audience".to_string(), None, "test-hash".to_string());
        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let compiled = chrono_tz::UTC
            .with_ymd_and_hms(2025, 3, 15, 18, 30, 0)
            .unwrap();
        let timezone = chrono_tz::Europe::London;

        let timesheet = Timesheet::new(
            HashMap::new(),
            date,
            compiled,
            timezone,
            vec![],
            HashMap::new(),
            meta,
        );

        let submittable = timesheet.submittable_timesheet();

        assert_eq!(submittable.date, timesheet.date);
        assert_eq!(submittable.timezone, timesheet.timezone);
    }

    #[test]
    fn test_canonical_form() {
        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let compiled = chrono_tz::UTC
            .with_ymd_and_hms(2025, 3, 15, 18, 30, 0)
            .unwrap();
        let timezone = chrono_tz::Europe::London;

        let submittable = SubmittableTimesheet {
            actor: HashMap::new(),
            version: "Faffage-generated timesheet v1.0 please see faffage.com for details"
                .to_string(),
            date,
            compiled,
            timezone,
            timeline: vec![],
            signatures: HashMap::new(),
        };

        let canonical = submittable.canonical_form().unwrap();

        assert!(!canonical.is_empty());
        let json_str = String::from_utf8(canonical).unwrap();
        assert!(json_str.contains("\"date\""));
    }
}
