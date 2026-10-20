//! Fuzz target: asset-token `transfer_batch` arithmetic and gating (issue #493).
//!
//! Mirrors the control flow of `AssetToken::transfer_batch` in
//! `contracts/asset-token/src/lib.rs` over a plain in-memory ledger, so the
//! summed-amount arithmetic and per-recipient compliance checks can be fuzzed
//! without the Soroban host (same approach as `dividend_arithmetic.rs`).
//!
//! Inputs are random batches that deliberately include edge-case amounts
//! (0, 1, -1, i128::MIN, i128::MAX, arbitrary i128), repeated recipients,
//! self-transfers, non-compliant recipients and a paused token.
//!
//! Invariants checked:
//! - Conservation: on success, the sum of all balances is unchanged.
//! - No negative balances on success.
//! - Every amount in a successful batch is strictly positive.
//! - Every recipient in a successful batch is compliant, and the token was
//!   not paused.
//! - A batch that is all self-transfers leaves every balance unchanged.
//!
//! # How to run
//!
//! ```bash
//! cd fuzz
//! cargo run --bin asset_token_transfer_batch
//! # Optionally pass a seed to reproduce a run:
//! cargo run --bin asset_token_transfer_batch -- 12345
//! ```

use arbitrary::{Arbitrary, Unstructured};

const ACCOUNTS: usize = 8;
const MAX_ENTRIES: usize = 16;
const ITERATIONS: usize = 100_000;
/// Keeps the sum of all balances within i128 so the supply invariant itself
/// cannot overflow.
const BALANCE_CAP: u128 = i128::MAX as u128 / ACCOUNTS as u128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Error {
    Paused,
    SenderNotCompliant,
    RecipientNotCompliant,
    InvalidAmount,
    InsufficientBalance,
    Overflow,
}

/// Amount generator biased toward the boundary values that matter for
/// `check_amount` and the balance checks.
#[derive(Debug, Arbitrary)]
enum EdgeAmount {
    Zero,
    One,
    MinusOne,
    Min,
    Max,
    Raw(i128),
}

impl EdgeAmount {
    fn value(&self) -> i128 {
        match self {
            EdgeAmount::Zero => 0,
            EdgeAmount::One => 1,
            EdgeAmount::MinusOne => -1,
            EdgeAmount::Min => i128::MIN,
            EdgeAmount::Max => i128::MAX,
            EdgeAmount::Raw(v) => *v,
        }
    }
}

/// One `(recipient, amount)` pair. `to` is reduced modulo `ACCOUNTS`, so
/// repeated recipients and self-transfers occur frequently.
#[derive(Debug, Arbitrary)]
struct Entry {
    to: u8,
    amount: EdgeAmount,
}

#[derive(Debug, Arbitrary)]
struct Input {
    paused: bool,
    compliant: [bool; ACCOUNTS],
    balances: [u128; ACCOUNTS],
    from: u8,
    transfers: Vec<Entry>,
}

#[derive(Clone)]
struct Ledger {
    balances: [i128; ACCOUNTS],
    compliant: [bool; ACCOUNTS],
    paused: bool,
}

/// Mirrors `transfer_batch`: the pause and sender gates run once, each entry
/// is checked in order, and a self-transfer is a no-op. A failing entry
/// returns `Err` and no ledger, which models the contract's revert.
fn transfer_batch(
    ledger: &Ledger,
    from: usize,
    transfers: &[(usize, i128)],
) -> Result<Ledger, Error> {
    if ledger.paused {
        return Err(Error::Paused);
    }
    if !ledger.compliant[from] {
        return Err(Error::SenderNotCompliant);
    }
    let mut next = ledger.clone();
    for &(to, amount) in transfers {
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        if !ledger.compliant[to] {
            return Err(Error::RecipientNotCompliant);
        }
        let from_bal = next.balances[from];
        if from_bal < amount {
            return Err(Error::InsufficientBalance);
        }
        if from == to {
            continue;
        }
        let new_to = next.balances[to]
            .checked_add(amount)
            .ok_or(Error::Overflow)?;
        next.balances[from] = from_bal - amount;
        next.balances[to] = new_to;
    }
    Ok(next)
}

fn check_one(input: &Input) {
    let from = input.from as usize % ACCOUNTS;
    let ledger = Ledger {
        balances: input.balances.map(|b| (b % BALANCE_CAP) as i128),
        compliant: input.compliant,
        paused: input.paused,
    };
    let transfers: Vec<(usize, i128)> = input
        .transfers
        .iter()
        .take(MAX_ENTRIES)
        .map(|e| (e.to as usize % ACCOUNTS, e.amount.value()))
        .collect();

    let Ok(after) = transfer_batch(&ledger, from, &transfers) else {
        // Rejection is correct behaviour for any invalid entry.
        return;
    };

    assert!(!ledger.paused, "batch succeeded while paused");
    assert!(
        transfers.iter().all(|&(_, amount)| amount > 0),
        "batch succeeded with a non-positive amount"
    );
    assert!(
        transfers.iter().all(|&(to, _)| ledger.compliant[to]),
        "batch succeeded with a non-compliant recipient"
    );
    assert!(
        after.balances.iter().all(|&b| b >= 0),
        "batch produced a negative balance"
    );

    let supply_before: i128 = ledger.balances.iter().sum();
    let supply_after: i128 = after.balances.iter().sum();
    assert_eq!(
        supply_before, supply_after,
        "transfer_batch changed total supply"
    );

    if transfers.iter().all(|&(to, _)| to == from) {
        assert_eq!(
            after.balances, ledger.balances,
            "self-transfer batch changed balances"
        );
    }
}

fn main() {
    let seed: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0x493);
    // xorshift64 needs a non-zero state.
    let mut state = seed.max(1);
    let mut raw = vec![0u8; 2048];
    for _ in 0..ITERATIONS {
        for byte in raw.iter_mut() {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            *byte = state as u8;
        }
        let mut u = Unstructured::new(&raw);
        let Ok(input) = Input::arbitrary(&mut u) else {
            continue;
        };
        check_one(&input);
    }
}
