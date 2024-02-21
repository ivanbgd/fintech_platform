/// **An application-specific error type**
#[derive(Debug, PartialEq)]
pub enum AccountingError {
    AccountNotFound(String),
    AccountUnderFunded(String, u64),
    AccountOverFunded(String, u64),
}

pub const SIGNER_NAME_NOT_VALID_MSG: &str = "The signer's name is not valid";
pub const SIGNER_EMPTY_NAME_MSG: &str = "Signer's name cannot be empty.";
