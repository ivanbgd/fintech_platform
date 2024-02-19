/// **An application-specific error type**
#[derive(Debug, PartialEq)]
pub enum AccountingError {
    AccountNotFound(String),
    AccountUnderFunded(String, u64),
    AccountOverFunded(String, u64),
}

pub const EMPTY_SIGNER_NAME: &str = "[ERROR] Signer's name cannot be empty.";
