#![cfg(target_arch = "wasm32")]

use faff_core_wasm::*;
use wasm_bindgen_test::*;

// Tests will run in Node.js when using: wasm-pack test --node

#[wasm_bindgen_test]
fn test_wasm_module_loads() {
    // Basic smoke test - module loads without panic
    assert!(true);
}

#[wasm_bindgen_test]
fn test_intent_creation() {
    // Test creating an Intent through WASM bindings
    let intent = Intent::new(
        Some("test-intent".to_string()),
        Some("work".to_string()),
        Some("development".to_string()),
        Some("coding".to_string()),
        Some("rust".to_string()),
        None, // trackers
    );

    assert_eq!(intent.alias(), Some("test-intent".to_string()));
    assert_eq!(intent.role(), Some("work".to_string()));
}

#[wasm_bindgen_test]
fn test_intent_minimal() {
    // Test creating an Intent with minimal fields
    let intent = Intent::new(Some("minimal".to_string()), None, None, None, None, None);

    assert_eq!(intent.alias(), Some("minimal".to_string()));
    assert_eq!(intent.role(), None);
}

// TODO: Add integration tests with mock JsStorage once we have test infrastructure
// These would test:
// - Workspace creation with JsStorage
// - Log operations
// - Plan operations
// - Timesheet operations
