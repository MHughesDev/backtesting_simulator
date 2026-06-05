# Engine C: NAV

**Status:** 🔲 Deferred — authored during its implementation phase. Architectural frame is fixed.

**`price_formation`:** `NAV` · **Routes here:** mutual funds (execution); attaches to ETFs for
valuation alongside Engine A.

## What this spec will define
- End-of-day NAV pricing; subscription/redemption processing; premium/discount tracking.
- Leveraged/inverse ETF **daily reset** and volatility-decay modeling.
- Composition with Engine A (ETF = A execution + C valuation).

## Fixed constraints (already decided)
- Implements the `Engine` trait; composes via the `HasNAV` capability
  ([engines/README.md](README.md)).
- Account state via injected `Account` ([ADR-0010](../../adr/0010-suite-does-not-own-portfolio.md)).
- Deterministic, point-in-time.

## Source asset spec
[etfs](../assets/etfs.md)
