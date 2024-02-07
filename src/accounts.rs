use crate::constants::CLIENT;
use crate::errors::AccountingError;
use crate::logic::is_valid_name;
use crate::tx::Tx;
use std::collections::BTreeMap;

/// **A type for managing accounts and their current currency balance**
///
/// Maps a `String` account name to an `u64` account balance.
#[derive(Debug)]
pub struct Accounts {
    accounts: BTreeMap<String, u64>,
}

impl Accounts {
    /// Returns an empty instance of the [`Accounts`] type
    pub fn new() -> Self {
        Accounts {
            accounts: BTreeMap::new(),
        }
    }

    /// Retrieves the balance of an account
    ///
    /// # Errors
    /// - Account doesn't exist, `AccountingError::AccountNotFound`
    pub fn balance_of(&self, signer: &str) -> Result<&u64, AccountingError> {
        self.accounts
            .get(signer)
            .ok_or(AccountingError::AccountNotFound(signer.to_string()))
    }

    /// Deposits the `amount` provided into the new `signer` account if it doesn't exist,
    /// or adds the `amount` to the existing account.
    ///
    /// # Errors
    /// - Attempted overflow (account over-funded), `AccountingError::AccountOverFunded`
    pub fn deposit(&mut self, signer: &str, amount: u64) -> Result<Tx, AccountingError> {
        if let Some(balance) = self.accounts.get_mut(signer) {
            (*balance)
                .checked_add(amount)
                .and_then(|r| {
                    *balance = r;
                    Some(r)
                })
                .ok_or(AccountingError::AccountOverFunded(
                    signer.to_string(),
                    amount,
                ))
                // Using map() here is an easy way to manipulate the non-error result only.
                .map(|_| Tx::Deposit {
                    account: signer.to_string(),
                    amount,
                })
        } else {
            self.accounts.insert(signer.to_string(), amount);
            Ok(Tx::Deposit {
                account: signer.to_string(),
                amount,
            })
        }
    }

    /// Withdraws the `amount` from the `signer` account, if it exists.
    ///
    /// # Errors
    /// - Account doesn't exist, `AccountingError::AccountNotFound`;
    /// - Attempted overflow (account under-funded), `AccountingError::AccountUnderFunded`.
    pub fn withdraw(&mut self, signer: &str, amount: u64) -> Result<Tx, AccountingError> {
        if let Some(balance) = self.accounts.get_mut(signer) {
            (*balance)
                .checked_sub(amount)
                .map(|r| {
                    *balance = r;
                    r
                })
                .ok_or(AccountingError::AccountUnderFunded(
                    signer.to_string(),
                    amount,
                ))
                .map(|_| Tx::Withdraw {
                    account: signer.to_string(),
                    amount,
                })
        } else {
            Err(AccountingError::AccountNotFound(signer.to_string()))
        }
    }

    /// Withdraws the amount from the sender's account and deposits it
    /// in the recipient's account if it wouldn't overflow.
    ///
    /// # Errors
    /// - Any of the two accounts doesn't exist, `AccountingError::AccountNotFound`;
    /// - Attempted overflow (sender's account under-funded), `AccountingError::AccountUnderFunded`;
    /// - Attempted overflow (recipient's account over-funded), `AccountingError::AccountOverFunded`.
    pub fn send(
        &mut self,
        sender: &str,
        recipient: &str,
        amount: u64,
    ) -> Result<(Tx, Tx), AccountingError> {
        // We don't have to check for the existence or balance of the sender in advance,
        // because both things are checked in `self.withdraw(sender, amount)`, which we call.
        // We are omitting that as a form of an optimization - we don't need to check
        // for the same things twice.

        // In contrast, we do need to check for the recipient's existence and balance in advance,
        // because we don't want to create a new account inside this function,
        // and we additionally need to check for potential over-funding of the
        // recipient's account, because we don't want to withdraw funds from
        // the sender's account if that would be the case.
        // We check for the recipient's balance in advance, defensively,
        // but we could alternatively try to deposit first and refund the
        // sender in case depositing fails because of an overflow.
        let recipient_balance = match self.accounts.get(recipient) {
            Some(balance) => *balance,
            None => return Err(AccountingError::AccountNotFound(recipient.to_string())),
        };
        if recipient_balance > u64::MAX - amount {
            return Err(AccountingError::AccountOverFunded(
                recipient.to_string(),
                amount,
            ));
        }

        let withdrawal = self.withdraw(sender, amount)?;
        let deposit = self.deposit(recipient, amount)?;

        Ok((withdrawal, deposit))
    }
}

