# Architecture

The current-state map of the system. This file records the architecture that has emerged from the
specs and ADRs — it does not propose one speculatively.

- It is **not** an ADR. ADRs capture point-in-time decisions and their rationale.
- It is **not** a spec. Specs define individual components and contracts.
- It **is** the single answer to "what does the system look like right now, and how do the pieces fit?"

Update this file whenever an ADR changes the shape of the system or a new component spec is approved.

---

## 1. Overview

A standalone, headless backtesting engine, structured as a **Rust core** (the per-event hot loop and
all engines) exposed to **Python** at the authoring/orchestration edges (ADR-0001). The caller
supplies instruments, market data, and a strategy/plan as JSON; a **Contract Validator** rejects
under-specified runs; a **Run Queue** schedules one or many runs; each **Single Run** replays a
deterministic event stream through the **engine selected by the instrument's `price_formation`
field**; a **Metrics Collector** emits per-trade records and aggregate metrics. The engine owns no
data, no models, and no portfolio — those are injected or caller-owned (ADR-0005, ADR-0010). The
shared contracts are a dependency-free kernel external systems depend on (ADR-0012).

## 2. Components

| Component | Responsibility | Spec |
|-----------|----------------|------|
| System overview | Master map linking all specs | [SYS-001](./specs/SYS-001-trading-simulator-overview.md) |
| Contract Validator | Validate instruments + per-`(instrument, engine)` data manifest; reject under-specified runs | [DATA-003](./specs/DATA-003-instrument-contract.md), [DATA-004](./specs/DATA-004-market-data-contract.md) |
| Runner & Run Queue | Schedule single/many concurrent runs; parallelism, determinism, streaming | [COMP-002](./specs/COMP-002-runner-and-run-queue.md) |
| Component Registry | Built-in / native / WASM components strategies wire together; trust tiers | [COMP-001](./specs/COMP-001-component-registry.md) |
| Engine A — Order Book (CLOB) | Equities, ETFs, CEX crypto, futures, perps, FX, listed-option execution | [COMP-003](./specs/COMP-003-engine-a-order-book.md) |
| Engine B — AMM | DEX pools (CPMM, concentrated liquidity, stable pools) | [COMP-004](./specs/COMP-004-engine-b-amm.md) |
| Engine C — NAV | Mutual-fund execution; ETF valuation layer | [COMP-005](./specs/COMP-005-engine-c-nav.md) |
| Engine D — Cash Flow | Bonds and fixed income | [COMP-006](./specs/COMP-006-engine-d-cashflow.md) |
| Engine E — Derivatives | Options, warrants, model-priced derivatives | [COMP-007](./specs/COMP-007-engine-e-derivatives.md) |
| Engine F — Synthetic | CFDs, swaps, structured notes | [COMP-008](./specs/COMP-008-engine-f-synthetic.md) |
| Engine G — Marketplace | NFTs and discrete-transaction assets | [COMP-009](./specs/COMP-009-engine-g-marketplace.md) |
| Engine H — Event Resolution | Prediction markets, binary event contracts | [COMP-010](./specs/COMP-010-engine-h-event-resolution.md) |
| Metrics Collector | Per-trade `TradeRecord` stream + universal & capability-gated metrics | [DATA-008](./specs/DATA-008-result-metrics-contract.md) |

## 3. Data Flow

```
Caller (trading platform or any conforming tool)
  │  provides: Instrument defs · Market data · [optional] Signals · [optional] Cohort sources
  │            Strategy / Plan JSON · [optional] AI model (port) · Account (port)
  ▼
Contract Validator  ──reject──► precise contract error if under-specified
  │ (validates instruments + data manifest per (instrument, engine))
  ▼
Run Queue (COMP-002)  ── schedules single or N concurrent runs ──►
  ▼
Single Run
  ├─ Deterministic clock (ts_event ordering, ns UTC)
  ├─ Event stream replay (MarketEvent)
  ├─ Engine selected by instrument.price_formation  (A…H)
  ├─ Strategy / Plan (declarative pipeline; MarketView capability-gated accessors)
  └─ [optional] Model port (inference-only, ts_available look-ahead enforced)
  ▼
Metrics Collector  ──► Result (TradeRecord stream + aggregate & per-capability metrics)
  ▼
returned to caller
```

