//! The type of the CLI application
//!
//! It can be:
//! - NonWeb
//! - Web
//!
//! The goal is to have unique logic and helper functions that various CLI apps can use.
//! This way, we only change logic and the helper functions, or add new ones, in one place,
//! and we can also test them in only one place.
//! We don't have to multiply similar code by copy-pasting it and modifying it a little.
//!
//! Our two CLI apps use two backends: a non-web one, and a web-based one.

/// **The type of the CLI application**
///
/// It can be:
/// - NonWeb
/// - Web
pub enum CliType {
    /// For the non-web backend
    NonWeb,

    /// For the web service backend
    Web,
}
