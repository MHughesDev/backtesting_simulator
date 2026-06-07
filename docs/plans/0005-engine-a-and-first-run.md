# Plan 0005 — Engine A & First End-to-End Run

**Date:** 2026-06-06
**Type:** Formal
**Author:** Agent
**Status:** Draft
**Derivation Status:** Current

## Goal

The system can execute a complete end-to-end backtest for a CLOB instrument (equity or CEX crypto
spot) from a Python-authored `RunRequest` JSON through the Rust engine to a `TradeRecord` stream.
This requires: Engine A (order book) in `crates/engines`, the `Strategy` trait and `MarketView`
in `crates/strategy`, universal metrics in `crates/metrics`, and single-run orchestration plus
`RunRequest` validation in `crates/runner`. At the end of this plan, one worked example in
`examples/` runs successfully with synthetic daily-bar data.

---

## Derived From

- Artifact: [docs/artifact.md](../artifact.md) — SC-1 (universal contract spine), SC-2 (realistic event-driven fills), SC-3 (loud failure on under-specification), SC-4 (deterministic)
- Architecture: [docs/architecture.md](../architecture.md) — §2 (Engine A, Contract Validator, Runner, Metrics Collector), §3 (data flow)
- Specs:
  - [COMP-003](../specs/COMP-003-engine-a-order-book.md) — Engine A full spec
  - [COMP-002](../specs/COMP-002-runner-and-run-queue.md) — Runner & Run Queue
  - [DATA-006](../specs/DATA-006-strategy-contract.md) — Strategy JSON pipeline
  - [DATA-002](../specs/DATA-002-run-request.md) — RunRequest schema + validation order §10
  - [DATA-008](../specs/DATA-008-result-metrics-contract.md) — TradeRecord + universal metrics (deferred; extend skeleton)
  - [DATA-009](../specs/DATA-009-equities-asset-spec.md) — Equities data requirements
  - [DATA-011](../specs/DATA-011-crypto-spot-cex-asset-spec.md) — Crypto spot data requirements
- ADRs:
  - [ADR-0004](../adr/0004-strategy-json-pipeline.md) — declarative JSON pipeline, no on_event callbacks
  - [ADR-0005](../adr/0005-strategy-not-stored-simulator-is-a-library.md) — simulator stores nothing
  - [ADR-0009](../adr/0009-end-state-system-no-mvp.md) — end-state (Engine A handles 6 asset classes via capability flags)

---

## Scope

**In scope:**
- Engine A: order-book fill simulation at all four fidelity levels (L3, L2, L1, Bar)
  - Bar fidelity: fill at next-bar open (not close); pessimistic stop fills
  - L1 fidelity: spread-aware fill at the touch
  - L2 fidelity: walk-the-book slippage from `BookSnapshot` / `BookDelta`
  - L3 fidelity: per-order queue reconstruction from `OrderBookOrderEvent`
  - Market, limit, stop, stop-limit order types; partial fills; fill at next event
  - All Engine A capability extensions declared but not fully exercised until Plan 0007:
    funding (`HasFunding`), liquidation (`HasLiquidation`), roll schedules (`HasRollSchedule`),
    swap rates (`HasSwapRates`), corporate actions (`HasCorporateActions`) — these are wired
    but can return `CapabilityNotActive` if not used in the first example
- `Strategy` trait: `on_market_view(view: &MarketView, account: &dyn Account) -> Vec<Order>`
- `MarketView`: capability-gated read-only accessors over the current bar/tick, signals, features
- Pipeline execution: parse strategy JSON, execute feature computation, apply alpha, sizing, risk
  checks, emit orders (→ DATA-006 pipeline stages)
- `RunRequest` validation: steps 1–7 from DATA-002 §10 (schema, params, instruments,
  data manifest, port presence, capability/order-type, data sufficiency)
