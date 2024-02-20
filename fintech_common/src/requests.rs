//! The request types (also called models in warp examples)

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

#[derive(Deserialize)]
pub struct OrderBookRequest {
    pub sort: bool,
    pub desc: bool,
}

#[derive(Deserialize)]
pub struct OrderBookByPriceRequest {
    pub desc: bool,
}
