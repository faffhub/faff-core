use super::models::{Log, Plan};
use super::storage::JsStorage;
use chrono::{Datelike, NaiveDate};
use faff_core::models::{Config as RustConfig, Log as RustLog, Plan as RustPlan};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::future_to_promise;

/// Workspace provides coordinated access to faff functionality.
///
/// All methods are async since they interact with JavaScript storage.
#[wasm_bindgen]
pub struct Workspace {
    storage: JsStorage,
    config: RustConfig,
}

#[wasm_bindgen]
impl Workspace {
    /// Create a new workspace with the given storage adapter.
    ///
    /// Returns Promise<Workspace>.
    #[wasm_bindgen(constructor)]
    pub fn new(storage: JsStorage) -> js_sys::Promise {
        future_to_promise(async move {
            let workspace = Workspace::create_async(storage)
                .await
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            Ok(JsValue::from(workspace))
        })
    }

    /// Get a log for the specified date.
    ///
    /// Returns Promise<Log>.
    #[wasm_bindgen(js_name = getLog)]
    pub fn get_log(&self, date: js_sys::Date) -> js_sys::Promise {
        let storage: JsStorage = self.storage.clone().unchecked_into();
        let timezone = self.config.timezone;

        future_to_promise(async move {
            let naive_date = js_date_to_naive_date(&date)?;
            let log_path = Self::log_path_for_date(&storage, &naive_date);

            let log = if storage.exists(&log_path) {
                let content = storage
                    .read_string(&log_path)
                    .await
                    .map_err(|e| JsValue::from_str(&format!("Failed to read log: {:?}", e)))?;

                let rust_log = RustLog::from_log_file(&content)
                    .map_err(|e| JsValue::from_str(&format!("Failed to parse log: {}", e)))?;

                Log { inner: rust_log }
            } else {
                // Return empty log if file doesn't exist
                Log {
                    inner: RustLog::new(naive_date, timezone, vec![]),
                }
            };

            Ok(JsValue::from(log))
        })
    }

    /// Save a log to disk.
    ///
    /// Returns Promise<void>.
    #[wasm_bindgen(js_name = saveLog)]
    pub fn save_log(&self, log: &Log) -> js_sys::Promise {
        let storage: JsStorage = self.storage.clone().unchecked_into();
        let log_inner = log.inner.clone();

        future_to_promise(async move {
            let log_path = Self::log_path_for_date(&storage, &log_inner.date);
            let trackers = HashMap::new(); // TODO: Get trackers from config/plan
            let content = log_inner.to_log_file(&trackers);

            // Ensure log directory exists
            let log_dir = storage.log_dir();
            storage
                .create_dir_all(&log_dir)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to create log dir: {:?}", e)))?;

            storage
                .write_string(&log_path, &content)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to write log: {:?}", e)))?;

            Ok(JsValue::undefined())
        })
    }

    /// Get a plan by ID.
    ///
    /// Returns Promise<Plan>.
    #[wasm_bindgen(js_name = getPlan)]
    pub fn get_plan(&self, plan_id: &str) -> js_sys::Promise {
        let storage: JsStorage = self.storage.clone().unchecked_into();
        let plan_id = plan_id.to_string();

        future_to_promise(async move {
            let plan_path = format!("{}/{}.toml", storage.plan_dir(), plan_id);

            let content = storage
                .read_string(&plan_path)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to read plan: {:?}", e)))?;

            let rust_plan: RustPlan = toml::from_str(&content)
                .map_err(|e| JsValue::from_str(&format!("Failed to parse plan: {}", e)))?;

            let plan = Plan { inner: rust_plan };
            Ok(JsValue::from(plan))
        })
    }

    /// Save a plan to disk.
    ///
    /// Returns Promise<void>.
    #[wasm_bindgen(js_name = savePlan)]
    pub fn save_plan(&self, plan: &Plan) -> js_sys::Promise {
        let storage: JsStorage = self.storage.clone().unchecked_into();
        let plan_inner = plan.inner.clone();

        future_to_promise(async move {
            let plan_id = plan_inner.id();
            let plan_path = format!("{}/{}.toml", storage.plan_dir(), plan_id);

            let content = plan_inner
                .to_toml()
                .map_err(|e| JsValue::from_str(&format!("Failed to serialize plan: {}", e)))?;

            // Ensure plan directory exists
            let plan_dir = storage.plan_dir();
            storage
                .create_dir_all(&plan_dir)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to create plan dir: {:?}", e)))?;

            storage
                .write_string(&plan_path, &content)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to write plan: {:?}", e)))?;

            Ok(JsValue::undefined())
        })
    }

    /// Get current time in configured timezone.
    #[wasm_bindgen(js_name = now)]
    pub fn now(&self) -> js_sys::Date {
        let now_utc = chrono::Utc::now();
        let now_local = now_utc.with_timezone(&self.config.timezone);
        chrono_to_js_date(&now_local)
    }

    /// Get today's date in configured timezone.
    #[wasm_bindgen(js_name = today)]
    pub fn today(&self) -> js_sys::Date {
        let today = self.now_rust().date_naive();
        naive_date_to_js_date(&today)
    }

    /// Get configured timezone name.
    #[wasm_bindgen(js_name = timezone)]
    pub fn timezone(&self) -> String {
        self.config.timezone.name().to_string()
    }
}

// Internal methods
impl Workspace {
    async fn create_async(storage: JsStorage) -> Result<Workspace, anyhow::Error> {
        let config_path = storage.config_file();
        let config_content = storage
            .read_string(&config_path)
            .await
            .map_err(|_| anyhow::anyhow!("Failed to read config file"))?;

        let config = RustConfig::from_toml(&config_content)?;

        Ok(Workspace { storage, config })
    }

    fn log_path_for_date(storage: &JsStorage, date: &NaiveDate) -> String {
        format!("{}/{}.toml", storage.log_dir(), date.format("%Y-%m-%d"))
    }

    fn now_rust(&self) -> chrono::DateTime<chrono_tz::Tz> {
        chrono::Utc::now().with_timezone(&self.config.timezone)
    }
}

// Helper functions for date/time conversion (duplicated from models.rs for now)

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

fn chrono_to_js_date(dt: &chrono::DateTime<chrono_tz::Tz>) -> js_sys::Date {
    let timestamp_ms = dt.timestamp_millis() as f64;
    js_sys::Date::new(&JsValue::from_f64(timestamp_ms))
}