- Single-run orchestrator: advance clock, yield events, call strategy, route orders to engine,
  collect fills, emit `TradeRecord` stream (→ COMP-002; DATA-002 §11 invariants)
- Universal metrics: per-trade `TradeRecord` completion; aggregate return series, Sharpe ratio,
  max drawdown, trade count (→ DATA-008 deferred but skeleton extended here)
- Reference `Account` adapter (convenience only): single-currency, simple cash + position ledger
  (→ DATA-002 §5; ADR-0010)
- Synthetic bar fixtures in `fixtures/`: 2 years of daily OHLCV for one equity and one crypto spot
- Worked example in `examples/equity_daily_crossover.rs`: EMA crossover strategy, daily bars,
  equity instrument, runs end-to-end (→ artifact SC-1 through SC-4)

**Out of scope:**
- Parallel run queue — Plan 0008
- Python bindings — Plan 0006
- Engines B–H — Plan 0007
- AI model inference — Plan 0009
- Full metrics contract (DATA-008 is deferred) — Plan 0010

---

## Dependencies

- Plan 0003 (contracts crate) — all 4 milestones complete.
- Plan 0004 (core crate) — all 4 milestones complete.
- DATA-008 (Result/Metrics Contract) is marked deferred; universal metrics here are a subset
  sufficient to validate the example (returns, Sharpe, drawdown).

---

## Risks

- Risk: ENGINE A spec (COMP-003) has open decision OD-4 (derivatives math) — not blocking Engine A
  itself, but the spec is large. → Mitigation: implement only the CLOB core in this plan;
  capability extensions (funding, liquidation) are wired but asserted unimplemented until Plan 0007.
- Risk: Strategy JSON pipeline (DATA-006) is complex (7 stages, named bindings). A full
  implementation is risky for one plan. → Mitigation: implement only the minimum pipeline stages
  needed for the EMA-crossover example (features + alpha + sizing + execution); defer model nodes,
  risk overlays, and plan composition to Plans 0007 and 0009.
- Risk: `DataSufficiencyError` (DATA-002 §10 step 7) requires per-engine minimum logic that
  references all engines. → Mitigation: implement only Engine A's per-engine minimum check here;
  other engines' checks are stubs returning `Ok(())` until their engine is built.

---

## Milestones

| # | Milestone | Source | Success Signal |
|---|-----------|--------|----------------|
| 1 | Engine A bar + L1 fidelity | COMP-003; DATA-009 | Market/limit/stop orders fill correctly at next-bar open on synthetic daily bars |
| 2 | Engine A L2 + L3 fidelity | COMP-003 | Walk-the-book and queue-position fills produce correct slippage on synthetic book snapshots |
| 3 | Strategy trait + MarketView | DATA-006; ADR-0004 | An EMA-crossover strategy JSON parses, compiles, and emits orders when the signal fires |
| 4 | RunRequest validation + single-run orchestrator | DATA-002 §10; COMP-002 | A valid RunRequest runs; an under-specified RunRequest is rejected with a typed error |
| 5 | Universal metrics + worked example | DATA-008; artifact SC-1–SC-4 | `examples/equity_daily_crossover` runs end-to-end and prints Sharpe + drawdown |

---

## Tasks by Milestone

### Milestone 1: Engine A bar + L1 fidelity

- `NOT STARTED` Define `Order` struct: `instrument_id`, `side: Side`, `order_type: OrderType`,
  `qty: Decimal`, `limit_price: Option<Money>`, `stop_price: Option<Money>`,
  `time_in_force: TimeInForce` (→ COMP-003 §order types)
- `NOT STARTED` Define `EngineA` struct implementing the `Engine` trait:
  `process_event(event: &MarketEvent, pending_orders: &[Order]) -> Vec<Fill>` (→ COMP-003)
- `NOT STARTED` Implement `FidelityLevel` detection: examine bound payload classes per instrument,
  select `Bar | L1 | L2 | L3`; emit `FidelityWarning` if lower than optimal (→ COMP-003 §fidelity)
