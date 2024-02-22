//! The "Fintech Web Client CLI" app's entry point.

use fintech_web_client_cli::logic::{get_base_url, main_loop};
use std::error::Error;

/// The "Fintech Web Client CLI" app's entry point.
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let base_url = get_base_url(std::env::args().nth(1));

    main_loop(base_url).await?;

    Ok(())
}
