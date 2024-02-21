//! The "Fintech CLI" app's entry point.

use fintech_cli::logic::main_loop;
use fintech_common::CliType;
// use fintech_common::cli::logic::main_loop;

/// The "Fintech CLI" app's entry point.
fn main() {
    // main_loop(CliType::NonWeb);
    main_loop();
}
