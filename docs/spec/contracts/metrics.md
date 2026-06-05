# Contract Spec: Result / Metrics

**Status:** 🔲 Deferred — to be authored during its implementation phase. The architectural
frame below is fixed; this file will be expanded into the full spec.

## Purpose

Defines what a run returns and how performance/risk metrics are computed. Per
[ADR-0010](../../adr/0010-suite-does-not-own-portfolio.md), the suite emits a per-trade
**`TradeRecord` stream** (and per-event marks); aggregate portfolio metrics are computed
**downstream** over those records plus the injected `Account` — not assumed by the core.

## What this spec will define

- The **universal metric set**: returns, volatility, Sharpe/Sortino, max drawdown, exposure,
  turnover, hit rate, fees paid, fill quality (slippage vs. arrival).
- **Per-capability extensions**: funding P&L (perps), price-impact/gas (AMM), greeks attribution
  & theta (options), yield/duration/convexity (bonds), tracking error (funds), floor/illiquidity
  (NFTs), Brier score (prediction markets).
- The **result schema** (equity curve, trade log, position history, model lineage, warnings).
- Streaming vs. batch result delivery.
- How metrics are selected by instrument capabilities.

## Fixed constraints (already decided)

- Computed over the `TradeRecord` stream defined in [run-request.md](../run-request.md) §7.
- The suite owns no portfolio; aggregate metrics are an optional analytics layer over the
  injected `Account`.
- Deterministic and reproducible.

## Inputs

- [run-request.md](../run-request.md) (TradeRecord shape), per-asset metric extensions in
  [assets/](assets/), [ADR-0010](../../adr/0010-suite-does-not-own-portfolio.md).
