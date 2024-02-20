//! The "Fintech Web Service's" entry point.

use fintech_common::trading_platform::TradingPlatform;
use fintech_common::{OrderBookByPriceRequest, OrderBookRequest};
use fintech_web_service::handlers;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::Filter;

/// The "Fintech Web Service's" entry point.
#[tokio::main]
async fn main() {
    if env::var_os("RUST_LOG").is_none() {
        env::set_var("RUST_LOG", "fintech=info");
    }
    pretty_env_logger::init();

    let log = warp::log("fintech");

    let trading_platform = Arc::new(Mutex::new(TradingPlatform::new()));
    let trading_platform_state = warp::any().map(move || trading_platform.clone());

    let balance_of = warp::path!("account")
        .and(warp::post())
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .and(trading_platform_state.clone())
        .and_then(handlers::balance_of);

    let deposit = warp::path!("account" / "deposit")
        .and(warp::post())
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .and(trading_platform_state.clone())
        .and_then(handlers::deposit);

    let withdraw = warp::path!("account" / "withdraw")
        .and(warp::post())
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .and(trading_platform_state.clone())
        .and_then(handlers::withdraw);

    let send = warp::path!("account" / "send")
        .and(warp::post())
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .and(trading_platform_state.clone())
        .and_then(handlers::send);

    let process_order = warp::path!("order")
        .and(warp::post())
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .and(trading_platform_state.clone())
        .and_then(handlers::process_order);

    let order_book = warp::path!("orderbook")
        .and(warp::get())
        .and(warp::query::<OrderBookRequest>())
        .and(trading_platform_state.clone())
        .and_then(handlers::order_book);

    let order_book_by_price = warp::path!("orderbookbyprice")
        .and(warp::get())
        .and(warp::query::<OrderBookByPriceRequest>())
        .and(trading_platform_state.clone())
        .and_then(handlers::order_book_by_price);

    let order_history = warp::path!("order" / "history")
        .and(warp::get())
        .and(trading_platform_state.clone())
        .and_then(handlers::order_history);

    let all_accounts = warp::path!("accounts")
        .and(warp::get())
        .and(trading_platform_state.clone())
        .and_then(handlers::all_accounts);

    let routes = balance_of
        .or(deposit)
        .or(withdraw)
        .or(send)
        .or(process_order)
        .or(order_book)
        .or(order_book_by_price)
        .or(order_history)
        .or(all_accounts)
        .with(log);

    // Start up the server
    warp::serve(routes).run(([127, 0, 0, 1], 8080)).await;
}
