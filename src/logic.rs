use crate::accounts::{print_accounts, print_single_account, Accounts};
use crate::constants::*;
use crate::tx::Tx;
use std::io::{stdin, stdout, Write};

pub fn main_loop() {
    let mut accounts = Accounts::new();
    let mut ledger: Vec<Tx> = Vec::new();

    loop {
        if let Some(line) = read_from_stdin(PROMPT) {
            let words = line.split_whitespace().collect::<Vec<_>>();
            let cmd = words[0].to_lowercase();

            match cmd.as_str() {
                HELP | "h" => help(),
                DEPOSIT | "d" => deposit(words, &mut accounts, &mut ledger),
                WITHDRAW | "w" => withdraw(words, &mut accounts, &mut ledger),
                SEND | "s" => send(words, &mut accounts, &mut ledger),
                PRINT | LEDGER | TX | "p" | "l" | "t" => print_ledger(&ledger),
                ACCOUNTS | "a" => print_accounts(&accounts),
                CLIENT | "c" => print_single_account(words, &accounts),
                QUIT | "q" => break,
                _ => println!("Unrecognized command; try `help`."),
            }
        }
    }
}

/// **Contains all existing commands.**
///
/// Wrapped by `help()` so we can unit-test the contents.
fn help_contents() -> String {
    let msg = format!(
        "{HELP} {DEPOSIT} {WITHDRAW} {SEND} {PRINT} {LEDGER} {TX} {ACCOUNTS} {CLIENT} {QUIT}"
    );
    msg
}

/// **Prints all existing commands.**
fn help() {
    println!("{}", help_contents());
}

/// **Reads standard input into a line.**
///
/// Signals an empty line so we can ignore it (in the main loop).
///
/// # Panics
/// Panics in case it can't write `label` to stdout,
/// or if it can't flush the stdout buffer.
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
            eprintln!("Failed to read line: {}", err);
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
        "Only non-negative integer numbers are allowed as the amount; you provided {}.",
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
/// An error can happen in the case the account would become over-funded.
/// We could pattern-match it for a different output format and the message
/// contents, but haven't done that here. Error is still printed.
fn deposit(words: Vec<&str>, accounts: &mut Accounts, ledger: &mut Vec<Tx>) {
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
            // eprintln!("{}", _err);  // "invalid digit found in string"
            cannot_parse(words[2]);
            return;
        }
    };

    if is_valid_name(signer) {
        let tx = accounts.deposit(signer, amount);
        println!("{:?}", tx);
        if tx.is_ok() {
            ledger.push(tx.expect("Failed to unwrap deposit tx."));
        }
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
/// Potential errors are if the account doesn't exist, or if it is under-funded.
/// We could pattern-match them for a different output format and the message
/// contents, but haven't done that here. Errors are still printed.
fn withdraw(words: Vec<&str>, accounts: &mut Accounts, ledger: &mut Vec<Tx>) {
    let words_len = words.len();

    if words_len < 3 {
        println!("The withdraw command: {WITHDRAW} 'signer full name' <amount>");
        return;
    }

    let signer = words[1..(words_len - 1)].join(" ");
    let signer = signer.trim_matches(|c| c == '\'' || c == '\"').trim();

    if let Ok(amount) = words[words_len - 1].parse::<u64>() {
        if is_valid_name(signer) {
            let tx = accounts.withdraw(signer, amount);
            println!("{:?}", tx);
            if tx.is_ok() {
                ledger.push(tx.expect("Failed to unwrap withdraw tx."));
            }
        }
    } else {
        cannot_parse(words[2]);
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
/// Potential errors are if any of the two accounts doesn't exist,
/// or if the sender's account is under-funded,
/// or if the recipient's account would be over-funded.
/// We could pattern-match them for a different output format and the message
/// contents, but haven't done that here. Errors are still printed.
fn send(words: Vec<&str>, accounts: &mut Accounts, ledger: &mut Vec<Tx>) {
    let words_len = words.len();

    if (words_len < 4) || !words.contains(&SEPARATOR) {
        println!("The send command: {SEND} 'sender full name' {SEPARATOR} 'recipient full name' <amount>");
        return;
    }

    let to_pos = words
        .iter()
        .position(|&r| r == SEPARATOR)
        .expect(r#"The send command must contain "{SEPARATOR}"."#);

    let sender = words[1..to_pos].join(" ");
    let sender = sender.trim_matches(|c| c == '\'' || c == '\"').trim();

    let recipient = words[to_pos + 1..words_len - 1].join(" ");
    let recipient = recipient.trim_matches(|c| c == '\'' || c == '\"').trim();

    if let Ok(amount) = words[words_len - 1].parse::<u64>() {
        if is_valid_name(sender) && is_valid_name(recipient) {
            let txs = accounts.send(sender, recipient, amount);
            println!("{:?}", txs);
            if txs.is_ok() {
                let (tx_deposit, tx_withdraw) = txs.expect("Failed to unpack the txs tuple.");
                ledger.push(tx_deposit);
                ledger.push(tx_withdraw);
            }
        }
    } else {
        cannot_parse(words[3]);
    }
}

/// **Prints the entire ledger (all transactions ever)**
fn print_ledger(ledger: &Vec<Tx>) {
    println!("The ledger: {:#?}", ledger);
}

#[cfg(test)]
mod tests {
    use super::help_contents;
    use crate::constants::SEPARATOR;

    #[test]
    fn test_help_contents() {
        let expected = "help deposit withdraw send print ledger tx accounts client quit"
            .trim()
            .to_string();
        assert_eq!(help_contents(), expected);
    }

    #[test]
    fn test_separator() {
        let expected = "--".to_string();
        assert_eq!(SEPARATOR, expected);
    }
}
