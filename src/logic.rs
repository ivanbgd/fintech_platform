use crate::constants::*;
use crate::trading_platform::TradingPlatform;
use std::io::{stdin, stdout, Write};

pub fn main_loop() {
    let mut trading_platform = TradingPlatform::new();

    loop {
        if let Some(line) = read_from_stdin(PROMPT) {
            let words = line.split_whitespace().collect::<Vec<_>>();
            let cmd = words[0].to_lowercase();

            match cmd.as_str() {
                HELP | "h" => help(),
                DEPOSIT | "d" => deposit(words, &mut trading_platform),
                WITHDRAW | "w" => withdraw(words, &mut trading_platform),
                SEND | "s" => send(words, &mut trading_platform),
                PRINT | LEDGER | TX_LOG | "p" | "l" | "t" => print_ledger(&trading_platform),
                ACCOUNTS | "a" => print_accounts(&trading_platform),
                CLIENT | "c" => print_single_account(words, &trading_platform),
                ORDER | "o" => order(),
                ORDER_BOOK | "ob" => order_book(words, &trading_platform),
                ORDER_BOOK_BY_PRICE | "obp" => order_book_by_price(words, &trading_platform),
                QUIT | "q" => break,
                _ => println!("Unrecognized command; try `help`."),
            }
        }
    }
}

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
    let msg = "h d w s p l t a c o ob obp q".to_string();
    msg
}