/// **Prints all accounts and their balances**
pub fn print_accounts(accounts: &Accounts) {
    println!("Accounts and their balances: {:#?}", accounts.accounts);
}

/// **Prints a single requested client**
///
/// The signer's name can consist of multiple words.
/// We can wrap the signer's name in single or double quotes,
/// but we don't have to use any quotes at all.
pub fn print_single_account(words: Vec<&str>, accounts: &Accounts) {
    let words_len = words.len();

    if words_len < 2 {
        println!("The client command: {} 'signer full name'", CLIENT);
        return;
    }

    let signer = words[1..].join(" ");
    let signer = signer.trim_matches(|c| c == '\'' || c == '\"').trim();

    if is_valid_name(signer) {
        match accounts.accounts.get(signer) {
            Some(balance) => println!(r#"The client "{}" has this balance: {}."#, signer, balance),
            None => println!(r#"The client "{}" doesn't exist."#, signer),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_multiple_ok() {
        let mut accounts = Accounts::new();
        let client = "Alice";

        let mut tx = accounts.deposit(client, 25);
        assert!(tx.is_ok());
        assert_eq!(
            Ok(Tx::Deposit {
                account: client.to_string(),
                amount: 25
            }),
            tx,
        );
        assert_eq!(&25, accounts.accounts.get(client).unwrap());

        tx = accounts.deposit(client, 50);
        assert!(tx.is_ok());
        assert_eq!(
            Tx::Deposit {
                account: client.to_string(),
                amount: 50
            },
            tx.unwrap(),
        );

        assert_eq!(&75, accounts.accounts.get(client).unwrap());
    }

    #[test]
    /// `u64::MAX` == 18446744073709551615, and we are trying to overflow it in this test.
    fn deposit_err_over_funded() {
        let mut accounts = Accounts::new();
        let client = "Bob";

        let mut tx = accounts.deposit(client, u64::MAX);
        assert!(tx.is_ok());

        tx = accounts.deposit(client, 10);
        assert!(tx.is_err());
        assert_eq!(
            Err(AccountingError::AccountOverFunded(client.to_string(), 10)),
            tx
        );

        assert_eq!(&(u64::MAX), accounts.accounts.get(client).unwrap());
    }

    #[test]
    fn withdraw_multiple_ok() {
        let mut accounts = Accounts::new();
        let client = "Charlie";

        let _ = accounts.deposit(client, 25);
        let tx = accounts.withdraw(client, 5);
        assert!(tx.is_ok());
        assert_eq!(
            Ok(Tx::Withdraw {
                account: client.to_string(),
                amount: 5,
            }),
            tx
        );
        assert_eq!(&20, accounts.accounts.get(client).unwrap());

        let tx = accounts.withdraw(client, 20);
        assert!(tx.is_ok());
        assert_eq!(
            Tx::Withdraw {
                account: client.to_string(),
                amount: 20,
            },
            tx.unwrap()
        );

        assert_eq!(&0, accounts.accounts.get(client).unwrap());
    }

    #[test]
    fn withdraw_err_doesnt_exist() {
        let mut accounts = Accounts::new();
        let client = "Charlie";

        let tx = accounts.withdraw(client, 100);

        assert!(tx.is_err());
        assert_eq!(
            Err(AccountingError::AccountNotFound(client.to_string())),
            tx
        );
        assert_eq!(
            AccountingError::AccountNotFound(client.to_string()),
            tx.unwrap_err()
        );

        assert!(accounts.accounts.get(client).is_none());
        assert_eq!(
            AccountingError::AccountNotFound(client.to_string()),
            accounts.balance_of(client).unwrap_err()
        );
    }

    #[test]
    fn withdraw_err_under_funded() {
        let mut accounts = Accounts::new();
        let client = "Maria";

        let _ = accounts.deposit(client, 25);
        let tx = accounts.withdraw(client, 125);
        assert!(tx.is_err());
        assert_eq!(
            Err(AccountingError::AccountUnderFunded(client.to_string(), 125)),
            tx,
        );

        assert_eq!(&25, accounts.accounts.get(client).unwrap());
    }

    #[test]
    fn send_ok() {
        let mut accounts = Accounts::new();
        let sender = "Alice";
        let recipient = "Bob";

        let _ = accounts.deposit(sender, 100);
        let _ = accounts.deposit(recipient, 50);

        let status = accounts.send(sender, recipient, 10);

        assert!(status.is_ok());

        assert_eq!(&90, accounts.accounts.get(sender).unwrap());
        assert_eq!(&60, accounts.accounts.get(recipient).unwrap());

        assert_eq!(&90, accounts.balance_of(sender).unwrap());
        assert_eq!(&60, accounts.balance_of(recipient).unwrap());
    }

    #[test]
    fn send_err_sender_doesnt_exist() {
        let mut accounts = Accounts::new();
        let sender = "Alice";
        let recipient = "Bob";

        let _ = accounts.deposit(recipient, 50);

        let status = accounts.send(sender, recipient, 10);

        assert!(status.is_err());
        assert_eq!(
            AccountingError::AccountNotFound(sender.to_string()),
            status.unwrap_err()
        );

        assert!(accounts.accounts.get(sender).is_none());
        assert_eq!(&50, accounts.accounts.get(recipient).unwrap());
    }

    #[test]
    fn send_err_recipient_doesnt_exist() {
        let mut accounts = Accounts::new();
        let sender = "Alice";
        let recipient = "Bob";

        let _ = accounts.deposit(sender, 100);

        let status = accounts.send(sender, recipient, 10);

        assert!(status.is_err());
        assert_eq!(
            AccountingError::AccountNotFound(recipient.to_string()),
            status.unwrap_err()
        );

        assert_eq!(&100, accounts.accounts.get(sender).unwrap());
        assert!(accounts.accounts.get(recipient).is_none());
    }

    #[test]
    fn send_err_no_one_exists() {
        let mut accounts = Accounts::new();
        let sender = "Alice";
        let recipient = "Bob";

        let status = accounts.send(sender, recipient, 10);

        assert!(status.is_err());

        // Recipient is checked first
        assert_eq!(
            AccountingError::AccountNotFound(recipient.to_string()),
            status.unwrap_err()
        );

        assert!(accounts.accounts.get(sender).is_none());
        assert!(accounts.accounts.get(recipient).is_none());
    }

    #[test]
    fn send_err_sender_under_funded() {
        let mut accounts = Accounts::new();
        let sender = "Alice";
        let recipient = "Bob";

        let _ = accounts.deposit(sender, 100);
        let _ = accounts.deposit(recipient, 50);

        let status = accounts.send(sender, recipient, 200);

        assert!(status.is_err());
        assert_eq!(
            AccountingError::AccountUnderFunded(sender.to_string(), 200),
            status.unwrap_err()
        );

        assert_eq!(&100, accounts.accounts.get(sender).unwrap());
        assert_eq!(&50, accounts.accounts.get(recipient).unwrap());
    }

    #[test]
    fn send_err_recipient_over_funded() {
        let mut accounts = Accounts::new();
        let sender = "Alice";
        let recipient = "Bob";

        let _ = accounts.deposit(sender, 100);
        let _ = accounts.deposit(recipient, u64::MAX);

        let status = accounts.send(sender, recipient, 10);

        assert!(status.is_err());
        assert_eq!(
            AccountingError::AccountOverFunded(recipient.to_string(), 10),
            status.unwrap_err()
        );

        assert_eq!(&100, accounts.accounts.get(sender).unwrap());
        assert_eq!(&u64::MAX, accounts.accounts.get(recipient).unwrap());
    }
}
