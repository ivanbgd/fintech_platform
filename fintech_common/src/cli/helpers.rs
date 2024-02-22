//! Helper functions that are common to CLI apps

use crate::cli::constants::*;
use crate::errors::SIGNER_NAME_NOT_VALID_MSG;
use crate::validation;
use std::io::{stdin, stdout, Write};

/// **Contains full variants of all existing commands.**
///
/// Wrapped by `help()` so we can unit-test the contents,
/// so that we don't forget to include a newly-added command to help.
fn help_contents_full() -> String {
    let msg = format!(
        "{HELP} {DEPOSIT} {WITHDRAW} {SEND} {PRINT} {LEDGER} {TX_LOG} {ACCOUNTS} \
         {CLIENT} {ORDER} {ORDER_BOOK} {ORDER_BOOK_BY_PRICE} {QUIT}"
    );
    msg
}

/// **Contains short variants of all existing commands.**
///
/// Wrapped by `help()` so we can unit-test the contents,
/// so that we don't forget to include a newly-added command to help.
fn help_contents_short() -> String {
    "h d w s p l t a c o ob obp q".to_string()
}

/// **Prints all existing commands in their full and short variants.**
pub fn help() {
    println!("{}", help_contents_full());
    println!("{}", help_contents_short());
}

/// **Reads standard input into a line.**
///
/// Signals an empty line so we can ignore it (in the main loop).
///
/// # Panics
/// Panics in case it can't write `label` to `stdout`,
/// or if it can't flush the `stdout` buffer.
pub fn read_from_stdin(label: &str) -> Option<String> {
    let mut lock = stdout().lock();
    write!(lock, "\n{label}").expect("Failed to write the label to stdout.");
    stdout()
        .flush()
        .expect("Failed to flush the stdout buffer.");

    let mut line = String::new();
    match stdin().read_line(&mut line) {
        Ok(_) => {
            if line.trim().is_empty() {
                None
            } else {
                Some(line.to_owned())
            }
        }
        Err(err) => {
            eprintln!("[ERROR] Failed to read line: {}", err);
            None
        }
    }
}

/// **Basic input validation for a signer's name**
///
/// Checks for:
/// - An empty string.
pub fn is_valid_name(signer: &str) -> bool {
    match validation::is_valid_name(signer) {
        Some(msg) => {
            eprintln!(
                "[ERROR] {}: \"{}\". {}",
                SIGNER_NAME_NOT_VALID_MSG, signer, msg
            );
            false
        }
        None => true,
    }
}

/// Prints an error message about not being able to parse
/// a string into an integer, so that our users can get a
/// more informative message than the provided generic message
/// that comes from the standard library, and which is:
/// "invalid digit found in string".
///
/// This function can be converted into a macro.
pub fn cannot_parse_number(word: &str) {
    eprintln!(
        "[ERROR] Only non-negative integer numbers are allowed as the amount; you provided '{}'.",
        word
    );
}

#[cfg(test)]
mod tests {
    use super::{help_contents_full, help_contents_short, is_valid_name};
    use crate::cli::constants::SEPARATOR;

    #[test]
    fn test_help_contents() {
        let expected = "help deposit withdraw send print ledger txlog accounts \
        client order orderbook orderbookbyprice quit"
            .trim()
            .to_string();
        assert_eq!(help_contents_full(), expected);
    }

    #[test]
    fn test_help_contents_short() {
        let expected = "h d w s p l t a c o ob obp q".to_string();
        assert_eq!(help_contents_short(), expected);
    }

    #[test]
    fn test_separator() {
        let expected = "--".to_string();
        assert_eq!(SEPARATOR, expected);
    }

    #[test]
    fn test_valid_name_passes() {
        assert!(is_valid_name("Ivan"));
    }

    #[test]
    fn test_empty_name_fails() {
        assert!(!is_valid_name(""));
    }
}
