# Spec: COMP-010 — Engine H: Event Resolution

**Spec ID:** COMP-010
**Type:** Component (execution engine)
**Status:** Approved
**Date:** 2026-06-06
**Author:** Agent

**`price_formation`:** `ORACLE`
**Routes here:** prediction markets, binary event contracts.

Engine H handles contracts whose value is settled by a **real-world outcome** reported by an
oracle. The tradable price is a **probability** in [0, 1]; at resolution the contract pays $1
(YES) or $0 (NO). The defining mechanic is **resolution**, not a calendar expiry.

---

## 1. The `Engine` trait

```rust
impl Engine for EventResolutionEngine {
    fn on_event(&mut self, ev: &MarketEvent, ctx: &mut EngineContext);  // lifecycle, fills, resolution
    fn submit_order(&mut self, o: Order, ctx: &mut EngineContext) -> OrderResult;
    fn settle(&mut self, ctx: &mut EngineContext);                       // pay out on resolution
    fn supports_order_type(&self, t: OrderType) -> bool;  // market (per the order-type matrix)
}
```

Cash is in the injected `Account` ([ADR-0010](../adr/0010-suite-does-not-own-portfolio.md)).
Per the order-type matrix, Engine H supports market orders; resting **limit** orders on the
YES/NO book are a documented extension (real venues like Polymarket run a CLOB) — see §10.

---

## 2. Market lifecycle (state machine)

```
Created → Active (trading) → Locked (event passed, awaiting oracle) → Resolved → Settled
```

The engine enforces the lifecycle: **fills are rejected once a market is `Locked`.** Trading only
occurs in `Active`; payout only after `Resolved`.

Lifecycle transitions are driven by `MarketLifecycleEvent` payloads (see [contracts/market-data.md](DATA-004-market-data-contract.md)
§2.24). Without a `MarketLifecycleEvent` stream, the engine cannot know when a market transitions
to `Locked` and will incorrectly allow fills up to the `Resolution` event — a `DataSufficiencyError`
with `degraded_fidelity` is emitted when this stream is absent, not a hard rejection, because
some datasets only have the resolution event. However, `MarketLifecycleEvent` is strongly
recommended for realistic lifecycle gating.

**Oracle events:** `OracleEvent` payloads carry proposal, dispute, and final settlement detail
used by the oracle risk model when `HasOracleRisk` is set. If `HasOracleRisk` is set but no
`OracleEvent` stream is provided, the engine falls back to the `dispute_occurred` field on the
`Resolution` payload.

**Realistic event ordering.** The expected sequence is:
`MarketLifecycleEvent: Locked` (event occurred, trading stops) → `OracleEvent: Proposal` (proposed
outcome) → an optional dispute window in which `OracleEvent: Dispute` may arrive →
`OracleEvent: FinalSettlement` → `Resolution` (binary payout). Two hard rules:

- **A `Dispute` never re-opens trading.** Once `Locked`, a market never returns to `Active`; a
  dispute only delays finality (and extends the oracle-risk window). Fills stay rejected throughout.
- **`Resolution` may arrive with no preceding `OracleEvent`.** Simple/centralized oracles (e.g.
  Kalshi) report a single outcome directly. When `OracleEvent`s *are* present, they gate the
  dispute/latency/oracle-risk modeling; when absent, resolution is taken at face value.

---

## 3. Probability-bounded trading

YES and NO tokens trade between $0.01 and $0.99 (tick = $0.01); `price(YES) + price(NO) = $1`.
The price **is** the market-implied probability. Fills follow order-book matching on the token
(reuse of the Engine A matching primitives), but bounded to [0, 1] and lifecycle-gated. No trade
can occur at exactly $0.00 or $1.00 before resolution.

---

## 4. Resolution & binary payoff

On a `Resolution` event (outcome + oracle):

```
if outcome == YES:  YES holders receive $1/contract,  NO holders receive $0
if outcome == NO :  NO  holders receive $1/contract,  YES holders receive $0
```

Open positions are closed at the binary payoff via the `Account`. Until resolution, unrealized
P&L is simply `position · current_price − cost_basis` (no model mark beyond the traded price).

---

## 5. Resolution timing uncertainty

Unlike a futures expiry, the resolution **date is not fixed** — it occurs when the event resolves
(and after any oracle dispute window). The engine waits for the `Resolution` event to close
positions; `resolution_deadline` bounds the horizon but actual timing is data-driven.

**Liquidity near resolution:** prediction market order books become materially illiquid as the
lock date approaches — volume drops, spreads widen, and the book becomes one-sided. The fill
model's accuracy degrades for strategies holding positions through the final days before locking.
This is an irreducible data-quality limitation, not an engine deficiency; results should surface
the `days_held_before_lock` distribution so analysts can assess exposure to this window.

---

## 6. Oracle risk

Resolution can be **disputed, delayed, or (rarely) wrong** — a non-market risk that cannot be
backtested away. On a dispute event the engine flags affected open positions as carrying oracle
risk, and can optionally model a probability of incorrect resolution (configurable, off by
default). This is surfaced in results rather than hidden.

---

## 7. Calibration metrics support

For prediction-market strategies, **calibration matters more than raw P&L**. The engine records
the strategy's **entry probability** (= entry price) at each trade so metrics can compute the
**Brier score** `mean((p − outcome)²)`, log score, and accuracy
([assets/prediction-markets.md](DATA-019-prediction-markets-asset-spec.md) §5).

---

## 8. Output

A `TradeRecord` per fill: `setup` (YES/NO, size, price), `sizing`, `execution` (fill price =
implied probability, fees, lifecycle state), `trigger`. The resolution payout is emitted as an
auxiliary record tagged `resolution` with the outcome and entry-probability for Brier scoring.

---

## 9. Determinism & ordering

- Lifecycle transitions and the `Resolution` event apply in `ts_event` order before matching.
- No resolution outcome dated after the decision is visible (look-ahead safety) — a strategy can
  never "see" the outcome before it is reported.
- Optional incorrect-resolution modeling draws from the run seed.

---

## 10. Extensions / open items

**CLOB extension (first-class for liquid markets):** Polymarket runs a full CLOB for major
markets (US elections, Fed meetings) on Polygon; Kalshi is an SEC-regulated exchange with
standard CLOB mechanics. Resting limit orders on YES/NO outcome tokens are *the normal trading
mechanic* for liquid prediction markets — not a fringe extension. The `HasClob` capability flag
(see [contracts/instrument.md](DATA-003-instrument-contract.md)) promotes limit order support in
Engine H: when `HasClob` is set, the order-type matrix for Engine H includes `limit` orders
and Engine A's matching primitives are used for YES/NO token order book fills. When `HasClob`
is not set, Engine H operates in market-order-only mode. This is the highest-priority open item
for this engine.

- Default oracle-dispute / incorrect-resolution model and its parameters.
- Multi-outcome (non-binary) markets — categorical resolution as an extension.
