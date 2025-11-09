use super::super::storage::JsStorageAdapter;
use faff_core::managers::IdentityManager as RustIdentityManager;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

/// IdentityManager handles cryptographic identities for signing timesheets.
///
/// Manages Ed25519 keypairs used to cryptographically sign work records.
#[wasm_bindgen]
pub struct IdentityManager {
    inner: Arc<RustIdentityManager>,
}

impl IdentityManager {
    /// Create from Rust manager
    pub(crate) fn from_rust(manager: Arc<RustIdentityManager>) -> Self {
        Self { inner: manager }
    }
}

#[wasm_bindgen]
impl IdentityManager {
    /// Create a new IdentityManager with storage.
    ///
    /// storage: JsStorageAdapter
    /// Returns IdentityManager.
    #[wasm_bindgen(constructor)]
    pub fn new(storage: JsStorageAdapter) -> IdentityManager {
        let js_storage = super::super::storage::JsStorage::new(storage);
        let storage_arc: Arc<dyn faff_core::storage::Storage> = Arc::new(js_storage);

        Self {
            inner: Arc::new(RustIdentityManager::new(storage_arc)),
        }
    }

    /// Create a new Ed25519 identity keypair.
    ///
    /// name: string identity name
    /// overwrite: optional boolean, whether to overwrite if identity already exists (default: false)
    /// Returns Promise<{signingKey: Uint8Array, verifyingKey: Uint8Array}>.
    #[wasm_bindgen(js_name = createIdentity)]
    pub fn create_identity(&self, name: &str, overwrite: Option<bool>) -> js_sys::Promise {
        let inner = self.inner.clone();
        let name = name.to_string();
        let overwrite = overwrite.unwrap_or(false);

        future_to_promise(async move {
            let signing_key = inner
                .create_identity(&name, overwrite)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to create identity: {}", e)))?;

            let obj = js_sys::Object::new();

            // Convert signing key to Uint8Array
            let signing_bytes = signing_key.to_bytes();
            let signing_array = js_sys::Uint8Array::from(&signing_bytes[..]);
            js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("signingKey"),
                &signing_array,
            )?;

            // Convert verifying key to Uint8Array
            let verifying_key = signing_key.verifying_key();
            let verifying_bytes = verifying_key.as_bytes();
            let verifying_array = js_sys::Uint8Array::from(&verifying_bytes[..]);
            js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("verifyingKey"),
                &verifying_array,
            )?;

            Ok(JsValue::from(obj))
        })
    }

    /// Get a specific identity by name.
    ///
    /// name: string identity name
    /// Returns Promise<Uint8Array | null> - the private signing key, or null if not found.
    #[wasm_bindgen(js_name = getIdentity)]
    pub fn get_identity(&self, name: &str) -> js_sys::Promise {
        let inner = self.inner.clone();
        let name = name.to_string();

        future_to_promise(async move {
            let signing_key = inner
                .get_identity(&name)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to get identity: {}", e)))?;

            match signing_key {
                Some(key) => {
                    let bytes = key.to_bytes();
                    let array = js_sys::Uint8Array::from(&bytes[..]);
                    Ok(JsValue::from(array))
                }
                None => Ok(JsValue::null()),
            }
        })
    }

    /// List all identities.
    ///
    /// Returns Promise<Map<string, Uint8Array>> - mapping of identity names to signing keys.
    #[wasm_bindgen(js_name = listIdentities)]
    pub fn list_identities(&self) -> js_sys::Promise {
        let inner = self.inner.clone();

        future_to_promise(async move {
            let identities = inner
                .list_identities()
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to list identities: {}", e)))?;

            // Convert to JS Map
            let map = js_sys::Map::new();
            for (name, key) in identities {
                let bytes = key.to_bytes();
                let array = js_sys::Uint8Array::from(&bytes[..]);
                map.set(&JsValue::from_str(&name), &array);
            }

            Ok(JsValue::from(map))
        })
    }

    /// Check if an identity exists.
    ///
    /// name: string identity name
    /// Returns: boolean
    #[wasm_bindgen(js_name = identityExists)]
    pub fn identity_exists(&self, name: &str) -> bool {
        self.inner.identity_exists(name)
    }

    /// Delete an identity.
    ///
    /// name: string identity name
    /// Returns Promise<void>.
    #[wasm_bindgen(js_name = deleteIdentity)]
    pub fn delete_identity(&self, name: &str) -> js_sys::Promise {
        let inner = self.inner.clone();
        let name = name.to_string();

        future_to_promise(async move {
            inner
                .delete_identity(&name)
                .await
                .map_err(|e| JsValue::from_str(&format!("Failed to delete identity: {}", e)))?;

            Ok(JsValue::undefined())
        })
    }
}
