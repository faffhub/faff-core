use super::models::Log;
use super::storage::{JsStorage, JsStorageAdapter};
use chrono::Datelike;
use faff_core::workspace::Workspace as RustWorkspace;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

/// Workspace provides coordinated access to faff functionality in WASM.
///
/// This wraps the actual faff_core::Workspace and exposes its functionality
/// through JavaScript Promises.
#[wasm_bindgen]
pub struct Workspace {
    inner: Arc<RustWorkspace>,
}

#[wasm_bindgen]
impl Workspace {
    /// Create a new workspace with the given storage adapter.
    ///
    /// Returns Promise<Workspace>.
    #[wasm_bindgen(constructor)]
    pub fn new(storage: JsStorageAdapter) -> js_sys::Promise {
        future_to_promise(async move {
            // Create JsStorage wrapper
            let js_storage = JsStorage::new(storage);

            // Create the real Workspace
            let workspace = RustWorkspace::with_storage(Arc::new(js_storage))
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to create workspace: {}", e)))?;

            let wasm_workspace = Workspace {
                inner: Arc::new(workspace),
            };

            Ok(JsValue::from(wasm_workspace))
        })
    }

    /// Get current time in configured timezone as JS Date.
    #[wasm_bindgen(js_name = now)]
    pub fn now(&self) -> js_sys::Date {
        let now = self.inner.now();
        chrono_to_js_date(&now)
    }

    /// Get today's date in configured timezone as JS Date.
    #[wasm_bindgen(js_name = today)]
    pub fn today(&self) -> js_sys::Date {
        let today = self.inner.today();
        naive_date_to_js_date(&today)
    }

    /// Get configured timezone name.
    #[wasm_bindgen(js_name = timezone)]
    pub fn timezone(&self) -> String {
        self.inner.timezone().name().to_string()
    }

    /// Check if a log exists for the given date.
    ///
    /// date: JS Date object
    /// Returns: boolean
    #[wasm_bindgen(js_name = logExists)]
    pub fn log_exists(&self, date: js_sys::Date) -> bool {
        if let Ok(naive_date) = js_date_to_naive_date(&date) {
            self.inner.logs().log_exists(naive_date)
        } else {
            false
        }
    }

    /// Get a log for the specified date.
    ///
    /// date: JS Date object
    /// Returns Promise<Log | null>.
    #[wasm_bindgen(js_name = getLog)]
    pub fn get_log(&self, date: js_sys::Date) -> js_sys::Promise {
        let logs = self.inner.logs().clone();

        future_to_promise(async move {
            let naive_date = js_date_to_naive_date(&date)?;

            let log = logs.get_log(naive_date)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get log: {}", e)))?;

            match log {
                Some(inner) => {
                    let log = Log { inner };
                    Ok(JsValue::from(log))
                }
                None => Ok(JsValue::null()),
            }
        })
    }

    /// List all log dates.
    ///
    /// Returns Promise<Date[]>.
    #[wasm_bindgen(js_name = listLogs)]
    pub fn list_logs(&self) -> js_sys::Promise {
        let logs = self.inner.logs().clone();

        future_to_promise(async move {
            let dates = logs.list_logs()
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to list logs: {}", e)))?;

            let array = js_sys::Array::new();
            for date in dates {
                array.push(&naive_date_to_js_date(&date));
            }

            Ok(JsValue::from(array))
        })
    }

    /// Get plans for a specific date.
    ///
    /// date: JS Date object
    /// Returns Promise<object> with plan data.
    #[wasm_bindgen(js_name = getPlans)]
    pub fn get_plans(&self, date: js_sys::Date) -> js_sys::Promise {
        let plans = self.inner.plans().clone();

        future_to_promise(async move {
            let naive_date = js_date_to_naive_date(&date)?;

            let plan_map = plans.get_plans(naive_date)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get plans: {}", e)))?;

            // Convert to JS object
            let obj = js_sys::Object::new();
            js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("count"),
                &JsValue::from_f64(plan_map.len() as f64),
            )?;

            Ok(JsValue::from(obj))
        })
    }

    /// Get trackers for a specific date.
    ///
    /// date: JS Date object
    /// Returns Promise<object> with tracker data.
    #[wasm_bindgen(js_name = getTrackers)]
    pub fn get_trackers(&self, date: js_sys::Date) -> js_sys::Promise {
        let plans = self.inner.plans().clone();

        future_to_promise(async move {
            let naive_date = js_date_to_naive_date(&date)?;

            let trackers = plans.get_trackers(naive_date)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get trackers: {}", e)))?;

            // Convert to JS object with tracker count
            let obj = js_sys::Object::new();
            js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("count"),
                &JsValue::from_f64(trackers.len() as f64),
            )?;

            Ok(JsValue::from(obj))
        })
    }

    /// Write a log to storage.
    ///
    /// log: Log object
    /// Returns Promise<void>.
    #[wasm_bindgen(js_name = writeLog)]
    pub fn write_log(&self, log: &Log) -> js_sys::Promise {
        let logs = self.inner.logs().clone();
        let plans = self.inner.plans().clone();
        let log_inner = log.inner.clone();

        future_to_promise(async move {
            let date = log_inner.date;

            // Get trackers for this date
            let trackers = plans.get_trackers(date)
                .await
                .unwrap_or_default();

            logs.write_log(&log_inner, &trackers)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to write log: {}", e)))?;

            Ok(JsValue::undefined())
        })
    }
}

// Helper functions for date/time conversion

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

fn chrono_to_js_date<T: chrono::TimeZone>(dt: &chrono::DateTime<T>) -> js_sys::Date {
    let timestamp_ms = dt.timestamp_millis() as f64;
    js_sys::Date::new(&JsValue::from_f64(timestamp_ms))
}
