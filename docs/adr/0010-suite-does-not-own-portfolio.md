# ADR-0010: The suite does not own a portfolio — per-trade model + injected `Account`

- **Status:** Accepted
- **Date:** 2026-06-04
- **Deciders:** Project owner
- **Informed by:** [spec/run-request.md](../specs/DATA-002-run-request.md); resolves OD/Q-ACCT-1;
  extends [ADR-0005](0005-strategy-not-stored-suite-is-a-library.md)

## Context

The suite is a library that stores nothing. An open question remained: does it own the
**portfolio/cash/position ledger** during a run? Owning a portfolio would bake in an opinionated
accounting model (single currency, fixed capital, particular settlement and margin rules) and
duplicate state the trading platform already owns. The owner has decided the suite should
**not assume a portfolio** — it should produce, for each trade decision, only the **sizing and
trade setup plus execution information**.

But several mechanics legitimately need account state: account-relative sizing
(`fixed_fractional`, `volatility_target`), account-relative risk (`max_drawdown`,
`max_position_pct`), and margin/liquidation for perps and options.

## Decision

The suite is a **per-trade execution simulator**. For each strategy decision it emits a
**TradeRecord** — the trade setup, the sizing decision (with provenance), and the simulated
execution (fill price/qty, fees, slippage, partials, rejections). It owns **no** cash/position
ledger and **no** equity curve.

Account state is supplied through an **injected `Account` port** owned by the caller. The suite
**queries** it (equity, buying power, positions, collateral) for sizing/risk/margin inputs and
**reports** simulated fills back to it; it never defines the accounting internals. The port is
required only when a strategy actually needs account state; pure absolute-sizing strategies need
no `Account`. A simple reference `Account` adapter ships as an *optional* caller-side
convenience, never as a core assumption.

Portfolio aggregation and metrics (Sharpe, drawdown, equity curve) are derived **downstream**
from the TradeRecord stream + the injected account, not by the core.

## Alternatives considered

- **Suite owns a built-in portfolio/accounting model** — turnkey, but bakes in opinionated
  accounting, duplicates the platform, and contradicts the library remit. Rejected.
- **No account access at all (pure absolute sizing)** — simplest, but breaks `fixed_fractional`,
  volatility targeting, drawdown risk, and margin/liquidation. Rejected.
- **Inject the ledger (chosen)** — keeps mechanics in the suite and the ledger in the caller;
  consistent with the `Model`/`Trainer` injection pattern.

## Consequences

- **Positive:** the suite assumes nothing about accounting; one mechanism (injected ports) for
  all caller-owned state; backtest/live parity (the live engine uses the same account model);
  the suite's output is exactly "sizing + trade setup + execution info" per trade.
- **Negative / accepted tradeoffs:** account-relative strategies require a caller-supplied
  `Account`; portfolio metrics are a downstream/optional layer rather than core; the injected
  account must itself be deterministic to preserve reproducibility.
- **Follow-ups:** define the `Account` port precisely; scope the optional reference adapter;
  specify how metrics consume the TradeRecord stream ([contracts/metrics.md](../specs/DATA-008-result-metrics-contract.md), TBD).