The routing decision happens in exactly one place: an instrument's `price_formation` field selects
the engine. Capability flags then gate which payload variants and order types are valid — there is
no `match asset_type` branching in engine logic.

## 4. External Dependencies

| Dependency | Used for | Justified by |
|------------|----------|--------------|
| Apache Arrow | Zero-copy data across the Rust↔Python boundary | [ADR-0001](./adr/0001-runtime-rust-python-hybrid.md) |
| PyO3 / maturin | Python bindings over the Rust core | [ADR-0001](./adr/0001-runtime-rust-python-hybrid.md) |
| Injected `Model` port (caller-supplied) | AI/ML inference (no model ownership) | [ADR-0006](./adr/0006-model-inference-and-training.md), [INTG-002](./specs/INTG-002-ai-model-inference-port.md) |
| Injected `Trainer` port (caller-supplied) | Opt-in point-in-time (re)training | [ADR-0007](./adr/0007-shared-training-pipeline-port.md), [INTG-003](./specs/INTG-003-training-port.md) |
| Injected `Account` port (caller-supplied) | Portfolio/ledger queries (no portfolio ownership) | [ADR-0010](./adr/0010-simulator-does-not-own-portfolio.md), [INTG-001](./specs/INTG-001-account-ledger-port.md) |

> Deliberately minimal: anything defining a trade, price, payoff, or metric is built in-house
> ([ADR-0002](./adr/0002-minimal-external-dependencies.md)).

## 5. Key Decisions

A map into [`adr/`](./adr/), newest first:

- [ADR-0012](./adr/0012-standalone-contracts-kernel.md) — standalone, dependency-free contracts kernel
- [ADR-0011](./adr/0011-component-registry-trust-model.md) — tiered component trust (built-in / native / WASM)
- [ADR-0010](./adr/0010-simulator-does-not-own-portfolio.md) — injected `Account`; simulator owns no portfolio
- [ADR-0009](./adr/0009-end-state-system-no-mvp.md) — build the end-state system, no MVP subset
- [ADR-0008](./adr/0008-training-scope-method-visibility-retention.md) — training scope/visibility/retention
- [ADR-0007](./adr/0007-shared-training-pipeline-port.md) — training via an injected port
- [ADR-0006](./adr/0006-model-inference-and-training.md) — model inference + opt-in training
- [ADR-0005](./adr/0005-strategy-not-stored-simulator-is-a-library.md) — simulator is a library; stores nothing
- [ADR-0004](./adr/0004-strategy-json-pipeline.md) — single declarative JSON strategy pipeline
- [ADR-0003](./adr/0003-capability-based-instrument-model.md) — capability-based instrument model
- [ADR-0002](./adr/0002-minimal-external-dependencies.md) — minimal external dependencies
- [ADR-0001](./adr/0001-runtime-rust-python-hybrid.md) — Rust/Python hybrid runtime

## 6. Known Constraints & Boundaries

- **No live execution, brokerage, or UI** — simulate-and-return only (artifact non-goals).
- **No data, model, or portfolio ownership** — all injected or caller-owned (ADR-0005, ADR-0006, ADR-0010).
- **Determinism is mandatory** — bit-identical results regardless of thread count; decimal money types,
  scoped RNG, documented tie-break ordering (artifact SC-4, FM-3).
- **No look-ahead** — `ts_available` / `knowable_ts` enforced for signals and reference data (artifact FM-4).
- **End-state scope** — all eleven asset classes, no MVP subset (ADR-0009).
