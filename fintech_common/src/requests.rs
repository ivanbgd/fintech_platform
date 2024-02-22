//! The request types (also called models in warp examples)

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct AccountBalanceRequest {
    pub signer: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AccountUpdateRequest {
    pub signer: String,
    pub amount: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AccountSendRequest {
    pub sender: String,
    pub recipient: String,
    pub amount: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderBookRequest {
    pub sort: Option<bool>,
    pub desc: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderBookByPriceRequest {
    pub desc: Option<bool>,
}
