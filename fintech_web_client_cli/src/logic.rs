use crate::DEFAULT_BASE_URL;
use fintech_common::cli::constants::*;
use fintech_common::errors::SIGNER_NAME_NOT_VALID_MSG;
use fintech_common::trading_platform::TradingPlatform;
use fintech_common::types::{Order, Side};
use fintech_common::CliType;
use fintech_common::{validation, AccountUpdateRequest};
use reqwest::{Client, StatusCode, Url};
use std::error::Error;
use std::io::{stdin, stdout, Write};

// pub fn main_loop(cli_type: CliType) {
pub async fn main_loop(base_url: Url) -> Result<(), Box<dyn Error>> {
    let client = reqwest::Client::new();

    let mut trading_platform = TradingPlatform::new();

    loop {
        if let Some(line) = read_from_stdin(PROMPT) {
            let words = line.split_whitespace().collect::<Vec<_>>();
            let cmd = words[0].to_lowercase();

            match cmd.as_str() {
                HELP | "h" => help(),
                DEPOSIT | "d" => deposit(words, &base_url, &client).await?,
                WITHDRAW | "w" => withdraw(words, &base_url, &client).await?,
                SEND | "s" => send(words, &mut trading_platform),
                PRINT | LEDGER | TX_LOG | "p" | "l" | "t" => print_ledger(&trading_platform),
                ACCOUNTS | "a" => print_accounts(&trading_platform),
                CLIENT | "c" => print_single_account(words, &trading_platform),
                ORDER | "o" => order(words, &mut trading_platform),
                ORDER_BOOK | "ob" => order_book(words, &trading_platform),
                ORDER_BOOK_BY_PRICE | "obp" => order_book_by_price(words, &trading_platform),
                QUIT | "q" => break,
                _ => println!("Unrecognized command; try `help`."),
            }
        }
    }

    Ok(())
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
    "h d w s p l t a c o ob obp q".to_string()
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
///
/// Checks for:
/// - An empty string.
fn is_valid_name(signer: &str) -> bool {
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
fn cannot_parse_number(word: &str) {
    eprintln!(
        "[ERROR] Only non-negative integer numbers are allowed as the amount; you provided '{}'.",
        word
    );
}

/// **Get base URL**
///
/// Tries to create a URL from the provided argument.
///
/// If that is not possible, falls back to a default.
///
/// It returns a URL in any case.
///
/// This is meant to be a base URL for all operations.
///
/// - If the provided argument is the `None` variant,
///   returns a default value as the base URL.
/// - If it's a `String`, tries to parse it into URL.
///   - If it's a valid URL string, returns it as URL.
///   - If it's a malformed URL string, returns the default.
///
/// The default value is [`DEFAULT_BASE_URL`].
pub fn get_base_url(base_url: Option<String>) -> Url {
    let base_url = base_url.unwrap_or_else(|| {
        println!(
            "No CLI base URL provided; using default: {}",
            DEFAULT_BASE_URL
        );
        DEFAULT_BASE_URL.into()
    });

    let base_url = Url::parse(base_url.as_str()).unwrap_or_else(|_| {
        println!(
            "Provided base URL could not be parsed; using default: {}",
            DEFAULT_BASE_URL
        );
        Url::parse(DEFAULT_BASE_URL).unwrap()
    });

    base_url
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
/// - Attempted overflow (account over-funded), `AccountingError::AccountOverFunded`
///
/// We could pattern-match it for a different output format and the message
/// contents, but haven't done that here. Error is still printed.
async fn deposit(words: Vec<&str>, base_url: &Url, client: &Client) -> Result<(), Box<dyn Error>> {
    let words_len = words.len();

    if words_len < 3 {
        println!("The deposit command: {DEPOSIT} 'signer full name' <amount>");
        return Ok(());
    }

    let signer = words[1..(words_len - 1)].join(" ");
    let signer = signer.trim_matches(|c| c == '\'' || c == '\"').trim();

    let amount = match words[words_len - 1].parse::<u64>() {
        Ok(amount) => amount,
        Err(_err) => {
            cannot_parse_number(words[words_len - 1]);
            return Ok(());
        }
    };

    let signer = signer.to_string();
    if is_valid_name(&signer) {
        let url = base_url.join("account/deposit")?;
        let response = client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&AccountUpdateRequest { signer, amount })
            .send()
            .await?;

        if response.status() == StatusCode::OK {
            let response_body = response.text().await?;
            println!("{}", response_body);
        } else {
            eprintln!("[ERROR] \"{}\"", response.text().await?);
        }
    }

    Ok(())
}

/// **Withdraw funds from an account**
///
/// The signer's name can consist of multiple words.
/// We can wrap the signer's name in single or double quotes,
/// but we don't have to use any quotes at all.
///
/// The withdrawal account needs to exist in advance, naturally.
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
/// - Account doesn't exist, `AccountingError::AccountNotFound`;
/// - Attempted overflow (account under-funded), `AccountingError::AccountUnderFunded`.
///
/// We could pattern-match them for a different output format and the message
/// contents, but haven't done that here. Errors are still printed.
async fn withdraw(words: Vec<&str>, base_url: &Url, client: &Client) -> Result<(), Box<dyn Error>> {
    let words_len = words.len();

    if words_len < 3 {
        println!("The withdraw command: {WITHDRAW} 'signer full name' <amount>");
        return Ok(());
    }

    let signer = words[1..(words_len - 1)].join(" ");
    let signer = signer
        .trim_matches(|c| c == '\'' || c == '\"')
        .trim()
        .to_string();

    if let Ok(amount) = words[words_len - 1].parse::<u64>() {
        if is_valid_name(&signer) {
            let url = base_url.join("account/withdraw")?;
            let response = client
                .post(url)
                .json(&AccountUpdateRequest { signer, amount })
                .send()
                .await?;

            if response.status() == StatusCode::OK {
                let response_body = response.text().await?;
                println!("{}", response_body);
            } else {
                eprintln!("[ERROR] \"{}\"", response.text().await?);
            }
        }
    } else {
        cannot_parse_number(words[words_len - 1]);
    }

    Ok(())
}

/// **Send funds from one account to another account**
///
/// The sender's or the recipient's name can consist of multiple words.
/// We can wrap the signer's and/or the recipient's name in single or double quotes,
/// but we don't have to use any quotes at all.
///
/// The withdrawal (the sender's) account needs to exist in advance, naturally.
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
/// - Any of the two accounts doesn't exist, `AccountingError::AccountNotFound`;
/// - Attempted overflow (sender's account under-funded), `AccountingError::AccountUnderFunded`;
/// - Attempted overflow (recipient's account over-funded), `AccountingError::AccountOverFunded`.
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
        cannot_parse_number(words[words_len - 1]);
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
///
/// Prints the signer's balance.
fn print_single_account(words: Vec<&str>, trading_platform: &TradingPlatform) {
    let words_len = words.len();

    if words_len < 2 {
        println!("The client command: {CLIENT} 'signer full name'");
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

/// **Create and process an order**
///
/// The signer's name can consist of multiple words.
/// We can wrap the signer's name in single or double quotes,
/// but we don't have to use any quotes at all.
///
/// The account needs to exist in advance.
///
/// Performs basic input validation of the signer's name, of the side,
/// and of the price and amount, which should be non-negative integers.
///
/// Prints a success or an error message depending on the status of the
/// receipt (of the processing of the order).
///
/// # Errors
/// - Account not found, `AccountingError::AccountNotFound`;
/// - Account has insufficient funds, `AccountingError::AccountUnderFunded`;
/// - Account would be over-funded, `AccountingError::AccountOverFunded`.
fn order(words: Vec<&str>, trading_platform: &mut TradingPlatform) {
    let words_len = words.len();

    if words_len < 5 {
        println!("The order command: {ORDER} 'signer full name' <side> <price> <amount>");
        return;
    }

    let signer = words[1..(words_len - 3)].join(" ");
    let signer = signer.trim_matches(|c| c == '\'' || c == '\"').trim();

    let side = match words[words_len - 3] {
        "buy" | "bid" => Side::Buy,
        "sell" | "ask" => Side::Sell,
        _ => {
            eprintln!(r#"[ERROR] Side can be either "buy"/"bid" or "sell"/"ask"."#);
            return;
        }
    };

    let price = match words[words_len - 2].parse::<u64>() {
        Ok(price) => price,
        Err(_err) => {
            cannot_parse_number(words[words_len - 2]);
            return;
        }
    };

    let amount = match words[words_len - 1].parse::<u64>() {
        Ok(amount) => amount,
        Err(_err) => {
            cannot_parse_number(words[words_len - 1]);
            return;
        }
    };

    if is_valid_name(signer) {
        let order = Order::new(price, amount, side, signer.to_string());
        let receipt = trading_platform.process_order(order);
        println!("{:?}", receipt);
    }
}

/// **Display the order book**
///
/// Both sides are combined together.
///
/// The command can optionally take words "sort" and "desc".
///
/// Optionally `sort`s the book by the ordinal sequence number;
/// `desc` stands for descending (considered only if `sort` is `true`).
///
/// By default, the order book isn't sorted.
///
/// If sorting is requested, the order is ascending by default.
fn order_book(words: Vec<&str>, trading_platform: &TradingPlatform) {
    println!(r#"The order book command: {ORDER_BOOK} ["sort"] ["desc"]"#);
    println!("By default, the order book isn't sorted.");
    println!("The optional sorting is done by ordinals, and is ascending by default.");

    let words_len = words.len();

    let mut sort = false;
    if words_len > 1 && words[1] == "sort" {
        sort = true;
    }

    let mut desc = false;
    if words_len > 2 && words[2] == "desc" {
        desc = true;
    }

    println!(
        "The order book: {:#?}",
        trading_platform.order_book(sort, desc)
    );
}

/// **Display the order book sorted by price points**
///
/// Both sides are combined together.
///
/// The command can optionally take word "desc".
///
/// Sorted first by price points ascending; optional `desc` is for descending order.
///
/// Inside of a price point, always ordered ascending by the ordinal sequence number.
fn order_book_by_price(words: Vec<&str>, trading_platform: &TradingPlatform) {
    println!(r#"The order book by price command: {ORDER_BOOK_BY_PRICE} ["desc"]"#);
    println!(
        "Sorted first by price points in ascending order; \
        optional \"desc\" is for descending order of prices."
    );
    println!("Inside of a price point, always ordered ascending by the ordinal sequence number.");

    let words_len = words.len();

    let mut desc = false;
    if words_len > 1 && words[1] == "desc" {
        desc = true;
    }

    println!(
        "The order book: {:#?}",
        trading_platform.order_book_by_price(desc)
    );
}

#[cfg(test)]
mod tests {
    use super::{get_base_url, help_contents_full, help_contents_short, is_valid_name};
    use crate::DEFAULT_BASE_URL;
    // use crate::constants::SEPARATOR; todo
    use fintech_common::cli::constants::SEPARATOR;

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

    #[test]
    fn test_default_url_none() {
        assert_eq!(get_base_url(None).to_string(), DEFAULT_BASE_URL);
    }

    #[test]
    fn test_default_url_empty() {
        assert_eq!(
            get_base_url(Some("".to_string())).to_string(),
            DEFAULT_BASE_URL
        );
    }

    #[test]
    fn test_default_url_bad() {
        assert_eq!(
            get_base_url(Some("https://333.333.333.333".to_string())).to_string(),
            DEFAULT_BASE_URL
        );
    }

    #[test]
    fn test_default_url_valid() {
        assert_eq!(
            get_base_url(Some(DEFAULT_BASE_URL.to_string())).to_string(),
            DEFAULT_BASE_URL
        );
    }

    #[test]
    fn test_valid_url() {
        assert_eq!(
            get_base_url(Some("http://127.0.0.1:3333".to_string())).to_string(),
            "http://127.0.0.1:3333/"
        );
    }
}
