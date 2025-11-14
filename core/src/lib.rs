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
pub use storage::{FileSystemStorage, Storage};
pub use workspace::Workspace;
