use crate::accounts::Accounts;
use crate::core::{MatchingEngine, Order, PartialOrder, Receipt, Side};
use crate::errors::AccountingError;
use crate::tx::Tx;

/// Manages accounts, validates, and orchestrates the processing of each order.
pub struct TradingPlatform {
    matching_engine: MatchingEngine,
    accounts: Accounts,
    tx_log: Vec<Tx>,
}

impl TradingPlatform {
    /// Creates a new instance without any data.
    pub fn new() -> Self {
        TradingPlatform {
            matching_engine: MatchingEngine::new(),
            accounts: Accounts::new(),
            tx_log: vec![],
        }
    }

    /// Fetches the complete order book
    ///
    /// Both sides are combined together.
    ///
    /// Optionally `sort`s the book by the ordinal sequence number;
    /// `asc` stands for ascending (considered only if `sort` is `true`).
    pub fn order_book(&self, sort: bool, asc: bool) -> Vec<PartialOrder> {
        let num_orders = self.matching_engine.asks.len() + self.matching_engine.bids.len();
        let mut book = Vec::with_capacity(num_orders);

        let asks = self.matching_engine.asks.clone();
        for (_price, heap) in asks {
            for order in heap {
                book.push(order);
            }
        }

        let bids = self.matching_engine.bids.clone();
        for (_price, heap) in bids {
            for order in heap {
                book.push(order);
            }
        }

        if sort {
            book.sort_unstable();

            // `impl PartialOrd for types::PartialOrder::partial_cmp` is using reverse order of ordinals;
            // that's why we have to negate that reversal here, by reversing again for ascending order.
            if asc {
                book.reverse();
            }
        }

        book
    }

    /// Fetches the complete order book
    ///
    /// Both sides are combined together.
    ///
    /// Sorted first by price points; `rev` is for descending order.
    ///
    /// Inside of a price point, ordered by the ordinal sequence number.
    pub fn order_book_by_price(&self, rev: bool) -> Vec<PartialOrder> {
        let mut asks = self.matching_engine.asks.clone();
        let mut bids = self.matching_engine.bids.clone();

        // An optimization.
        // We add a smaller tree to a larger tree. This ensures fewer self-balancing operations.
        // The size is determined by the number of elements, which are heaps at different price points.
        // Sizes of heaps, or total number of partial orders, are not relevant for this.
        let asks_larger = asks.len() >= bids.len();

        // We are iterating over the smaller of the two BTrees,
        // i.e., over the smaller of the two sides of the order book,
        // and adding those fewer partial orders to the larger data structure,
        // so that we have fewer iterations and consequently fewer self-balancing BST operations.
        let mut combined_book = if asks_larger {
            for (price, bids_heap) in bids {
                asks.entry(price).or_insert(bids_heap);
            }
            asks
        } else {
            for (price, asks_heap) in asks {
                bids.entry(price).or_insert(asks_heap);
            }
            bids
        };

        let num_orders = self.matching_engine.asks.len() + self.matching_engine.bids.len();
        let mut book: Vec<PartialOrder> = Vec::with_capacity(num_orders);

        if !rev {
            for heap in combined_book.values_mut() {
                while let Some(order) = heap.pop() {
                    book.push(order);
                }
            }
        } else {
            for (_price, heap) in combined_book.iter_mut().rev() {
                while let Some(order) = heap.pop() {
                    book.push(order);
                }
            }
        }

        book
    }

    ///
    pub fn balance_of(&mut self, signer: &str) -> Result<&u64, AccountingError> {
        self.accounts.balance_of(signer)
    }

    /// Deposit funds
    pub fn deposit(&mut self, signer: &str, amount: u64) -> Result<Tx, AccountingError> {
        let result = self.accounts.deposit(signer, amount)?;
        self.tx_log.push(result.clone());
        Ok(result)
    }

    /// Withdraw funds
    pub fn withdraw(&mut self, signer: &str, amount: u64) -> Result<Tx, AccountingError> {
        let result = self.accounts.withdraw(signer, amount)?;
        self.tx_log.push(result.clone());
        Ok(result)
    }

