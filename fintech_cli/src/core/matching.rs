use fintech_common::core::types::{Order, PartialOrder, Receipt, Side};
use fintech_common::errors::AccountingError;
use std::collections::{BTreeMap, BinaryHeap};

/// **A FIFO matching engine**
///
/// An [`Order`] contains a spot price, and not a range of prices.
///
/// Matching an order comprises two steps:
/// 1. The price is always considered first for an order.
///    If a matching price is found in the order book's price range, the best price is considered first.
///    The best price means the lowest in case of bidding (buying), and the highest in case of asking (selling).
///    If a matching price isn't found for an order, step 2 is skipped and nothing happens.
/// 2. After the best matching price is found, the lowest ordinal number **at the price** is considered.
///    The oldest order at the price will have the lowest ordinal number of all the orders at that price.
///    So, the algorithm works on the FIFO (First In First Out), or the FCFS (First Come First Serve) basis.
///    The algorithm tries to be fair by favoring the orders that came before other orders at the same price.
///
/// All orders (bids, asks) are recorded, even if they are unsuccessful (unmatched).
/// A single receipt can hold matches from multiple matched orders. If that's the case,
/// it's still counted as one match in the receipt.
///
/// When bidding (buying), the lowest ask price is considered first by the matching algorithm.
/// When asking (selling), the highest buy price is considered first by the matching algorithm.
///
/// That is how we made our matching engine - this is what makes the most sense.
/// This makes the matching engine symmetrical and consequently the most fair to all participants, to both sides.
///
/// If there are sellers at 12 and 13, and then a buyer comes, who is willing to buy at as high as 15,
/// the matching algorithm favours the seller at 12.
/// This is the most fair to both the buyer and to the seller, because they offered less than the other seller,
/// meaning they are willing to take less, so they get precedence.
///
/// Symmetrically, if there are buyers at 12 and 13, and then a seller comes,
/// who is willing to sell at as low as 10, the matching algorithm favours the buyer at 13.
/// This is the most fair to both the seller and to the buyer, too,
/// because they offered more than the other buyer, so they get priority.
///
/// It uses a FIFO (FCFS) algorithm for matching orders, which means that an order that came first
/// *at a given price* is served first (First Come First Served), i.e., before other orders at the
/// same price that came after it.
///
/// *Note:*
/// The live project's implementation works in the opposite way than my implementation,
/// but only in case of selling. The buying case works in the same way.
/// But, this means that their implementation is asymmetrical, and hence not fair.
pub struct MatchingEngine {
    /// The order's unique ordinal (linear) sequence number.
    pub ordinal: u64,
    /// The "Ask" or "Sell" side of the order book; ordered by the price first, and then by the ordinal number (FIFO).
    /// Maps the price by which it is sorted ascending first to a priority queue of [`PartialOrder`]s.
    pub asks: BTreeMap<u64, BinaryHeap<PartialOrder>>,
    /// The "Bid" or "Buy" side of the order book; ordered by the price first, and then by the ordinal number (FIFO).
    /// Maps the price by which it is sorted ascending first to a priority queue of [`PartialOrder`]s.
    pub bids: BTreeMap<u64, BinaryHeap<PartialOrder>>,
    /// The history of all previous orders, or receipts, to be more precise,
    /// matched or unmatched, for record keeping.
    pub history: Vec<Receipt>,
}

impl MatchingEngine {
    /// Creates a new [`MatchingEngine`] with ordinal of 0 and empty sides of the order book.
    pub fn new() -> MatchingEngine {
        MatchingEngine {
            ordinal: 0_u64,
            asks: BTreeMap::new(),
            bids: BTreeMap::new(),
            history: Vec::new(),
        }
    }

    /// Processes an [`Order`] and returns a [`Receipt`].
    ///
    /// A receipt contains the order's ordinal sequence number (`u64`),
    /// and a vector of fully- or partially-matched partial orders from the other side
    /// of the order book (`Vec<PartialOrder>`).
    ///
    /// Tries to match the `order` to the contents of the appropriate side of the order book.
    /// If a part of the order remains, it adds it to the order book for future matching.
    ///
    /// An `Order` is turned into a `PartialOrder`, and it is then processed as such.
    ///
    /// If the order wasn't fully matched at the end, after going through the entire price range,
    /// this function updates the partial order's current amount as if that were an `Order` and its
    /// initial amount field. So, it's like creating a new `Order`, but in the form of a `PartialOrder`.
    /// It retains its ordinal, side, signer and price.
    /// It gets a new `current_amount` and its `remaining_amount` becomes equal to the new `current_amount`.
    /// So, `process` potentially updates the `current_amount` field of the `Order` it gets,
    /// just in the form of a `PartialOrder`.
    ///
    /// It calls `match_order` which updates the `remaining_amount` field of the
    /// *already existing* `PartialOrder`s in the entry book, that we are trying to match
    /// our `order` with.
    ///
    /// # Returns
    /// - `Ok(Receipt)`
    ///
    /// # Errors
    /// - Doesn't return an error variant.
    /// - The return type of `Result<Receipt, AccountingError>` was chosen for consistency with rest of code.
    pub fn process(&mut self, order: Order) -> Result<Receipt, AccountingError> {
        // We record every order, even if it turns out to be unmatched
        // at the moment of entering the order book or any time later when processed.
        // It may be matched at some point, either fully, or partially.
        // That is why we set a unique ordinal number for every order that comes in.
        self.ordinal += 1;

        let original_amount = order.get_initial_amount();

        // This is the order that we get and that we are trying to find matches for
        // among the already existing orders in the order book.
        // We just convert into a partial order so that we have some metadata
        // to help us through the matching process.
        let mut partial_order = order.into_partial_order(self.ordinal, original_amount);

        // Orders are matched to the opposite side of the order book.
        let receipt = match partial_order.side {
            Side::Buy => {
                // Fetch all orders in the expected price range from the opposite side of the order book.
                // The best price in case of buying (bidding) is the lowest price, so we start with it.
                // We take a mutable reference to the min-heap so the matching engine can remove any matching entries.
                let sell_entries = self.asks.range_mut(0..=partial_order.price);

                let buy_receipt = MatchingEngine::match_order(&partial_order, sell_entries)?;
                let matched_buy_amount: u64 =
                    buy_receipt.matches.iter().map(|po| po.current_amount).sum();

                // After going through the entire price range, if some unmatched amount
                // to buy remains, update the existing order.
                // After updating it, we need to put it back in the order book because
                // `match_order` removes the matching entries.
                if matched_buy_amount < original_amount {
                    partial_order.current_amount = original_amount - matched_buy_amount;
                    partial_order.remaining_amount = partial_order.current_amount;
                    let heap = self
                        .bids
                        .entry(partial_order.price)
                        .or_insert(BinaryHeap::new());
                    heap.push(partial_order);
                }

                buy_receipt
            }
            Side::Sell => {
                // Fetch all orders in the expected price range from the opposite side of the order book.
                // The best price in case of selling (asking) is the highest price, so we reverse the iterator.
                // Note: The course creator doesn't reverse the iterator. I think it is a bug on their account.
                // We take a mutable reference to the min-heap so the matching engine can remove any matching entries.
                let buy_entries = self.bids.range_mut(partial_order.price..=u64::MAX).rev();

                let sell_receipt = MatchingEngine::match_order(&partial_order, buy_entries)?;
                let matched_sell_amount: u64 = sell_receipt
                    .matches
                    .iter()
                    .map(|po| po.current_amount)
                    .sum();

                // After going through the entire price range, if some unmatched amount
                // to sell remains, update the existing order.
                // After updating it, we need to put it back in the order book because
                // `match_order` removes the matching entries.
                if matched_sell_amount < original_amount {
                    partial_order.current_amount = original_amount - matched_sell_amount;
                    partial_order.remaining_amount = partial_order.current_amount;
                    let heap = self
                        .asks
                        .entry(partial_order.price)
                        .or_insert(BinaryHeap::new());
                    heap.push(partial_order);
                }

                sell_receipt
            }
        };

        // Clean-up: Remove price entries without orders from the order book.
        self.asks.retain(|_price, heap| !heap.is_empty());
        self.bids.retain(|_price, heap| !heap.is_empty());

        // Keep a record of all orders, even unmatched ones.
        self.history.push(receipt.clone());

        Ok(receipt)
    }

