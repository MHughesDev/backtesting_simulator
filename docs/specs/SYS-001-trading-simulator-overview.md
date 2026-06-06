# Spec: SYS-001 — Trading Simulator Overview

**Spec ID:** SYS-001
**Type:** System-overview (master specification routing to individual specs)
**Status:** Approved
**Date:** 2026-06-06
**Author:** Agent

**Role:** The north-star overview. Every significant detail lives in a linked sub-spec;
this file states the principles once and routes you to the right place.


---

## Table of contents

1. [Purpose and non-goals](#1-purpose-and-non-goals)
2. [Foundational principles](#2-foundational-principles)
3. [System map (end-to-end)](#3-system-map-end-to-end)
4. [Asset taxonomy](#4-asset-taxonomy) → `specs/assets/`
5. [The contracts](#5-the-contracts) → `specs/contracts/`
6. [Execution engines](#6-execution-engines) → `specs/engines/`
7. [Run queue and execution model](#7-run-queue-and-execution-model) → `specs/runner.md`
8. [Integration boundary](#8-integration-boundary)
9. [Performance approach](#9-performance-approach)
10. [Open decisions](#10-open-decisions)
11. [Glossary](#11-glossary)

---

## 1. Purpose and non-goals

A **standalone, headless backtesting engine** for simulating trading strategies against
historical data. It is the processing core a trading platform (or any conforming caller)
utilizes — not itself a platform, broker, data vendor, or UI.

### Goals

1. **Universal** — one contract spine spanning every tradable asset class.
2. **Realistic** — event-driven, path-dependent simulation (order matching, AMM pricing,
   funding/liquidation, derivatives valuation) not just vectorized signal replay.
3. **Fast** — minimize start-to-finish run latency; first-class run queue for many concurrent
   backtests. See [ADR-0001](../adr/0001-runtime-rust-python-hybrid.md).
4. **Embeddable** — exposed to Python so a trading platform can drive it.
5. **Honest** — reject under-specified runs with precise errors; never silently
   produce plausible-but-wrong P&L.

### Non-goals

- No live order execution, brokerage, or UI.
- **No data ownership.** The caller supplies data satisfying our contracts.
- **No model ownership.** AI models are a called dependency; never trained or stored here.

---

## 2. Foundational principles

| Principle | Statement |
|---|---|
| **Asset ≠ Engine** | Asset *category* does not determine simulation. *Price formation* does. |
| **Venue + Contract → Engine** | The `price_formation` field on an instrument selects the engine — the only place where the routing decision is made. |
| **Capabilities, not type switches** | Instruments advertise capability flags. Engines and strategies react to capabilities. No `match asset_type` anywhere in engine logic. |
| **Thin required spine, opt-in payloads** | Every event shares a small required envelope; payloads are typed variants, gated by capability. |
| **Required is relative** | "Required" data is declared per `(instrument, engine)` by a **data manifest** — not globally. |
| **Provide-or-derive** | Some values (greeks, continuous price) may be supplied by caller or computed by the engine if absent. |
| **Own the contracts** | Anything defining a trade, price, payoff, or metric is built in-house. See [ADR-0002](../adr/0002-minimal-external-dependencies.md). |
| **Fail loudly on under-specification** | A run that does not satisfy its data manifest produces a contract error, not garbage results. |

---

## 3. System map (end-to-end)

This is the full call chain from a caller submitting a backtest to receiving results:

```
Caller (trading platform or any tool)
  │
  │  provides:
  │    ─ Instrument definitions  (satisfying Instrument Contract)
  │    ─ Historical market data  (satisfying Market Data Contract per instrument)
  │    ─ [optional] Exogenous signals (news/social/macro/media — Signals Contract, ts_available)
  │    ─ [optional] Cohort sources (market-wide feeds that materialize instruments dynamically)
  │    ─ Strategy or Plan        (satisfying Strategy / Plan Contract)
  │    ─ [optional] AI model     (satisfying Model Contract)
  │
  ▼
┌─────────────────────────────────────────────────────────┐
│  trading_simulator                                  │
│                                                         │
│  Contract Validator                                     │
│    ├─ Validates instrument definitions                  │
│    └─ Validates data manifest per (instrument, engine)  │
│         → rejects with precise error if under-specified │
│                                                         │
│  Run Queue (crates/runner)                              │
│    └─ Schedules single or many concurrent runs          │
│                                                         │
│  Single Run                                             │
│    ├─ Deterministic clock (ts_event ordering, ns UTC)   │
│    ├─ Event stream (MarketEvent replay)                 │
│    ├─ Engine (selected by price_formation)              │
│    │    ├─ A  Order Book                                │
│    │    ├─ B  AMM                                       │
│    │    ├─ C  NAV                                       │
│    │    ├─ D  Cash Flow                                 │
│    │    ├─ E  Derivatives                               │
│    │    ├─ F  Synthetic                                 │
│    │    ├─ G  Marketplace                               │
│    │    └─ H  Event Resolution                          │
│    ├─ Strategy / Plan (declarative pipeline; no callback)│
│    │    └─ MarketView (capability-gated accessors)      │
│    └─ [optional] Model (inference-only, no look-ahead)  │
│                                                         │
│  Metrics Collector                                      │
│    ├─ Universal metrics (returns, Sharpe, drawdown, …)  │
│    └─ Per-capability extensions (funding P&L, greeks    │
│         attribution, slippage/gas, Brier score, …)      │
└─────────────────────────────────────────────────────────┘
  │
  ▼
Result (satisfying Result/Metrics Contract)
  └─ returned to caller
```

**Conceptual hierarchy** from economics down to risk:

```
Economic Primitive  →  Venue Mechanics  →  Execution Engine
      →  Contract Type  →  Valuation Model  →  Instrument
            →  Strategy  →  Portfolio  →  Risk  →  AI Agent
```

---

## 4. Asset taxonomy

The simulator covers eleven asset classes (the full end-state set — there is no MVP subset, see
[ADR-0009](../adr/0009-end-state-system-no-mvp.md)). Each is defined in full in its own spec —
covering: what the asset is, what a backtest of it requires, the full data contract, which
engine handles it, and the implications for system design.

| # | Asset class | Sub-types | Engine | Spec |
|---|---|---|---|---|
| 1 | **Equities** | Stocks, REITs, ADRs, tokenized stocks | A | [assets/equities.md](DATA-009-equities-asset-spec.md) |
| 2 | **ETFs & Funds** | ETFs, ETNs, inverse/leveraged, mutual funds | A + C | [assets/etfs.md](DATA-010-etfs-and-funds-asset-spec.md) |
| 3 | **Crypto Spot (CEX)** | BTC, ETH, altcoins on centralized exchanges | A | [assets/crypto-spot-cex.md](DATA-011-crypto-spot-cex-asset-spec.md) |
| 4 | **DEX / AMM** | Uniswap v2/v3, Raydium, Curve, stablecoin pools | B | [assets/dex-amm.md](DATA-012-dex-amm-asset-spec.md) |
| 5 | **Futures (expiring)** | Equity, commodity, energy, crypto, rate futures | A | [assets/futures.md](DATA-013-futures-asset-spec.md) |
| 6 | **Perpetuals** | Linear, inverse, BTC/ETH perps | A | [assets/perpetuals.md](DATA-014-perpetuals-asset-spec.md) |
| 7 | **Options** | Equity, ETF, index, crypto options; warrants | E | [assets/options.md](DATA-015-options-asset-spec.md) |
| 8 | **Bonds & Fixed Income** | Treasuries, corporate, municipal, MBS, CDs | D | [assets/bonds.md](DATA-016-bonds-fixed-income-asset-spec.md) |
| 9 | **FX** | Major, minor, exotic pairs | A | [assets/fx.md](DATA-017-fx-asset-spec.md) |
| 10 | **NFTs** | ERC-721, SPL NFTs, collections | G | [assets/nfts.md](DATA-018-nfts-asset-spec.md) |
| 11 | **Prediction Markets** | Binary events, Polymarket-style | H | [assets/prediction-markets.md](DATA-019-prediction-markets-asset-spec.md) |

Assets taxonomy overview: [assets/README.md](README.md)

---

## 5. The contracts

The system's public interface. The caller satisfies the input contracts; the simulator supplies
outputs. Detailed specifications live in `specs/contracts/` and the top-level specs
[run-request.md](DATA-002-run-request.md), [component-registry.md](COMP-001-component-registry.md),
[DATA_TAXONOMY.md](DATA-001-data-taxonomy.md), and [ENGINE_DEEP_DIVE.md](../reference/ENGINE_DEEP_DIVE.md).

| Contract | Role | Spec | Status |
|---|---|---|---|
| **Instrument** | Identity, venue, `price_formation` (engine selector), capabilities, metadata | [contracts/instrument.md](DATA-003-instrument-contract.md) | ✅ Defined |
| **Market Data** | Universal envelope + typed, capability-gated payload variants (Market-Data Plane) | [contracts/market-data.md](DATA-004-market-data-contract.md) | ✅ Defined |
| **Signals** | Exogenous-Signal Plane: news/social/macro/media; `ts_available` look-ahead; multi-source binding; multimodal `MediaReference` model bundles | [contracts/signals.md](DATA-005-signals-contract.md) | ✅ Defined |
| **Strategy** | Single JSON declarative pipeline (universe→features→models→alpha→sizing→risk→execution); cross-instrument references (`data:`, `signal:`, `feature:`); scanner universe for cohorts | [contracts/strategy.md](DATA-006-strategy-contract.md) | ✅ Defined |
| **Plan** | Multi-strategy composition: screen→entry→exit by data-flow, or concurrent independents; `account_mode` (shared/isolated), `conflict_policy` (net/priority/reject) | [contracts/plan.md](DATA-007-plan-contract.md) | ✅ Defined |
| **Model** (port) | AI/ML inference interface; look-ahead safety; injected; multimodal context bundles from signals | [contracts/model.md](INTG-002-ai-model-inference-port.md) | ✅ Defined |
| **Training** (`Trainer` port) | Opt-in PIT (re)training; pause-train-resume; walk-forward + refit cache; injected | [contracts/training.md](INTG-003-training-port.md) | ✅ Defined |
| **Account** (port) | Caller-owned ledger the simulator queries; **no assumed portfolio** | [run-request.md](DATA-002-run-request.md) §5 | ✅ Defined |
| **Run Request** | Per-invocation binding of data (event streams + reference data), time, parameters, ports, output; data manifest validation | [run-request.md](DATA-002-run-request.md) | ✅ Defined |
| **Component Registry** | Built-in / native / WASM components strategies wire together; trust tiers (Rust / trusted native / WASM sandbox) | [component-registry.md](COMP-001-component-registry.md) | ✅ Defined |
| **Result / Metrics** | Per-trade `TradeRecord` stream; aggregate metrics (returns, Sharpe, drawdown, greeks attribution, etc.); injected `Account` for ledger | [contracts/metrics.md](DATA-008-result-metrics-contract.md) | 🔲 Deferred |

Data is organized into **three planes** — the Market-Data Plane (what engines fill against), the
Exogenous-Signal Plane (news/social/macro/media that inform decisions but never set a fill price,
governed by a `ts_available` clock), and the Operational/Meta Plane (records the simulator emits). The
complete venue-neutral data model is in [DATA_TAXONOMY.md](DATA-001-data-taxonomy.md).

**Event streams vs. reference data:** Bindings carry a `binding_type` field. Event streams replay
through the clock (Market Data, Signals, corporate actions); reference data is loaded once and
queried point-in-time by `effective_ts` + `knowable_ts` (e.g., credit spreads, credit ratings,
roll schedules, exchange calendars). See [run-request.md](DATA-002-run-request.md) §4.

Strategies run in two **topologies**, both on the same Run Request and the same engines:

1. **Single / multi-asset** — trade one or a few named instruments, optionally analyzing
   **watch-only** reference instruments (e.g. trade ETH while a model forecasts BTC). The traded
   set (`universe`) is a subset of the run's `instruments`; cross-instrument references
   read the watch-only ones via `data:<instrument>.field` (market data), `signal:<instrument>.<id>`
   (signals), or `feature:<id>@<instrument>` (computed features). [contracts/strategy.md](DATA-006-strategy-contract.md) §4.
2. **Universe-wide scan (cohort)** — survey a large, membership-changing universe (DEX pairs on a
   chain, NFT collections, IPOs, prediction markets) via a `scanner` universe over a
   **cohort data source** that materializes instruments point-in-time. A `selector` strategy screens
   candidates by market-data + signal filters (engine-agnostic); one or more `entry` strategies time
   the actual trades — composed in a **Plan** ([contracts/plan.md](DATA-007-plan-contract.md)) by data-flow
   (screen→entry→exit), never nested. Multi-strategy Plans support `account_mode` (shared/isolated
   capital) and `conflict_policy` (net/priority/reject for opposing intents on one asset).

Contracts overview: [contracts/README.md](README.md). Full spec index & readiness:
[spec/README.md](README.md).

The **Instrument Contract** is the router. Its `price_formation` field is the single
decision point for engine selection. Capability flags determine which payload variants are
valid and which order types are permitted. This is how the system avoids type-switches
scattered through engine logic. See [ADR-0003](../adr/0003-capability-based-instrument-model.md).

---

## 6. Execution engines

Eight engines, each owning one distinct price-formation mechanic. Selection rule:
**if price formation changes, build a new engine; if it stays the same, extend the existing one.**
Engines compose via capability flags (e.g. ETF = Engine A execution + Engine C valuation).

| Engine | Name | `price_formation` value | Spec |
|---|---|---|---|
| **A** | Order Book | `CLOB` | [engines/engine-a-order-book.md](COMP-003-engine-a-order-book.md) |
| **B** | AMM | `AMM` | [engines/engine-b-amm.md](COMP-004-engine-b-amm.md) |
| **C** | NAV | `NAV` | [engines/engine-c-nav.md](COMP-005-engine-c-nav.md) |
| **D** | Cash Flow | `DEALER` | [engines/engine-d-cashflow.md](COMP-006-engine-d-cashflow.md) |
| **E** | Derivatives | `CHAIN` | [engines/engine-e-derivatives.md](COMP-007-engine-e-derivatives.md) |
| **F** | Synthetic | `OTC` | [engines/engine-f-synthetic.md](COMP-008-engine-f-synthetic.md) |
| **G** | Marketplace | `MARKETPLACE` | [engines/engine-g-marketplace.md](COMP-009-engine-g-marketplace.md) |
| **H** | Event Resolution | `ORACLE` | [engines/engine-h-event-resolution.md](COMP-010-engine-h-event-resolution.md) |

Engines overview and selection rules: [engines/README.md](README.md)

---

## 7. Run queue and execution model

- **Event-driven core.** Deterministic clock replays `MarketEvent`s in `ts_event` order.
- **Look-ahead safety.** A strategy or model may only observe data with `ts_event ≤ current_ts`.
  Enforced by the contract.
- **Vectorized pre-compute + event-driven execution.** Indicators and model features may be
  bulk-computed over the full dataset before replay; the engine then drives fills event-by-event.
  This gives both speed and fill realism.
- **Run queue.** Submitting N backtests (parameter sweeps, multi-asset portfolios) is a
  first-class concern of the simulator. Parallelism is GIL-free Rust. The trading platform may
  add higher-level orchestration above it, but the primitive lives here.

Full spec: [run-request.md](DATA-002-run-request.md) (the per-invocation document) and `runner.md` *(TBD)*

---

## 8. Integration boundary

The simulator is a **library**: it processes strategies passed in at runtime and **stores
nothing** — not strategies, users, data, or models (see
[ADR-0005](../adr/0005-strategy-not-stored-simulator-is-a-library.md)). The trading platform owns
all product state and a live engine that honors the *same* order/execution semantics the simulator
simulates, so a strategy behaves identically in backtest and live.

| Concern | Owner |
|---|---|
| Historical market data | **Caller** |
| AI model weights & training **algorithms** (the `Trainer` impl, injected) | **Caller** |
| *When* to retrain + assembling point-in-time training data + pause-train-resume | **Simulator** |
| Any non-backtest (e.g. live) training orchestration | **Caller** (out of scope here) |
| Training method *implementation* (named by id in the strategy, resolved by the injected `Trainer`) | **Caller** |
| Strategy storage, versioning, user selection | **Caller** |
| Portfolio / cash-position ledger / accounting (injected `Account` port) | **Caller** |
| Portfolio-level metrics aggregation | **Caller / optional analytics layer** |
| Live trading / order execution (parity with simulator) | **Caller** |
| Job orchestration, UI | **Caller** |
| Strategy JSON format & validation | **Simulator** |
| Instrument & market-data contracts | **Simulator** |
| Engines, fills, valuation | **Simulator** |
| Strategy & model *interfaces* (not their content) | **Simulator** |
| Run queue primitive | **Simulator** |
| Data manifest validation and error reporting | **Simulator** |

---

## 9. Performance approach

- Rust core for the per-event hot loop; Python only at authoring and orchestration edges.
  See [ADR-0001](../adr/0001-runtime-rust-python-hybrid.md).
- Apache Arrow crosses the Rust↔Python boundary zero-copy.
- Parallel backtest queue via work-stealing; engines are `Send + Sync`.
- Benchmarks tracked in `benches/`; start-to-finish latency and hot-loop throughput are
  tracked product metrics, not aspirations.

---

## 10. Open decisions

The table below is the **blocking** subset. The full design backlog — blocking and
exploratory — lives in [`docs/open-questions.md`](../open-questions.md).

**Recent resolutions (2026-06):** The AA (market-data depth/derivation) and BB (multi-asset/scanning)
batches resolved 16 new payload types, 11 capability flags, cross-instrument references, scanner
universes, cohort data sources, the Plan multi-strategy layer, signal `ts_available` look-ahead,
and `MediaReference` multimodal model bundles. These are fully specified in [DATA_TAXONOMY.md](DATA-001-data-taxonomy.md),
[contracts/market-data.md](DATA-004-market-data-contract.md), [contracts/instrument.md](DATA-003-instrument-contract.md),
[contracts/strategy.md](DATA-006-strategy-contract.md), [contracts/signals.md](DATA-005-signals-contract.md),
[contracts/plan.md](DATA-007-plan-contract.md), and [run-request.md](DATA-002-run-request.md).

| # | Decision | Where it blocks | Status |
|---|---|---|---|
| ~~OD-1~~ | ~~MVP engine scope~~ | [ADR-0009](../adr/0009-end-state-system-no-mvp.md) | ✅ Resolved |
| OD-2 | Run-queue boundary: how much orchestration lives in simulator vs. platform | `specs/runner.md` | ⏳ Deferred to phase planning |
| ~~OD-8~~ | ~~Run Request schema~~ | [run-request.md](DATA-002-run-request.md) | ✅ Resolved |
| ~~Q-ACCT-1~~ | ~~Does the simulator own the portfolio ledger?~~ | [ADR-0010](../adr/0010-simulator-does-not-own-portfolio.md) | ✅ Resolved |
| OD-3 | Arrow Tier-A confirmation (or bespoke columnar layout) | Perf/implementation | ⏳ Deferred to build phase |
| OD-4 | Derivatives math: build in Rust vs. optional QuantLib plugin | [engine-e-derivatives.md](COMP-007-engine-e-derivatives.md) | ⏳ Deferred to build phase |
| ~~OD-5~~ | ~~Strategy-authoring form~~ | [ADR-0004](../adr/0004-strategy-json-pipeline.md) | ✅ Resolved |
| ~~OD-6~~ | ~~Multi-strategy portfolios~~ | [contracts/plan.md](DATA-007-plan-contract.md) | ✅ Resolved (BB-5) |
| ~~OD-7~~ | ~~Component registry trust model~~ | [ADR-0011](../adr/0011-component-registry-trust-model.md) | ✅ Resolved |
| OD-9 | Expression-vs-component boundary: how much logic is allowed in JSON expressions | [component-registry.md](COMP-001-component-registry.md) §9 | ⏳ Deferred to authoring phase |
| OD-10 | Model registry & reproducibility: stable `model_id@version` resolution over time | [contracts/model.md](INTG-002-ai-model-inference-port.md) §8 | ⏳ Deferred to model phase |
| ~~OD-11~~ | ~~Shared-contracts repo topology~~ | [ADR-0012](../adr/0012-standalone-contracts-kernel.md) | ✅ Resolved |
| OD-12 | Validation gating of freshly trained artifacts (reject-and-keep-incumbent policy) | [contracts/training.md](INTG-003-training-port.md) §8 | ⏳ Deferred to training phase |

---

## 11. Glossary

| Term | Definition |
|---|---|
| **CLOB** | Central Limit Order Book — price formation via resting bids and offers |
| **AMM** | Automated Market Maker — price formation via an algorithmic invariant (e.g. x·y=k) |
| **NAV** | Net Asset Value — fund price determined by the value of underlying holdings |
| **Capability** | A flag on an Instrument enabling specific payload types, order types, and engine features |
| **Price formation** | How an instrument's price is determined; the single field that selects the engine |
| **Data manifest** | The declared minimum (and optional) data for a given `(instrument, engine)` pairing |
| **Provide-or-derive** | A value the caller may supply, or the engine computes if absent |
| **Look-ahead** | The bug of a strategy observing data dated after the current event time |
| **Mark price** | A manipulation-resistant reference price used for liquidation calculation (perps, derivatives) |
| **Continuous contract** | A synthetic time-series constructed from rolling expiring futures contracts |
| **IV surface** | A 2D grid of implied volatility by strike and expiry at a given point in time |
| **Brier score** | Calibration metric for probabilistic forecasts; primary metric for prediction-market strategies |
| **Insight** | An alpha-stage output: direction + confidence per instrument |
| **Component registry** | The store of named, typed, reusable code components (indicators, alpha/sizing functions, selectors) that strategy JSON references by ID |
| **Run Request** | The per-invocation document (separate from Strategy JSON) binding data, dates, capital, seeds, and parameter values/sweeps |
| **Walk-forward** | Refitting a model on a rolling point-in-time window so no future data leaks into training |
| **Trainer (port)** | The injected interface the simulator calls to (re)train a model; implemented by the shared training package, never by the simulator |
| **Model artifact** | The versioned output of a training run (a registry handle); the simulator stores no weights |
| **Refit cache** | Cache keyed by `(base_version, method, data_window, params, seed)` so identical training is done once across refits and sweeps |
| **Hot-swap** | Replacing the active model version with a newly trained one (atomic at sim time in backtest; async in live) |
| **Pause-train-resume** | Backtest mechanic: freeze the sim clock at a refit point, train on PIT data, swap the model, resume |
| **Parity** | The guarantee that a strategy behaves identically in backtest and live, differing only in data feed |
| **Per-trade model** | The simulator simulates each trade decision and emits a TradeRecord; it owns no portfolio |
| **TradeRecord** | The simulator's per-decision output: trade setup + sizing decision + execution information |
| **Account (port)** | Injected, caller-owned ledger the simulator queries for equity/positions/collateral and reports fills to |
| **Component** | A named, reusable building block (indicator, sizer, selector, …) that fills one pipeline slot; strategies wire components, they don't contain logic |
| **Component registry** | The set of available components, in tiers: built-in (Rust), trusted native, and sandboxed WASM |
| **WASM sandbox** | A sealed WebAssembly runtime that runs untrusted/AI components with no I/O, clock, or network — enforcing purity and look-ahead safety by construction |
| **Shared kernel (contracts)** | The standalone, dependency-free package of cross-boundary types/ports that the simulator, training package, and live platform all depend on |
| **Clean price** | Bond quoted price excluding accrued coupon interest |
| **Dirty price** | Bond actual purchase price = clean price + accrued interest |
| **Roll yield** | Gain or loss from rolling an expiring futures contract; positive in backwardation, negative in contango |
| **Floor price** | The lowest listed ask price in an NFT collection |
| **Engine** | A self-contained price-formation simulation module selected by `price_formation` |
| **`ts_event`** | When a thing actually happened; the canonical clock for ordering and Market-Data Plane look-ahead |
| **`ts_available`** | When a strategy could *first have known* a datum; the look-ahead clock for the Exogenous-Signal Plane (handles publication lag — e.g. a merger announced before it is effective) |
| **`ts_recv`** | When the system received an event; used for latency modeling; `≥ ts_event` |
| **Market-Data Plane** | Prices/books/pool-state/funding the engine simulates fills against; governed by `ts_event`; the only data that can set a fill price |
| **Exogenous-Signal Plane** | External information (news, social, fundamentals, macro, on-chain analytics, media) that informs decisions but **never** sets a fill price; governed by `ts_available` |
| **Operational / Meta Plane** | Records the simulator emits about its own run (lineage, warnings, decisions, audit) |
| **Venue-neutral** | The simulator recognizes generic normalized payloads, never vendor feed dialects; caller-owned **adapters** translate vendor feeds into the contracts |
| **Signal** | A pre-computed exogenous value (`SignalEvent`/`EntityMetric`) referenced by the strategy via `signal:<id>` |
| **MediaReference** | A point-in-time *pointer* (URI + modality) to raw text/image/video; the simulator never decodes it — the injected `Model` port loads it for multimodal inference |
| **ContextBundle** | The PIT collection of exogenous items assembled per inference call from a model node's `context_inputs` |
| **DerivedBar** | A bar the engine builds inline from finer data (trade prints, fallback quote-mid); flagged `derived` with `source_class`; callers never supply it |
| **Reference data** | A binding loaded once and *queried by timestamp* (not replayed as clock events) — e.g. `UniverseMembership`, roll schedules, calendars; carries `effective_ts` + `knowable_ts` |
| **Cohort** | A large, membership-changing universe bound as a single market-wide source that **materializes instruments point-in-time** as they appear (DEX pairs, NFT collections, new listings) |
| **Scanner universe** | A `universe.type` that screens a cohort by point-in-time market-data + signal filters; engine-agnostic; selects candidates, does not by itself decide entries |
| **Watch-only instrument** | An instrument in the run's `instruments` (data bound) but not in the traded `universe`; read via cross-instrument references for analysis only |
| **Cross-instrument reference** | A strategy reference qualified with another instrument's id (`data:<instrument>.field`) — trade one asset while analyzing another |
| **Plan** | The layer above Strategy: multiple flat strategies composed by data-flow (screen→entry→exit) or run concurrently; never nested |
| **Strategy role** | A strategy's job in a Plan: `selector` (emits candidates), `entry`, `exit`, or `standalone` |
| **`account_mode`** | Plan setting: `shared` (strategies net into one injected `Account`) or `isolated` (per-strategy partition) |
| **ADL (auto-deleveraging)** | A perp-venue event that force-closes profitable, high-leverage positions when the insurance fund is exhausted; can reach the strategy's own position |
| **Event stream** | A time-series binding replayed through the clock in `ts_event` order (market data, signals, corporate actions); distinguished from reference data |
| **Reference data** | A binding loaded once and queried point-in-time by `effective_ts` + `knowable_ts` (e.g., credit spreads, exchange calendars, roll schedules); never replayed as events |
| **DerivedBar** | A bar engine constructs inline from finer data (trade prints or fallback quote-mid); flagged `derived: true` + `source_class`; caller never supplies it |
| **MediaReference** | A point-in-time pointer (URI + modality: text/image/video) to raw media; simulator never decodes it — injected `Model` port loads it for multimodal inference |
| **ContextBundle** | The point-in-time collection of exogenous items assembled per inference call from a model node's `context_inputs` (signals, media references, cross-instrument data) |
| **Cohort** | A large, membership-changing universe bound as a single market-wide data source that materializes instruments point-in-time as they appear (DEX pairs, NFT collections, new listings) |
| **Scanner** | A `universe.type` that screens a cohort by point-in-time market-data + signal filters; engine-agnostic; selects candidates, does not by itself decide entries |
| **Plan** | The layer above Strategy: multiple flat strategies composed by data-flow (screen→entry→exit) or run concurrently; never nested; enables multi-strategy capital allocation and conflict resolution |
| **Conflict policy** | Plan setting for resolving opposing intents on one asset: `net` (default; combine), `priority` (first wins), `reject` (both rejected) |
| **Binding type** | A classification on data sources: `event_stream` (replayed) or `reference` (point-in-time queried) |
| **Derived data** | Any payload the engine computes if absent from caller input (adjusted bars, continuous futures, implied vol surfaces); flagged with `derived: true` + `source_class` |
| **Derived-data provenance** | Metadata (`derived: true`, `source_class`) on computed payloads; emitted via `output.emit: ["derived_data"]` so caller may persist it for audit |
| **`effective_ts`** | When a reference-data entry becomes valid; used for point-in-time lookups (e.g., effective date of a credit-spread change) |
| **`knowable_ts`** | When a reference-data entry could first have been known; enforces look-ahead safety on reference data (e.g., announcement date of an event, before effective date) |
