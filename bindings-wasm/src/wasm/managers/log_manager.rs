use super::super::models::Log;
use chrono::Datelike;
use faff_core::managers::LogManager as RustLogManager;
use faff_core::workspace::Workspace as RustWorkspace;
use std::collections::HashMap;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

/// LogManager handles reading and writing daily work logs.
///
/// This wraps faff_core::managers::LogManager and exposes its functionality
/// through JavaScript Promises.
#[wasm_bindgen]
pub struct LogManager {
    inner: Arc<RustLogManager>,
    workspace: Option<Arc<RustWorkspace>>,
}

impl LogManager {
    /// Create from Rust manager with workspace reference
    pub(crate) fn from_rust(manager: Arc<RustLogManager>, workspace: Arc<RustWorkspace>) -> Self {
        Self {
            inner: manager,
            workspace: Some(workspace),
        }
    }
}

#[wasm_bindgen]
impl LogManager {
    /// Check if a log exists for the given date.
    ///
    /// date: JS Date object
    /// Returns: boolean
    #[wasm_bindgen(js_name = logExists)]
    pub fn log_exists(&self, date: js_sys::Date) -> bool {
        if let Ok(naive_date) = js_date_to_naive_date(&date) {
            self.inner.log_exists(naive_date)
        } else {
            false
        }
    }

    /// Get the file path for a log.
    ///
    /// date: JS Date object
    /// Returns: string path
    #[wasm_bindgen(js_name = logFilePath)]
    pub fn log_file_path(&self, date: js_sys::Date) -> Result<String, JsValue> {
        let naive_date = js_date_to_naive_date(&date)?;
        Ok(self
            .inner
            .log_file_path(naive_date)
            .to_string_lossy()
            .into_owned())
    }

    /// Read raw log file contents.
    ///
    /// date: JS Date object
    /// Returns Promise<string>.
    #[wasm_bindgen(js_name = readLogRaw)]
    pub fn read_log_raw(&self, date: js_sys::Date) -> js_sys::Promise {
        let inner = self.inner.clone();

        future_to_promise(async move {
            let naive_date = js_date_to_naive_date(&date)?;

            let contents = inner
                .read_log_raw(naive_date)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to read log: {}", e)))?;

            Ok(JsValue::from_str(&contents))
        })
    }

    /// Write raw log file contents.
    ///
    /// date: JS Date object
    /// contents: string
    /// Returns Promise<void>.
    #[wasm_bindgen(js_name = writeLogRaw)]
    pub fn write_log_raw(&self, date: js_sys::Date, contents: &str) -> js_sys::Promise {
        let inner = self.inner.clone();
        let contents = contents.to_string();

        future_to_promise(async move {
            let naive_date = js_date_to_naive_date(&date)?;

            inner
                .write_log_raw(naive_date, &contents)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to write log: {}", e)))?;

            Ok(JsValue::undefined())
        })
    }

    /// List all log dates.
    ///
    /// Returns Promise<Date[]>.
    #[wasm_bindgen(js_name = listLogDates)]
    pub fn list_log_dates(&self) -> js_sys::Promise {
        let inner = self.inner.clone();

        future_to_promise(async move {
            let dates = inner
                .list_logs()
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to list logs: {}", e)))?;

            let array = js_sys::Array::new();
            for date in dates {
                array.push(&naive_date_to_js_date(&date));
            }

            Ok(JsValue::from(array))
        })
    }

    /// List all logs (returns Log objects).
    ///
    /// Returns Promise<Log[]>.
    #[wasm_bindgen(js_name = listLogs)]
    pub fn list_logs(&self) -> js_sys::Promise {
        let inner = self.inner.clone();

        future_to_promise(async move {
            let dates = inner
                .list_logs()
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to list logs: {}", e)))?;

            let array = js_sys::Array::new();
            for date in dates {
                let log = inner
                    .get_log(date)
                    .await
                    .map_err(|e| JsValue::from_str(&format!("Failed to get log: {}", e)))?;
                array.push(&JsValue::from(Log { inner: log }));
            }

            Ok(JsValue::from(array))
        })
    }

