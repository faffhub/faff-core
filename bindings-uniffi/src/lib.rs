//! UniFFI binding layer for faff-core.
//!
//! Mirrors the existing PyO3 bindings (../bindings-python) but exposes the
//! workspace through UniFFI so that uniffi-bindgen-go can generate native
//! Go bindings consumable by faff-tui without shelling out to faff-cli.
//!
//! Stage 3 surface: read the daily log, start/stop sessions, vocabulary
//! queries, and a `recent_prototypes` convenience that replaces the old
//! plan-level "intent" concept with history-derived session shapes.

// `Arc` is referenced by the scaffolding generated from `faff_core.udl`
// (constructors are wrapped in `Arc<Self>` by UniFFI), so it must be in
// scope for the macro expansion even though no source line uses it.
#[allow(unused_imports)]
use std::sync::Arc;

use std::collections::HashMap;

use chrono::{DateTime, Duration, NaiveDate};
use chrono_tz::Tz;
use faff_core::models::plan::SessionHint as RustSessionHint;
use faff_core::models::Session as RustSession;
use faff_core::workspace::Workspace as RustWorkspace;
use tokio::runtime::Runtime;

#[derive(Debug, thiserror::Error)]
pub enum FaffError {
    #[error("workspace init failed: {0}")]
    WorkspaceInit(String),
    #[error("invalid date '{0}': expected YYYY-MM-DD")]
    InvalidDate(String),
    #[error("no active session")]
    NoActiveSession,
    #[error("{0}")]
    Other(String),
}

/// Mirrors the UDL `Session` dictionary. Time fields are RFC3339 strings.
pub struct Session {
    pub title: Option<String>,
    pub role: Option<String>,
    pub impact: Option<String>,
    pub mode: Option<String>,
    pub subject: Option<String>,
    pub trackers: Vec<String>,
    pub start: String,
    pub end: Option<String>,
    pub note: Option<String>,
}

impl From<&RustSession> for Session {
    fn from(s: &RustSession) -> Self {
        Self {
            title: s.title.clone(),
            role: s.role.clone(),
            impact: s.impact.clone(),
            mode: s.mode.clone(),
            subject: s.subject.clone(),
            trackers: s.trackers.clone(),
            start: s.start.to_rfc3339(),
            end: s.end.as_ref().map(|d| d.to_rfc3339()),
            note: s.note.clone(),
        }
    }
}

/// Mirrors the UDL `Log` dictionary.
pub struct Log {
    pub date: String,
    pub timezone: String,
    pub sessions: Vec<Session>,
}

/// Mirrors the UDL `Tracker` dictionary.
pub struct Tracker {
    pub id: String,
    pub name: String,
}

/// Mirrors the UDL `SessionHint` dictionary.
pub struct SessionHint {
    pub title: String,
    pub role: Option<String>,
    pub impact: Option<String>,
    pub mode: Option<String>,
    pub subject: Option<String>,
    pub trackers: Vec<String>,
}

impl From<RustSessionHint> for SessionHint {
    fn from(h: RustSessionHint) -> Self {
        Self {
            title: h.title,
            role: h.role,
            impact: h.impact,
            mode: h.mode,
            subject: h.subject,
            trackers: h.trackers,
        }
    }
}

/// Mirrors the UDL `SessionPrototype` dictionary.
pub struct SessionPrototype {
    pub title: Option<String>,
    pub role: Option<String>,
    pub impact: Option<String>,
    pub mode: Option<String>,
    pub subject: Option<String>,
    pub trackers: Vec<String>,
    pub count: u32,
    pub last_used: String,
}

/// Hash key for grouping sessions into prototypes. `trackers` is sorted
/// before construction so ordering differences don't split groups.
#[derive(Hash, Eq, PartialEq, Clone)]
struct ProtoKey {
    title: Option<String>,
    role: Option<String>,
    impact: Option<String>,
    mode: Option<String>,
    subject: Option<String>,
    trackers: Vec<String>,
}

/// UniFFI-exported handle around `Arc<faff_core::workspace::Workspace>`.
///
/// We keep a private tokio runtime alive for the lifetime of the workspace
/// so that every async call doesn't pay the cost of spinning a new one
/// (which is what bindings-python does today).
pub struct FaffWorkspace {
    inner: Arc<RustWorkspace>,
    rt: Runtime,
}

impl FaffWorkspace {
    pub fn new() -> Result<Self, FaffError> {
        let rt = Runtime::new().map_err(|e| FaffError::Other(format!("tokio runtime: {e}")))?;
        let inner = rt
            .block_on(RustWorkspace::new())
            .map_err(|e| FaffError::WorkspaceInit(e.to_string()))?;
        Ok(Self { inner, rt })
    }

    pub fn today(&self) -> String {
        self.inner.today().to_string()
    }

    pub fn timezone(&self) -> String {
        self.inner.timezone().name().to_string()
    }

