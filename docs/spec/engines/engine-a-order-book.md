# Engine A: Order Book (CLOB)

**Status:** 🔲 Deferred — authored during its implementation phase. Architectural frame is fixed.

**`price_formation`:** `CLOB` · **Routes here:** equities, ETFs (execution), CEX crypto spot,
futures, perpetuals, FX, listed-option execution.

## What this spec will define
- Central limit order book matching; market/limit/stop/stop-limit orders; time-in-force.
- Partial fills, queue priority, slippage when depth is absent, fees (incl. maker/taker tiers).
- Capability extensions: sessions (equities), funding & liquidation (perps), expiry & roll
  (futures), swap rates (FX), token/corporate events.

## Fixed constraints (already decided)
- Implements the `Engine` trait and the order-type matrix ([engines/README.md](README.md)).
- Reads account/margin state via the injected `Account` port — no assumed portfolio
  ([ADR-0010](../../adr/0010-suite-does-not-own-portfolio.md)).
- Deterministic, point-in-time; capability-gated.

## Source asset specs
[equities](../assets/equities.md) · [etfs](../assets/etfs.md) ·
[crypto-spot-cex](../assets/crypto-spot-cex.md) · [futures](../assets/futures.md) ·
[perpetuals](../assets/perpetuals.md) · [fx](../assets/fx.md)
