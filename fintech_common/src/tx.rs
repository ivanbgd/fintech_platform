use serde::{Deserialize, Serialize};

/// **A transaction type**
///
/// Transactions should be able to rebuild a ledger's state
/// when they are applied in the same sequence to an empty state.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum Tx {
    Deposit { account: String, amount: u64 },
    Withdraw { account: String, amount: u64 },
}
