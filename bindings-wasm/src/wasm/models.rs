use chrono::{DateTime, Datelike, NaiveDate};
use chrono_tz::Tz;
use faff_core::models::{
    Log as RustLog, Plan as RustPlan, Session as RustSession, Timesheet as RustTimesheet,
};
use wasm_bindgen::prelude::*;

/// A work session with start/end times and semantic classification.
#[wasm_bindgen]
#[derive(Clone)]
pub struct Session {
    inner: RustSession,
}

#[wasm_bindgen]
impl Session {
    #[wasm_bindgen(constructor)]
    pub fn new(
        alias: Option<String>,
        role: Option<String>,
        objective: Option<String>,
        action: Option<String>,
        subject: Option<String>,
        trackers: Option<Vec<String>>,
        start: js_sys::Date,
        end: Option<js_sys::Date>,
        note: Option<String>,
    ) -> Result<Session, JsValue> {
        let start_dt = js_date_to_chrono(&start)?;
        let end_dt = end.as_ref().map(js_date_to_chrono).transpose()?;

        Ok(Self {
            inner: RustSession::new(
                alias,
                role,
                objective,
                action,
                subject,
                trackers.unwrap_or_default(),
                start_dt,
                end_dt,
                note,
            ),
        })
    }

    #[wasm_bindgen(getter)]
    pub fn alias(&self) -> Option<String> {
        self.inner.alias.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn role(&self) -> Option<String> {
        self.inner.role.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn objective(&self) -> Option<String> {
        self.inner.objective.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn action(&self) -> Option<String> {
        self.inner.action.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn subject(&self) -> Option<String> {
        self.inner.subject.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn trackers(&self) -> Vec<String> {
        self.inner.trackers.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn start(&self) -> js_sys::Date {
        chrono_to_js_date(&self.inner.start)
    }

    #[wasm_bindgen(getter)]
    pub fn end(&self) -> Option<js_sys::Date> {
        self.inner.end.map(|dt| chrono_to_js_date(&dt))
    }

    #[wasm_bindgen(getter)]
    pub fn note(&self) -> Option<String> {
        self.inner.note.clone()
    }

    /// Get duration in milliseconds. Returns null if session has no end time.
    #[wasm_bindgen(getter)]
    pub fn duration(&self) -> Option<f64> {
        self.inner
            .duration()
            .ok()
            .map(|d| d.num_milliseconds() as f64)
    }

    /// Get elapsed time in milliseconds for active sessions (no end time).
    /// Returns duration since start to provided `now` time.
    /// Returns null if session has an end time (use duration() instead).
    #[wasm_bindgen(js_name = elapsed)]
    pub fn elapsed(&self, now: js_sys::Date) -> Result<Option<f64>, JsValue> {
        if self.inner.end.is_some() {
            // Session is closed, use duration() instead
            return Ok(None);
        }

        let now_dt = js_date_to_chrono(&now)?;
        let elapsed = self.inner.elapsed(now_dt);
        Ok(Some(elapsed.num_milliseconds() as f64))
    }

    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.inner).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = fromJSON)]
    pub fn from_json(value: &JsValue) -> Result<Session, JsValue> {
        let inner: RustSession = serde_wasm_bindgen::from_value(value.clone())
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Self { inner })
    }
}

/// A log represents one day of work with multiple sessions.
#[wasm_bindgen]
#[derive(Clone)]
pub struct Log {
    pub(crate) inner: RustLog,
}

#[wasm_bindgen]
impl Log {
    #[wasm_bindgen(constructor)]
    pub fn new(date: js_sys::Date, timezone: &str) -> Result<Log, JsValue> {
        let naive_date = js_date_to_naive_date(&date)?;
        let tz: Tz = timezone
            .parse()
            .map_err(|_| JsValue::from_str(&format!("Invalid timezone: {}", timezone)))?;

        Ok(Self {
            inner: RustLog::new(naive_date, tz, vec![]),
        })
    }

    #[wasm_bindgen(getter)]
    pub fn date(&self) -> js_sys::Date {
        naive_date_to_js_date(&self.inner.date)
    }

    #[wasm_bindgen(getter)]
    pub fn timezone(&self) -> String {
        self.inner.timezone.name().to_string()
    }

    #[wasm_bindgen(getter)]
    pub fn timeline(&self) -> Vec<Session> {
        self.inner
            .timeline
            .iter()
            .map(|s| Session { inner: s.clone() })
            .collect()
    }

    /// Get currently active (open) session if any
    #[wasm_bindgen(js_name = activeSession)]
    pub fn active_session(&self) -> Option<Session> {
        self.inner
            .active_session()
            .map(|s| Session { inner: s.clone() })
    }

    /// Check if all sessions are closed
    #[wasm_bindgen(js_name = isClosed)]
    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.inner).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Calculate summary statistics for this log
    ///
    /// now: JS Date for calculating duration of open sessions
    /// Returns an object with:
    ///   totalMinutes: number
    ///   byAlias: Record<string, number>
    ///   byTracker: Record<string, number>
    ///   byTrackerSource: Record<string, number>
    ///   meanReflectionScore: number | null
    pub fn summary(&self, now: js_sys::Date) -> Result<JsValue, JsValue> {
        let now_dt = js_date_to_chrono(&now)?;
        let summary = self.inner.summary(now_dt);

        // Convert to a JS-friendly format
        let obj = js_sys::Object::new();
        js_sys::Reflect::set(&obj, &"totalMinutes".into(), &summary.total_minutes.into())?;
        js_sys::Reflect::set(
            &obj,
            &"byAlias".into(),
            &serde_wasm_bindgen::to_value(&summary.by_alias)
                .map_err(|e| JsValue::from_str(&e.to_string()))?,
        )?;
        js_sys::Reflect::set(
            &obj,
            &"byTracker".into(),
            &serde_wasm_bindgen::to_value(&summary.by_tracker)
                .map_err(|e| JsValue::from_str(&e.to_string()))?,
        )?;
        js_sys::Reflect::set(
            &obj,
            &"byTrackerSource".into(),
            &serde_wasm_bindgen::to_value(&summary.by_tracker_source)
                .map_err(|e| JsValue::from_str(&e.to_string()))?,
        )?;
        js_sys::Reflect::set(
            &obj,
            &"meanReflectionScore".into(),
            &match summary.mean_reflection_score {
                Some(score) => JsValue::from_f64(score),
                None => JsValue::NULL,
            },
        )?;

        Ok(obj.into())
    }

    // Note: Log doesn't support from_json because it uses custom TOML parsing
    // Use Workspace.getLog() to load logs from storage
}

/// A plan defines vocabulary and templates for work tracking.
#[wasm_bindgen]
#[derive(Clone)]
pub struct Plan {
    pub(crate) inner: RustPlan,
}

#[wasm_bindgen]
impl Plan {
    #[wasm_bindgen(getter)]
    pub fn source(&self) -> String {
        self.inner.source.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.inner.id()
    }

