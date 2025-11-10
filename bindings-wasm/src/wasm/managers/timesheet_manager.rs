use super::super::models::Timesheet;
use super::super::storage::JsStorageAdapter;
use chrono::Datelike;
use faff_core::managers::TimesheetManager as RustTimesheetManager;
use faff_core::workspace::Workspace as RustWorkspace;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

/// TimesheetManager handles reading and writing cryptographically signed timesheets.
///
/// Timesheets are compiled from logs and submitted to external systems.
#[wasm_bindgen]
pub struct TimesheetManager {
    inner: Arc<RustTimesheetManager>,
    workspace: Option<Arc<RustWorkspace>>,
}

impl TimesheetManager {
    /// Create from Rust manager with workspace reference
    pub(crate) fn from_rust(
        manager: Arc<RustTimesheetManager>,
        workspace: Arc<RustWorkspace>,
    ) -> Self {
        Self {
            inner: manager,
            workspace: Some(workspace),
        }
    }
}

#[wasm_bindgen]
impl TimesheetManager {
    /// Create a new TimesheetManager with storage.
    ///
    /// storage: JsStorageAdapter
    /// Returns TimesheetManager.
    #[wasm_bindgen(constructor)]
    pub fn new(storage: JsStorageAdapter) -> TimesheetManager {
        let js_storage = super::super::storage::JsStorage::new(storage);
        let storage_arc: Arc<dyn faff_core::storage::Storage> = Arc::new(js_storage);

        Self {
            inner: Arc::new(RustTimesheetManager::new(storage_arc)),
            workspace: None,
        }
    }

    /// Write a timesheet to storage.
    ///
    /// timesheet: Timesheet object
    /// Returns Promise<void>.
    #[wasm_bindgen(js_name = writeTimesheet)]
    pub fn write_timesheet(&self, timesheet: &Timesheet) -> js_sys::Promise {
        let inner = self.inner.clone();
        let timesheet_inner = timesheet.inner.clone();

        future_to_promise(async move {
            inner
                .write_timesheet(&timesheet_inner)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to write timesheet: {}", e)))?;

            Ok(JsValue::undefined())
        })
    }

    /// Get a timesheet for a specific audience and date.
    ///
    /// audience_id: string audience ID
    /// date: JS Date object
    /// Returns Promise<Timesheet | null>.
    #[wasm_bindgen(js_name = getTimesheet)]
    pub fn get_timesheet(&self, audience_id: &str, date: js_sys::Date) -> js_sys::Promise {
        let inner = self.inner.clone();
        let audience_id = audience_id.to_string();

        future_to_promise(async move {
            let naive_date = js_date_to_naive_date(&date)?;

            let timesheet = inner
                .get_timesheet(&audience_id, naive_date)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get timesheet: {}", e)))?;

            match timesheet {
                Some(inner) => Ok(JsValue::from(Timesheet { inner })),
                None => Ok(JsValue::null()),
            }
        })
    }

    /// List all timesheets, optionally filtered by date.
    ///
    /// date: optional JS Date object to filter by
    /// Returns Promise<Timesheet[]>.
    #[wasm_bindgen(js_name = listTimesheets)]
    pub fn list_timesheets(&self, date: Option<js_sys::Date>) -> js_sys::Promise {
        let inner = self.inner.clone();

        future_to_promise(async move {
            let naive_date = date.as_ref().map(js_date_to_naive_date).transpose()?;

            let timesheets = inner
                .list_timesheets(naive_date)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to list timesheets: {}", e)))?;

            let array = js_sys::Array::new();
            for timesheet in timesheets {
                array.push(&JsValue::from(Timesheet { inner: timesheet }));
            }

            Ok(JsValue::from(array))
        })
    }

    /// Find timesheets that are stale (log has changed since compilation).
    ///
    /// Requires workspace reference.
    ///
    /// date: optional JS Date object to filter by
    /// Returns Promise<Timesheet[]>.
    #[wasm_bindgen(js_name = findStaleTimesheets)]
    pub fn find_stale_timesheets(&self, date: Option<js_sys::Date>) -> js_sys::Promise {
        let workspace = match &self.workspace {
            Some(ws) => ws.clone(),
            None => {
                return js_sys::Promise::reject(&JsValue::from_str(
                    "TimesheetManager has no workspace reference",
                ));
            }
        };

        let inner = self.inner.clone();

        future_to_promise(async move {
            let log_manager = workspace.logs();
            let naive_date = date.as_ref().map(js_date_to_naive_date).transpose()?;

            let stale = inner
                .find_stale_timesheets(log_manager, naive_date)
                .await
                .map_err(|e| {
                    JsValue::from_str(&format!("Failed to find stale timesheets: {}", e))
                })?;

            let array = js_sys::Array::new();
            for timesheet in stale {
                array.push(&JsValue::from(Timesheet { inner: timesheet }));
            }

            Ok(JsValue::from(array))
        })
    }

    /// Find timesheets with failed submissions.
    ///
    /// date: optional JS Date object to filter by
    /// Returns Promise<Timesheet[]>.
    #[wasm_bindgen(js_name = findFailedSubmissions)]
    pub fn find_failed_submissions(&self, date: Option<js_sys::Date>) -> js_sys::Promise {
        let inner = self.inner.clone();

        future_to_promise(async move {
            let naive_date = date.as_ref().map(js_date_to_naive_date).transpose()?;

            let failed = inner
                .find_failed_submissions(naive_date)
                .await
                .map_err(|e| {
                    JsValue::from_str(&format!("Failed to find failed submissions: {}", e))
                })?;

            let array = js_sys::Array::new();
            for timesheet in failed {
                array.push(&JsValue::from(Timesheet { inner: timesheet }));
            }

            Ok(JsValue::from(array))
        })
    }

    /// Sign a timesheet with the given signing identities.
    ///
    /// Requires workspace reference.
    ///
    /// timesheet: Timesheet object
    /// signing_ids: array of identity IDs to use for signing
    /// Returns Promise<Timesheet> - the signed timesheet.
    #[wasm_bindgen(js_name = signTimesheet)]
    pub fn sign_timesheet(
        &self,
        timesheet: &Timesheet,
        signing_ids: Vec<String>,
    ) -> js_sys::Promise {
        let workspace = match &self.workspace {
            Some(ws) => ws.clone(),
            None => {
                return js_sys::Promise::reject(&JsValue::from_str(
                    "TimesheetManager has no workspace reference",
                ));
            }
        };

        let inner = self.inner.clone();
        let timesheet_inner = timesheet.inner.clone();

        future_to_promise(async move {
            let identity_manager = workspace.identities();

            let signed = inner
                .sign_timesheet(&timesheet_inner, &signing_ids, identity_manager)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to sign timesheet: {}", e)))?;

            Ok(JsValue::from(Timesheet { inner: signed }))
        })
    }

    // TODO: Plugin-dependent methods to add after implementing PluginManager:
    // - audiences()
    // - getAudience(audience_id)
    // - compile(log, plugin)
    // - submit(timesheet)
}

// Helper functions for date conversion

fn js_date_to_naive_date(date: &js_sys::Date) -> Result<chrono::NaiveDate, JsValue> {
    let year = date.get_utc_full_year() as i32;
    let month = date.get_utc_month() + 1; // JS months are 0-indexed
    let day = date.get_utc_date();

    chrono::NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| JsValue::from_str("Invalid date"))
}