/// **Prints all existing commands in their full and short variants.**
fn help() {
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
fn read_from_stdin(label: &str) -> Option<String> {
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
pub fn is_valid_name(signer: &str) -> bool {
    if signer.trim().is_empty() {
        println!("Signer's name cannot be empty.");
        false
    } else {
        true
    }
}

/// Prints an error message about not being able to parse
/// a string into an integer, so that our users can get a
/// more informative message than the provided generic message
/// that comes from the standard library, and which is:
/// "invalid digit found in string".
///
/// This function can be converted into a macro.
fn cannot_parse(word: &str) {
    eprintln!(
        "[ERROR] Only non-negative integer numbers are allowed as the amount; you provided '{}'.",
        word
    );
}

/// **Deposit funds to an account**
///
/// The signer's name can consist of multiple words.
/// We can wrap the signer's name in single or double quotes,
/// but we don't have to use any quotes at all.
///
/// The deposit account doesn't need to exist in advance.
/// If it doesn't exist, it will be created on this occasion.
/// It is allowed to deposit 0, and this transaction will be recorded.
///
/// Performs basic input validation of the signer's name,
/// and of the amount, which should be a non-negative integer.
///
/// Prints a success or an error message depending on the status of the
/// transaction, and records the transaction in the success case.
///
/// # Errors
/// An error can happen in case the account would become over-funded.
///
/// We could pattern-match it for a different output format and the message
/// contents, but haven't done that here. Error is still printed.
fn deposit(words: Vec<&str>, trading_platform: &mut TradingPlatform) {
    let words_len = words.len();

    if words_len < 3 {
        println!("The deposit command: {DEPOSIT} 'signer full name' <amount>");
        return;
    }

    let signer = words[1..(words_len - 1)].join(" ");
    let signer = signer.trim_matches(|c| c == '\'' || c == '\"').trim();

    let amount = match words[words_len - 1].parse::<u64>() {
        Ok(amount) => amount,
        Err(_err) => {
            cannot_parse(words[words_len - 1]);
            return;
        }
    };

    if is_valid_name(signer) {
        let tx = trading_platform.deposit(signer, amount);
        println!("{:?}", tx);
    }
}

/// **Withdraw funds from an account**
///
/// The signer's name can consist of multiple words.
/// We can wrap the signer's name in single or double quotes,
/// but we don't have to use any quotes at all.
///
/// The withdraw account needs to exist in advance, naturally.
/// If it doesn't exist, an error message will be output to
/// the user, but the execution won't break.
///
/// It is allowed to withdraw 0, and this transaction will be recorded.
///
/// Performs basic input validation of the signer's name,
/// and of the amount, which should be a non-negative integer.
///
/// Prints a success or an error message depending on the status of the
/// transaction, and records the transaction in the success case.
///
/// # Errors
/// Potential errors are if the account doesn't exist, or if it is under-funded.
///
/// We could pattern-match them for a different output format and the message
/// contents, but haven't done that here. Errors are still printed.
fn withdraw(words: Vec<&str>, trading_platform: &mut TradingPlatform) {
    let words_len = words.len();

    if words_len < 3 {
        println!("The withdraw command: {WITHDRAW} 'signer full name' <amount>");
        return;
    }

    let signer = words[1..(words_len - 1)].join(" ");
    let signer = signer.trim_matches(|c| c == '\'' || c == '\"').trim();

    if let Ok(amount) = words[words_len - 1].parse::<u64>() {
        if is_valid_name(signer) {
            let tx = trading_platform.withdraw(signer, amount);
            println!("{:?}", tx);
        }
    } else {
        cannot_parse(words[words_len - 1]);
    }
}

/// **Send funds from one account to another account**
///
/// The sender's or the recipient's name can consist of multiple words.
/// We can wrap the signer's and/or the recipient's name in single or double quotes,
/// but we don't have to use any quotes at all.
///
/// The withdraw (the sender's) account needs to exist in advance, naturally.
/// If it doesn't exist, an error message will be output to
/// the user, but the execution won't break.
///
/// The deposit (the recipient's) account needs to exist in advance, too.
/// If it doesn't exist, an error message will be output to
/// the user, but the execution won't break.
///
/// It is allowed to send 0, and these two transactions,
/// withdraw and deposit, will be recorded.
///
/// Performs basic input validation of the sender's and recipient's name,
/// and of the amount, which should be a non-negative integer.
///
/// Prints a success or an error message depending on the status of the
/// transaction, and records the two transactions in the success case.
///
/// # Errors
/// Potential errors are if any of the two accounts doesn't exist,
/// or if the sender's account is under-funded,
/// or if the recipient's account would be over-funded.
///
/// We could pattern-match them for a different output format and the message
/// contents, but haven't done that here. Errors are still printed.
fn send(words: Vec<&str>, trading_platform: &mut TradingPlatform) {
    let words_len = words.len();

    if (words_len < 4) || !words.contains(&SEPARATOR) {
        println!("The send command: {SEND} 'sender full name' {SEPARATOR} 'recipient full name' <amount>");
        return;
    }

    let to_pos = words
        .iter()
        .position(|&r| r == SEPARATOR)
        .expect(format!("The send command must contain '{}'.", SEPARATOR).as_str());

    let sender = words[1..to_pos].join(" ");
    let sender = sender.trim_matches(|c| c == '\'' || c == '\"').trim();

    let recipient = words[to_pos + 1..words_len - 1].join(" ");
    let recipient = recipient.trim_matches(|c| c == '\'' || c == '\"').trim();

    if let Ok(amount) = words[words_len - 1].parse::<u64>() {
        if is_valid_name(sender) && is_valid_name(recipient) {
            let txs = trading_platform.send(sender, recipient, amount);
            println!("{:?}", txs);
        }
    } else {
        cannot_parse(words[words_len - 1]);
    }
}

/// **Print the entire ledger (all transactions ever) - transaction log**
fn print_ledger(trading_platform: &TradingPlatform) {
    println!(
        "The ledger (full transaction log): {:#?}",
        trading_platform.tx_log
    );
}

/// **Print all accounts and their balances**
pub fn print_accounts(trading_platform: &TradingPlatform) {
    println!(
        "Accounts and their balances: {:#?}",
        trading_platform.accounts.accounts
    );
}

/// **Print a single requested client**
///
/// The signer's name can consist of multiple words.
/// We can wrap the signer's name in single or double quotes,
/// but we don't have to use any quotes at all.
fn print_single_account(words: Vec<&str>, trading_platform: &TradingPlatform) {
    let words_len = words.len();

    if words_len < 2 {
        println!("The client command: {} 'signer full name'", CLIENT);
        return;
    }

    let signer = words[1..].join(" ");
    let signer = signer.trim_matches(|c| c == '\'' || c == '\"').trim();

    if is_valid_name(signer) {
        match trading_platform.accounts.accounts.get(signer) {
            Some(balance) => {
                println!(
                    r#"The client "{}" has the following balance: {}."#,
                    signer, balance
                )
            }
            None => println!(r#"The client "{}" doesn't exist."#, signer),
        }
    }
}

/// **Create an order**
fn order() {}

/// **Display the order book**
///
/// Both sides are combined together.
///
/// The command can optionally take words "sort" and "asc".
///
/// Optionally `sort`s the book by the ordinal sequence number;
/// `asc` stands for ascending (considered only if `sort` is `true`).
///
/// By default, the order book isn't sorted.
///
/// By default, if sorting is requested, the order is descending.
fn order_book(words: Vec<&str>, trading_platform: &TradingPlatform) {
    println!(r#"The order book command: {} ["sort"] ["asc"]"#, ORDER_BOOK);

    let words_len = words.len();

    let mut sort = false;
    if words_len > 1 && words[1] == "sort" {
        sort = true;
    }

    let mut asc = false;
    if words_len > 2 && words[2] == "asc" {
        asc = true;
    }

    println!(
        "The order book: {:#?}",
        trading_platform.order_book(sort, asc)
    );
}

/// **Display the order book sorted by price points**
///
/// Both sides are combined together.
///
/// The command can optionally take word "desc".
///
/// Sorted first by price points; `desc` is for descending order.
///
/// Inside of a price point, ordered by the ordinal sequence number.
///
/// The default order is ascending, in case "desc" isn't provided.
fn order_book_by_price(words: Vec<&str>, trading_platform: &TradingPlatform) {
    println!(
        r#"The order book by price command: {} ["desc"]"#,
        ORDER_BOOK_BY_PRICE
    );

    let words_len = words.len();

    let mut rev = false;
    if words_len > 1 && words[1] == "desc" {
        rev = true;
    }

    println!(
        "The order book: {:#?}",
        trading_platform.order_book_by_price(rev)
    );
}

#[cfg(test)]
mod tests {
    use super::help_contents_full;
    use super::help_contents_short;
    use crate::constants::SEPARATOR;

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
}