- `NOT STARTED` Implement bar-fidelity fill: market order fills at next-bar `open`; pessimistic
  stop fill at bar `low` (for stops) / `high` (for sell-stops); never at `close`
  (→ COMP-003; DATA-002 §9 `intrabar_fill: pessimistic`; README look-ahead safety)
- `NOT STARTED` Implement L1-fidelity fill: market order fills at best ask (buy) or best bid
  (sell) from `Quote`; spread-aware (→ COMP-003)
- `NOT STARTED` Implement fee application: flat-rate fee from `Instrument.fee_schedule` or
  `execution_defaults`; debit from fill (→ COMP-003; DATA-011 maker/taker)
- `NOT STARTED` Unit-test bar fidelity: buy market order on bar data fills at next bar's open;
  buy limit below open fills on next bar open if open ≤ limit; sell stop fills at bar low
  (→ COMP-003; artifact SC-2)
- `NOT STARTED` Unit-test L1 fidelity: buy at ask; partial fill if qty > available touch size
  (→ COMP-003)

### Milestone 2: Engine A L2 + L3 fidelity

- `NOT STARTED` Implement L2 walk-the-book: consume `BookSnapshot` depth levels greedily;
  compute volume-weighted average fill price; partial fill if order exceeds total available
  depth (→ COMP-003 §L2)
- `NOT STARTED` Implement `BookDelta` application: apply incremental deltas to reconstructed
  book state (→ DATA-004 `BookDelta` spec)
- `NOT STARTED` Implement L3 per-order queue: reconstruct individual order queue from
  `OrderBookOrderEvent` stream; assign queue position; fill when matching orders ahead are
  exhausted (→ COMP-003 §L3)
- `NOT STARTED` Unit-test L2: order splits across two price levels; check VWAP fill price
  (→ COMP-003)
- `NOT STARTED` Unit-test L3: order queued behind existing orders fills only after leading
  orders at the same price are consumed (→ COMP-003)
- `NOT STARTED` Implement `TradingStatus` event handling: reject orders when instrument is
  halted (→ DATA-004 `TradingStatus`; COMP-003)

### Milestone 3: Strategy trait + MarketView

- `NOT STARTED` Define `Strategy` trait:
  `fn on_market_view(&self, view: &MarketView, account: &dyn Account) -> Vec<Order>`
  (→ DATA-006; ADR-0004)
- `NOT STARTED` Define `MarketView` struct with capability-gated read methods:
  `bar(instrument_id, interval) -> Option<&Bar>`, `quote(...) -> Option<&Quote>`,
  `book_snapshot(...) -> Option<&BookSnapshot>`, `signal(id) -> Option<&SignalEvent>`,
  `feature(id) -> Option<f64>` (→ DATA-006 §cross-instrument references; ADR-0003)
- `NOT STARTED` Implement JSON strategy pipeline executor (minimum stages for EMA crossover):
  - Feature stage: EMA computation from bar close series (→ DATA-006 §features)
  - Alpha stage: crossover signal (fast_ema > slow_ema → long; otherwise flat) (→ DATA-006 §alpha)
  - Sizing stage: `fixed_qty` sizing (→ DATA-006 §sizing)
  - Execution stage: emit `Order` (market order) (→ DATA-006 §execution)
- `NOT STARTED` Implement strategy JSON parsing from `DATA-006` schema into the pipeline
  executor (→ DATA-006; ADR-0004 single JSON document)
- `NOT STARTED` Unit-test pipeline: parse the EMA-crossover strategy JSON; verify orders are
  emitted on crossover; verify no order is emitted when signal is flat (→ DATA-006)

### Milestone 4: RunRequest validation + single-run orchestrator

- `NOT STARTED` Implement `RunRequest` JSON parsing: deserialize the full schema from DATA-002
  §3; bind `instruments`, `data`, `time`, `parameters`, `output` (→ DATA-002 §3)
