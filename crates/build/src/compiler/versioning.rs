use crate::error::BuildError;
use semver::{Version, VersionReq};

/// Resolves the highest available version that satisfies the given semver requirement.
pub fn resolve_target_version(req_str: &str, available_versions: &[String]) -> Result<String, BuildError> {
    let req =
        VersionReq::parse(req_str).map_err(|e| BuildError::VersionResolution(format!("Invalid semver: {}", e)))?;

    let mut best_match: Option<Version> = None;

    for v_str in available_versions {
        if let Some(version) = Version::parse(v_str).ok().filter(|v| req.matches(v)) {
            match &best_match {
                Some(current) if version > *current => best_match = Some(version),
                None => best_match = Some(version),
                _ => {}
            }
        }
    }

    best_match
        .map(|v| v.to_string())
        .ok_or_else(|| BuildError::VersionResolution(format!("No available compiler versions satisfy {}", req_str)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_mock_versions() -> Vec<String> {
        vec![
            "0.3.0".to_string(),
            "0.3.1".to_string(),
            "0.4.0".to_string(),
            "0.4.1".to_string(),
            "0.4.2".to_string(),
            "0.5.0".to_string(),
            "1.0.0".to_string(),
            "1.0.1".to_string(),
            "1.1.0".to_string(),
            "2.0.0".to_string(),
        ]
    }

    #[test]
    fn test_resolve_caret() {
        let versions = get_mock_versions();
        // For < 1.0.0, caret allows patch updates but locks the minor version
        assert_eq!(resolve_target_version("^0.4.0", &versions).unwrap(), "0.4.2");
        assert_eq!(resolve_target_version("^0.3.0", &versions).unwrap(), "0.3.1");
        // For >= 1.0.0, caret allows minor and patch updates but locks the major version
        assert_eq!(resolve_target_version("^1.0.0", &versions).unwrap(), "1.1.0");
    }

    #[test]
    fn test_resolve_caret_prevents_breaking_changes() {
        let versions = get_mock_versions();
        // ^0.4.0 must NOT jump to 0.5.0
        assert_eq!(resolve_target_version("^0.4.0", &versions).unwrap(), "0.4.2");
        // ^1.0.0 must NOT jump to 2.0.0
        assert_eq!(resolve_target_version("^1.0.0", &versions).unwrap(), "1.1.0");
    }

    #[test]
    fn test_resolve_tilde() {
        let versions = get_mock_versions();
        assert_eq!(resolve_target_version("~0.4.0", &versions).unwrap(), "0.4.2");
        // Tilde always restricts to patch-level updates when major/minor are specified
        assert_eq!(resolve_target_version("~1.0.0", &versions).unwrap(), "1.0.1");
    }

    #[test]
    fn test_resolve_greater_than_equal() {
        let versions = get_mock_versions();
        // Should pick the absolute highest available version
        assert_eq!(resolve_target_version(">=0.4.0", &versions).unwrap(), "2.0.0");
    }

    #[test]
    fn test_resolve_exact() {
        let versions = get_mock_versions();
        assert_eq!(resolve_target_version("=1.0.0", &versions).unwrap(), "1.0.0");
    }

    #[test]
    fn test_resolve_wildcard() {
        let versions = get_mock_versions();
        assert_eq!(resolve_target_version("*", &versions).unwrap(), "2.0.0");
    }

    #[test]
    fn test_resolve_multiple_conditions() {
        let versions = get_mock_versions();
        assert_eq!(resolve_target_version(">=0.3.0, <0.4.1", &versions).unwrap(), "0.4.0");
        assert_eq!(resolve_target_version(">=1.0.0, <2.0.0", &versions).unwrap(), "1.1.0");
    }

    #[test]
    fn test_resolve_no_match() {
        let versions = get_mock_versions();
        assert!(resolve_target_version(">=3.0.0", &versions).is_err());
    }
}
