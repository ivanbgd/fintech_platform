# Fintech Platform

[Three-Project Series: Build a Fintech Platform in Rust](https://www.manning.com/liveprojectseries/fintech-platform-ser)

 - [Project 1: Fundamentals and Accounting](https://www.manning.com/liveproject/fundamentals-and-accounting)
 - [Project 2: Core Algorithms and Data Structures](https://www.manning.com/liveproject/core-algorithms-and-data-structures)
 - [Project 3: A Shared Marketplace API](https://www.manning.com/liveproject/shared-marketplace-api)

### The Original Description
Future Finance Labs, a fintech scale-up company, wants to expand its customer base by offering
a lightning-fast exchange platform for professional traders.
As its star developer, you’ll use the Rust programming language to create a prototype
of the exchange that will accommodate high-frequency trading
and serve as the foundation for additional future financial products.

You’ll build an interactive command line program that will constitute the core accounting structure
for setting up accounts and manipulating data.

You’ll create a matching engine that enables traders to find the best trading partners
and showcases the blazing-fast core of the exchange platform.

You’ll extend your Rust HTTP API by setting up a [warp](https://crates.io/crates/warp) web service that will interact with
an additional trading platform, by building a shared marketplace that will be a blueprint for
additional Rust web services, small and large.

## Notes
- My implementation is a much improved version of the project's requirements and starter code.
  For example:
  - My implementation contains a much improved version of the matching engine, which is heavily-documented.  
    Additionally, the matching engine contains a vast amount of unit tests.
  - I have generally added more unit tests than required, for various parts of the application.
  - I have implemented a simple input validation, to serve as an example.
- My implementation has two CLI applications, where they only have one:
  1. A non-web variant,
  2. A web-variant (this is the one that they have); communicates with the web service package.
  - Namely, they started with the non-web variant and then changed it into a web variant,
    but I decided to keep the original one and add the new one.
  - This calls for a slightly different organization of code.
    Mainly, my common library is not exactly the same as their.
- I use the above-mentioned input validation in both CLI apps,
  as well as in the web service, as an example usage.

## The Most Notable Crates Used
- [warp](https://crates.io/crates/warp), as web framework
- [Tokio](https://tokio.rs/), as an asynchronous runtime
- [reqwest](https://docs.rs/reqwest/latest/reqwest/), as an HTTP client
- [Serde](https://serde.rs/), for Rust data structures serialization and deserialization
- [pretty_env_logger](https://crates.io/crates/pretty_env_logger), for pretty logging

## Running the Apps
From the project (workspace) directory:
- Non-web: `cargo run -p fintech_cli` or `cargo run` (default binary)
- Web Service:
  - With a specified logging level: `export RUST_LOG=<log_level> && cargo run -p fintech_web_service`,
    where log level can be trace, debug, info, warn or error.
    - For example: `export RUST_LOG=trace && cargo run -p fintech_web_service`
  - Default logging level is info: `cargo run -p fintech_web_service`
- Web Client CLI:
  - With a default web service URL: `cargo run -p fintech_web_client_cli`
  - With a provided web service URL: `cargo run -p fintech_web_client_cli -- http://127.0.0.1:8080/`

Use `cargo run --release` for the Release mode instead of the default Debug mode.

## Potential Improvements and Additions
- Checking if a seller really has the amount they wish to sell;
- Removal of an account;
- Clearing everything: all accounts and entire transaction log.
