/// **Basic input validation for a signer's name**
///
/// Checks for:
/// - An empty string.
pub fn is_valid_name(signer: &str) -> bool {
    !signer.trim().is_empty()
}
