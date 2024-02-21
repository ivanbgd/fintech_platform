use crate::errors::SIGNER_EMPTY_NAME_MSG;

/// **Basic input validation for a signer's name**
///
/// Checks for:
/// - An empty string.
///
/// Returns an optional message with the reason for failure.
///
/// If the name is valid, the return value is `None`.
///
/// This is useful in the general case in which validation can fail
/// for multiple reasons, and we want to differentiate between them.
pub fn is_valid_name(signer: &str) -> Option<&str> {
    if signer.trim().is_empty() {
        return Some(SIGNER_EMPTY_NAME_MSG);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::is_valid_name;

    #[test]
    fn test_valid_name_passes() {
        assert!(is_valid_name("Ivan").is_none());
    }

    #[test]
    fn test_empty_name_fails() {
        assert!(is_valid_name("").is_some());
    }
}
