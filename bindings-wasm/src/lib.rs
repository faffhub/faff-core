#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

// Re-export the main types (only for wasm32)
#[cfg(target_arch = "wasm32")]
pub use wasm::managers::*;
#[cfg(target_arch = "wasm32")]
pub use wasm::models::*;
#[cfg(target_arch = "wasm32")]
pub use wasm::workspace::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    // Set up console error panic hook for better error messages
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}
