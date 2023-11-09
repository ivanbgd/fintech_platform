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
    /// Sorted by price points; `rev` is for descending order.
    ///
    /// Inside a price point, ordered by ordinal sequence number.
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
        todo!();
    }

    /// Deposit funds
    pub fn deposit(&mut self, signer: &str, amount: u64) -> Result<Tx, AccountingError> {
        todo!();
    }

    /// Withdraw funds
    pub fn withdraw(&mut self, signer: &str, amount: u64) -> Result<Tx, AccountingError> {
        todo!();
    }

    /// Transfer funds between sender and recipient
    pub fn send(
        &mut self,
        sender: &str,
        recipient: &str,
        amount: u64,
    ) -> Result<(Tx, Tx), AccountingError> {
        todo!();
    }

    /// Process a given order and apply the outcome to the accounts involved.
    /// Note that there are very few safeguards in place.
    pub fn process_order(&mut self, order: Order) -> Result<Receipt, AccountingError> {
        self.matching_engine.process(order)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_order_requires_deposit_to_order() {
        let mut trading_platform = TradingPlatform::new();

        assert_eq!(
            trading_platform.process_order(Order::new(10, 1, Side::Sell, String::from("Alice"))),
            Err(AccountingError::AccountNotFound("Alice".to_string()))
        );
        assert!(trading_platform.matching_engine.asks.is_empty());
        assert!(trading_platform.matching_engine.bids.is_empty());
    }

    #[test]
    fn process_order_partially_match_order_updates_accounts() {
        let mut trading_platform = TradingPlatform::new();

        // Set up accounts
        assert!(trading_platform.accounts.deposit("Alice", 100).is_ok());
        assert!(trading_platform.accounts.deposit("Bob", 100).is_ok());

        let alice_receipt = trading_platform
            .process_order(Order::new(10, 1, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let bob_receipt = trading_platform
            .process_order(Order::new(10, 2, Side::Buy, String::from("Bob")))
            .unwrap();

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
        assert!(trading_platform.matching_engine.asks.is_empty());
        assert_eq!(1, trading_platform.matching_engine.bids.len());

        // Check the account balances
        assert_eq!(Ok(&110), trading_platform.accounts.balance_of("Alice"));
        assert_eq!(Ok(&90), trading_platform.accounts.balance_of("Bob"));
    }

    #[test]
    fn process_order_fully_match_order_updates_accounts() {
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
    fn process_order_fully_match_order_multi_match_updates_accounts() {
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
    fn process_order_fully_match_order_no_self_match_updates_accounts() {
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

        let bob_receipt = trading_platform
            .process_order(Order::new(10, 2, Side::Buy, String::from("Alice")))
            .unwrap();

        assert_eq!(
            vec![PartialOrder {
                price: 10,
                current_amount: 1,
                remaining_amount: 0,
                side: Side::Sell,
                signer: "Charlie".to_string(),
                ordinal: 2
            }],
            bob_receipt.matches,
        );

        // A fully matched order doesn't remain in the book
        assert_eq!(1, trading_platform.matching_engine.asks.len());
        assert_eq!(1, trading_platform.matching_engine.bids.len());

        // Check account balances
        assert_eq!(Ok(&90), trading_platform.accounts.balance_of("Alice"));
        assert_eq!(Ok(&110), trading_platform.accounts.balance_of("Charlie"));
    }

    #[test]
    fn process_order_no_match_updates_accounts() {
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
    fn order_book_sorted_both_ways() {
        let mut trading_platform = TradingPlatform::new();

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
}
