//! Handler functions

use crate::errors::{WebServiceAccountingError, WebServiceStringError};
use fintech_common::errors::EMPTY_SIGNER_NAME;
use fintech_common::trading_platform::TradingPlatform;
use fintech_common::types::Order;
use fintech_common::validation::is_valid_name;
use fintech_common::{
    AccountBalanceRequest, AccountUpdateRequest, OrderBookByPriceRequest, OrderBookRequest,
    SendRequest,
};
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::{Rejection, Reply};

/// The `balance_of` handler
///
/// Responds with the signer's balance.
pub async fn balance_of(
    request: AccountBalanceRequest,
    trading_platform: Arc<Mutex<TradingPlatform>>,
) -> Result<impl Reply, Rejection> {
    // todo: extract into fn
    if !is_valid_name(&request.signer) {
        // todo: add a log msg
        return Err(warp::reject::custom(WebServiceStringError(
            EMPTY_SIGNER_NAME.to_string(),
        )));
    }

    match trading_platform.lock().await.balance_of(&request.signer) {
        Ok(balance) => Ok(warp::reply::json(balance)),
        Err(acc_err) => Err(warp::reject::custom(WebServiceAccountingError(acc_err))),
    }
}

/// The `deposit` handler
pub async fn deposit(
    request: AccountUpdateRequest,
    trading_platform: Arc<Mutex<TradingPlatform>>,
) -> Result<impl Reply, Rejection> {
    if !is_valid_name(&request.signer) {
        return Err(warp::reject::custom(WebServiceStringError(
            EMPTY_SIGNER_NAME.to_string(),
        )));
    }

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
pub async fn withdraw(
    request: AccountUpdateRequest,
    trading_platform: Arc<Mutex<TradingPlatform>>,
) -> Result<impl Reply, Rejection> {
    if !is_valid_name(&request.signer) {
        return Err(warp::reject::custom(WebServiceStringError(
            EMPTY_SIGNER_NAME.to_string(),
        )));
    }

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
pub async fn send(
    request: SendRequest,
    trading_platform: Arc<Mutex<TradingPlatform>>,
) -> Result<impl Reply, Rejection> {
    if !is_valid_name(&request.sender) || !is_valid_name(&request.recipient) {
        return Err(warp::reject::custom(WebServiceStringError(
            EMPTY_SIGNER_NAME.to_string(),
        )));
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

/// The `process_order` handler
pub async fn process_order(
    order: Order,
    trading_platform: Arc<Mutex<TradingPlatform>>,
) -> Result<impl Reply, Rejection> {
    if !is_valid_name(&order.signer) {
        return Err(warp::reject::custom(WebServiceStringError(
            EMPTY_SIGNER_NAME.to_string(),
        )));
    }

    match trading_platform.lock().await.process_order(order) {
        Ok(receipt) => Ok(warp::reply::json(&receipt)),
        Err(acc_err) => Err(warp::reject::custom(WebServiceAccountingError(acc_err))),
    }
}

/// The `order_book` handler
///
/// /orderbook?sort=true&desc=false
pub async fn order_book(
    request: OrderBookRequest,
    trading_platform: Arc<Mutex<TradingPlatform>>,
) -> Result<impl Reply, Infallible> {
    let book = trading_platform
        .lock()
        .await
        .order_book(request.sort, request.desc);
    let response = warp::reply::json(&book);
    Ok(response)
}

/// The `order_book_by_price` handler
///
/// /orderbookbyprice?desc=false
pub async fn order_book_by_price(
    request: OrderBookByPriceRequest,
    trading_platform: Arc<Mutex<TradingPlatform>>,
) -> Result<impl Reply, Infallible> {
    let book = trading_platform
        .lock()
        .await
        .order_book_by_price(request.desc);
    let response = warp::reply::json(&book);
    Ok(response)
}

/// The `order_history` handler
///
/// Responds with the entire ledger (all transactions ever) - transaction log - entire order history
pub async fn order_history(
    trading_platform: Arc<Mutex<TradingPlatform>>,
) -> Result<impl Reply, Infallible> {
    let history = &trading_platform.lock().await.tx_log;
    let response = warp::reply::json(&history);
    Ok(response)
}

/// The `all_accounts` handler
///
/// Responds with all accounts and their balances
pub async fn all_accounts(
    trading_platform: Arc<Mutex<TradingPlatform>>,
) -> Result<impl Reply, Infallible> {
    let accounts = &trading_platform.lock().await.accounts.accounts;
    let response = warp::reply::json(&accounts);
    Ok(response)
}
