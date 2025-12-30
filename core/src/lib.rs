// Core modules
pub mod managers;
pub mod models;
pub mod storage;
pub mod workspace;

// Utilities
pub mod utils;

// Python plugin support
#[cfg(feature = "python")]
pub mod plugins;

// Re-export commonly used items for convenience
#[cfg(not(target_arch = "wasm32"))]
pub use storage::FileSystemStorage;
pub use storage::Storage;
pub use workspace::Workspace;
