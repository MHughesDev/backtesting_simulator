# Plan 0004 — Core Crate

**Date:** 2026-06-06
**Type:** Formal
**Author:** Agent
**Status:** Draft
**Derivation Status:** Current

## Goal

`crates/core` is fully implemented: the deterministic nanosecond-UTC event clock, the ordered
event-stream replay iterator, canonical ID types, and the fixed-point/decimal money type (with
arithmetic that never loses precision on monetary operations). All four components pass unit tests
that prove correctness and the anti-look-ahead invariant (no event at `ts_event = T+n` is visible
when the clock is at `T`). This crate is consumed by every subsequent crate; stabilizing it before
Plan 0005 begins is the key dependency.

---

## Derived From

- Artifact: [docs/artifact.md](../artifact.md) — SC-4 (deterministic & reproducible), FM-3 (non-determinism under parallelism)
- Architecture: [docs/architecture.md](../architecture.md) — §1 overview (deterministic clock, ns UTC), §2 (Single Run: deterministic clock + event stream replay)
- Specs:
  - [SYS-001](../specs/SYS-001-trading-simulator-overview.md) §7 (run queue and execution model: event-driven core, look-ahead safety, vectorized pre-compute)
  - [DATA-002](../specs/DATA-002-run-request.md) §11 (invariants: deterministic, point-in-time)
  - [DATA-001](../specs/DATA-001-data-taxonomy.md) — `ts_event` vs `ts_available` ordering semantics
- ADRs:
  - [ADR-0001](../adr/0001-runtime-rust-python-hybrid.md) — Rust core, GIL-free parallelism (`Send + Sync`)
  - [ADR-0002](../adr/0002-minimal-external-dependencies.md) — no floating-point money; use decimal

---

## Scope

**In scope:**
- `SimulationClock` struct: current simulation timestamp (`Timestamp = u64` nanoseconds UTC),
  advance semantics (only moves forward), `current_ts()` accessor
- `EventStream` iterator: takes a sorted or pre-sorted slice of `MarketEvent`, yields events in
  strictly ascending `ts_event` order, panics if the source is out of order (fail-loud)
- Look-ahead enforcement: `EventStream` never yields an event with `ts_event > clock.current_ts()`
  (enforced by design, not by a runtime check that can be bypassed)
- `RunId`, `InstrumentId` (if not fully owned by `contracts`), `CorrelationId` newtypes
- Scoped RNG wrapper: deterministic `StdRng` seeded per run from `RunRequest.determinism.seed`;
  `Send + Sync` so runs are independent (→ artifact SC-4; FM-3)
- `Money` type: thin wrapper over `rust_decimal::Decimal` with:
  - Arithmetic operators that return `Money` (prevent mixing with raw `Decimal`)
  - `from_currency(amount, currency)` constructor
  - Explicit rounding methods (`round_to_tick`, `round_half_up`)
  - No implicit floating-point promotion
- `TimeRange` struct: `[start: Timestamp, end: Timestamp)` with `contains()`, `warmup` offset
- `WarmupGate`: tracks whether the warmup window has passed; no decisions are emitted while gated
  (→ DATA-002 §4a warmup gating hard rule)

**Out of scope:**
- Bar derivation logic (inline during replay) — that is `EventStream`-adjacent but lives in
  `crates/engines` driven by `crates/runner`; the `core` clock just advances
- Parallel run queue threading — Plan 0008
- Python bindings — Plan 0006

---

## Dependencies

- Plan 0002 (workspace scaffold) must be complete.
- Plan 0003 (contracts crate) Milestone 1 must be done (`Timestamp`, `MarketEvent` envelope
  available); Plans 0003 and 0004 can overlap on this milestone.

---

## Risks

- Risk: Integer overflow on `Timestamp` arithmetic (nanoseconds since epoch on a `u64` is fine
  through ~year 2554, but subtraction can underflow). → Mitigation: Use `checked_sub`/`checked_add`
  everywhere; panic on overflow in debug mode, saturate in release, document the choice.
- Risk: `WarmupGate` interacts subtly with the look-ahead enforcement — if the gate is checked
  after the clock advances past warmup, a strategy could see data during warmup.
  → Mitigation: `WarmupGate` is checked as the outermost guard in `EventStream::next()`, before
  the strategy decision layer is invoked. Unit-test this interaction.
- Risk: `Money` arithmetic accumulates rounding error across many operations even with `Decimal`.
  → Mitigation: Test a sequence of 10,000 trades for cumulative rounding drift; accept only
  zero drift for exact-representable inputs.

---

## Milestones

| # | Milestone | Source | Success Signal |
|---|-----------|--------|----------------|
| 1 | `SimulationClock` + look-ahead enforcement | SYS-001 §7; artifact SC-4 | Clock advances monotonically; unit test proves look-ahead events are withheld |
| 2 | `EventStream` ordered replay | DATA-001; DATA-002 §11 | Sorted and unsorted input both produce correctly ordered output; out-of-order input panics |
| 3 | `Money` type + `WarmupGate` | artifact SC-4; DATA-002 §4a | Money arithmetic is exact for all test cases; WarmupGate gates decisions correctly |
| 4 | IDs + scoped RNG | artifact SC-4; FM-3 | Two `EventStream`s with different seeds produce different stochastic sequences; same seed → same sequence |

