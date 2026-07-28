use std::path::Path;

use anyhow::{Result, bail};

/// Sanitizes a string so it can be safely used as a path component.
pub fn sanitize_path_part(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect();

    let trimmed = sanitized.trim_matches(['-', '.', '_']);
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Validates that `value` is a single non-empty path segment without traversal.
pub fn validate_path_segment(value: &str, kind: &str) -> Result<()> {
    if value.is_empty() {
        bail!("{kind} must not be empty");
    }
    let mut components = Path::new(value).components();
    let is_single_normal_component =
        matches!(components.next(), Some(std::path::Component::Normal(_)))
            && components.next().is_none();
    if value.contains('\\') || !is_single_normal_component {
        bail!("{kind} contains invalid path component: {value}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_path_segment_rejects_dot_segments() {
        assert!(validate_path_segment(".", "name").is_err());
        assert!(validate_path_segment("..", "name").is_err());
        assert!(validate_path_segment("foo/bar", "name").is_err());
        assert!(validate_path_segment("zh_cn", "lang").is_ok());
    }
}
