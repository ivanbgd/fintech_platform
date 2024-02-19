//! The "Fintech Web Service's" entry point.

use fintech_web_service::handlers;
use fintech_web_service::trading_platform::TradingPlatform;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::Filter;

/// The "Fintech Web Service's" entry point.
#[tokio::main]
async fn main() {
    if env::var_os("RUST_LOG").is_none() {
        env::set_var("RUST_LOG", "fintech=debug");
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

    let order = warp::path!("order")
        .and(warp::post())
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .and(trading_platform_state.clone())
        .and_then(handlers::order);

    let routes = balance_of
        .or(deposit)
        .or(withdraw)
        .or(send)
        .or(order)
        .with(log);

    // Start up the server
    warp::serve(routes).run(([127, 0, 0, 1], 8080)).await;
}
