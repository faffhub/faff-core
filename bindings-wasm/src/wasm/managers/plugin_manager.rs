use wasm_bindgen::prelude::*;

/// PluginManager handles loading and running Python plugins.
///
/// NOTE: This is a stub implementation for WASM. The plugin system is primarily
/// designed for Python environments. This stub exists to maintain API parity,
/// but plugin functionality is not available in WASM/browser environments.
#[wasm_bindgen]
pub struct PluginManager {
    _private: (), // Prevents construction
}

#[wasm_bindgen]
impl PluginManager {
    /// Plugins are not supported in WASM environments.
    ///
    /// This constructor exists for API parity but will always throw an error.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<PluginManager, JsValue> {
        Err(JsValue::from_str(
            "PluginManager is not supported in WASM environments. Plugins require Python runtime."
        ))
    }
}

// Note: We intentionally do not implement from_rust() for PluginManager
// because the Workspace won't expose it in WASM builds (it requires the "python" feature).
