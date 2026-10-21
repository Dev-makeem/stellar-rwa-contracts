#![cfg(test)]
//! Edge-case tests for `approve` (zero input, events, paused state, cost).
use super::*;
use compliance::{ComplianceContract, ComplianceContractClient};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events, Ledger},
    Address, Env, IntoVal, String,
};

struct S {
    env: Env,
    token: AssetTokenContractClient<'static>,
    admin: Address,
}

fn setup() -> S {
    let env = Env::default();
    env.mock_all_auths();
    let cid = env.register(ComplianceContract, ());
    let compliance = ComplianceContractClient::new(&env, &cid);
    let admin = Address::generate(&env);
    compliance.initialize(&admin);
    compliance.add_to_allowlist(&admin, &admin, &String::from_str(&env, "US"), &0);
    let tid = env.register(AssetTokenContract, ());
    let token = AssetTokenContractClient::new(&env, &tid);
    token.initialize(
        &admin,
        &String::from_str(&env, "Manhattan Loft"),
        &String::from_str(&env, "MLOFT"),
        &String::from_str(&env, "real_estate"),
        &1_000i128,
        &2u32,
        &cid,
        &String::from_str(&env, "A tokenized NYC loft"),
        &50_000_000i128,
    );
    S { env, token, admin }
}

// ---- #532: zero / minimal input ----

#[test]
fn zero_amount_approve_succeeds_and_reads_zero() {
    let s = setup();
    let spender = Address::generate(&s.env);
    s.token.approve(&s.admin, &spender, &0, &0);
    assert_eq!(s.token.allowance(&s.admin, &spender), 0);
}

#[test]
fn zero_amount_clears_existing_allowance() {
    let s = setup();
    let spender = Address::generate(&s.env);
    let exp = s.env.ledger().sequence() + 100;
    s.token.approve(&s.admin, &spender, &500, &exp);
    assert_eq!(s.token.allowance(&s.admin, &spender), 500);
    s.token.approve(&s.admin, &spender, &0, &exp);
    assert_eq!(s.token.allowance(&s.admin, &spender), 0);
}

#[test]
fn zero_amount_ignores_past_expiration() {
    let s = setup();
    s.env.ledger().with_mut(|l| l.sequence_number = 50);
    let spender = Address::generate(&s.env);
    assert!(s.token.try_approve(&s.admin, &spender, &0, &1).is_ok());
}

#[test]
fn positive_amount_with_past_expiration_rejected() {
    let s = setup();
    s.env.ledger().with_mut(|l| l.sequence_number = 50);
    let spender = Address::generate(&s.env);
    assert!(s.token.try_approve(&s.admin, &spender, &1, &10).is_err());
}

#[test]
fn negative_amount_rejected() {
    let s = setup();
    let spender = Address::generate(&s.env);
    assert!(s.token.try_approve(&s.admin, &spender, &-1, &1_000).is_err());
}
