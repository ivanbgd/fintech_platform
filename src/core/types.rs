use std::cmp::{Ordering, Reverse};

/// A simplified side of a position ([`PartialOrder`]) or of an [`Order`]
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Side {
    /// Want to buy
    Buy,
    /// Want to sell
    Sell,
}

/// An order for a symbol to buy or sell an `amount` of at the given `price`
///
/// The price is the highest price to pay at or the lowest price to sell at,
/// per unit, depending on the side.
///
/// Holds several fields at the moment of entering the order book.
/// Generally, all those fields are constant, and should not be changed.
/// In our implementation, we have made the field `initial_amount` constant in code,
/// but it has a getter, `get_initial_amount`.
///
/// Our implementation lacks the `symbol` field for simplicity.
#[derive(Clone, Eq, PartialEq)]
pub struct Order {
    /// Highest price to pay at or lowest price to sell at, per unit, depending on the side
    pub price: u64,
    /// Initial number of units to trade when the order enters the order book;
    /// it's private because it is constant, but has a getter.
    initial_amount: u64,
    /// The side in the order book (buy or sell)
    pub side: Side,
    /// The signer's account
    pub signer: String,
}

impl Order {
    pub fn new(price: u64, initial_amount: u64, side: Side, signer: String) -> Self {
        Self {
            price,
            initial_amount,
            side,
            signer,
        }
    }

    /// Converts an [`Order`] into a [`PartialOrder`] with the added parameters.
    pub fn into_partial_order(self, ordinal: u64, remaining_amount: u64) -> PartialOrder {
        PartialOrder {
            price: self.price,
            current_amount: self.initial_amount,
            side: self.side,
            signer: self.signer,
            ordinal,
            remaining_amount,
        }
    }

    pub fn get_initial_amount(&self) -> u64 {
        self.initial_amount
    }
}

/// A position represents an unfilled order that is kept in the system for later filling.
///
/// The [`Order`] struct lacks the properties to store any metadata, so we have a `PartialOrder`,
/// which allows us to keep track of the current state of an `Order`
/// (for example, whether a part of the amount has been matched).
///
/// The `current_amount` field represents the current number of units in the order,
/// at the beginning of the processing of the partial order.
/// So, this is an initial amount at the beginning of a single partial order processing, and not
/// necessarily its global initial amount, when it was first introduced in the system
/// (in the order book), i.e., when it was created from an `Order`.
///
/// Each partial order starts with its own initial amount, stored in `current_amount`,
/// and we keep record of all orders that have ever been processed in the system, and
/// this includes all partial orders, too, not only the original orders.
///
/// The `remaining_amount` field is the value that we store at the end of an order processing.
#[derive(Clone, Debug, Eq, Ord, PartialEq)]
pub struct PartialOrder {
    /// Price per unit. This gets stored in the receipt as the best price of a matched order.
    /// So, it may start as one value, and end as another, better, value.
    pub price: u64,
    /// Current number of units in the order, at the beginning of the processing of the partial order.
    /// So, this is an initial amount at the beginning of a single partial order processing, and not
    /// necessarily its global initial amount, when it was first introduced in the system
    /// (in the order book), i.e., when it was created from an `Order`.
    pub current_amount: u64,
    /// Buy or sell side of the book
    pub side: Side,
    /// Signer of the order
    pub signer: String,
    /// The order's unique sequence number
    pub ordinal: u64,
    /// Remaining number of units after potential matches
    pub remaining_amount: u64,
}

impl PartialOrd for PartialOrder {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // A `BinaryHeap` is a max-heap by default, so we have to `Reverse`
        // the comparison to create a min-heap which we need.
        Reverse(self.ordinal).partial_cmp(&Reverse(other.ordinal))
    }
}

impl PartialOrder {
    /// Probably incorrect and not used.
    ///
    /// Splits one [`PartialOrder`] in two by taking a specified `take` amount.
    ///
    /// It modifies the current partial order in place by reducing its remaining amount
    /// by the `take` amount, and then clones it into a new partial order.
    ///
    /// The new partial order's current (initial) amount is then overwritten by the `take` value,
    /// and its price is overwritten by the `price` value, and it is then returned by this function.
    pub fn take_from(current_po: &mut PartialOrder, take: u64, price: u64) -> PartialOrder {
        current_po.remaining_amount -= take;
        let mut new_pos = current_po.clone();
        new_pos.current_amount = take; // I think this is wrong!!! Then docstrings are wrong, too!
        new_pos.price = price; // I think this is wrong!!! Then docstrings are wrong, too!
        new_pos
    }
}

/// A receipt issued to the caller for accepting an [`Order`]
///
/// It contains the **best** price from a matched order.
/// So, it may contain a different price from the original price of an order.
/// It's a receipt, or a bill, meaning it has to contain the price paid, not a price offered.
///
/// For example, somebody is willing to sell at 8, and then a buyer who is willing to pay 10 comes in.
/// The buyer will pay 8, naturally, and this will be the price that gets stored in the receipt.
/// So, even though the buyer's order holds the price of 10, their receipt will hold the updated price of 8.
///
/// Symmetrically, if there is a buyer willing to pay 10, and then comes a seller who is willing to sell at 8,
/// the seller will sell at 10, naturally. Then the receipt will contain 10 as price, instead of the original 8.
///
/// That is how we made our matching engine - this is what makes the most sense.
/// This makes the matching engine symmetrical and consequently the most fair to all participants, to both sides.
///
/// The live project's implementation works in the opposite way than my implementation,
/// but only in case of selling. The buying case works in the same way.
/// But, this means that their implementation is asymmetrical.
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd)]
pub struct Receipt {
    /// Sequence number
    pub ordinal: u64,
    /// Matches that happened immediately
    pub matches: Vec<PartialOrder>,
}
