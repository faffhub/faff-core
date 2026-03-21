use super::super::models::Plan;
use super::super::storage::JsStorageAdapter;
use chrono::Datelike;
use faff_core::managers::PlanManager as RustPlanManager;
use faff_core::workspace::Workspace as RustWorkspace;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

/// PlanManager handles reading and writing work plans and vocabularies.
///
/// Plans define the vocabulary (roles, modes, impacts, subjects) and
/// predefined intents that can be used in logs.
#[wasm_bindgen]
pub struct PlanManager {
    inner: Arc<RustPlanManager>,
    workspace: Option<Arc<RustWorkspace>>,
}

impl PlanManager {
    /// Create from Rust manager with workspace reference
    pub(crate) fn from_rust(manager: Arc<RustPlanManager>, workspace: Arc<RustWorkspace>) -> Self {
        Self {
            inner: manager,
            workspace: Some(workspace),
        }
    }
}

#[wasm_bindgen]
impl PlanManager {
    /// Create a new PlanManager with storage.
    ///
    /// storage: JsStorageAdapter
    /// Returns PlanManager.
    #[wasm_bindgen(constructor)]
    pub fn new(storage: JsStorageAdapter) -> PlanManager {
        let js_storage = super::super::storage::JsStorage::new(storage);
        let storage_arc: Arc<dyn faff_core::storage::Storage> = Arc::new(js_storage);

        Self {
            inner: Arc::new(RustPlanManager::new(storage_arc)),
            workspace: None,
        }
    }

    /// Get all plans valid for a given date.
    ///
    /// date: JS Date object
    /// Returns Promise<Map<string, Plan>> - mapping of source names to Plans.
    #[wasm_bindgen(js_name = getPlans)]
    pub fn get_plans(&self, date: js_sys::Date) -> js_sys::Promise {
        let inner = self.inner.clone();

        future_to_promise(async move {
            let naive_date = js_date_to_naive_date(&date)?;

            let plans = inner
                .get_plans(naive_date)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get plans: {}", e)))?;

            // Convert to JS Map
            let map = js_sys::Map::new();
            for (source, plan) in plans {
                map.set(
                    &JsValue::from_str(&source),
                    &JsValue::from(Plan { inner: plan }),
                );
            }

            Ok(JsValue::from(map))
        })
    }

    /// Get all roles from plans valid for a given date.
    ///
    /// date: JS Date object
    /// Returns Promise<string[]>.
    #[wasm_bindgen(js_name = getRoles)]
    pub fn get_roles(&self, date: js_sys::Date) -> js_sys::Promise {
        let inner = self.inner.clone();

        future_to_promise(async move {
            let naive_date = js_date_to_naive_date(&date)?;

            let roles = inner
                .get_roles(naive_date)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get roles: {}", e)))?;

            let array = js_sys::Array::new();
            for role in roles {
                array.push(&JsValue::from_str(&role));
            }

            Ok(JsValue::from(array))
        })
    }

    /// Get all impacts from plans valid for a given date.
    ///
    /// date: JS Date object
    /// Returns Promise<string[]>.
    #[wasm_bindgen(js_name = getImpacts)]
    pub fn get_impacts(&self, date: js_sys::Date) -> js_sys::Promise {
        let inner = self.inner.clone();

        future_to_promise(async move {
            let naive_date = js_date_to_naive_date(&date)?;

            let impacts = inner
                .get_impacts(naive_date)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get impacts: {}", e)))?;

            let array = js_sys::Array::new();
            for impact in impacts {
                array.push(&JsValue::from_str(&impact));
            }

            Ok(JsValue::from(array))
        })
    }

    /// Get all modes from plans valid for a given date.
    ///
    /// date: JS Date object
    /// Returns Promise<string[]>.
    #[wasm_bindgen(js_name = getModes)]
    pub fn get_modes(&self, date: js_sys::Date) -> js_sys::Promise {
        let inner = self.inner.clone();

        future_to_promise(async move {
            let naive_date = js_date_to_naive_date(&date)?;

            let modes = inner
                .get_modes(naive_date)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get modes: {}", e)))?;

            let array = js_sys::Array::new();
            for mode in modes {
                array.push(&JsValue::from_str(&mode));
            }

            Ok(JsValue::from(array))
        })
    }

    /// Get all subjects from plans valid for a given date.
    ///
    /// date: JS Date object
    /// Returns Promise<string[]>.
    #[wasm_bindgen(js_name = getSubjects)]
    pub fn get_subjects(&self, date: js_sys::Date) -> js_sys::Promise {
        let inner = self.inner.clone();

        future_to_promise(async move {
            let naive_date = js_date_to_naive_date(&date)?;

            let subjects = inner
                .get_subjects(naive_date)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get subjects: {}", e)))?;

            let array = js_sys::Array::new();
            for subject in subjects {
                array.push(&JsValue::from_str(&subject));
            }

            Ok(JsValue::from(array))
        })
    }