    #[wasm_bindgen(getter, js_name = validFrom)]
    pub fn valid_from(&self) -> js_sys::Date {
        naive_date_to_js_date(&self.inner.valid_from)
    }

    #[wasm_bindgen(getter, js_name = validUntil)]
    pub fn valid_until(&self) -> Option<js_sys::Date> {
        self.inner.valid_until.map(|d| naive_date_to_js_date(&d))
    }

    #[wasm_bindgen(getter)]
    pub fn roles(&self) -> Vec<String> {
        self.inner.roles.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn actions(&self) -> Vec<String> {
        self.inner.actions.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn objectives(&self) -> Vec<String> {
        self.inner.objectives.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn subjects(&self) -> Vec<String> {
        self.inner.subjects.clone()
    }

    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.inner).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = fromJSON)]
    pub fn from_json(value: &JsValue) -> Result<Plan, JsValue> {
        let inner: RustPlan = serde_wasm_bindgen::from_value(value.clone())
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Self { inner })
    }
}

/// A cryptographically signed, immutable record of work for external submission.
#[wasm_bindgen]
#[derive(Clone)]
pub struct Timesheet {
    pub(crate) inner: RustTimesheet,
}

#[wasm_bindgen]
impl Timesheet {
    #[wasm_bindgen(getter)]
    pub fn date(&self) -> js_sys::Date {
        naive_date_to_js_date(&self.inner.date)
    }

    #[wasm_bindgen(getter)]
    pub fn compiled(&self) -> js_sys::Date {
        chrono_to_js_date(&self.inner.compiled)
    }

    #[wasm_bindgen(getter)]
    pub fn timezone(&self) -> String {
        self.inner.timezone.name().to_string()
    }

    #[wasm_bindgen(getter)]
    pub fn timeline(&self) -> Vec<Session> {
        self.inner
            .timeline
            .iter()
            .map(|s| Session { inner: s.clone() })
            .collect()
    }

    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.inner).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = fromJSON)]
    pub fn from_json(value: &JsValue) -> Result<Timesheet, JsValue> {
        let inner: RustTimesheet = serde_wasm_bindgen::from_value(value.clone())
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Self { inner })
    }
}

// Helper functions for date/time conversion

fn js_date_to_chrono(date: &js_sys::Date) -> Result<DateTime<Tz>, JsValue> {
    let timestamp_ms = date.get_time() as i64;
    let timestamp_secs = timestamp_ms / 1000;
    let timestamp_nanos = ((timestamp_ms % 1000) * 1_000_000) as u32;

    let dt = DateTime::from_timestamp(timestamp_secs, timestamp_nanos)
        .ok_or_else(|| JsValue::from_str("Invalid timestamp"))?;

    Ok(dt.with_timezone(&chrono_tz::UTC))
}

fn chrono_to_js_date(dt: &DateTime<Tz>) -> js_sys::Date {
    let timestamp_ms = dt.timestamp_millis() as f64;
    js_sys::Date::new(&JsValue::from_f64(timestamp_ms))
}

fn js_date_to_naive_date(date: &js_sys::Date) -> Result<NaiveDate, JsValue> {
    let year = date.get_utc_full_year() as i32;
    let month = date.get_utc_month() + 1; // JS months are 0-indexed
    let day = date.get_utc_date();

    NaiveDate::from_ymd_opt(year, month, day).ok_or_else(|| JsValue::from_str("Invalid date"))
}

fn naive_date_to_js_date(date: &NaiveDate) -> js_sys::Date {
    js_sys::Date::new_with_year_month_day(
        date.year() as u32,
        (date.month() - 1) as i32, // JS months are 0-indexed
        date.day() as i32,
    )
}