    pub fn get_log(&self, date: String) -> Result<Log, FaffError> {
        let parsed = parse_date(&date)?;
        let log = self
            .rt
            .block_on(self.inner.logs().get_log(parsed))
            .map_err(|e| FaffError::Other(e.to_string()))?;
        Ok(Log {
            date: log.date.to_string(),
            timezone: log.timezone.name().to_string(),
            sessions: log.timeline.iter().map(Session::from).collect(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start_session(
        &self,
        title: Option<String>,
        role: Option<String>,
        impact: Option<String>,
        mode: Option<String>,
        subject: Option<String>,
        trackers: Vec<String>,
        note: Option<String>,
    ) -> Result<(), FaffError> {
        let now = self.inner.now();
        self.rt
            .block_on(self.inner.logs().start_session(
                title, role, impact, mode, subject, trackers, now, note,
            ))
            .map_err(|e| FaffError::Other(e.to_string()))
    }

    pub fn stop_current_session(&self) -> Result<(), FaffError> {
        self.rt
            .block_on(self.inner.logs().stop_current_session())
            .map_err(|e| FaffError::Other(e.to_string()))
    }

    /// Replace the semantic fields of today's currently active session.
    /// Mirrors `LogManager::update_active_session`.
    ///
    /// `anyhow` erases types so we map the "no active session" string back
    /// to the typed `NoActiveSession` variant on a best-effort basis. The
    /// rest fall through to `Other`.
    #[allow(clippy::too_many_arguments)]
    pub fn update_active_session(
        &self,
        title: Option<String>,
        role: Option<String>,
        impact: Option<String>,
        mode: Option<String>,
        subject: Option<String>,
        trackers: Vec<String>,
        note: Option<String>,
    ) -> Result<(), FaffError> {
        self.rt
            .block_on(self.inner.logs().update_active_session(
                title, role, impact, mode, subject, trackers, note,
            ))
            .map_err(|e| {
                let msg = e.to_string();
                if msg.to_lowercase().contains("active session") {
                    FaffError::NoActiveSession
                } else {
                    FaffError::Other(msg)
                }
            })
    }

    pub fn get_roles(&self, date: String) -> Result<Vec<String>, FaffError> {
        let parsed = parse_date(&date)?;
        self.rt
            .block_on(self.inner.plans().get_roles(parsed))
            .map_err(|e| FaffError::Other(e.to_string()))
    }

    pub fn get_impacts(&self, date: String) -> Result<Vec<String>, FaffError> {
        let parsed = parse_date(&date)?;
        self.rt
            .block_on(self.inner.plans().get_impacts(parsed))
            .map_err(|e| FaffError::Other(e.to_string()))
    }

    pub fn get_modes(&self, date: String) -> Result<Vec<String>, FaffError> {
        let parsed = parse_date(&date)?;
        self.rt
            .block_on(self.inner.plans().get_modes(parsed))
            .map_err(|e| FaffError::Other(e.to_string()))
    }

    pub fn get_subjects(&self, date: String) -> Result<Vec<String>, FaffError> {
        let parsed = parse_date(&date)?;
        self.rt
            .block_on(self.inner.plans().get_subjects(parsed))
            .map_err(|e| FaffError::Other(e.to_string()))
    }

    pub fn get_trackers(&self, date: String) -> Result<Vec<Tracker>, FaffError> {
        let parsed = parse_date(&date)?;
        let map = self
            .rt
            .block_on(self.inner.plans().get_trackers(parsed))
            .map_err(|e| FaffError::Other(e.to_string()))?;
        let mut out: Vec<Tracker> = map
            .into_iter()
            .map(|(id, name)| Tracker { id, name })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    pub fn get_session_hints(&self, date: String) -> Result<Vec<SessionHint>, FaffError> {
        let parsed = parse_date(&date)?;
        let hints = self
            .rt
            .block_on(self.inner.plans().get_session_hints(parsed))
            .map_err(|e| FaffError::Other(e.to_string()))?;
        Ok(hints.into_iter().map(SessionHint::from).collect())
    }

    pub fn recent_prototypes(&self, days: u32) -> Result<Vec<SessionPrototype>, FaffError> {
        if days == 0 {
            return Ok(Vec::new());
        }
        let today = self.inner.today();
        let cutoff = today - Duration::days(i64::from(days) - 1);

        let log_dates = self
            .rt
            .block_on(self.inner.logs().list_logs())
            .map_err(|e| FaffError::Other(e.to_string()))?;

        // Accumulator: prototype key -> (count, most recent start time).
        let mut acc: HashMap<ProtoKey, (u32, DateTime<Tz>)> = HashMap::new();

        for d in log_dates.iter().rev() {
            if *d < cutoff || *d > today {
                continue;
            }
            let log = match self.rt.block_on(self.inner.logs().get_log(*d)) {
                Ok(l) => l,
                Err(_) => continue,
            };
            for s in &log.timeline {
                let mut sorted_trackers = s.trackers.clone();
                sorted_trackers.sort();
                let key = ProtoKey {
                    title: s.title.clone(),
                    role: s.role.clone(),
                    impact: s.impact.clone(),
                    mode: s.mode.clone(),
                    subject: s.subject.clone(),
                    trackers: sorted_trackers,
                };
                let entry = acc.entry(key).or_insert((0, s.start));
                entry.0 += 1;
                if s.start > entry.1 {
                    entry.1 = s.start;
                }
            }
        }

        let mut prototypes: Vec<SessionPrototype> = acc
            .into_iter()
            .map(|(k, (count, last_used))| SessionPrototype {
                title: k.title,
                role: k.role,
                impact: k.impact,
                mode: k.mode,
                subject: k.subject,
                trackers: k.trackers,
                count,
                last_used: last_used.to_rfc3339(),
            })
            .collect();

        // Rank: most-used first, ties broken by most-recently-used.
        prototypes.sort_by(|a, b| {
            b.count
                .cmp(&a.count)
                .then_with(|| b.last_used.cmp(&a.last_used))
        });

        Ok(prototypes)
    }
}

fn parse_date(s: &str) -> Result<NaiveDate, FaffError> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| FaffError::InvalidDate(s.to_string()))
}

uniffi::include_scaffolding!("faff_core");
