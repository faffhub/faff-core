/// Get the version of the faff-core library
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_exists() {
        let v = version();
        assert!(!v.is_empty(), "Version should not be empty");
    }
}
