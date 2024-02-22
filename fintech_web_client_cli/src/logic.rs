use crate::DEFAULT_BASE_URL;
use fintech_common::cli::constants::*;
use fintech_common::cli::helpers::*;
use fintech_common::requests::*;
use fintech_common::tx::Tx;
use fintech_common::types::{Order, PartialOrder, Receipt, Side};
use reqwest::{header, Client, StatusCode, Url};
use std::collections::BTreeMap;
use std::error::Error;

pub async fn main_loop(base_url: Url) -> Result<(), Box<dyn Error>> {
    let client = Client::new();

    loop {
        if let Some(line) = read_from_stdin(PROMPT) {
            let words = line.split_whitespace().collect::<Vec<_>>();
            let cmd = words[0].to_lowercase();

            match cmd.as_str() {
                HELP | "h" => help(),
                DEPOSIT | "d" => deposit(words, &client, &base_url).await?,
                WITHDRAW | "w" => withdraw(words, &client, &base_url).await?,
                SEND | "s" => send(words, &client, &base_url).await?,
                PRINT | LEDGER | TX_LOG | "p" | "l" | "t" => {
                    print_ledger(&client, &base_url).await?
                }
                ACCOUNTS | "a" => print_accounts(&client, &base_url).await?,
                CLIENT | "c" => print_single_account(words, &client, &base_url).await?,
                ORDER | "o" => order(words, &client, &base_url).await?,
                ORDER_BOOK | "ob" => order_book(words, &client, &base_url).await?,
                ORDER_BOOK_BY_PRICE | "obp" => {
                    order_book_by_price(words, &client, &base_url).await?
                }
                QUIT | "q" => break,
                _ => println!("Unrecognized command; try `help`."),
            }
        }
    }

    Ok(())
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

/// **Send a POST request for `deposit` and `withdraw`**
async fn account_update_request(
    client: &Client,
    base_url: &Url,
    path: &str,
    signer: &str,
    amount: u64,
) -> Result<(), Box<dyn Error>> {
    let signer = signer.to_string();
    let url = base_url.join(path)?;

    let response = client
        .post(url)
        .json(&AccountUpdateRequest { signer, amount })
        .send()
        .await?;

    if response.status().is_success() {
        let tx: Tx = response.json().await?;
        println!("{:?}", tx);
    } else {
        eprintln!("[ERROR] \"{}\"", response.text().await?);
    }

    Ok(())
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
async fn deposit(words: Vec<&str>, client: &Client, base_url: &Url) -> Result<(), Box<dyn Error>> {
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

    if is_valid_name(&signer) {
        account_update_request(client, base_url, "account/deposit", signer, amount).await?;
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
async fn withdraw(words: Vec<&str>, client: &Client, base_url: &Url) -> Result<(), Box<dyn Error>> {
    let words_len = words.len();

    if words_len < 3 {
        println!("The withdraw command: {WITHDRAW} 'signer full name' <amount>");
        return Ok(());
    }

    let signer = words[1..(words_len - 1)].join(" ");
    let signer = signer.trim_matches(|c| c == '\'' || c == '\"').trim();

    if let Ok(amount) = words[words_len - 1].parse::<u64>() {
        if is_valid_name(&signer) {
            account_update_request(client, base_url, "account/withdraw", signer, amount).await?;
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
async fn send(words: Vec<&str>, client: &Client, base_url: &Url) -> Result<(), Box<dyn Error>> {
    let words_len = words.len();

    if (words_len < 4) || !words.contains(&SEPARATOR) {
        println!("The send command: {SEND} 'sender full name' {SEPARATOR} 'recipient full name' <amount>");
        return Ok(());
    }

    let to_pos = words
        .iter()
        .position(|&r| r == SEPARATOR)
        .expect(format!("The send command must contain '{}'.", SEPARATOR).as_str());

    let sender = words[1..to_pos].join(" ");
    let sender = sender
        .trim_matches(|c| c == '\'' || c == '\"')
        .trim()
        .to_string();

    let recipient = words[to_pos + 1..words_len - 1].join(" ");
    let recipient = recipient
        .trim_matches(|c| c == '\'' || c == '\"')
        .trim()
        .to_string();

    if let Ok(amount) = words[words_len - 1].parse::<u64>() {
        if is_valid_name(&sender) && is_valid_name(&recipient) {
            let url = base_url.join("account/send")?;
            let response = client
                .post(url)
                .json(&AccountSendRequest {
                    sender,
                    recipient,
                    amount,
                })
                .send()
                .await?;

            if response.status().is_success() {
                let txs: (Tx, Tx) = response.json().await?;
                println!("{:?}", txs);
            } else {
                eprintln!("[ERROR] \"{}\"", response.text().await?);
            }
        }
    } else {
        cannot_parse_number(words[words_len - 1]);
    }

    Ok(())
}

/// **Print the entire ledger (all transactions ever) - transaction log**
async fn print_ledger(client: &Client, base_url: &Url) -> Result<(), Box<dyn Error>> {
    let url = base_url.join("order/history")?;
    let response = client.get(url).send().await?;

    if response.status() == StatusCode::OK {
        let history: Vec<Tx> = response.json().await?;
        println!(
            "The ledger (full transaction log, complete order history): {:#?}",
            history
        );
    } else {
        eprintln!("[ERROR] \"{}\"", response.text().await?);
    }

    Ok(())
}

/// **Print all accounts and their balances**
pub async fn print_accounts(client: &Client, base_url: &Url) -> Result<(), Box<dyn Error>> {
    let url = base_url.join("accounts")?;
    let response = client.get(url).send().await?;

    if response.status() == StatusCode::OK {
        let accounts: BTreeMap<String, u64> = response.json().await?;
        println!("Accounts and their balances: {:#?}", accounts);
    } else {
        eprintln!("[ERROR] \"{}\"", response.text().await?);
    }

    Ok(())
}

/// **Print a single requested client**
///
/// The signer's name can consist of multiple words.
/// We can wrap the signer's name in single or double quotes,
/// but we don't have to use any quotes at all.
///
/// Prints the signer's balance.
async fn print_single_account(
    words: Vec<&str>,
    client: &Client,
    base_url: &Url,
) -> Result<(), Box<dyn Error>> {
    let words_len = words.len();

    if words_len < 2 {
        println!("The client command: {CLIENT} 'signer full name'");
        return Ok(());
    }

    let signer = words[1..].join(" ");
    let signer = signer
        .trim_matches(|c| c == '\'' || c == '\"')
        .trim()
        .to_string();

    if is_valid_name(&signer) {
        let url = base_url.join("account")?;
        let response = client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&AccountBalanceRequest {
                signer: signer.clone(),
            })
            .send()
            .await?;

        match response.status().as_u16() {
            200..=299 => {
                let balance: u64 = response.json().await?;
                println!(
                    r#"The client "{}" has the following balance: {}."#,
                    signer, balance
                )
            }
            400..=599 => {
                eprintln!(r#"The client "{}" doesn't exist."#, signer);
                eprintln!(
                    "[ERROR] {} \"{}\"",
                    response.status(),
                    response.text().await?
                );
            }
            _ => println!("[ERROR] Unexpected status code: {}", response.status()),
        }
    }

    Ok(())
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
async fn order(words: Vec<&str>, client: &Client, base_url: &Url) -> Result<(), Box<dyn Error>> {
    let words_len = words.len();

    if words_len < 5 {
        println!("The order command: {ORDER} 'signer full name' <side> <price> <amount>");
        return Ok(());
    }

    let signer = words[1..(words_len - 3)].join(" ");
    let signer = signer.trim_matches(|c| c == '\'' || c == '\"').trim();

    let side = match words[words_len - 3] {
        "buy" | "bid" => Side::Buy,
        "sell" | "ask" => Side::Sell,
        _ => {
            eprintln!(r#"[ERROR] Side can be either "buy"/"bid" or "sell"/"ask"."#);
            return Ok(());
        }
    };

    let price = match words[words_len - 2].parse::<u64>() {
        Ok(price) => price,
        Err(_err) => {
            cannot_parse_number(words[words_len - 2]);
            return Ok(());
        }
    };

    let amount = match words[words_len - 1].parse::<u64>() {
        Ok(amount) => amount,
        Err(_err) => {
            cannot_parse_number(words[words_len - 1]);
            return Ok(());
        }
    };

    if is_valid_name(signer) {
        let order = Order::new(price, amount, side, signer.to_string());

        let url = base_url.join("order")?;
        let response = client.post(url).json(&order).send().await?;

        if response.status() == StatusCode::OK {
            let receipt: Receipt = response.json().await?;
            println!("{:?}", receipt);
        } else {
            eprintln!(r#"The client "{}" doesn't exist."#, signer);
            eprintln!("[ERROR] \"{}\"", response.text().await?);
        }
    }

    Ok(())
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
async fn order_book(
    words: Vec<&str>,
    client: &Client,
    base_url: &Url,
) -> Result<(), Box<dyn Error>> {
    println!(r#"The order book command: {ORDER_BOOK} ["sort"] ["desc"]"#);
    println!("By default, the order book isn't sorted.");
    println!("The optional sorting is done by ordinals, and is ascending by default.");

    let words_len = words.len();

    let mut sort = Some(false);
    if words_len > 1 && words[1] == "sort" {
        sort = Some(true);
    }

    let mut desc = Some(false);
    if words_len > 2 && words[2] == "desc" {
        desc = Some(true);
    }

    let url = base_url.join("orderbook")?;
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static("fintech_web_client_cli"),
    );
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/json"),
    );
    let response = client
        .get(url)
        .headers(headers)
        .query(&OrderBookRequest { sort, desc })
        .send()
        .await?;

    if response.status() == StatusCode::OK {
        let book = response.json::<Vec<PartialOrder>>().await?;
        println!("\nThe order book: {:#?}", book);
    } else {
        eprintln!("[ERROR] \"{}\"", response.text().await?);
    }

    Ok(())
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
async fn order_book_by_price(
    words: Vec<&str>,
    client: &Client,
    base_url: &Url,
) -> Result<(), Box<dyn Error>> {
    println!(r#"The order book by price command: {ORDER_BOOK_BY_PRICE} ["desc"]"#);
    println!(
        "Sorted first by price points in ascending order; \
        optional \"desc\" is for descending order of prices."
    );
    println!("Inside of a price point, always ordered ascending by the ordinal sequence number.");

    let words_len = words.len();

    let mut desc = Some(false);
    if words_len > 1 && words[1] == "desc" {
        desc = Some(true);
    }

    let url = base_url.join("orderbookbyprice")?;
    let response = client
        .get(url)
        .query(&OrderBookByPriceRequest { desc })
        .send()
        .await?;

    if response.status() == StatusCode::OK {
        let book: Vec<PartialOrder> = response.json().await?;
        println!("\nThe order book sorted by price points: {:#?}", book);
    } else {
        eprintln!("[ERROR] \"{}\"", response.text().await?);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::get_base_url;
    use crate::DEFAULT_BASE_URL;

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