    /// Delete a log for a given date.
    ///
    /// date: JS Date object
    /// Returns Promise<void>.
    #[wasm_bindgen(js_name = deleteLog)]
    pub fn delete_log(&self, date: js_sys::Date) -> js_sys::Promise {
        let inner = self.inner.clone();

        future_to_promise(async move {
            let naive_date = js_date_to_naive_date(&date)?;

            inner
                .delete_log(naive_date)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to delete log: {}", e)))?;

            Ok(JsValue::undefined())
        })
    }

    /// Get the timezone name.
    ///
    /// Returns: string (e.g., "America/New_York")
    #[wasm_bindgen(js_name = timezone)]
    pub fn timezone(&self) -> String {
        self.inner.timezone().name().to_string()
    }

    /// Get a log for a given date (returns empty log if file doesn't exist).
    ///
    /// date: JS Date object
    /// Returns Promise<Log>.
    #[wasm_bindgen(js_name = getLog)]
    pub fn get_log(&self, date: js_sys::Date) -> js_sys::Promise {
        let inner = self.inner.clone();

        future_to_promise(async move {
            let naive_date = js_date_to_naive_date(&date)?;

            let log = inner
                .get_log(naive_date)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get log: {}", e)))?;

            Ok(JsValue::from(Log { inner: log }))
        })
    }

    /// Write a log to storage.
    ///
    /// log: Log object
    /// trackers: object (map of tracker keys to field names)
    /// Returns Promise<void>.
    #[wasm_bindgen(js_name = writeLog)]
    pub fn write_log(&self, log: &Log, trackers: JsValue) -> js_sys::Promise {
        let inner = self.inner.clone();
        let log_inner = log.inner.clone();

        future_to_promise(async move {
            // Convert JsValue to HashMap<String, String>
            let trackers_map: HashMap<String, String> = if trackers.is_object() {
                serde_wasm_bindgen::from_value(trackers)
                    .map_err(|e| JsValue::from_str(&format!("Invalid trackers object: {}", e)))?
            } else {
                HashMap::new()
            };

            inner
                .write_log(&log_inner, &trackers_map)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to write log: {}", e)))?;

            Ok(JsValue::undefined())
        })
    }

    /// Start a new session.
    ///
    /// If there's an active session, it will be stopped at the start time.
    /// Validates that start_time is not in the future and doesn't conflict
    /// with existing sessions.
    ///
    /// title: optional string title
    /// role: optional string role
    /// impact: optional string impact
    /// mode: optional string mode
    /// subject: optional string subject
    /// trackers: optional array of tracker IDs
    /// startTime: optional JS Date object (defaults to now)
    /// note: optional string note
    /// Returns Promise<void>.
    #[wasm_bindgen(js_name = startSession)]
    pub fn start_session(
        &self,
        title: Option<String>,
        role: Option<String>,
        impact: Option<String>,
        mode: Option<String>,
        subject: Option<String>,
        trackers: Option<Vec<String>>,
        start_time: Option<js_sys::Date>,
        note: Option<String>,
    ) -> js_sys::Promise {
        let workspace = match &self.workspace {
            Some(ws) => ws.clone(),
            None => {
                return js_sys::Promise::reject(&JsValue::from_str(
                    "LogManager has no workspace reference",
                ));
            }
        };

        let inner = self.inner.clone();

        future_to_promise(async move {
            let start = match start_time {
                Some(dt) => js_date_to_chrono(&dt)?,
                None => workspace.now(),
            };

            inner
                .start_session(
                    title,
                    role,
                    impact,
                    mode,
                    subject,
                    trackers.unwrap_or_default(),
                    start,
                    note,
                )
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to start session: {}", e)))?;

            Ok(JsValue::undefined())
        })
    }

    /// Stop the currently active session.
    ///
    /// Returns Promise<void>.
    #[wasm_bindgen(js_name = stopCurrentSession)]
    pub fn stop_current_session(&self) -> js_sys::Promise {
        let inner = self.inner.clone();

        future_to_promise(async move {
            inner
                .stop_current_session()
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to stop session: {}", e)))?;

            Ok(JsValue::undefined())
        })
    }