---

## Tasks by Milestone

### Milestone 1: SimulationClock + look-ahead enforcement

- `NOT STARTED` Define `Timestamp` as `u64` nanoseconds UTC (if not already in `contracts`,
  re-export from there) with `checked_add`, `checked_sub`, and display formatting
  (→ SYS-001 §11 glossary `ts_event`)
- `NOT STARTED` Implement `SimulationClock` struct with `current_ts: Timestamp`,
  `advance(to: Timestamp)` (must be ≥ current; panics otherwise), `current_ts()` accessor
  (→ SYS-001 §7 deterministic clock)
- `NOT STARTED` Implement `TimeRange` struct with `start`, `end`, `warmup_offset: Option<Duration>`,
  `contains(ts: Timestamp) -> bool`, `warmup_start() -> Timestamp` (→ DATA-002 §3 time block)
- `NOT STARTED` Unit-test `SimulationClock`: advance is monotonic; double-advance to same ts
  is idempotent; advance backwards panics; `current_ts()` returns correct value
  (→ artifact SC-4; FM-3)

### Milestone 2: EventStream ordered replay

- `NOT STARTED` Implement `EventStream` struct wrapping a `Vec<MarketEvent>` sorted by
  `ts_event`; `new(events: Vec<MarketEvent>) -> Result<Self, OutOfOrderError>` validates
  monotonic ordering on construction (→ DATA-001; DATA-002 §11)
- `NOT STARTED` Implement `Iterator for EventStream` yielding `&MarketEvent` only when
  `event.ts_event <= clock.current_ts()` — withholds future events structurally
  (→ SYS-001 §7 look-ahead safety)
- `NOT STARTED` Implement `merge_streams(streams: Vec<EventStream>) -> EventStream` that
  k-way merges multiple sorted streams into one sorted stream with tie-break rule: for equal
  `ts_event`, sort by `instrument_id` lexicographically for determinism
  (→ DATA-002 §11 invariant 3: deterministic)
- `NOT STARTED` Unit-test look-ahead: advance clock to T; assert no event at T+1 is yielded;
  advance to T+1; assert event at T+1 is yielded (→ artifact SC-4 FM-4)
- `NOT STARTED` Unit-test merge: two streams with interleaved events produce correct merged order
  and stable tie-break ordering (→ artifact SC-4)

### Milestone 3: Money type + WarmupGate

- `NOT STARTED` Implement `Money` newtype over `rust_decimal::Decimal` with:
  `Add`, `Sub`, `Mul<Decimal>`, `Div<Decimal>`, `Neg`, `PartialOrd`, `Display` impls;
  `ZERO` and `from_str_exact(s: &str)` constructors; `round_to_tick(tick: Decimal) -> Money`
  (→ ADR-0002; artifact FM-1 "fills against wrong price")
- `NOT STARTED` Implement `WarmupGate` struct: initialized with `warmup_end: Timestamp`;
  `is_open(current_ts: Timestamp) -> bool` returns true only after warmup has passed;
  immutable after construction (→ DATA-002 §4a warmup gating hard rule)
- `NOT STARTED` Unit-test `Money`: addition, subtraction, multiplication; test that
  `0.1 + 0.2 == 0.3` holds exactly (not the float version); test `round_to_tick`
  (→ artifact SC-4; ADR-0002)
- `NOT STARTED` Unit-test `WarmupGate`: gate is closed before `warmup_end`, open at and
  after `warmup_end` (→ DATA-002 §4a)

### Milestone 4: IDs + scoped RNG

- `NOT STARTED` Define `RunId` as a `uuid::Uuid` newtype with serde (→ DATA-002 §3 `request_id`)
- `NOT STARTED` Define `CorrelationId` as a `String` newtype (echoed in results; never
  persisted) (→ DATA-002 §3)
- `NOT STARTED` Implement `ScopedRng` wrapper over `rand::rngs::StdRng`: constructed from
  `(run_id: RunId, seed: u64)`, `Send + Sync`, produces deterministic sequence; two `ScopedRng`
  instances with the same `(run_id, seed)` produce identical output (→ artifact SC-4; ADR-0001
  GIL-free parallelism requires `Send + Sync`)
- `NOT STARTED` Unit-test `ScopedRng`: same seed → same 1000-element sequence;
  different seeds → different sequences; move to another thread via `std::thread::spawn`
  without compile error (→ artifact FM-3)
- `NOT STARTED` Run `cargo test --package core` with zero warnings (→ CI; artifact SC-4)

---

## Open Questions

- [ ] Should `EventStream` sort on construction (accepting unsorted input and correcting it), or
  require sorted input and reject on out-of-order (fail-loud)? DATA-002 §11 says deterministic —
  sort-and-correct hides bad input; reject-on-out-of-order is more honest. Recommend reject;
  confirm before Milestone 2.
- [ ] `ts_event` tie-breaking: when two events share the same nanosecond, the sort order must be
  deterministic and documented. Propose: sort by `instrument_id` asc then `payload_class` ordinal.
  Confirm and record in a comment or ADR note before releasing Milestone 2.

---

## Change Log

| Date | Change | Author |
|------|--------|--------|
| 2026-06-06 | Initial draft | Agent |