    /// Processes a [`PartialOrder`] and returns a [`Receipt`].
    ///
    /// Matches an order (a [`PartialOrder`], to be more accurate) with the provided side of the order book.
    ///
    /// Removes the fully-matching (fully-exhausted) entries from the (given side of the) order book.
    ///
    /// Updates the `remaining_amount` field of the already existing `PartialOrder`s in the entry book,
    /// in cases of both full and partial matches.
    ///
    /// Puts both full and partial matches in the `Receipt` that it returns at the end.
    ///
    /// Updates the price in the receipt, i.e., a bill, for the matched orders.
    /// It updates it for the matched part of an existing order, and for a fully-matched order.
    ///
    /// It doesn't update the price for the remaining part of a partially-matched order, naturally.
    ///
    /// # Parameters
    /// - `partial_order`: A new [`PartialOrder`] to match in the order book.
    /// - `price_range_entries`: A pre-filtered iterator for the existing order book entries in the
    ///    requested price range, ordered by the best price:
    ///    an iterator over tuples of prices (key, `u64`)
    ///    and accompanying priority queues of pending orders at those prices (value, `BinaryHeap<PartialOrder>`).
    ///
    /// # Returns
    /// - `Ok(Receipt)`
    ///
    /// # Errors
    /// - Doesn't return an error variant.
    /// - The return type of `Result<Receipt, AccountingError>` was chosen for consistency with rest of code.
    fn match_order<'a, T>(
        partial_order: &PartialOrder,
        mut price_range_entries: T,
    ) -> Result<Receipt, AccountingError>
    where
        T: Iterator<Item = (&'a u64, &'a mut BinaryHeap<PartialOrder>)>,
    {
        // Remaining amount to match.
        let mut remaining_amount = partial_order.current_amount;

        // A list of matched partial orders.
        let mut matches: Vec<PartialOrder> = vec![];

        // Each matching position's amount is subtracted.
        'outer: while remaining_amount > 0 {
            // The iterator contains all order book entries at the given price points (u64), from a given price range,
            // in the form of a priority queue (BinaryHeap<PartialOrder>), and here we iterate over those entries.
            match price_range_entries.next() {
                Some((price, price_entry)) => {
                    // The heap `price_entry` should not be empty at this point,
                    // because even if we add a new blank heap,
                    // which we do at the end of either arm of the `MatchingEngine::process` function,
                    // we immediately fill it with a new element of the `PartialOrder` type.
                    // Still, to be fully correct and on the safe side, we check that it really isn't empty.
                    // We take a mutable reference because we want to mutate the heap.

                    // List of self-matches, i.e., when we encounter signer's own order during iteration.
                    // We skip those, but we have to return them at the end.
                    let mut self_matches = vec![];

                    // The inner loop:
                    while let Some(mut current_partial_order) = price_entry.pop() {
                        // Check for self-matching and skip it, because it is not allowed.
                        if current_partial_order.signer == partial_order.signer {
                            self_matches.push(current_partial_order);
                        } else {
                            if remaining_amount < current_partial_order.remaining_amount {
                                // We've fully matched the required amount,
                                // and the existing order hasn't been fully exhausted.
                                // This is the final iteration of the outer (or any) loop.
                                // The unmatched amount from the current partial order will retain its
                                // original price, the one that it had at the moment when processing of
                                // the order began.
                                // But, the matched part of the amount may have the price updated!
                                // It will get the current price, which may be different than the
                                // order's original price.
                                current_partial_order.remaining_amount -= remaining_amount;
                                let mut new_partial_order = current_partial_order.clone();
                                new_partial_order.current_amount =
                                    new_partial_order.remaining_amount;
                                price_entry.push(new_partial_order);
                                current_partial_order.price = *price;
                                matches.push(current_partial_order);
                                break 'outer;
                            } else {
                                // We may still have some remaining amount unmatched,
                                // and we'll potentially have to keep iterating after this.
                                // We have exhausted the current partial order in this case.
                                remaining_amount -= current_partial_order.remaining_amount;
                                current_partial_order.price = *price;
                                current_partial_order.remaining_amount = 0;
                                matches.push(current_partial_order);
                                if remaining_amount == 0 {
                                    break 'outer;
                                }
                            }
                        }
                    }

                    for m in self_matches {
                        price_entry.push(m);
                    }
                }
                None => {
                    // Nothing left to match with - no more price points to explore; we've exhausted the iterator.
                    break 'outer;
                }
            }
        }

        Ok(Receipt {
            ordinal: partial_order.ordinal,
            matches,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_increment_ordinal_matching_engine() {
        let mut matching_engine = MatchingEngine::new();
        assert_eq!(0, matching_engine.ordinal);

        let receipt = matching_engine
            .process(Order::new(10, 1, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(1, matching_engine.ordinal);
        assert_eq!(matching_engine.ordinal, receipt.ordinal);

        let receipt = matching_engine
            .process(Order::new(10, 1, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(2, matching_engine.ordinal);
        assert_eq!(receipt.ordinal, matching_engine.ordinal);

        let receipt = matching_engine
            .process(Order::new(10, 1, Side::Sell, String::from("Charlie")))
            .unwrap();
        assert_eq!(3, matching_engine.ordinal);
        assert_eq!(receipt.ordinal, matching_engine.ordinal);
    }

    #[test]
    fn process_partially_matched_buy_order_same_price_seller_first() {
        let mut matching_engine = MatchingEngine::new();

        let alice_receipt = matching_engine
            .process(Order::new(10, 1, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        // We also keep a record of unmatched orders, not only the matched ones.
        assert_eq!(1, matching_engine.history.len());

        let bob_receipt = matching_engine
            .process(Order::new(10, 2, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(2, bob_receipt.ordinal);
        assert_eq!(1, bob_receipt.matches.len());
        assert_eq!(
            PartialOrder {
                price: 10,
                current_amount: 1,
                side: Side::Sell,
                signer: String::from("Alice"),
                ordinal: 1,
                remaining_amount: 0,
            },
            bob_receipt.matches[0]
        );

        // A fully matched order doesn't remain in the book.
        assert!(matching_engine.asks.is_empty());

        assert_eq!(1, matching_engine.bids.len());
        assert_eq!(1, matching_engine.bids.get(&10).unwrap().len());
        assert_eq!(
            PartialOrder {
                price: 10,
                current_amount: 1,
                side: Side::Buy,
                signer: String::from("Bob"),
                ordinal: 2,
                remaining_amount: 1,
            },
            matching_engine
                .bids
                .get(&10)
                .unwrap()
                .peek()
                .unwrap()
                .to_owned()
        );

        // We also keep a record of unmatched orders, not only the matched ones.
        assert_eq!(2, matching_engine.history.len());

        assert_eq!(
            vec![
                Receipt {
                    ordinal: 1,
                    matches: vec![],
                },
                Receipt {
                    ordinal: 2,
                    matches: vec![PartialOrder {
                        price: 10,
                        current_amount: 1,
                        side: Side::Sell,
                        signer: String::from("Alice"),
                        ordinal: 1,
                        remaining_amount: 0,
                    }],
                }
            ],
            matching_engine.history
        );
    }

    #[test]
    fn process_partially_matched_sell_order_same_price_seller_first() {
        let mut matching_engine = MatchingEngine::new();

        let alice_receipt = matching_engine
            .process(Order::new(10, 2, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let bob_receipt = matching_engine
            .process(Order::new(10, 1, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(2, bob_receipt.ordinal);
        assert_eq!(1, bob_receipt.matches.len());
        assert_eq!(
            PartialOrder {
                price: 10,
                current_amount: 2,
                side: Side::Sell,
                signer: String::from("Alice"),
                ordinal: 1,
                remaining_amount: 1,
            },
            bob_receipt.matches[0]
        );

        // A fully matched order doesn't remain in the book.
        assert!(matching_engine.bids.is_empty());

        assert_eq!(1, matching_engine.asks.len());
        assert_eq!(1, matching_engine.asks.get(&10).unwrap().len());
        assert_eq!(
            PartialOrder {
                price: 10,
                current_amount: 1,
                side: Side::Sell,
                signer: String::from("Alice"),
                ordinal: 1,
                remaining_amount: 1,
            },
            matching_engine
                .asks
                .get(&10)
                .unwrap()
                .peek()
                .unwrap()
                .to_owned()
        );

        // We also keep a record of unmatched orders, not only the matched ones.
        assert_eq!(2, matching_engine.history.len());
    }

    #[test]
    fn process_partially_matched_buy_order_same_price_buyer_first() {
        let mut matching_engine = MatchingEngine::new();

        let bob_receipt = matching_engine
            .process(Order::new(10, 2, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(1, bob_receipt.ordinal);
        assert_eq!(0, bob_receipt.matches.len());

        // We also keep a record of unmatched orders, not only the matched ones.
        assert_eq!(1, matching_engine.history.len());

        let alice_receipt = matching_engine
            .process(Order::new(10, 1, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(2, alice_receipt.ordinal);
        assert_eq!(
            PartialOrder {
                price: 10,
                current_amount: 2,
                side: Side::Buy,
                signer: String::from("Bob"),
                ordinal: 1,
                remaining_amount: 1,
            },
            alice_receipt.matches[0]
        );

        // A fully matched order doesn't remain in the book.
        assert!(matching_engine.asks.is_empty());

        assert_eq!(1, matching_engine.bids.len());
        assert_eq!(1, matching_engine.bids.get(&10).unwrap().len());
        assert_eq!(
            PartialOrder {
                price: 10,
                current_amount: 1,
                side: Side::Buy,
                signer: String::from("Bob"),
                ordinal: 1,
                remaining_amount: 1,
            },
            matching_engine
                .bids
                .get(&10)
                .unwrap()
                .peek()
                .unwrap()
                .to_owned()
        );

        // We also keep a record of unmatched orders, not only the matched ones.
        assert_eq!(2, matching_engine.history.len());

        assert_eq!(
            vec![
                Receipt {
                    ordinal: 1,
                    matches: vec![],
                },
                Receipt {
                    ordinal: 2,
                    matches: vec![PartialOrder {
                        price: 10,
                        current_amount: 2,
                        side: Side::Buy,
                        signer: String::from("Bob"),
                        ordinal: 1,
                        remaining_amount: 1,
                    }],
                }
            ],
            matching_engine.history
        );
    }

    #[test]
    fn process_partially_matched_sell_order_same_price_buyer_first() {
        let mut matching_engine = MatchingEngine::new();

        let bob_receipt = matching_engine
            .process(Order::new(10, 1, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(1, bob_receipt.ordinal);
        assert_eq!(0, bob_receipt.matches.len());

        let alice_receipt = matching_engine
            .process(Order::new(10, 2, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(2, alice_receipt.ordinal);
        assert_eq!(
            PartialOrder {
                price: 10,
                current_amount: 1,
                side: Side::Buy,
                signer: String::from("Bob"),
                ordinal: 1,
                remaining_amount: 0,
            },
            alice_receipt.matches[0]
        );

        // A fully matched order doesn't remain in the book.
        assert!(matching_engine.bids.is_empty());

        assert_eq!(1, matching_engine.asks.len());
        assert_eq!(1, matching_engine.asks.get(&10).unwrap().len());
        assert_eq!(
            PartialOrder {
                price: 10,
                current_amount: 1,
                side: Side::Sell,
                signer: String::from("Alice"),
                ordinal: 2,
                remaining_amount: 1,
            },
            matching_engine
                .asks
                .get(&10)
                .unwrap()
                .peek()
                .unwrap()
                .to_owned()
        );

        // We also keep a record of unmatched orders, not only the matched ones.
        assert_eq!(2, matching_engine.history.len());
    }

    #[test]
    fn process_partially_matched_buy_order_different_prices_seller_first() {
        let mut matching_engine = MatchingEngine::new();

        let alice_receipt = matching_engine
            .process(Order::new(10, 1, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let bob_receipt = matching_engine
            .process(Order::new(11, 2, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(2, bob_receipt.ordinal);
        assert_eq!(1, bob_receipt.matches.len());
        assert_eq!(
            PartialOrder {
                price: 10,
                current_amount: 1,
                side: Side::Sell,
                signer: String::from("Alice"),
                ordinal: 1,
                remaining_amount: 0,
            },
            bob_receipt.matches[0]
        );

        // A fully matched order doesn't remain in the book.
        assert!(matching_engine.asks.is_empty());

        assert_eq!(1, matching_engine.bids.len());
        assert_eq!(1, matching_engine.bids.get(&11).unwrap().len());
        assert_eq!(
            PartialOrder {
                price: 11,
                current_amount: 1,
                side: Side::Buy,
                signer: String::from("Bob"),
                ordinal: 2,
                remaining_amount: 1,
            },
            matching_engine
                .bids
                .get(&11)
                .unwrap()
                .peek()
                .unwrap()
                .to_owned()
        );

        // We also keep a record of unmatched orders, not only the matched ones.
        assert_eq!(2, matching_engine.history.len());
    }

    #[test]
    fn process_partially_matched_sell_order_different_prices_seller_first() {
        let mut matching_engine = MatchingEngine::new();

        let alice_receipt = matching_engine
            .process(Order::new(10, 2, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let bob_receipt = matching_engine
            .process(Order::new(11, 1, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(2, bob_receipt.ordinal);
        assert_eq!(1, bob_receipt.matches.len());
        assert_eq!(
            PartialOrder {
                price: 10,
                current_amount: 2,
                side: Side::Sell,
                signer: String::from("Alice"),
                ordinal: 1,
                remaining_amount: 1,
            },
            bob_receipt.matches[0]
        );

        // A fully matched order doesn't remain in the book.
        assert!(matching_engine.bids.is_empty());

        assert_eq!(1, matching_engine.asks.len());
        assert_eq!(1, matching_engine.asks.get(&10).unwrap().len());
        assert_eq!(
            PartialOrder {
                price: 10,
                current_amount: 1,
                side: Side::Sell,
                signer: String::from("Alice"),
                ordinal: 1,
                remaining_amount: 1,
            },
            matching_engine
                .asks
                .get(&10)
                .unwrap()
                .peek()
                .unwrap()
                .to_owned()
        );

        // We also keep a record of unmatched orders, not only the matched ones.
        assert_eq!(2, matching_engine.history.len());
    }

    #[test]
    fn process_partially_matched_buy_order_different_prices_buyer_first() {
        let mut matching_engine = MatchingEngine::new();

        let bob_receipt = matching_engine
            .process(Order::new(11, 2, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(1, bob_receipt.ordinal);
        assert_eq!(0, bob_receipt.matches.len());

        let alice_receipt = matching_engine
            .process(Order::new(10, 1, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(2, alice_receipt.ordinal);
        assert_eq!(
            PartialOrder {
                price: 11,
                current_amount: 2,
                side: Side::Buy,
                signer: String::from("Bob"),
                ordinal: 1,
                remaining_amount: 1,
            },
            alice_receipt.matches[0]
        );

        // A fully matched order doesn't remain in the book.
        assert!(matching_engine.asks.is_empty());

        assert_eq!(1, matching_engine.bids.len());
        assert_eq!(1, matching_engine.bids.get(&11).unwrap().len());
        assert_eq!(
            PartialOrder {
                price: 11,
                current_amount: 1,
                side: Side::Buy,
                signer: String::from("Bob"),
                ordinal: 1,
                remaining_amount: 1,
            },
            matching_engine
                .bids
                .get(&11)
                .unwrap()
                .peek()
                .unwrap()
                .to_owned()
        );

        // We also keep a record of unmatched orders, not only the matched ones.
        assert_eq!(2, matching_engine.history.len());
    }

    #[test]
    fn process_partially_matched_sell_order_different_prices_buyer_first() {
        let mut matching_engine = MatchingEngine::new();

        let bob_receipt = matching_engine
            .process(Order::new(11, 1, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(1, bob_receipt.ordinal);
        assert_eq!(0, bob_receipt.matches.len());

        let alice_receipt = matching_engine
            .process(Order::new(10, 2, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(2, alice_receipt.ordinal);
        assert_eq!(
            PartialOrder {
                price: 11,
                current_amount: 1,
                side: Side::Buy,
                signer: String::from("Bob"),
                ordinal: 1,
                remaining_amount: 0,
            },
            alice_receipt.matches[0]
        );

        // A fully matched order doesn't remain in the book.
        assert!(matching_engine.bids.is_empty());

        assert_eq!(1, matching_engine.asks.len());
        assert_eq!(1, matching_engine.asks.get(&10).unwrap().len());
        assert_eq!(
            PartialOrder {
                price: 10,
                current_amount: 1,
                side: Side::Sell,
                signer: String::from("Alice"),
                ordinal: 2,
                remaining_amount: 1,
            },
            matching_engine
                .asks
                .get(&10)
                .unwrap()
                .peek()
                .unwrap()
                .to_owned()
        );

        // We also keep a record of unmatched orders, not only the matched ones.
        assert_eq!(2, matching_engine.history.len());
    }

    #[test]
    fn process_fully_matched_orders_same_price() {
        let mut matching_engine = MatchingEngine::new();

        let alice_receipt = matching_engine
            .process(Order::new(10, 2, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let bob_receipt = matching_engine
            .process(Order::new(10, 2, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(2, bob_receipt.ordinal);
        assert_eq!(
            vec![PartialOrder {
                price: 10,
                current_amount: 2,
                side: Side::Sell,
                signer: "Alice".to_string(),
                ordinal: 1,
                remaining_amount: 0,
            }],
            bob_receipt.matches,
        );

        assert!(matching_engine.asks.is_empty());
        assert!(matching_engine.bids.is_empty());

        // We also keep a record of unmatched orders, not only the matched ones.
        assert_eq!(2, matching_engine.history.len());
    }

    #[test]
    fn process_fully_matched_orders_different_prices() {
        let mut matching_engine = MatchingEngine::new();

        let alice_receipt = matching_engine
            .process(Order::new(10, 2, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let bob_receipt = matching_engine
            .process(Order::new(11, 2, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(2, bob_receipt.ordinal);
        assert_eq!(
            vec![PartialOrder {
                price: 10,
                current_amount: 2,
                side: Side::Sell,
                signer: "Alice".to_string(),
                ordinal: 1,
                remaining_amount: 0,
            }],
            bob_receipt.matches,
        );

        assert!(matching_engine.asks.is_empty());
        assert!(matching_engine.bids.is_empty());
    }

    #[test]
    fn process_fully_matched_orders_multi_partial_match_same_prices_two_sellers_one_buyer() {
        let mut matching_engine = MatchingEngine::new();

        let alice_receipt = matching_engine
            .process(Order::new(10, 1, Side::Sell, String::from("Alice")))
            .unwrap();
        assert!(alice_receipt.matches.is_empty());
        assert_eq!(1, alice_receipt.ordinal);

        assert_eq!(1, matching_engine.asks.len());
        assert_eq!(0, matching_engine.bids.len());
        assert_eq!(1, matching_engine.history.len());

        let charlie_receipt = matching_engine
            .process(Order::new(10, 1, Side::Sell, String::from("Charlie")))
            .unwrap();
        assert!(charlie_receipt.matches.is_empty());
        assert_eq!(2, charlie_receipt.ordinal);

        // Even though we have a total of two asks by Alice and Charlie, they are at the same price point,
        // so they are counted as one entry in the "asks" side of the order book.
        assert_eq!(1, matching_engine.asks.len());

        // This is a way to count the ask orders at a price level.
        assert_eq!(2, matching_engine.asks.get(&10).unwrap().len());

        // There are no bids at this point.
        assert_eq!(0, matching_engine.bids.len());

        // Even though both orders were unmatched, we still keep a record of them.
        assert_eq!(2, matching_engine.history.len());

        let bob_receipt = matching_engine
            .process(Order::new(10, 2, Side::Buy, String::from("Bob")))
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

        assert!(matching_engine.asks.is_empty());
        assert!(matching_engine.bids.is_empty());
        assert_eq!(3, matching_engine.history.len());
    }

    #[test]
    fn process_fully_matched_orders_multi_partial_match_different_prices_two_sellers_one_buyer() {
        let mut matching_engine = MatchingEngine::new();

        let alice_receipt = matching_engine
            .process(Order::new(12, 1, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        assert_eq!(1, matching_engine.asks.len());
        assert_eq!(0, matching_engine.bids.len());
        assert_eq!(1, matching_engine.history.len());

        let charlie_receipt = matching_engine
            .process(Order::new(10, 1, Side::Sell, String::from("Charlie")))
            .unwrap();
        assert_eq!(2, charlie_receipt.ordinal);
        assert!(charlie_receipt.matches.is_empty());

        // We have a total of two asks by Alice and Charlie, they are at different price points, hence 2.
        assert_eq!(2, matching_engine.asks.len());

        // This is a way to count the ask orders at a price level.
        assert_eq!(1, matching_engine.asks.get(&10).unwrap().len());
        assert_eq!(1, matching_engine.asks.get(&12).unwrap().len());

        // There are no bids at this moment.
        assert_eq!(0, matching_engine.bids.len());

        // Even though both orders were unmatched, we still keep a record of them.
        assert_eq!(2, matching_engine.history.len());

        let bob_receipt = matching_engine
            .process(Order::new(15, 2, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(3, bob_receipt.ordinal);
        assert_eq!(
            vec![
                PartialOrder {
                    price: 10,
                    current_amount: 1,
                    remaining_amount: 0,
                    side: Side::Sell,
                    signer: "Charlie".to_string(),
                    ordinal: 2
                },
                PartialOrder {
                    price: 12,
                    current_amount: 1,
                    remaining_amount: 0,
                    side: Side::Sell,
                    signer: "Alice".to_string(),
                    ordinal: 1
                },
            ],
            bob_receipt.matches,
        );

        assert!(matching_engine.asks.is_empty());
        assert!(matching_engine.bids.is_empty());
        assert_eq!(3, matching_engine.history.len());
    }

    #[test]
    fn process_fully_matched_orders_multi_partial_match_different_prices_two_buyers_one_seller() {
        let mut matching_engine = MatchingEngine::new();

        let alice_receipt = matching_engine
            .process(Order::new(10, 1, Side::Buy, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        assert_eq!(0, matching_engine.asks.len());
        assert_eq!(1, matching_engine.bids.len());
        assert_eq!(1, matching_engine.history.len());

        let charlie_receipt = matching_engine
            .process(Order::new(12, 1, Side::Buy, String::from("Charlie")))
            .unwrap();
        assert_eq!(2, charlie_receipt.ordinal);
        assert!(charlie_receipt.matches.is_empty());

        // We have a total of two bids by Alice and Charlie, they are at different price points, hence 2.
        assert_eq!(2, matching_engine.bids.len());

        // This is a way to count the ask orders at a price level.
        assert_eq!(1, matching_engine.bids.get(&10).unwrap().len());
        assert_eq!(1, matching_engine.bids.get(&12).unwrap().len());

        // There are no asks at this point.
        assert_eq!(0, matching_engine.asks.len());

        // Even though both orders were unmatched, we still keep a record of them.
        assert_eq!(2, matching_engine.history.len());

        let bob_receipt = matching_engine
            .process(Order::new(8, 2, Side::Sell, String::from("Bob")))
            .unwrap();
        assert_eq!(3, bob_receipt.ordinal);
        assert_eq!(
            vec![
                PartialOrder {
                    price: 12,
                    current_amount: 1,
                    remaining_amount: 0,
                    side: Side::Buy,
                    signer: "Charlie".to_string(),
                    ordinal: 2
                },
                PartialOrder {
                    price: 10,
                    current_amount: 1,
                    remaining_amount: 0,
                    side: Side::Buy,
                    signer: "Alice".to_string(),
                    ordinal: 1
                }
            ],
            bob_receipt.matches,
        );

        assert!(matching_engine.asks.is_empty());
        assert!(matching_engine.bids.is_empty());
        assert_eq!(3, matching_engine.history.len());
    }

    #[test]
    fn process_partially_matched_buy_order_multi_partial_match_diff_prices_two_sellers_one_buyer_1()
    {
        let mut matching_engine = MatchingEngine::new();

        let alice_receipt = matching_engine
            .process(Order::new(11, 1, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let charlie_receipt = matching_engine
            .process(Order::new(10, 1, Side::Sell, String::from("Charlie")))
            .unwrap();
        assert_eq!(2, charlie_receipt.ordinal);
        assert!(charlie_receipt.matches.is_empty());

        let bob_receipt = matching_engine
            .process(Order::new(11, 3, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(3, bob_receipt.ordinal);
        assert_eq!(2, bob_receipt.matches.len());
        assert_eq!(
            vec![
                PartialOrder {
                    price: 10,
                    current_amount: 1,
                    remaining_amount: 0,
                    side: Side::Sell,
                    signer: "Charlie".to_string(),
                    ordinal: 2
                },
                PartialOrder {
                    price: 11,
                    current_amount: 1,
                    remaining_amount: 0,
                    side: Side::Sell,
                    signer: "Alice".to_string(),
                    ordinal: 1
                }
            ],
            bob_receipt.matches,
        );

        // A fully matched order doesn't remain in the book.
        assert!(matching_engine.asks.is_empty());

        assert_eq!(1, matching_engine.bids.len());
        assert_eq!(1, matching_engine.bids.get(&11).unwrap().len());
        assert_eq!(
            PartialOrder {
                price: 11,
                current_amount: 1,
                side: Side::Buy,
                signer: String::from("Bob"),
                ordinal: 3,
                remaining_amount: 1,
            },
            matching_engine
                .bids
                .get(&11)
                .unwrap()
                .peek()
                .unwrap()
                .to_owned()
        );

        assert_eq!(3, matching_engine.history.len());
    }

    #[test]
    fn process_partially_matched_buy_order_multi_partial_match_diff_prices_two_sellers_one_buyer_2()
    {
        let mut matching_engine = MatchingEngine::new();

        let alice_receipt = matching_engine
            .process(Order::new(11, 1, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let charlie_receipt = matching_engine
            .process(Order::new(10, 1, Side::Sell, String::from("Charlie")))
            .unwrap();
        assert_eq!(2, charlie_receipt.ordinal);
        assert!(charlie_receipt.matches.is_empty());

        let bob_receipt = matching_engine
            .process(Order::new(10, 2, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(3, bob_receipt.ordinal);
        assert_eq!(
            vec![PartialOrder {
                price: 10,
                current_amount: 1,
                side: Side::Sell,
                signer: String::from("Charlie"),
                ordinal: 2,
                remaining_amount: 0,
            }],
            bob_receipt.matches
        );

        assert_eq!(1, matching_engine.asks.len());
        assert_eq!(1, matching_engine.asks.get(&11).unwrap().len());
        assert_eq!(
            PartialOrder {
                price: 11,
                current_amount: 1,
                side: Side::Sell,
                signer: String::from("Alice"),
                ordinal: 1,
                remaining_amount: 1,
            },
            matching_engine
                .asks
                .get(&11)
                .unwrap()
                .peek()
                .unwrap()
                .to_owned()
        );

        assert_eq!(1, matching_engine.bids.len());
        assert_eq!(1, matching_engine.bids.get(&10).unwrap().len());
        assert_eq!(
            PartialOrder {
                price: 10,
                current_amount: 1,
                side: Side::Buy,
                signer: String::from("Bob"),
                ordinal: 3,
                remaining_amount: 1,
            },
            matching_engine
                .bids
                .get(&10)
                .unwrap()
                .peek()
                .unwrap()
                .to_owned()
        );

        assert_eq!(3, matching_engine.history.len());
    }

    #[test]
    fn process_fully_matched_order_no_self_match() {
        let mut matching_engine = MatchingEngine::new();

        let alice_receipt = matching_engine
            .process(Order::new(10, 1, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let charlie_receipt = matching_engine
            .process(Order::new(10, 1, Side::Sell, String::from("Charlie")))
            .unwrap();
        assert_eq!(2, charlie_receipt.ordinal);
        assert!(charlie_receipt.matches.is_empty());

        let alice_receipt = matching_engine
            .process(Order::new(10, 2, Side::Buy, String::from("Alice")))
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

        assert_eq!(1, matching_engine.asks.len());
        assert_eq!(1, matching_engine.bids.len());
    }

    #[test]
    fn process_no_match_all_sellers() {
        let mut matching_engine = MatchingEngine::new();

        let alice_receipt = matching_engine
            .process(Order::new(10, 2, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let bob_receipt = matching_engine
            .process(Order::new(11, 2, Side::Sell, String::from("Bob")))
            .unwrap();
        assert_eq!(2, bob_receipt.ordinal);
        assert!(bob_receipt.matches.is_empty());

        assert_eq!(2, matching_engine.asks.len());
        assert_eq!(0, matching_engine.bids.len());
    }

    #[test]
    fn process_no_match_all_buyers() {
        let mut matching_engine = MatchingEngine::new();

        let alice_receipt = matching_engine
            .process(Order::new(10, 2, Side::Buy, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let bob_receipt = matching_engine
            .process(Order::new(11, 2, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(2, bob_receipt.ordinal);
        assert!(bob_receipt.matches.is_empty());

        assert_eq!(0, matching_engine.asks.len());
        assert_eq!(2, matching_engine.bids.len());
    }

    #[test]
    fn process_no_match_in_prices() {
        let mut matching_engine = MatchingEngine::new();

        let alice_receipt = matching_engine
            .process(Order::new(11, 2, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let bob_receipt = matching_engine
            .process(Order::new(10, 2, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(2, bob_receipt.ordinal);
        assert!(bob_receipt.matches.is_empty());

        assert_eq!(1, matching_engine.asks.len());
        assert_eq!(1, matching_engine.bids.len());
    }

    /// Tests updating of `PartialOrder`'s `current_amount` and `remaining_amount` fields.
    ///
    /// First exhausts a seller's order through multiple orders,
    /// and then exhausts a buyer's order through multiple orders.
    #[test]
    fn process_exhaust_sellers_and_buyers_through_multiple_orders() {
        let mut matching_engine = MatchingEngine::new();

        let alice_receipt = matching_engine
            .process(Order::new(10, 8, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);

        // Alice doesn't get a receipt because there was no match.
        assert!(alice_receipt.matches.is_empty());

        assert_eq!(1, matching_engine.asks.len());
        assert_eq!(0, matching_engine.bids.len());

        // The matching engine contains an ask at 10.
        assert_eq!(1, matching_engine.asks.get(&10).unwrap().len());
        assert_eq!(
            PartialOrder {
                price: 10,
                current_amount: 8,
                side: Side::Sell,
                signer: String::from("Alice"),
                ordinal: 1,
                remaining_amount: 8,
            },
            matching_engine
                .asks
                .get(&10)
                .unwrap()
                .peek()
                .unwrap()
                .to_owned()
        );

        let bob_receipt = matching_engine
            .process(Order::new(10, 2, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(2, bob_receipt.ordinal);
        assert_eq!(
            vec![PartialOrder {
                price: 10,
                current_amount: 8,
                side: Side::Sell,
                signer: "Alice".to_string(),
                ordinal: 1,
                remaining_amount: 6,
            }],
            bob_receipt.matches,
        );

        assert_eq!(1, matching_engine.asks.len());
        assert_eq!(0, matching_engine.bids.len());

        // The matching engine contains an ask at 10.
        assert_eq!(1, matching_engine.asks.get(&10).unwrap().len());
        assert_eq!(
            PartialOrder {
                price: 10,
                current_amount: 6,
                side: Side::Sell,
                signer: String::from("Alice"),
                ordinal: 1,
                remaining_amount: 6,
            },
            matching_engine
                .asks
                .get(&10)
                .unwrap()
                .peek()
                .unwrap()
                .to_owned()
        );

        let charlie_receipt = matching_engine
            .process(Order::new(11, 4, Side::Buy, String::from("Charlie")))
            .unwrap();
        assert_eq!(3, charlie_receipt.ordinal);
        assert_eq!(
            vec![PartialOrder {
                price: 10,
                current_amount: 6,
                side: Side::Sell,
                signer: "Alice".to_string(),
                ordinal: 1,
                remaining_amount: 2,
            }],
            charlie_receipt.matches,
        );

        assert_eq!(1, matching_engine.asks.len());
        assert_eq!(0, matching_engine.bids.len());

        // The matching engine contains an ask at 10.
        assert_eq!(1, matching_engine.asks.get(&10).unwrap().len());
        assert_eq!(
            PartialOrder {
                price: 10,
                current_amount: 2,
                side: Side::Sell,
                signer: String::from("Alice"),
                ordinal: 1,
                remaining_amount: 2,
            },
            matching_engine
                .asks
                .get(&10)
                .unwrap()
                .peek()
                .unwrap()
                .to_owned()
        );

        // At the very moment Donna enters the order book, her initial (current) and remaining amounts are both 5.
        // But, after her order's been processed, her remaining amount is 3, because she was only able to buy 2.
        // Her current amount is also updated to 3.
        // Alice's sell order becomes exhausted.
        let donna_receipt = matching_engine
            .process(Order::new(10, 5, Side::Buy, String::from("Donna")))
            .unwrap();
        assert_eq!(4, donna_receipt.ordinal);
        assert_eq!(
            vec![PartialOrder {
                price: 10,
                current_amount: 2,
                side: Side::Sell,
                signer: "Alice".to_string(),
                ordinal: 1,
                remaining_amount: 0,
            }],
            donna_receipt.matches,
        );

        assert_eq!(0, matching_engine.asks.len());
        assert_eq!(1, matching_engine.bids.len());

        // The matching engine contains a bid at 10.
        assert_eq!(1, matching_engine.bids.get(&10).unwrap().len());
        assert_eq!(
            PartialOrder {
                price: 10,
                current_amount: 3,
                side: Side::Buy,
                signer: String::from("Donna"),
                ordinal: 4,
                remaining_amount: 3,
            },
            matching_engine
                .bids
                .get(&10)
                .unwrap()
                .peek()
                .unwrap()
                .to_owned()
        );

        let emma_receipt = matching_engine
            .process(Order::new(8, 2, Side::Sell, String::from("Emma")))
            .unwrap();
        assert_eq!(5, emma_receipt.ordinal);
        assert_eq!(
            vec![PartialOrder {
                price: 10,
                current_amount: 3,
                side: Side::Buy,
                signer: "Donna".to_string(),
                ordinal: 4,
                remaining_amount: 1,
            }],
            emma_receipt.matches,
        );

        assert_eq!(0, matching_engine.asks.len());
        assert_eq!(1, matching_engine.bids.len());

        // The matching engine contains a bid at 10.
        assert_eq!(1, matching_engine.bids.get(&10).unwrap().len());
        assert_eq!(
            PartialOrder {
                price: 10,
                current_amount: 1,
                side: Side::Buy,
                signer: String::from("Donna"),
                ordinal: 4,
                remaining_amount: 1,
            },
            matching_engine
                .bids
                .get(&10)
                .unwrap()
                .peek()
                .unwrap()
                .to_owned()
        );

        // This exhausts the Donna's buy order.
        let filip_receipt = matching_engine
            .process(Order::new(9, 3, Side::Sell, String::from("Filip")))
            .unwrap();
        assert_eq!(6, filip_receipt.ordinal);
        assert_eq!(
            vec![PartialOrder {
                price: 10,
                current_amount: 1,
                side: Side::Buy,
                signer: "Donna".to_string(),
                ordinal: 4,
                remaining_amount: 0,
            }],
            filip_receipt.matches,
        );

        assert_eq!(1, matching_engine.asks.len());
        assert_eq!(0, matching_engine.bids.len());

        // The matching engine contains an ask at 9.
        assert_eq!(1, matching_engine.asks.get(&9).unwrap().len());
        assert_eq!(
            PartialOrder {
                price: 9,
                current_amount: 2,
                side: Side::Sell,
                signer: String::from("Filip"),
                ordinal: 6,
                remaining_amount: 2,
            },
            matching_engine
                .asks
                .get(&9)
                .unwrap()
                .peek()
                .unwrap()
                .to_owned()
        );

        // This exhausts the Filip's sell order.
        let gina_receipt = matching_engine
            .process(Order::new(9, 2, Side::Buy, String::from("Gina")))
            .unwrap();
        assert_eq!(7, gina_receipt.ordinal);
        assert_eq!(
            vec![PartialOrder {
                price: 9,
                current_amount: 2,
                side: Side::Sell,
                signer: "Filip".to_string(),
                ordinal: 6,
                remaining_amount: 0,
            }],
            gina_receipt.matches,
        );

        assert_eq!(0, matching_engine.asks.len());
        assert_eq!(0, matching_engine.bids.len());

        assert_eq!(
            vec![
                Receipt {
                    ordinal: 1,
                    matches: vec![],
                },
                Receipt {
                    ordinal: 2,
                    matches: vec![PartialOrder {
                        price: 10,
                        current_amount: 8,
                        side: Side::Sell,
                        signer: "Alice".to_string(),
                        ordinal: 1,
                        remaining_amount: 6,
                    }],
                },
                Receipt {
                    ordinal: 3,
                    matches: vec![PartialOrder {
                        price: 10,
                        current_amount: 6,
                        side: Side::Sell,
                        signer: "Alice".to_string(),
                        ordinal: 1,
                        remaining_amount: 2,
                    }]
                },
            ],
            matching_engine.history[..3]
        );

        // We also keep a record of unmatched orders, not only the matched ones.
        assert_eq!(7, matching_engine.history.len());
    }

    /// An encompassing test
    ///
    /// This is perhaps the only test we need, but the previous one should be kept, too.
    /// Smaller tests are better for development as they are simpler.
    /// They test a smaller number of features, but that is fine.
    ///
    /// We add sellers first, then buyers, then sellers again, to test everything symmetrically.
    /// We use different prices and price ranges to achieve all that.
    /// We test that price always takes precedence over the ordinal sequence number.
    /// We include testing against a self-match in both ways.
    #[test]
    fn process_all_combinations() {
        let mut matching_engine = MatchingEngine::new();

        assert_eq!(0, matching_engine.asks.len());
        assert_eq!(0, matching_engine.bids.len());

        let alice_receipt = matching_engine
            .process(Order::new(11, 3, Side::Sell, String::from("Alice")))
            .unwrap();
        assert_eq!(1, alice_receipt.ordinal);
        assert!(alice_receipt.matches.is_empty());

        let charlie_receipt = matching_engine
            .process(Order::new(10, 5, Side::Sell, String::from("Charlie")))
            .unwrap();
        assert_eq!(2, charlie_receipt.ordinal);
        assert!(charlie_receipt.matches.is_empty());

        assert_eq!(2, matching_engine.asks.len());
        assert_eq!(0, matching_engine.bids.len());

        // Exhaust a bid. Price of 12 is higher than either ask.
        // Charlie came later than Alice, but asks for less than Alice, so he should be considered first.
        // This is a case where price takes precedence over ordinal, as it should.
        // Bob is willing to pay 12, but he pays 10 to Charlie, and Bob is done.
        let bob_receipt = matching_engine
            .process(Order::new(12, 2, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(3, bob_receipt.ordinal);
        assert_eq!(
            vec![PartialOrder {
                price: 10,
                current_amount: 5,
                side: Side::Sell,
                signer: String::from("Charlie"),
                ordinal: 2,
                remaining_amount: 3,
            }],
            bob_receipt.matches
        );

        assert_eq!(2, matching_engine.asks.len());
        assert_eq!(0, matching_engine.bids.len());

        // The following complex scenario tests for buying at lower price first,
        // and for lower ordinals taking precedence over higher ordinals at the same price point.
        // We test ordinals' precedence at two price points.
        // Our goal is not to test the data structures that are used in our implementation, because
        // they don't belong to us and we assume that they were properly tested.
        // That is not what we are doing here.
        // What we are doing here is to test that proper data structures were used, and that our
        // implementation of the matching algorithm is correct, consequently.

        let maria_receipt = matching_engine
            .process(Order::new(10, 2, Side::Sell, String::from("Maria")))
            .unwrap();
        assert_eq!(4, maria_receipt.ordinal);
        assert!(maria_receipt.matches.is_empty());

        // A test against self-matching.
        let donna_receipt = matching_engine
            .process(Order::new(10, 2, Side::Sell, String::from("Donna")))
            .unwrap();
        assert_eq!(5, donna_receipt.ordinal);
        assert!(donna_receipt.matches.is_empty());

        let mark_receipt = matching_engine
            .process(Order::new(12, 8, Side::Sell, String::from("Mark")))
            .unwrap();
        assert_eq!(6, mark_receipt.ordinal);
        assert!(mark_receipt.matches.is_empty());

        let dianne_receipt = matching_engine
            .process(Order::new(9, 1, Side::Sell, String::from("Dianne")))
            .unwrap();
        assert_eq!(7, dianne_receipt.ordinal);
        assert!(dianne_receipt.matches.is_empty());

        let maria_receipt = matching_engine
            .process(Order::new(11, 1, Side::Sell, String::from("Maria")))
            .unwrap();
        assert_eq!(8, maria_receipt.ordinal);
        assert!(maria_receipt.matches.is_empty());

        // The four asks are at 9, 10, 11, 12.
        assert_eq!(4, matching_engine.asks.len());
        assert_eq!(0, matching_engine.bids.len());

        // Charlie, Maria and Donna are current sellers at 10.
        assert_eq!(3, matching_engine.asks.get(&10).unwrap().len());

        let sellers_at_10 = vec![
            PartialOrder {
                price: 10,
                current_amount: 3,
                side: Side::Sell,
                signer: String::from("Charlie"),
                ordinal: 2,
                remaining_amount: 3,
            },
            PartialOrder {
                price: 10,
                current_amount: 2,
                side: Side::Sell,
                signer: String::from("Maria"),
                ordinal: 4,
                remaining_amount: 2,
            },
            PartialOrder {
                price: 10,
                current_amount: 2,
                side: Side::Sell,
                signer: String::from("Donna"),
                ordinal: 5,
                remaining_amount: 2,
            },
        ];
        let mut expected_sellers_at_10 = BinaryHeap::from(sellers_at_10);
        let mut exact_sellers_at_10 = matching_engine.asks.get(&10).unwrap().clone();
        for _ in 0..3 {
            let expected_seller = expected_sellers_at_10.pop().unwrap();
            let exact_seller = exact_sellers_at_10.pop().unwrap();
            assert_eq!(expected_seller, exact_seller);
        }

        // Exhaust five asks fully, and one partially.
        // Mark's order won't be fully matched/exhausted, but it will be partially matched/exhausted.
        // A bid price of 12 is >= some asks, so we expect the equal asks to be included, too, as they should,
        // and there is enough quantity on both sides. Mark's price is equal.
        // Donna's bid is exhausted.
        let donna_receipt = matching_engine
            .process(Order::new(12, 12, Side::Buy, String::from("Donna")))
            .unwrap();
        assert_eq!(9, donna_receipt.ordinal);
        assert_eq!(
            vec![
                PartialOrder {
                    price: 9,
                    current_amount: 1,
                    side: Side::Sell,
                    signer: String::from("Dianne"),
                    ordinal: 7,
                    remaining_amount: 0,
                },
                PartialOrder {
                    price: 10,
                    current_amount: 3,
                    side: Side::Sell,
                    signer: String::from("Charlie"),
                    ordinal: 2,
                    remaining_amount: 0,
                },
                PartialOrder {
                    price: 10,
                    current_amount: 2,
                    side: Side::Sell,
                    signer: String::from("Maria"),
                    ordinal: 4,
                    remaining_amount: 0,
                },
                PartialOrder {
                    price: 11,
                    current_amount: 3,
                    side: Side::Sell,
                    signer: String::from("Alice"),
                    ordinal: 1,
                    remaining_amount: 0,
                },
                PartialOrder {
                    price: 11,
                    current_amount: 1,
                    side: Side::Sell,
                    signer: String::from("Maria"),
                    ordinal: 8,
                    remaining_amount: 0,
                },
                PartialOrder {
                    price: 12,
                    current_amount: 8,
                    side: Side::Sell,
                    signer: String::from("Mark"),
                    ordinal: 6,
                    remaining_amount: 6,
                }
            ],
            donna_receipt.matches
        );

        // Donna and Mark are selling at 10 and 12, respectively.
        assert_eq!(2, matching_engine.asks.len());
        // There are no buyers currently.
        assert_eq!(0, matching_engine.bids.len());

        // A bid at 7 is too low to buy, but it goes to book.
        let bob_receipt = matching_engine
            .process(Order::new(7, 2, Side::Buy, String::from("Bob")))
            .unwrap();
        assert_eq!(10, bob_receipt.ordinal);
        assert!(bob_receipt.matches.is_empty());

        // Donna and Mark are selling at 10 and 12, respectively.
        assert_eq!(2, matching_engine.asks.len());
        // Bob is buying at 7.
        assert_eq!(1, matching_engine.bids.len());

        assert_eq!(1, matching_engine.bids.get(&7).unwrap().len());
        assert_eq!(
            PartialOrder {
                price: 7,
                current_amount: 2,
                side: Side::Buy,
                signer: String::from("Bob"),
                ordinal: 10,
                remaining_amount: 2,
            },
            matching_engine
                .bids
                .get(&7)
                .unwrap()
                .peek()
                .unwrap()
                .to_owned()
        );

        // An ask at 18 is too high to sell, but it goes to book.
        let maria_receipt = matching_engine
            .process(Order::new(18, 3, Side::Sell, String::from("Maria")))
            .unwrap();
        assert_eq!(11, maria_receipt.ordinal);
        assert!(maria_receipt.matches.is_empty());

        // Donna, Mark and Maria are selling at 10, 12 and 18, respectively.
        assert_eq!(3, matching_engine.asks.len());
        // Bob is buying at 7.
        assert_eq!(1, matching_engine.bids.len());

        let don_receipt = matching_engine
            .process(Order::new(9, 2, Side::Buy, String::from("Don")))
            .unwrap();
        assert_eq!(12, don_receipt.ordinal);
        assert!(don_receipt.matches.is_empty());

        // Donna, Mark and Maria are selling at 10, 12 and 18, respectively.
        assert_eq!(3, matching_engine.asks.len());
        // Bob and Don are buying at 7 and 9, respectively. This is still not enough to buy.
        assert_eq!(2, matching_engine.bids.len());

        // A test against self-matching.
        let jane_receipt = matching_engine
            .process(Order::new(8, 2, Side::Buy, String::from("Jane")))
            .unwrap();
        assert_eq!(13, jane_receipt.ordinal);
        assert!(jane_receipt.matches.is_empty());

        // Donna, Mark and Maria are selling at 10, 12 and 18, respectively.
        assert_eq!(3, matching_engine.asks.len());
        // Bob, Don and Jane are buying at 7, 9 and 8, respectively. This is not enough to buy.
        assert_eq!(3, matching_engine.bids.len());

        // Consider Don first at 9 even though he came after Bob, because Bob buys at 7.
        // Don's order will be fully exhausted.
        // Bob's order will be partially exhausted, though, as Jane is willing to go as low as 7.
        // Only Jane, of all four sellers, is willing to go below 10, which is what Bob and Don are offering.
        // Self-matches are skipped.
        // This Jane's selling order will be fully matched/exhausted.
        let jane_receipt = matching_engine
            .process(Order::new(7, 3, Side::Sell, String::from("Jane")))
            .unwrap();
        assert_eq!(14, jane_receipt.ordinal);
        assert_eq!(
            vec![
                PartialOrder {
                    price: 9,
                    current_amount: 2,
                    side: Side::Buy,
                    signer: String::from("Don"),
                    ordinal: 12,
                    remaining_amount: 0,
                },
                PartialOrder {
                    price: 7,
                    current_amount: 2,
                    side: Side::Buy,
                    signer: String::from("Bob"),
                    ordinal: 10,
                    remaining_amount: 1,
                },
            ],
            jane_receipt.matches
        );

        // Donna, Mark and Maria are selling at 10, 12 and 18, respectively.
        assert_eq!(3, matching_engine.asks.len());
        // Bob is still buying at 7, and Jane at 8.
        assert_eq!(2, matching_engine.bids.len());

        let don_receipt = matching_engine
            .process(Order::new(9, 2, Side::Buy, String::from("Don")))
            .unwrap();
        assert_eq!(15, don_receipt.ordinal);
        assert!(don_receipt.matches.is_empty());

        // Donna, Mark and Maria are selling at 10, 12 and 18, respectively.
        assert_eq!(3, matching_engine.asks.len());
        // Bob, Don and Jane are buying at 7, 9 and 8, respectively. This is not enough to buy.
        assert_eq!(3, matching_engine.bids.len());

        // Consider Don first at 9 even though he came after Bob, because Bob buys at 7.
        // Don's order will be fully exhausted.
        // Bob's order will be fully exhausted, too, as Jane is willing to go as low as 7.
        // Only Jane, of all four sellers, is willing to go below 10, which is what Bob and Don are offering.
        // Self-matches are skipped.
        // This Jane's selling order will be partially matched/exhausted.
        let jane_receipt = matching_engine
            .process(Order::new(7, 5, Side::Sell, String::from("Jane")))
            .unwrap();
        assert_eq!(16, jane_receipt.ordinal);
        assert_eq!(
            vec![
                PartialOrder {
                    price: 9,
                    current_amount: 2,
                    side: Side::Buy,
                    signer: String::from("Don"),
                    ordinal: 15,
                    remaining_amount: 0,
                },
                PartialOrder {
                    price: 7,
                    current_amount: 1,
                    side: Side::Buy,
                    signer: String::from("Bob"),
                    ordinal: 10,
                    remaining_amount: 0,
                },
            ],
            jane_receipt.matches
        );

        // Donna, Mark, Maria and Jane are selling at 10, 12, 18 and 7, respectively.
        assert_eq!(4, matching_engine.asks.len());
        // Jane is still buying at 8.
        assert_eq!(1, matching_engine.bids.len());

        // We also keep a record of unmatched orders, not only the matched ones.
        assert_eq!(16, matching_engine.history.len());
    }
}