    /// Transfer funds between sender and recipient
    pub fn send(
        &mut self,
        sender: &str,
        recipient: &str,
        amount: u64,
    ) -> Result<(Tx, Tx), AccountingError> {
        let result = self.accounts.send(sender, recipient, amount)?;
        let result_copy = result.clone();
        let tx_withdraw = result_copy.0;
        let tx_deposit = result_copy.1;
        self.tx_log.push(tx_withdraw);
        self.tx_log.push(tx_deposit);
        Ok(result)
    }

    /// Process a given order and apply the outcome to the accounts involved.
    ///
    /// Note that there are very few safeguards in place.
    ///
    /// The account from the order is expected to exist, regardless of its side.
    /// If it doesn't exist, the [`AccountingError::AccountNotFound`] error is returned,
    /// containing the order signer's account (name).
    pub fn process_order(&mut self, order: Order) -> Result<Receipt, AccountingError> {
        let order_signer = &order.signer.clone();
        let account_balance = *self.balance_of(order_signer)?;

        let order_side = order.side.clone();

        // For Buy orders, guard for solvency, i.e., make sure the account has
        // a sufficiently high balance to buy amount * price.
        // A buyer puts the highest price that they are willing to pay,
        // and if they find a cheaper deal, good for them.
        // What matters is that they have enough funds in the worst case,
        // and that's what we're checking here.
        if order_side == Side::Buy {
            let required_amount = order.get_initial_amount() * order.price;
            if account_balance < required_amount {
                return Err(AccountingError::AccountUnderFunded(
                    order_signer.to_string(),
                    required_amount,
                ));
            }
        }

        // Run the matching
        let receipt = self.matching_engine.process(order)?;

        // This is the total value of the order that was realized.
        // Namely, in the Buy case, it can be lower than the worst case, which is good for the buyer.
        // Conversely, in the Sell case, it can be higher than the worst case, which is good for the seller.
        // It is not used anywhere, though.
        let _total_realized: u64 = receipt
            .matches
            .iter()
            .map(|po| {
                (po.current_amount.checked_sub(po.remaining_amount))
                    .expect("Current amount of a partial order less than its remaining amount!")
                    .checked_mul(po.price)
                    .expect("Product overflowed!")
            })
            .sum();

        // Move funds in accordance with the trade requirements
        match order_side {
            Side::Buy => {
                for po in &receipt.matches {
                    self.send(
                        order_signer,
                        po.signer.as_str(),
                        po.current_amount
                            .checked_sub(po.remaining_amount)
                            .expect(
                                "Current amount of a partial order less than its remaining amount!",
                            )
                            .checked_mul(po.price)
                            .expect("Product overflowed!"),
                    )?;
                }
            }
            Side::Sell => {
                for po in &receipt.matches {
                    self.send(
                        po.signer.as_str(),
                        order_signer,
                        po.current_amount
                            .checked_sub(po.remaining_amount)
                            .expect(
                                "Current amount of a partial order less than its remaining amount!",
                            )
                            .checked_mul(po.price)
                            .expect("Product overflowed!"),
                    )?;
                }
            }
        }

        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_book_sorted_both_ways() {
        let mut trading_platform = TradingPlatform::new();

        // Set up accounts
        assert!(trading_platform.accounts.deposit("Alice", 100).is_ok());
        assert!(trading_platform.accounts.deposit("Bob", 100).is_ok());
        assert!(trading_platform.accounts.deposit("Charlie", 100).is_ok());
        assert!(trading_platform.accounts.deposit("Donna", 100).is_ok());
        assert!(trading_platform.accounts.deposit("Eleanor", 100).is_ok());

        trading_platform
            .process_order(Order::new(15, 1, Side::Sell, String::from("Alice")))
            .unwrap();
        trading_platform
            .process_order(Order::new(12, 3, Side::Buy, String::from("Bob")))
            .unwrap();
        trading_platform
            .process_order(Order::new(14, 2, Side::Sell, String::from("Charlie")))
            .unwrap();
        trading_platform
            .process_order(Order::new(10, 4, Side::Buy, String::from("Donna")))
            .unwrap();
        trading_platform
            .process_order(Order::new(14, 5, Side::Sell, String::from("Eleanor")))
            .unwrap();

        assert_eq!(5, trading_platform.order_book(false, true).len());

        let mut expected = ["Alice", "Bob", "Charlie", "Donna", "Eleanor"];
        assert_eq!(
            expected,
            trading_platform
                .order_book(true, true)
                .iter()
                .map(|po| po.signer.as_str())
                .collect::<Vec<_>>()
                .as_slice()
        );

        expected.reverse();
        assert_eq!(
            expected.to_vec(),
            trading_platform
                .order_book(true, false)
                .iter()
                .map(|po| po.signer.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn order_book_by_price_sorted_both_ways() {
        let mut trading_platform = TradingPlatform::new();

        // Set up accounts
        assert!(trading_platform.accounts.deposit("Alice", 100).is_ok());
        assert!(trading_platform.accounts.deposit("Bob", 100).is_ok());
        assert!(trading_platform.accounts.deposit("Charlie", 100).is_ok());
        assert!(trading_platform.accounts.deposit("Donna", 100).is_ok());
        assert!(trading_platform.accounts.deposit("Eleanor", 100).is_ok());

        trading_platform
            .process_order(Order::new(15, 1, Side::Sell, String::from("Alice")))
            .unwrap();
        trading_platform
            .process_order(Order::new(12, 3, Side::Buy, String::from("Bob")))
            .unwrap();
        trading_platform
            .process_order(Order::new(14, 2, Side::Sell, String::from("Charlie")))
            .unwrap();
        trading_platform
            .process_order(Order::new(10, 4, Side::Buy, String::from("Donna")))
            .unwrap();
        trading_platform
            .process_order(Order::new(14, 5, Side::Sell, String::from("Eleanor")))
            .unwrap();

        assert_eq!(5, trading_platform.order_book_by_price(false).len());

        let mut expected = ["Donna", "Bob", "Charlie", "Eleanor", "Alice"];
        assert_eq!(
            expected,
            trading_platform
                .order_book_by_price(false)
                .iter()
                .map(|po| po.signer.as_str())
                .collect::<Vec<_>>()
                .as_slice()
        );

        expected = ["Alice", "Charlie", "Eleanor", "Bob", "Donna"];
        assert_eq!(
            expected.to_vec(),
            trading_platform
                .order_book_by_price(true)
                .iter()
                .map(|po| po.signer.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn deposit_works() {
        let mut trading_platform = TradingPlatform::new();

        assert_eq!(
            Ok(Tx::Deposit {
                account: "Alice".to_string(),
                amount: 100
            }),
            trading_platform.deposit("Alice", 100)
        );

        // Check the account balance
        assert_eq!(Ok(&100), trading_platform.balance_of("Alice"));
    }

    #[test]
    fn deposit_overflows() {
        let mut trading_platform = TradingPlatform::new();

        assert!(trading_platform.deposit("Alice", u64::MAX).is_ok());

        assert_eq!(
            AccountingError::AccountOverFunded("Alice".to_string(), 1),
            trading_platform.deposit("Alice", 1).unwrap_err()
        );

        // Check the account balance
        assert_eq!(Ok(&u64::MAX), trading_platform.balance_of("Alice"));
    }

    #[test]
    fn deposit_multiple_works() {
        let mut trading_platform = TradingPlatform::new();

        assert!(trading_platform.deposit("Alice", 100).is_ok());
        assert!(trading_platform.deposit("Alice", 20).is_ok());

        // Check the account balance
        assert_eq!(Ok(&120), trading_platform.balance_of("Alice"));
    }

    #[test]
    fn withdraw_works() {
        let mut trading_platform = TradingPlatform::new();

        assert!(trading_platform.deposit("Alice", 100).is_ok());
        assert_eq!(
            Ok(Tx::Withdraw {
                account: "Alice".to_string(),
                amount: 30
            }),
            trading_platform.withdraw("Alice", 30)
        );

        // Check the account balance
        assert_eq!(Ok(&70), trading_platform.balance_of("Alice"));
    }

    #[test]
    fn withdraw_multiple_works() {
        let mut trading_platform = TradingPlatform::new();

        assert!(trading_platform.deposit("Alice", 100).is_ok());
        assert!(trading_platform.withdraw("Alice", 30).is_ok());
        assert_eq!(
            Ok(Tx::Withdraw {
                account: "Alice".to_string(),
                amount: 20
            }),
            trading_platform.withdraw("Alice", 20)
        );

        // Check the account balance
        assert_eq!(Ok(&50), trading_platform.balance_of("Alice"));
    }

    #[test]
    fn deposit_withdraw_multiple_works() {
        let mut trading_platform = TradingPlatform::new();

        assert!(trading_platform.deposit("Alice", 100).is_ok());
        assert!(trading_platform.withdraw("Alice", 30).is_ok());
        assert!(trading_platform.deposit("Alice", 10).is_ok());
        assert!(trading_platform.withdraw("Alice", 20).is_ok());

        // Check the account balance
        assert_eq!(Ok(&60), trading_platform.balance_of("Alice"));
    }

    #[test]
    fn withdraw_err_doesnt_exist() {
        let mut trading_platform = TradingPlatform::new();

        let tx = trading_platform.withdraw("Alice", 30);

        assert!(tx.is_err());
        assert_eq!(
            AccountingError::AccountNotFound("Alice".to_string()),
            tx.unwrap_err()
        );

        assert_eq!(
            AccountingError::AccountNotFound("Alice".to_string()),
            trading_platform.balance_of("Alice").unwrap_err()
        );
    }

    #[test]
    fn withdraw_err_under_funded() {
        let mut trading_platform = TradingPlatform::new();

        assert!(trading_platform.deposit("Alice", 100).is_ok());

        let tx = trading_platform.withdraw("Alice", 130);

        assert!(tx.is_err());
        assert_eq!(
            AccountingError::AccountUnderFunded("Alice".to_string(), 130),
            tx.unwrap_err()
        );

        assert_eq!(Ok(&100), trading_platform.balance_of("Alice"));
    }

    #[test]
    fn send_ok() {
        let mut trading_platform = TradingPlatform::new();

        assert!(trading_platform.deposit("Alice", 100).is_ok());
        assert!(trading_platform.deposit("Bob", 50).is_ok());

        let status = trading_platform.send("Alice", "Bob", 10);

        assert!(status.is_ok());

        assert_eq!(Ok(&90), trading_platform.balance_of("Alice"));
        assert_eq!(Ok(&60), trading_platform.balance_of("Bob"));
    }

    #[test]
    fn send_err_sender_doesnt_exist() {
        let mut trading_platform = TradingPlatform::new();

        assert!(trading_platform.deposit("Bob", 50).is_ok());

        let status = trading_platform.send("Alice", "Bob", 10);

        assert_eq!(
            AccountingError::AccountNotFound("Alice".to_string()),
            status.unwrap_err()
        );

        assert_eq!(
            AccountingError::AccountNotFound("Alice".to_string()),
            trading_platform.balance_of("Alice").unwrap_err()
        );
        assert_eq!(Ok(&50), trading_platform.balance_of("Bob"));
    }

    #[test]
    fn send_err_recipient_doesnt_exist() {
        let mut trading_platform = TradingPlatform::new();

        assert!(trading_platform.deposit("Alice", 100).is_ok());

        let status = trading_platform.send("Alice", "Bob", 10);

        assert_eq!(
            AccountingError::AccountNotFound("Bob".to_string()),
            status.unwrap_err()
        );

        assert_eq!(Ok(&100), trading_platform.balance_of("Alice"));
        assert_eq!(
            AccountingError::AccountNotFound("Bob".to_string()),
            trading_platform.balance_of("Bob").unwrap_err()
        );
    }

    #[test]
    fn send_err_no_one_exists() {
        let mut trading_platform = TradingPlatform::new();

        let status = trading_platform.send("Alice", "Bob", 10);

        assert_eq!(
            AccountingError::AccountNotFound("Bob".to_string()),
            status.unwrap_err()
        );

        assert_eq!(
            AccountingError::AccountNotFound("Alice".to_string()),
            trading_platform.balance_of("Alice").unwrap_err()
        );
        assert_eq!(
            AccountingError::AccountNotFound("Bob".to_string()),
            trading_platform.balance_of("Bob").unwrap_err()
        );
    }

    #[test]
    fn send_err_sender_under_funded() {
        let mut trading_platform = TradingPlatform::new();

        assert!(trading_platform.deposit("Alice", 100).is_ok());
        assert!(trading_platform.deposit("Bob", 50).is_ok());

        let status = trading_platform.send("Alice", "Bob", 200);

        assert_eq!(
            AccountingError::AccountUnderFunded("Alice".to_string(), 200),
            status.unwrap_err()
        );

        assert_eq!(Ok(&100), trading_platform.balance_of("Alice"));
        assert_eq!(Ok(&50), trading_platform.balance_of("Bob"));
    }

    #[test]
    fn send_err_recipient_over_funded() {
        let mut trading_platform = TradingPlatform::new();

        assert!(trading_platform.deposit("Alice", 100).is_ok());
        assert!(trading_platform.deposit("Bob", u64::MAX).is_ok());

        let status = trading_platform.send("Alice", "Bob", 10);

        assert_eq!(
            AccountingError::AccountOverFunded("Bob".to_string(), 10),
            status.unwrap_err()
        );

        assert_eq!(Ok(&100), trading_platform.balance_of("Alice"));
        assert_eq!(Ok(&u64::MAX), trading_platform.balance_of("Bob"));
    }

    #[test]
    fn process_order_requires_for_the_sell_account_to_exist_to_be_able_to_order() {
        let mut trading_platform = TradingPlatform::new();

        assert_eq!(
            trading_platform.process_order(Order::new(10, 1, Side::Sell, String::from("Alice"))),
            Err(AccountingError::AccountNotFound("Alice".to_string()))
        );
        assert!(trading_platform.matching_engine.asks.is_empty());
        assert!(trading_platform.matching_engine.bids.is_empty());
    }

    #[test]
    fn process_order_requires_for_the_buy_account_to_exist_to_be_able_to_order() {
        let mut trading_platform = TradingPlatform::new();

        assert_eq!(
            trading_platform.process_order(Order::new(10, 1, Side::Buy, String::from("Alice"))),
            Err(AccountingError::AccountNotFound("Alice".to_string()))
        );
        assert!(trading_platform.matching_engine.asks.is_empty());
        assert!(trading_platform.matching_engine.bids.is_empty());
    }

    #[test]
    fn process_order_checks_for_balance_in_buy_case_underfunded() {
        let mut trading_platform = TradingPlatform::new();

        assert!(trading_platform.deposit("Alice", 100).is_ok());

        let alice_receipt =
            trading_platform.process_order(Order::new(10, 11, Side::Buy, String::from("Alice")));
        assert_eq!(
            AccountingError::AccountUnderFunded("Alice".to_string(), 110),
            alice_receipt.unwrap_err()
        );
    }

    #[test]
    fn process_order_partially_match_order_updates_accounts_seller_first_1() {
        let mut trading_platform = TradingPlatform::new();

        // Set up accounts
        assert!(trading_platform.deposit("Alice", 100).is_ok());
        assert!(trading_platform.deposit("Bob", 100).is_ok());

        let alice_receipt = trading_platform
            .process_order(Order::new(10, 1, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let bob_receipt = trading_platform
            .process_order(Order::new(10, 2, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(2, bob_receipt.ordinal);
        assert_eq!(
            vec![PartialOrder {
                price: 10,
                current_amount: 1,
                remaining_amount: 0,
                side: Side::Sell,
                signer: "Alice".to_string(),
                ordinal: 1
            }],
            bob_receipt.matches,
        );

        assert_eq!(0, trading_platform.matching_engine.asks.len());
        assert_eq!(1, trading_platform.matching_engine.bids.len());

        // Check the account balances
        assert_eq!(Ok(&110), trading_platform.balance_of("Alice"));
        assert_eq!(Ok(&90), trading_platform.balance_of("Bob"));
    }

    #[test]
    fn process_order_partially_match_order_updates_accounts_seller_first_2() {
        let mut trading_platform = TradingPlatform::new();

        // Set up accounts
        assert!(trading_platform.accounts.deposit("Alice", 100).is_ok());
        assert!(trading_platform.accounts.deposit("Bob", 100).is_ok());

        let alice_receipt = trading_platform
            .process_order(Order::new(10, 2, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let bob_receipt = trading_platform
            .process_order(Order::new(10, 1, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(2, bob_receipt.ordinal);
        assert_eq!(
            vec![PartialOrder {
                price: 10,
                current_amount: 2,
                remaining_amount: 1,
                side: Side::Sell,
                signer: "Alice".to_string(),
                ordinal: 1
            }],
            bob_receipt.matches,
        );

        assert_eq!(1, trading_platform.matching_engine.asks.len());
        assert_eq!(0, trading_platform.matching_engine.bids.len());

        // Check the account balances
        assert_eq!(Ok(&110), trading_platform.accounts.balance_of("Alice"));
        assert_eq!(Ok(&90), trading_platform.accounts.balance_of("Bob"));
    }

    #[test]
    fn process_order_partially_match_order_updates_accounts_buyer_first_1() {
        let mut trading_platform = TradingPlatform::new();

        // Set up accounts
        assert!(trading_platform.accounts.deposit("Alice", 100).is_ok());
        assert!(trading_platform.accounts.deposit("Bob", 100).is_ok());

        let alice_receipt = trading_platform
            .process_order(Order::new(10, 1, Side::Buy, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let bob_receipt = trading_platform
            .process_order(Order::new(10, 2, Side::Sell, String::from("Bob")))
            .unwrap();
        assert_eq!(2, bob_receipt.ordinal);
        assert_eq!(
            vec![PartialOrder {
                price: 10,
                current_amount: 1,
                remaining_amount: 0,
                side: Side::Buy,
                signer: "Alice".to_string(),
                ordinal: 1
            }],
            bob_receipt.matches,
        );

        assert_eq!(1, trading_platform.matching_engine.asks.len());
        assert_eq!(0, trading_platform.matching_engine.bids.len());

        // Check the account balances
        assert_eq!(Ok(&90), trading_platform.accounts.balance_of("Alice"));
        assert_eq!(Ok(&110), trading_platform.accounts.balance_of("Bob"));
    }

    #[test]
    fn process_order_partially_match_order_updates_accounts_buyer_first_2() {
        let mut trading_platform = TradingPlatform::new();

        // Set up accounts
        assert!(trading_platform.accounts.deposit("Alice", 100).is_ok());
        assert!(trading_platform.accounts.deposit("Bob", 100).is_ok());

        let alice_receipt = trading_platform
            .process_order(Order::new(10, 2, Side::Buy, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let bob_receipt = trading_platform
            .process_order(Order::new(10, 1, Side::Sell, String::from("Bob")))
            .unwrap();
        assert_eq!(2, bob_receipt.ordinal);
        assert_eq!(
            vec![PartialOrder {
                price: 10,
                current_amount: 2,
                remaining_amount: 1,
                side: Side::Buy,
                signer: "Alice".to_string(),
                ordinal: 1
            }],
            bob_receipt.matches,
        );

        assert_eq!(0, trading_platform.matching_engine.asks.len());
        assert_eq!(1, trading_platform.matching_engine.bids.len());

        // Check the account balances
        assert_eq!(Ok(&90), trading_platform.accounts.balance_of("Alice"));
        assert_eq!(Ok(&110), trading_platform.accounts.balance_of("Bob"));
    }

    #[test]
    fn process_order_fully_match_order_updates_accounts_seller_first() {
        let mut trading_platform = TradingPlatform::new();

        // Set up accounts
        assert!(trading_platform.accounts.deposit("Alice", 100).is_ok());
        assert!(trading_platform.accounts.deposit("Bob", 100).is_ok());

        let alice_receipt = trading_platform
            .process_order(Order::new(10, 2, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let bob_receipt = trading_platform
            .process_order(Order::new(10, 2, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(2, bob_receipt.ordinal);
        assert_eq!(
            vec![PartialOrder {
                price: 10,
                current_amount: 2,
                remaining_amount: 0,
                side: Side::Sell,
                signer: "Alice".to_string(),
                ordinal: 1
            }],
            bob_receipt.matches,
        );

        // A fully matched order doesn't remain in the book
        assert!(trading_platform.matching_engine.asks.is_empty());
        assert!(trading_platform.matching_engine.bids.is_empty());

        // Check the account balances
        assert_eq!(Ok(&120), trading_platform.accounts.balance_of("Alice"));
        assert_eq!(Ok(&80), trading_platform.accounts.balance_of("Bob"));
    }

    #[test]
    fn process_order_fully_match_order_updates_accounts_buyer_first() {
        let mut trading_platform = TradingPlatform::new();

        // Set up accounts
        assert!(trading_platform.accounts.deposit("Alice", 100).is_ok());
        assert!(trading_platform.accounts.deposit("Bob", 100).is_ok());

        let alice_receipt = trading_platform
            .process_order(Order::new(10, 2, Side::Buy, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let bob_receipt = trading_platform
            .process_order(Order::new(10, 2, Side::Sell, String::from("Bob")))
            .unwrap();
        assert_eq!(2, bob_receipt.ordinal);
        assert_eq!(
            vec![PartialOrder {
                price: 10,
                current_amount: 2,
                remaining_amount: 0,
                side: Side::Buy,
                signer: "Alice".to_string(),
                ordinal: 1
            }],
            bob_receipt.matches,
        );

        // A fully matched order doesn't remain in the book
        assert!(trading_platform.matching_engine.asks.is_empty());
        assert!(trading_platform.matching_engine.bids.is_empty());

        // Check the account balances
        assert_eq!(Ok(&80), trading_platform.accounts.balance_of("Alice"));
        assert_eq!(Ok(&120), trading_platform.accounts.balance_of("Bob"));
    }

    #[test]
    fn process_order_fully_match_order_multi_match_updates_accounts_sellers_first() {
        let mut trading_platform = TradingPlatform::new();

        // Set up accounts
        assert!(trading_platform.accounts.deposit("Alice", 100).is_ok());
        assert!(trading_platform.accounts.deposit("Bob", 100).is_ok());
        assert!(trading_platform.accounts.deposit("Charlie", 100).is_ok());

        let alice_receipt = trading_platform
            .process_order(Order::new(10, 1, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let charlie_receipt = trading_platform
            .process_order(Order::new(10, 1, Side::Sell, String::from("Charlie")))
            .unwrap();
        assert_eq!(2, charlie_receipt.ordinal);
        assert!(charlie_receipt.matches.is_empty());

        let bob_receipt = trading_platform
            .process_order(Order::new(10, 2, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(3, bob_receipt.ordinal);
        assert_eq!(
            vec![
                PartialOrder {
                    price: 10,
                    current_amount: 1,
                    remaining_amount: 0,
                    side: Side::Sell,
                    signer: "Alice".to_string(),
                    ordinal: 1
                },
                PartialOrder {
                    price: 10,
                    current_amount: 1,
                    remaining_amount: 0,
                    side: Side::Sell,
                    signer: "Charlie".to_string(),
                    ordinal: 2
                }
            ],
            bob_receipt.matches,
        );

        // A fully matched order doesn't remain in the book
        assert!(trading_platform.matching_engine.asks.is_empty());
        assert!(trading_platform.matching_engine.bids.is_empty());

        // Check account balances
        assert_eq!(Ok(&110), trading_platform.accounts.balance_of("Alice"));
        assert_eq!(Ok(&80), trading_platform.accounts.balance_of("Bob"));
        assert_eq!(Ok(&110), trading_platform.accounts.balance_of("Charlie"));
    }

    #[test]
    fn process_order_fully_match_order_no_self_match_updates_accounts_sellers_first() {
        let mut trading_platform = TradingPlatform::new();

        // Set up accounts
        assert!(trading_platform.accounts.deposit("Alice", 100).is_ok());
        assert!(trading_platform.accounts.deposit("Charlie", 100).is_ok());

        let alice_receipt = trading_platform
            .process_order(Order::new(10, 1, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let charlie_receipt = trading_platform
            .process_order(Order::new(10, 1, Side::Sell, String::from("Charlie")))
            .unwrap();
        assert_eq!(2, charlie_receipt.ordinal);
        assert!(charlie_receipt.matches.is_empty());

        let alice_receipt = trading_platform
            .process_order(Order::new(10, 2, Side::Buy, String::from("Alice")))
            .unwrap();
        assert_eq!(3, alice_receipt.ordinal);
        assert_eq!(
            vec![PartialOrder {
                price: 10,
                current_amount: 1,
                remaining_amount: 0,
                side: Side::Sell,
                signer: "Charlie".to_string(),
                ordinal: 2
            }],
            alice_receipt.matches,
        );

        // A fully matched order doesn't remain in the book
        assert_eq!(1, trading_platform.matching_engine.asks.len());
        assert_eq!(1, trading_platform.matching_engine.bids.len());

        // Check account balances
        assert_eq!(Ok(&90), trading_platform.accounts.balance_of("Alice"));
        assert_eq!(Ok(&110), trading_platform.accounts.balance_of("Charlie"));
    }

    #[test]
    fn process_order_no_match_doesnt_update_accounts_all_sellers() {
        let mut trading_platform = TradingPlatform::new();

        // Set up accounts
        assert!(trading_platform.accounts.deposit("Alice", 100).is_ok());
        assert!(trading_platform.accounts.deposit("Bob", 100).is_ok());

        let alice_receipt = trading_platform
            .process_order(Order::new(10, 2, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let bob_receipt = trading_platform
            .process_order(Order::new(11, 2, Side::Sell, String::from("Bob")))
            .unwrap();
        assert_eq!(2, bob_receipt.ordinal);
        assert!(bob_receipt.matches.is_empty());

        assert_eq!(2, trading_platform.order_book(false, false).len());

        // Check the account balances
        assert_eq!(Ok(&100), trading_platform.accounts.balance_of("Alice"));
        assert_eq!(Ok(&100), trading_platform.accounts.balance_of("Bob"));
    }

    #[test]
    fn process_order_no_match_doesnt_update_accounts_all_buyers() {
        let mut trading_platform = TradingPlatform::new();

        // Set up accounts
        assert!(trading_platform.accounts.deposit("Alice", 100).is_ok());
        assert!(trading_platform.accounts.deposit("Bob", 100).is_ok());

        let alice_receipt = trading_platform
            .process_order(Order::new(10, 2, Side::Buy, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let bob_receipt = trading_platform
            .process_order(Order::new(11, 2, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(2, bob_receipt.ordinal);
        assert!(bob_receipt.matches.is_empty());

        assert_eq!(2, trading_platform.order_book(false, false).len());

        // Check the account balances
        assert_eq!(Ok(&100), trading_platform.accounts.balance_of("Alice"));
        assert_eq!(Ok(&100), trading_platform.accounts.balance_of("Bob"));
    }

    #[test]
    fn process_order_no_match_doesnt_update_accounts() {
        let mut trading_platform = TradingPlatform::new();

        // Set up accounts
        assert!(trading_platform.accounts.deposit("Alice", 100).is_ok());
        assert!(trading_platform.accounts.deposit("Bob", 100).is_ok());

        let alice_receipt = trading_platform
            .process_order(Order::new(12, 2, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let bob_receipt = trading_platform
            .process_order(Order::new(11, 2, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(2, bob_receipt.ordinal);
        assert!(bob_receipt.matches.is_empty());

        assert_eq!(2, trading_platform.order_book(false, false).len());

        // Check the account balances
        assert_eq!(Ok(&100), trading_platform.accounts.balance_of("Alice"));
        assert_eq!(Ok(&100), trading_platform.accounts.balance_of("Bob"));
    }
}