    /// Get all trackers from plans valid for a given date.
    ///
    /// date: JS Date object
    /// Returns Promise<Map<string, string>> - mapping of tracker IDs to names.
    #[wasm_bindgen(js_name = getTrackers)]
    pub fn get_trackers(&self, date: js_sys::Date) -> js_sys::Promise {
        let inner = self.inner.clone();

        future_to_promise(async move {
            let naive_date = js_date_to_naive_date(&date)?;

            let trackers = inner
                .get_trackers(naive_date)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get trackers: {}", e)))?;

            // Convert to JS Map
            let map = js_sys::Map::new();
            for (id, name) in trackers {
                map.set(&JsValue::from_str(&id), &JsValue::from_str(&name));
            }

            Ok(JsValue::from(map))
        })
    }

    /// Get the plan containing a specific tracker ID.
    ///
    /// tracker_id: string
    /// date: JS Date object
    /// Returns Promise<Plan | null>.
    #[wasm_bindgen(js_name = getPlanByTrackerId)]
    pub fn get_plan_by_tracker_id(&self, tracker_id: &str, date: js_sys::Date) -> js_sys::Promise {
        let inner = self.inner.clone();
        let tracker_id = tracker_id.to_string();

        future_to_promise(async move {
            let naive_date = js_date_to_naive_date(&date)?;

            let plan = inner
                .get_plan_by_tracker_id(&tracker_id, naive_date)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get plan: {}", e)))?;

            match plan {
                Some(inner) => Ok(JsValue::from(Plan { inner })),
                None => Ok(JsValue::null()),
            }
        })
    }

    /// Get the local plan for a given date (returns null if it doesn't exist).
    ///
    /// date: JS Date object
    /// Returns Promise<Plan | null>.
    #[wasm_bindgen(js_name = getLocalPlan)]
    pub fn get_local_plan(&self, date: js_sys::Date) -> js_sys::Promise {
        let inner = self.inner.clone();

        future_to_promise(async move {
            let naive_date = js_date_to_naive_date(&date)?;

            let plan = inner
                .get_local_plan(naive_date)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get plan: {}", e)))?;

            match plan {
                Some(inner) => Ok(JsValue::from(Plan { inner })),
                None => Ok(JsValue::null()),
            }
        })
    }

    /// Get the local plan for a given date (creates empty one if it doesn't exist).
    ///
    /// date: JS Date object
    /// Returns Promise<Plan>.
    #[wasm_bindgen(js_name = getLocalPlanOrCreate)]
    pub fn get_local_plan_or_create(&self, date: js_sys::Date) -> js_sys::Promise {
        let inner = self.inner.clone();

        future_to_promise(async move {
            let naive_date = js_date_to_naive_date(&date)?;

            let plan = inner
                .get_local_plan_or_create(naive_date)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get plan: {}", e)))?;

            Ok(JsValue::from(Plan { inner: plan }))
        })
    }

    /// Write a plan to storage.
    ///
    /// plan: Plan object
    /// Returns Promise<void>.
    #[wasm_bindgen(js_name = writePlan)]
    pub fn write_plan(&self, plan: &Plan) -> js_sys::Promise {
        let inner = self.inner.clone();
        let plan_inner = plan.inner.clone();

        future_to_promise(async move {
            inner
                .write_plan(&plan_inner)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to write plan: {}", e)))?;

            Ok(JsValue::undefined())
        })
    }

    /// Replace a field value across all plans.
    ///
    /// field: string field name (role, impact, mode, subject)
    /// old_value: string old value to replace
    /// new_value: string new value
    /// Returns Promise<number> - number of plans updated.
    #[wasm_bindgen(js_name = replaceFieldInAllPlans)]
    pub fn replace_field_in_all_plans(
        &self,
        field: &str,
        old_value: &str,
        new_value: &str,
    ) -> js_sys::Promise {
        let inner = self.inner.clone();
        let field = field.to_string();
        let old_value = old_value.to_string();
        let new_value = new_value.to_string();

        future_to_promise(async move {
            let plans_updated = inner
                .replace_field_in_all_plans(&field, &old_value, &new_value)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to replace field: {}", e)))?;

            Ok(JsValue::from_f64(plans_updated as f64))
        })
    }

    /// Get usage statistics for a field across all plans.
    ///
    /// field: string field name (role, impact, mode, subject)
    /// Returns Promise<Map<string, number>> - field value -> intent count.
    #[wasm_bindgen(js_name = getFieldUsageStats)]
    pub fn get_field_usage_stats(&self, field: &str) -> js_sys::Promise {
        let inner = self.inner.clone();
        let field = field.to_string();

        future_to_promise(async move {
            let stats = inner
                .get_field_usage_stats(&field)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get stats: {}", e)))?;

            // Convert to JS Map
            let map = js_sys::Map::new();
            for (key, value) in stats {
                map.set(&JsValue::from_str(&key), &JsValue::from_f64(value as f64));
            }

            Ok(JsValue::from(map))
        })
    }
}

// Helper functions for date conversion

fn js_date_to_naive_date(date: &js_sys::Date) -> Result<chrono::NaiveDate, JsValue> {
    let year = date.get_utc_full_year() as i32;
    let month = date.get_utc_month() + 1; // JS months are 0-indexed
    let day = date.get_utc_date();

    chrono::NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| JsValue::from_str("Invalid date"))
}
