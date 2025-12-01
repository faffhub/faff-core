#[cfg(not(target_arch = "wasm32"))]
mod events;
#[cfg(not(target_arch = "wasm32"))]
mod file_system;
mod traits;

#[cfg(not(target_arch = "wasm32"))]
pub use events::{EventStreamHandle, StorageEvent};
#[cfg(not(target_arch = "wasm32"))]
pub use file_system::FileSystemStorage;
pub use traits::Storage;
