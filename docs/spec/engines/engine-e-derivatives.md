# Engine E: Derivatives

**Status:** 🔲 Deferred — authored during its implementation phase. Architectural frame is fixed.

**`price_formation`:** `CHAIN` · **Routes here:** options, warrants; derivative valuation for
futures/perps.

## What this spec will define
- Option pricing from an IV surface + underlying; greeks (provide-or-derive); the baseline model
  (Black-Scholes) and how callers swap models (OD-4).
- Early-exercise optimality (American) and assignment; expiry settlement (cash/physical).
- Margin and liquidation for derivative positions.

## Fixed constraints (already decided)
- Implements the `Engine` trait; supports market/limit/exercise order types
  ([engines/README.md](README.md)).
- A volatility input is a hard data-manifest requirement ([assets/options.md](../assets/options.md)).
- Account/margin state via injected `Account` ([ADR-0010](../../adr/0010-suite-does-not-own-portfolio.md)).
- Deterministic, point-in-time.

## Open item
- OD-4: build derivatives math in Rust vs. optional QuantLib plugin behind our trait.

## Source asset spec
[options](../assets/options.md)
