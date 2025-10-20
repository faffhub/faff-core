mod wasm;

use wasm_bindgen::prelude::*;

// Re-export the main types
pub use wasm::models::*;
pub use wasm::workspace::*;

#[wasm_bindgen(start)]
pub fn start() {
    // Set up console error panic hook for better error messages
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}