    /// Replace a field value across all log sessions.
    ///
    /// field: string field name (role, impact, mode, subject)
    /// old_value: string old value to replace
    /// new_value: string new value
    /// trackers: object (map of tracker keys to field names)
    /// Returns Promise<{logsUpdated: number, sessionsUpdated: number}>.
    #[wasm_bindgen(js_name = replaceFieldInAllLogs)]
    pub fn replace_field_in_all_logs(
        &self,
        field: &str,
        old_value: &str,
        new_value: &str,
        trackers: JsValue,
    ) -> js_sys::Promise {
        let inner = self.inner.clone();
        let field = field.to_string();
        let old_value = old_value.to_string();
        let new_value = new_value.to_string();

        future_to_promise(async move {
            // Convert JsValue to HashMap<String, String>
            let trackers_map: HashMap<String, String> = if trackers.is_object() {
                serde_wasm_bindgen::from_value(trackers)
                    .map_err(|e| JsValue::from_str(&format!("Invalid trackers object: {}", e)))?
            } else {
                HashMap::new()
            };

            let (logs_updated, sessions_updated) = inner
                .replace_field_in_all_logs(&field, &old_value, &new_value, &trackers_map)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to replace field: {}", e)))?;

            let obj = js_sys::Object::new();
            js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("logsUpdated"),
                &JsValue::from_f64(logs_updated as f64),
            )?;
            js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("sessionsUpdated"),
                &JsValue::from_f64(sessions_updated as f64),
            )?;

            Ok(JsValue::from(obj))
        })
    }

    /// Get usage statistics for a field across all logs.
    ///
    /// field: string field name (role, impact, mode, subject)
    /// Returns Promise<{sessionCount: Map<string, number>, logDates: Map<string, Date[]>}>.
    #[wasm_bindgen(js_name = getFieldUsageStats)]
    pub fn get_field_usage_stats(&self, field: &str) -> js_sys::Promise {
        let inner = self.inner.clone();
        let field = field.to_string();

        future_to_promise(async move {
            let (session_count, log_dates) = inner
                .get_field_usage_stats(&field)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get stats: {}", e)))?;

            // Convert session counts to JS Map
            let session_map = js_sys::Map::new();
            for (key, value) in session_count {
                session_map.set(&JsValue::from_str(&key), &JsValue::from_f64(value as f64));
            }

            // Convert log dates to JS Map of arrays
            let dates_map = js_sys::Map::new();
            for (key, dates) in log_dates {
                let date_array = js_sys::Array::new();
                for date in dates {
                    date_array.push(&naive_date_to_js_date(&date));
                }
                dates_map.set(&JsValue::from_str(&key), &date_array);
            }

            let obj = js_sys::Object::new();
            js_sys::Reflect::set(&obj, &JsValue::from_str("sessionCount"), &session_map)?;
            js_sys::Reflect::set(&obj, &JsValue::from_str("logDates"), &dates_map)?;

            Ok(JsValue::from(obj))
        })
    }
}

// Helper functions for date/time conversion

fn js_date_to_chrono(date: &js_sys::Date) -> Result<chrono::DateTime<chrono_tz::Tz>, JsValue> {
    let timestamp_ms = date.get_time() as i64;
    let timestamp_secs = timestamp_ms / 1000;
    let timestamp_nanos = ((timestamp_ms % 1000) * 1_000_000) as u32;

    let dt = chrono::DateTime::from_timestamp(timestamp_secs, timestamp_nanos)
        .ok_or_else(|| JsValue::from_str("Invalid timestamp"))?;

    Ok(dt.with_timezone(&chrono_tz::UTC))
}

fn js_date_to_naive_date(date: &js_sys::Date) -> Result<chrono::NaiveDate, JsValue> {
    let year = date.get_utc_full_year() as i32;
    let month = date.get_utc_month() + 1; // JS months are 0-indexed
    let day = date.get_utc_date();

    chrono::NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| JsValue::from_str("Invalid date"))
}

fn naive_date_to_js_date(date: &chrono::NaiveDate) -> js_sys::Date {
    js_sys::Date::new_with_year_month_day(
        date.year() as u32,
        (date.month() - 1) as i32, // JS months are 0-indexed
        date.day() as i32,
    )
}
