# Plan 0001 — MVP roadmap

> **⚠️ SUPERSEDED.** There is no MVP. Per [ADR-0009](../adr/0009-end-state-system-no-mvp.md),
> the system is specified and built to its **end state** (all assets, all 8 engines, full
> surface). A forthcoming long-term plan will decompose the end-state into high-level phases,
> with per-phase files of discrete atomic tasks — none of which reduce the product. This file
> is retained only for history and will be replaced by that plan.

**Status:** Superseded by the forthcoming end-state phased plan.

This is a high-level phase plan, not a committed schedule. It exists to order the work and
surface the decisions each phase needs.

## Guiding posture

*Design all 8 engine interfaces now so nothing is architecturally blocked; build the
highest-volume engines deep first.* (Recommended; to be confirmed and recorded in an ADR.)

## Phase 0 — Foundations (docs & decisions) ← current

- [x] Runtime decision (ADR-0001)
- [x] Dependency posture (ADR-0002)
- [x] Capability-based instrument model (ADR-0003)
- [x] Master spec skeleton
- [ ] Confirm MVP engine scope → new ADR
- [ ] Confirm run-queue boundary (suite vs. platform) → new ADR
- [ ] Resolve strategy-authoring form (callbacks vs. graph/DSL) → new ADR

## Phase 1 — Contract spine (Rust)

- [ ] `crates/contracts`: Instrument, capabilities, `MarketEvent` envelope + payload variants
- [ ] Required-data manifest model + validation
- [ ] `crates/core`: clock, deterministic event stream, ids
- [ ] Field-level per-asset data-contract spec under `docs/specs/`
- [ ] Conformance tests + tiny synthetic fixtures

## Phase 2 — First engine end-to-end (Engine A: Order Book)

- [ ] `crates/engines`: order-book matching, market/limit orders, slippage, partial fills, fees
- [ ] `crates/strategy`: `Strategy` trait + `MarketView` + capability-gated accessors
- [ ] `crates/metrics`: universal metrics core
- [ ] `crates/runner`: single-run orchestration
- [ ] One worked example (stocks or CEX crypto) in `examples/`

## Phase 3 — Python boundary

- [ ] `crates/pybind` (PyO3) + `python/btsuite` strategy SDK
- [ ] Arrow zero-copy data hand-off
- [ ] Author + run a strategy from Python end-to-end

## Phase 4 — Breadth + concurrency

- [ ] Engine B (AMM) and/or Engine E (Derivatives) per confirmed scope
- [ ] Run queue (parallel, GIL-free) in `crates/runner`
- [ ] Model Contract + first AI-model adapter (ONNX or `tch`)
- [ ] Benchmarks in `benches/`; track start-to-finish latency

## Phase 5+ — Remaining engines

- [ ] C (NAV), D (Cash Flow), F (Synthetic), G (Marketplace), H (Event) as prioritized
- [ ] Per-asset metric extensions

## Open decisions blocking later phases

See MASTER_SPEC §10.
