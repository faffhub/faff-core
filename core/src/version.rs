/// Get the version of the faff-core library
pub fn version() -> String {
    let pkg_version = env!("CARGO_PKG_VERSION");
    let git_sha = option_env!("VERGEN_GIT_SHA").unwrap_or("unknown");
    let git_branch = option_env!("VERGEN_GIT_BRANCH").unwrap_or("unknown");
    let git_time = option_env!("VERGEN_GIT_COMMIT_TIMESTAMP").unwrap_or("unknown");
    let git_dirty = option_env!("VERGEN_GIT_DIRTY").unwrap_or("false");

    if git_sha == "unknown" {
        pkg_version.to_string()
    } else if git_dirty == "true" {
        format!("{pkg_version} ({git_branch} {git_sha}-dirty, {git_time})")
    } else {
        format!("{pkg_version} ({git_branch} {git_sha}, {git_time})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_exists() {
        let v = version();
        assert!(!v.is_empty(), "Version should not be empty");
        // Testing dirty flag
    }
}
