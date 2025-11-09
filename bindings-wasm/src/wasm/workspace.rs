use super::managers::{IdentityManager, LogManager, PlanManager, TimesheetManager};
use super::storage::{JsStorage, JsStorageAdapter};
use chrono::Datelike;
use faff_core::workspace::Workspace as RustWorkspace;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

/// Workspace provides coordinated access to faff functionality in WASM.
///
/// The Workspace owns managers for different aspects of the system:
/// - logs: LogManager for daily work logs
/// - plans: PlanManager for vocabulary and intents
/// - timesheets: TimesheetManager for compiled, signed records
/// - identities: IdentityManager for cryptographic signing keys
#[wasm_bindgen]
pub struct Workspace {
    inner: Arc<RustWorkspace>,
    // Cache the manager wrappers
    logs: LogManager,
    plans: PlanManager,
    timesheets: TimesheetManager,
    identities: IdentityManager,
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

            // Wrap in Arc so we can share it with managers
            let inner_arc = Arc::new(workspace);

            // Create manager wrappers from the Rust managers
            let logs = LogManager::from_rust(inner_arc.logs().clone(), inner_arc.clone());
            let plans = PlanManager::from_rust(
                Arc::new(inner_arc.plans().clone()),
                inner_arc.clone(),
            );
            let timesheets = TimesheetManager::from_rust(
                Arc::new(inner_arc.timesheets().clone()),
                inner_arc.clone(),
            );
            let identities =
                IdentityManager::from_rust(Arc::new(inner_arc.identities().clone()));

            let wasm_workspace = Workspace {
                inner: inner_arc,
                logs,
                plans,
                timesheets,
                identities,
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

    /// Get the LogManager.
    ///
    /// Returns: LogManager
    #[wasm_bindgen(getter)]
    pub fn logs(&self) -> LogManager {
        // We need to clone because wasm-bindgen requires returning by value
        // The LogManager itself contains Arc internally, so this is cheap
        LogManager::from_rust(self.inner.logs().clone(), self.inner.clone())
    }

    /// Get the PlanManager.
    ///
    /// Returns: PlanManager
    #[wasm_bindgen(getter)]
    pub fn plans(&self) -> PlanManager {
        PlanManager::from_rust(Arc::new(self.inner.plans().clone()), self.inner.clone())
    }

    /// Get the TimesheetManager.
    ///
    /// Returns: TimesheetManager
    #[wasm_bindgen(getter)]
    pub fn timesheets(&self) -> TimesheetManager {
        TimesheetManager::from_rust(
            Arc::new(self.inner.timesheets().clone()),
            self.inner.clone(),
        )
    }

    /// Get the IdentityManager.
    ///
    /// Returns: IdentityManager
    #[wasm_bindgen(getter)]
    pub fn identities(&self) -> IdentityManager {
        IdentityManager::from_rust(Arc::new(self.inner.identities().clone()))
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
