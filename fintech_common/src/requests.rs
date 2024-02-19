use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct AccountBalanceRequest {
    pub signer: String,
}

#[derive(Deserialize, Serialize)]
pub struct AccountUpdateRequest {
    pub signer: String,
    pub amount: u64,
}

#[derive(Deserialize, Serialize)]
pub struct SendRequest {
    pub sender: String,
    pub recipient: String,
    pub amount: u64,
}
