use super::super::models::{Intent, Plan};
use super::super::storage::JsStorageAdapter;
use chrono::Datelike;
use faff_core::managers::PlanManager as RustPlanManager;
use faff_core::workspace::Workspace as RustWorkspace;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

/// PlanManager handles reading and writing work plans and vocabularies.
///
/// Plans define the vocabulary (roles, actions, objectives, subjects) and
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
                map.set(&JsValue::from_str(&source), &JsValue::from(Plan { inner: plan }));
            }

            Ok(JsValue::from(map))
        })
    }

    /// Get all intents from plans valid for a given date.
    ///
    /// date: JS Date object
    /// Returns Promise<Intent[]>.
    #[wasm_bindgen(js_name = getIntents)]
    pub fn get_intents(&self, date: js_sys::Date) -> js_sys::Promise {
        let inner = self.inner.clone();

        future_to_promise(async move {
            let naive_date = js_date_to_naive_date(&date)?;

            let intents = inner
                .get_intents(naive_date)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get intents: {}", e)))?;

            let array = js_sys::Array::new();
            for intent in intents {
                array.push(&JsValue::from(Intent { inner: intent }));
            }

            Ok(JsValue::from(array))
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

    /// Get all objectives from plans valid for a given date.
    ///
    /// date: JS Date object
    /// Returns Promise<string[]>.
    #[wasm_bindgen(js_name = getObjectives)]
    pub fn get_objectives(&self, date: js_sys::Date) -> js_sys::Promise {
        let inner = self.inner.clone();

        future_to_promise(async move {
            let naive_date = js_date_to_naive_date(&date)?;

            let objectives = inner
                .get_objectives(naive_date)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get objectives: {}", e)))?;

            let array = js_sys::Array::new();
            for objective in objectives {
                array.push(&JsValue::from_str(&objective));
            }

            Ok(JsValue::from(array))
        })
    }

    /// Get all actions from plans valid for a given date.
    ///
    /// date: JS Date object
    /// Returns Promise<string[]>.
    #[wasm_bindgen(js_name = getActions)]
    pub fn get_actions(&self, date: js_sys::Date) -> js_sys::Promise {
        let inner = self.inner.clone();

        future_to_promise(async move {
            let naive_date = js_date_to_naive_date(&date)?;

            let actions = inner
                .get_actions(naive_date)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get actions: {}", e)))?;

            let array = js_sys::Array::new();
            for action in actions {
                array.push(&JsValue::from_str(&action));
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

    /// Find an intent by ID across all plan files.
    ///
    /// intent_id: string
    /// Returns Promise<{source: string, intent: Intent, planFilePath: string} | null>.
    #[wasm_bindgen(js_name = findIntentById)]
    pub fn find_intent_by_id(&self, intent_id: &str) -> js_sys::Promise {
        let inner = self.inner.clone();
        let intent_id = intent_id.to_string();

        future_to_promise(async move {
            let result = inner
                .find_intent_by_id(&intent_id)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to find intent: {}", e)))?;

            match result {
                Some((source, intent, path)) => {
                    let obj = js_sys::Object::new();
                    js_sys::Reflect::set(&obj, &JsValue::from_str("source"), &JsValue::from_str(&source))?;
                    js_sys::Reflect::set(
                        &obj,
                        &JsValue::from_str("intent"),
                        &JsValue::from(Intent { inner: intent }),
                    )?;
                    js_sys::Reflect::set(
                        &obj,
                        &JsValue::from_str("planFilePath"),
                        &JsValue::from_str(&path.to_string_lossy()),
                    )?;
                    Ok(JsValue::from(obj))
                }
                None => Ok(JsValue::null()),
            }
        })
    }

    /// Update an intent by ID across all plan files.
    ///
    /// intent_id: string
    /// updated_intent: Intent object
    /// Returns Promise<Plan | null> - updated Plan or null if intent not found.
    #[wasm_bindgen(js_name = updateIntentById)]
    pub fn update_intent_by_id(&self, intent_id: &str, updated_intent: &Intent) -> js_sys::Promise {
        let inner = self.inner.clone();
        let intent_id = intent_id.to_string();
        let updated_intent = updated_intent.inner.clone();

        future_to_promise(async move {
            let result = inner
                .update_intent_by_id(&intent_id, updated_intent)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to update intent: {}", e)))?;

            match result {
                Some(inner) => Ok(JsValue::from(Plan { inner })),
                None => Ok(JsValue::null()),
            }
        })
    }

    /// Replace a field value across all plans.
    ///
    /// field: string field name (role, objective, action, subject)
    /// old_value: string old value to replace
    /// new_value: string new value
    /// Returns Promise<{plansUpdated: number, intentsUpdated: number}>.
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
            let (plans_updated, intents_updated) = inner
                .replace_field_in_all_plans(&field, &old_value, &new_value)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to replace field: {}", e)))?;

            let obj = js_sys::Object::new();
            js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("plansUpdated"),
                &JsValue::from_f64(plans_updated as f64),
            )?;
            js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("intentsUpdated"),
                &JsValue::from_f64(intents_updated as f64),
            )?;

            Ok(JsValue::from(obj))
        })
    }

    /// Get usage statistics for a field across all plans.
    ///
    /// field: string field name (role, objective, action, subject)
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
