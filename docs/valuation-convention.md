# Valuation Unit Convention

**All valuations are stored as USD CENTS in i128, not dollars.**

## Why Cents?

Avoiding floating-point arithmetic. 1 cent = 1 unit.

## Convention

- USD 100.50 = 10,050 (cents)
- USD 1.99 = 199 (cents)
- USD 0.01 = 1 (cent)

## Conversion

```rust
// To cents: multiply by 100
dollars_to_cents(100.5) = 10_050

// From cents: divide by 100
cents_to_dollars(10_050) = 100.5
```

## In Registry Contract

`register_asset(..., 50000)` stores **$500.00** (50,000 cents)

All queries return cents. Convert on client side if needed.

## Token and Registry Consistency

The asset token and registry store separate valuation fields, and they are
intentionally independent (not kept in sync automatically):

- `registry::register_asset` copies the supplied valuation into the registry
  entry and TVL. After that, only `registry::update_valuation` changes it.
- `asset-token::update_valuation` changes only the token metadata. It does not
  call the registry, so the registry entry and TVL can go stale.

The two `update_valuation` entrypoints are separate admin actions, each with
its own authority. Whoever changes a token's valuation must also call
`registry::update_valuation` for the matching asset id if the registry should
reflect it. Clients and operators must verify both records.

## In Documentation

Every valuation reference must state: **"in USD cents"**
