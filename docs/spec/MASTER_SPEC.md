# Master Specification

**Status:** Living document — sections expand as sub-specs are written.
**Role:** The north-star overview. Every significant detail lives in a linked sub-spec;
this file states the principles once and routes you to the right place.

---

## Table of contents

1. [Purpose and non-goals](#1-purpose-and-non-goals)
2. [Foundational principles](#2-foundational-principles)
3. [System map (end-to-end)](#3-system-map-end-to-end)
4. [Asset taxonomy](#4-asset-taxonomy) → `spec/assets/`
5. [The five contracts](#5-the-five-contracts) → `spec/contracts/`
6. [Execution engines](#6-execution-engines) → `spec/engines/`
7. [Run queue and execution model](#7-run-queue-and-execution-model) → `spec/runner.md`
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
  │    ─ Strategy implementation (satisfying Strategy Contract)
  │    ─ [optional] AI model     (satisfying Model Contract)
  │
  ▼
┌─────────────────────────────────────────────────────────┐
│  backtesting_suite                                      │
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
│    ├─ Strategy (on_event callback)                      │
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

The suite covers eleven asset classes in the MVP. Each is defined in full in its own spec —
covering: what the asset is, what a backtest of it requires, the full data contract, which
engine handles it, and the implications for system design.

| # | Asset class | Sub-types | Engine | Spec |
|---|---|---|---|---|
| 1 | **Equities** | Stocks, REITs, ADRs, tokenized stocks | A | [assets/equities.md](assets/equities.md) |
| 2 | **ETFs & Funds** | ETFs, ETNs, inverse/leveraged, mutual funds | A + C | [assets/etfs.md](assets/etfs.md) |
| 3 | **Crypto Spot (CEX)** | BTC, ETH, altcoins on centralized exchanges | A | [assets/crypto-spot-cex.md](assets/crypto-spot-cex.md) |
| 4 | **DEX / AMM** | Uniswap v2/v3, Raydium, Curve, stablecoin pools | B | [assets/dex-amm.md](assets/dex-amm.md) |
| 5 | **Futures (expiring)** | Equity, commodity, energy, crypto, rate futures | A | [assets/futures.md](assets/futures.md) |
| 6 | **Perpetuals** | Linear, inverse, BTC/ETH perps | A | [assets/perpetuals.md](assets/perpetuals.md) |
| 7 | **Options** | Equity, ETF, index, crypto options; warrants | E | [assets/options.md](assets/options.md) |
| 8 | **Bonds & Fixed Income** | Treasuries, corporate, municipal, MBS, CDs | D | [assets/bonds.md](assets/bonds.md) |
| 9 | **FX** | Major, minor, exotic pairs | A | [assets/fx.md](assets/fx.md) |
| 10 | **NFTs** | ERC-721, SPL NFTs, collections | G | [assets/nfts.md](assets/nfts.md) |
| 11 | **Prediction Markets** | Binary events, Polymarket-style | H | [assets/prediction-markets.md](assets/prediction-markets.md) |

Assets taxonomy overview: [assets/README.md](assets/README.md)

---

## 5. The five contracts

The system's public interface. Detailed field-level specifications live in `spec/contracts/`.

| Contract | Role | Spec |
|---|---|---|
| **Instrument** | Identity, venue, `price_formation` (engine selector), capabilities, metadata | [contracts/instrument.md](contracts/instrument.md) |
| **Market Data** | Universal envelope + typed payload variants | [contracts/market-data.md](contracts/market-data.md) |
| **Strategy** | Universal `on_event` interface; capability-gated accessors | [contracts/strategy.md](contracts/strategy.md) |
| **Model** | AI/ML inference interface; look-ahead safety | [contracts/model.md](contracts/model.md) |
| **Result / Metrics** | Universal metrics + per-capability extensions | [contracts/metrics.md](contracts/metrics.md) |

Contracts overview: [contracts/README.md](contracts/README.md)

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
| **A** | Order Book | `CLOB` | [engines/engine-a-order-book.md](engines/engine-a-order-book.md) |
| **B** | AMM | `AMM` | [engines/engine-b-amm.md](engines/engine-b-amm.md) |
| **C** | NAV | `NAV` | [engines/engine-c-nav.md](engines/engine-c-nav.md) |
| **D** | Cash Flow | `DEALER` | [engines/engine-d-cashflow.md](engines/engine-d-cashflow.md) |
| **E** | Derivatives | `CHAIN` | [engines/engine-e-derivatives.md](engines/engine-e-derivatives.md) |
| **F** | Synthetic | `OTC` | [engines/engine-f-synthetic.md](engines/engine-f-synthetic.md) |
| **G** | Marketplace | `MARKETPLACE` | [engines/engine-g-marketplace.md](engines/engine-g-marketplace.md) |
| **H** | Event Resolution | `ORACLE` | [engines/engine-h-event-resolution.md](engines/engine-h-event-resolution.md) |

Engines overview and selection rules: [engines/README.md](engines/README.md)

---

## 7. Run queue and execution model

- **Event-driven core.** Deterministic clock replays `MarketEvent`s in `ts_event` order.
- **Look-ahead safety.** A strategy or model may only observe data with `ts_event ≤ current_ts`.
  Enforced by the contract.
- **Vectorized pre-compute + event-driven execution.** Indicators and model features may be
  bulk-computed over the full dataset before replay; the engine then drives fills event-by-event.
  This gives both speed and fill realism.
- **Run queue.** Submitting N backtests (parameter sweeps, multi-asset portfolios) is a
  first-class concern of the suite. Parallelism is GIL-free Rust. The trading platform may
  add higher-level orchestration above it, but the primitive lives here.

Full spec: [runner.md](runner.md) *(TBD)*

---

## 8. Integration boundary

The suite is a **library**: it processes strategies passed in at runtime and **stores
nothing** — not strategies, users, data, or models (see
[ADR-0005](../adr/0005-strategy-not-stored-suite-is-a-library.md)). The trading platform owns
all product state and a live engine that honors the *same* order/execution semantics the suite
simulates, so a strategy behaves identically in backtest and live.

| Concern | Owner |
|---|---|
| Historical market data | **Caller** |
| AI model weights & training **algorithms** (the `Trainer` impl, injected) | **Caller** |
| *When* to retrain + assembling point-in-time training data + pause-train-resume | **Suite** |
| Live async training orchestration + hot-swap | **Caller** |
| Strategy storage, versioning, user selection | **Caller** |
| Live trading / order execution (parity with suite) | **Caller** |
| Job orchestration, UI | **Caller** |
| Strategy JSON format & validation | **Suite** |
| Instrument & market-data contracts | **Suite** |
| Engines, fills, valuation | **Suite** |
| Strategy & model *interfaces* (not their content) | **Suite** |
| Run queue primitive | **Suite** |
| Data manifest validation and error reporting | **Suite** |

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

| # | Decision | Where it blocks |
|---|---|---|
| OD-1 | MVP engine scope: design-all-build-core vs. all-8 vs. A+E only | `plans/0001-mvp-roadmap.md` |
| OD-2 | Run-queue boundary: how much orchestration lives in suite vs. platform | `spec/runner.md` |
| OD-3 | Arrow Tier-A confirmation (or bespoke columnar layout) | `ADR-0002` |
| OD-4 | Derivatives math: build in Rust vs. optional QuantLib plugin | `spec/engines/engine-e-derivatives.md` |
| ~~OD-5~~ | ~~Strategy-authoring form~~ → **Resolved:** single JSON declarative pipeline + component registry; no `on_event` blob | [ADR-0004](../adr/0004-strategy-json-pipeline.md) |
| OD-6 | Multi-strategy portfolios: one run = one strategy, or shared capital pool with portfolio-level netting/risk | `spec/contracts/strategy.md` §15 |
| OD-7 | Component registry trust model: Rust / PyO3 / WASM, and sandboxing for determinism | `spec/contracts/strategy.md` §15 |
| OD-8 | Run Request schema: data bindings, dates, capital, seeds, parameter sweeps (separate from Strategy JSON) | `spec/run-request.md` (TBD) |
| OD-9 | Expression-vs-component boundary: how much logic is allowed in JSON expressions | `spec/contracts/strategy.md` §15 |
| OD-10 | Model registry & reproducibility: stable `model_id@version` resolution over time | `spec/contracts/model.md` §8 |
| OD-11 | Training repo topology: extract standalone `*-contracts` package vs. depend on this repo's `crates/contracts` | [ADR-0007](../adr/0007-shared-training-pipeline-port.md) |
| OD-12 | Validation gating of freshly trained artifacts (reject-and-keep-incumbent policy) | `spec/contracts/training.md` §8 |

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
| **Trainer (port)** | The injected interface the suite calls to (re)train a model; implemented by the shared training package, never by the suite |
| **Model artifact** | The versioned output of a training run (a registry handle); the suite stores no weights |
| **Refit cache** | Cache keyed by `(base_version, method, data_window, params, seed)` so identical training is done once across refits and sweeps |
| **Hot-swap** | Replacing the active model version with a newly trained one (atomic at sim time in backtest; async in live) |
| **Pause-train-resume** | Backtest mechanic: freeze the sim clock at a refit point, train on PIT data, swap the model, resume |
| **Parity** | The guarantee that a strategy behaves identically in backtest and live, differing only in data feed |
| **Clean price** | Bond quoted price excluding accrued coupon interest |
| **Dirty price** | Bond actual purchase price = clean price + accrued interest |
| **Roll yield** | Gain or loss from rolling an expiring futures contract; positive in backwardation, negative in contango |
| **Floor price** | The lowest listed ask price in an NFT collection |
| **Engine** | A self-contained price-formation simulation module selected by `price_formation` |
