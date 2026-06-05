# Engine B: AMM

**Status:** 🔲 Deferred — authored during its implementation phase. Architectural frame is fixed.

**`price_formation`:** `AMM` · **Routes here:** DEX pools (Uniswap v2/v3, Raydium, Curve,
stablecoin pools).

## What this spec will define
- Pool-state pricing: CPMM (x·y=k) for v2; concentrated-liquidity tick-crossing for v3; Curve
  StableSwap invariant.
- Market swaps only (exact-in / exact-out); price impact; LP fees; gas costs; optional MEV tax.
- The fidelity dial (full tick data vs. constant-active-liquidity approximation).

## Fixed constraints (already decided)
- Implements the `Engine` trait; **rejects all order types except swaps**
  ([engines/README.md](README.md)).
- Gas is a first-class P&L line; account state via injected `Account`
  ([ADR-0010](../../adr/0010-suite-does-not-own-portfolio.md)).
- Deterministic, point-in-time.

## Source asset spec
[dex-amm](../assets/dex-amm.md)
