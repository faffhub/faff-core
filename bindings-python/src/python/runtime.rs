use std::sync::OnceLock;
use tokio::runtime::Runtime;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// Returns the shared Tokio runtime for all Python binding methods.
///
/// Creating a new Runtime for every method call is expensive (thread spawn etc.).
/// This global instance is initialised once and reused across all manager calls.
pub fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| Runtime::new().expect("Failed to create Tokio runtime"))
}