- `NOT STARTED` Implement `ContractValidator::validate(req: &RunRequest) -> Result<ValidatedRun, Vec<ContractError>>`
  executing steps 1–7 from DATA-002 §10:
  1. Schema parse (already done via serde)
  2. Parameter bounds check
  3. Instrument validation (calls `Instrument::validate()`)
  4. Data manifest check per instrument
  5. Port presence check
  6. Capability/order-type check
  7. Data sufficiency check for Engine A
  (→ DATA-002 §10; DATA-003; COMP-003; artifact SC-3)
- `NOT STARTED` Implement `SingleRun` orchestrator:
  1. Construct `SimulationClock` from `time.start`
  2. Merge all bound event streams into one sorted `EventStream`
  3. Loop: advance clock, yield next event, update `MarketView`, call `Strategy::on_market_view`,
     route orders to engine, collect `Fill`s, report fills to `Account`, emit `TradeRecord`
  4. Emit final metrics on run completion
  (→ COMP-002; DATA-002 §11 invariants; SYS-001 §7)
- `NOT STARTED` Implement `WarmupGate` integration: strategy receives no `on_market_view` calls
  until warmup end; feature indicator state accumulates silently during warmup (→ DATA-002 §4a)
- `NOT STARTED` Integration-test: Run with valid RunRequest → produces TradeRecord stream;
  Run with missing `Bar` data → `ManifestViolation`; Run with future `ts_available` signal →
  signal is withheld until `ts_available` reached (→ artifact SC-3; FM-4)

### Milestone 5: Universal metrics + worked example

- `NOT STARTED` Implement `MetricsCollector` that accumulates `TradeRecord`s and computes:
  cumulative return, Sharpe ratio (annualized), max drawdown, total trades, win rate
  (→ DATA-008 skeleton; artifact SC-2)
- `NOT STARTED` Implement reference `Account` adapter: tracks cash balance and positions;
  `apply_fill` debits cash and updates position; `equity()` returns cash + market value
  (→ DATA-002 §5; ADR-0010)
- `NOT STARTED` Create `fixtures/equity_daily.arrow`: 2 years of synthetic AAPL-like daily bars
  (generated deterministically from a fixed seed) (→ Plan 0002 fixtures directory)
- `NOT STARTED` Write `examples/equity_daily_crossover.rs`: constructs an `Instrument` (AAPL-like
  equity), a RunRequest binding `fixtures/equity_daily.arrow`, an EMA-10/30 crossover strategy
  JSON, runs via `SingleRun`, prints `MetricsSummary` (→ artifact SC-1 through SC-4)
- `NOT STARTED` Verify the example compiles and runs: `cargo run --example equity_daily_crossover`
  exits 0 and prints non-empty metrics (→ artifact SC-2; SC-4)
- `NOT STARTED` Add determinism test: run the example twice with the same seed; assert
  `TradeRecord` streams are byte-identical (→ artifact SC-4; FM-3)

---

## Open Questions

- [ ] DATA-008 (Result/Metrics Contract) is formally deferred; the metrics subset here (return,
  Sharpe, drawdown) must not contradict the eventual full spec. Review DATA-008 before releasing
  Milestone 5 to ensure no incompatible choices were made.
- [ ] The Strategy JSON pipeline in DATA-006 has many stages; Plan 0005 only implements the
  minimum. Document which stages are `unimplemented!()` stubs with clear error messages, so callers
  are not silently mis-served.
- [ ] Should `ContractValidator` accumulate all errors and return `Vec<ContractError>`, or fail
  at the first error? DATA-002 §10 says "precise contract error" (singular in some readings).
  Recommend: accumulate all errors per step, return the full list. Confirm before Milestone 4.

---

## Change Log

| Date | Change | Author |
|------|--------|--------|
| 2026-06-06 | Initial draft | Agent |
