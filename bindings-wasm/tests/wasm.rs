#![cfg(target_arch = "wasm32")]

use faff_core_wasm::*;
use wasm_bindgen_test::*;

// Tests will run in Node.js when using: wasm-pack test --node

#[wasm_bindgen_test]
fn test_wasm_module_loads() {
    // Basic smoke test - module loads without panic
    assert!(true);
}

// TODO: Add integration tests with mock JsStorage once we have test infrastructure
// These would test:
// - Workspace creation with JsStorage
// - Log operations
// - Plan operations
// - Timesheet operations
