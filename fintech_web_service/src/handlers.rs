//! Handler functions

use crate::errors::{WebServiceAccountingError, WebServiceStringError};
use fintech_common::errors::SIGNER_NAME_NOT_VALID_MSG;
use fintech_common::requests::*;
use fintech_common::trading_platform::TradingPlatform;
use fintech_common::types::Order;
use fintech_common::validation;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::{Rejection, Reply};

/// **Basic input validation for a signer's name**
///
/// Checks for:
/// - An empty string.
fn is_valid_name(signer: &str) -> Result<(), Rejection> {
    match validation::is_valid_name(signer) {
        Some(msg) => {
            let ret_msg = format!("{}: \"{}\". {}", SIGNER_NAME_NOT_VALID_MSG, signer, msg);
            log::warn!("{}", ret_msg);
            Err(warp::reject::custom(WebServiceStringError(ret_msg)))
        }
        None => Ok(()),
    }
}

/// The `deposit` handler
///
/// POST
pub async fn deposit(
    request: AccountUpdateRequest,
    trading_platform: Arc<Mutex<TradingPlatform>>,
) -> Result<impl Reply, Rejection> {
    log::debug!("deposit; request = {:?}", request);

    match trading_platform
        .lock()
        .await
        .deposit(&request.signer, request.amount)
    {
        Ok(tx) => Ok(warp::reply::json(&tx)),
        Err(acc_err) => Err(warp::reject::custom(WebServiceAccountingError(acc_err))),
    }
}

/// The `withdraw` handler
///
/// POST
pub async fn withdraw(
    request: AccountUpdateRequest,
    trading_platform: Arc<Mutex<TradingPlatform>>,
) -> Result<impl Reply, Rejection> {
    log::debug!("withdraw; request = {:?}", request);

    match trading_platform
        .lock()
        .await
        .withdraw(&request.signer, request.amount)
    {
        Ok(tx) => Ok(warp::reply::json(&tx)),
        Err(acc_err) => Err(warp::reject::custom(WebServiceAccountingError(acc_err))),
    }
}

/// The `send` handler
///
/// POST
pub async fn send(
    request: AccountSendRequest,
    trading_platform: Arc<Mutex<TradingPlatform>>,
) -> Result<impl Reply, Rejection> {
    log::debug!("send; request = {:?}", request);

    if let Some(rejection) = is_valid_name(&request.sender).err() {
        return Err(rejection);
    }
    if let Some(rejection) = is_valid_name(&request.recipient).err() {
        return Err(rejection);
    }

    match trading_platform
        .lock()
        .await
        .send(&request.sender, &request.recipient, request.amount)
    {
        Ok(txs) => Ok(warp::reply::json(&txs)),
        Err(acc_err) => Err(warp::reject::custom(WebServiceAccountingError(acc_err))),
    }
}

/// The `balance_of` handler
///
/// Responds with the signer's balance.
///
/// POST
pub async fn balance_of(
    request: AccountBalanceRequest,
    trading_platform: Arc<Mutex<TradingPlatform>>,
) -> Result<impl Reply, Rejection> {
    log::debug!("balance_of; request = {:?}", request);

    if let Some(rejection) = is_valid_name(&request.signer).err() {
        return Err(rejection);
    }

    match trading_platform.lock().await.balance_of(&request.signer) {
        Ok(balance) => Ok(warp::reply::json(balance)),
        Err(acc_err) => Err(warp::reject::custom(WebServiceAccountingError(acc_err))),
    }
}

/// The `process_order` handler
///
/// POST
pub async fn process_order(
    order: Order,
    trading_platform: Arc<Mutex<TradingPlatform>>,
) -> Result<impl Reply, Rejection> {
    log::debug!("process_order; order = {:?}", order);

    match trading_platform.lock().await.process_order(order) {
        Ok(receipt) => Ok(warp::reply::json(&receipt)),
        Err(acc_err) => Err(warp::reject::custom(WebServiceAccountingError(acc_err))),
    }
}

/// The `order_history` handler
///
/// Responds with the entire ledger (all transactions ever) - transaction log - entire order history
///
/// GET /order/history
pub async fn order_history(
    trading_platform: Arc<Mutex<TradingPlatform>>,
) -> Result<impl Reply, Infallible> {
    log::debug!("order_history");
    let history = &trading_platform.lock().await.tx_log;
    let response = warp::reply::json(&history);
    Ok(response)
}

/// The `all_accounts` handler
///
/// Responds with all accounts and their balances
///
/// GET /accounts
pub async fn all_accounts(
    trading_platform: Arc<Mutex<TradingPlatform>>,
) -> Result<impl Reply, Infallible> {
    log::debug!("all_accounts");
    let accounts = &trading_platform.lock().await.accounts.accounts;
    let response = warp::reply::json(&accounts);
    Ok(response)
}

/// **Fetches the complete order book**
///
/// The `order_book` handler
///
/// Both sides are combined together.
///
/// Optionally `sort`s the book by the ordinal sequence number;
/// `desc` stands for descending (considered only if `sort` is `true`).
///
/// If `sort` or `desc` are `None`, they are treated as `false`.
///
/// By default, the order book isn't sorted.
///
/// If sorting is requested, the order is ascending by default.
///
/// GET /orderbook (sort=false and desc=false by default)
///
/// GET /orderbook?sort=true&desc=true
pub async fn order_book(
    request: OrderBookRequest,
    trading_platform: Arc<Mutex<TradingPlatform>>,
) -> Result<impl Reply, Infallible> {
    log::debug!("order_book; request = {:?}", request);
    let book = trading_platform
        .lock()
        .await
        .order_book(request.sort.unwrap_or(false), request.desc.unwrap_or(false));
    let response = warp::reply::json(&book);
    Ok(response)
}

/// **Fetches the complete order book sorted by price**
///
/// The `order_book_by_price` handler
///
/// Both sides are combined together.
///
/// Sorted first by price points ascending;
/// the optional query parameter `desc` is for descending order.
///
/// If `desc` isn't provided, it is treated as `false`.
///
/// Inside of a price point, always ordered ascending by the ordinal sequence number.
///
/// GET /orderbookbyprice (desc=false by default)
///
/// GET /orderbookbyprice?desc=true
pub async fn order_book_by_price(
    request: OrderBookByPriceRequest,
    trading_platform: Arc<Mutex<TradingPlatform>>,
) -> Result<impl Reply, Infallible> {
    log::debug!("order_book_by_price; request = {:?}", request);
    let book = trading_platform
        .lock()
        .await
        .order_book_by_price(request.desc.unwrap_or(false));
    let response = warp::reply::json(&book);
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::is_valid_name;

    #[test]
    fn test_valid_name_passes() {
        assert!(is_valid_name("Ivan").is_ok());
    }

    #[test]
    fn test_empty_name_fails() {
        assert!(is_valid_name("").is_err());
    }
}
