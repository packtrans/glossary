use anyhow::{Result, bail};

/// Validates query `limit` (default 10, maximum 50).
pub fn validate_query_limit(limit: Option<usize>) -> Result<usize> {
    const DEFAULT: usize = 10;
    const MAX: usize = 50;
    match limit {
        None => Ok(DEFAULT),
        Some(0) => bail!("limit must be at least 1"),
        Some(n) if n > MAX => bail!("limit must be at most {MAX}"),
        Some(n) => Ok(n),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_limit_defaults_and_caps() {
        assert_eq!(validate_query_limit(None).unwrap(), 10);
        assert_eq!(validate_query_limit(Some(1)).unwrap(), 1);
        assert_eq!(validate_query_limit(Some(50)).unwrap(), 50);
        assert!(validate_query_limit(Some(0)).is_err());
        assert!(validate_query_limit(Some(51)).is_err());
    }
}
