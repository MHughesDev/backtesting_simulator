# Plan 0008 — Run Queue & Concurrency

**Date:** 2026-06-06
**Type:** Formal
**Author:** Agent
**Status:** Draft
**Derivation Status:** Current

## Goal

`crates/runner` implements the full parallel run queue: submitting N `RunRequest`s (a parameter
sweep, a multi-asset portfolio, walk-forward windows) dispatches them to a work-stealing thread
pool. Results are bit-identical regardless of thread count. The Python GIL is not held during
engine execution. Benchmarks in `benches/` track two product metrics: start-to-finish latency for
a single run and hot-loop events-per-second throughput. Both metrics gate future performance
regressions in CI.

---

## Derived From

- Artifact: [docs/artifact.md](../artifact.md) — SC-4 (deterministic regardless of thread count), SC-5 (fast; first-class parallel run queue; tracked latency/throughput)
- Architecture: [docs/architecture.md](../architecture.md) — §2 (Runner & Run Queue), §9 (performance approach: work-stealing, GIL-free)
- Specs:
  - [COMP-002](../specs/COMP-002-runner-and-run-queue.md) — Runner & Run Queue full spec
  - [DATA-002](../specs/DATA-002-run-request.md) §8 (parameter sweep — grid/random/Bayesian) and §11 (invariant 3: deterministic)
  - [SYS-001](../specs/SYS-001-trading-simulator-overview.md) §7 (run queue: concurrency, GIL-free, work-stealing)
- ADRs:
  - [ADR-0001](../adr/0001-runtime-rust-python-hybrid.md) — GIL-free Rust parallelism; `rayon` work-stealing
  - [ADR-0005](../adr/0005-strategy-not-stored-simulator-is-a-library.md) — stateless; no shared mutable state across runs

---

## Scope

**In scope:**
- `RunQueue` struct: accepts `Vec<RunRequest>`, dispatches runs via `rayon::ThreadPool`
  (work-stealing); returns `Vec<RunResult>` in submission order
- Parameter sweep expansion: `RunRequest.parameters` with `sweep: grid | range | random` expanded
  into concrete `RunRequest`s before dispatch (→ DATA-002 §8)
- Determinism invariant: given the same `RunRequest`, two calls to `RunQueue::submit()` with
  different thread counts produce byte-identical `TradeRecord` streams (→ artifact SC-4; FM-3)
- GIL release: `py.allow_threads(|| run_queue.submit(requests))` in Python binding; assert no
  Python objects are touched inside Rayon tasks (→ ADR-0001; SC-5)
- Benchmarks in `benches/`:
  - `bench_single_run_latency`: time from `RunQueue::submit([request])` to first `TradeRecord`;
    track P50, P95 in CI (→ artifact SC-5)
  - `bench_hot_loop_throughput`: measure events/second on a 10-million-event synthetic stream;
    track as events/sec in CI (→ artifact SC-5)
- CI performance gate: `cargo bench` output stored as baseline; a >10% regression in either
  metric blocks merge (→ artifact SC-5)
- `output.mode = "stream"`: incremental results emitted as runs complete, not batched at end
  (→ DATA-002 §7 output.mode)

**Out of scope:**
- Bayesian sweep search (grid and random are in-scope; Bayesian is deferred — open question OD-2)
- Distributed execution across machines — out of scope for this plan
- Python-side sweep orchestration — the run queue's primitive is the engine primitive; higher-level
  orchestration lives in the caller's platform (→ ADR-0005)
- Walk-forward window generation logic — the caller constructs the RunRequest list; the run queue
  executes it (OD-2 boundary)

---

## Dependencies

- Plan 0005 (Engine A + single-run orchestrator) fully complete.
- Plan 0006 (Python boundary) fully complete (GIL-release test requires the pybind layer).
- Open question OD-2 (run-queue boundary: how much orchestration in simulator vs. platform)
  must be resolved to the extent of: "simulator provides work-stealing dispatch + parameter
  expansion; caller handles walk-forward window construction and Bayesian search." Record in
  open-questions.md before Milestone 1.

---

## Risks

- Risk: `rayon` work-stealing does not guarantee insertion order in results; callers may expect
  results in submission order. → Mitigation: collect `(index, result)` tuples; sort by index
  before returning; add a unit test that verifies order is preserved across varying thread counts.
- Risk: Benchmark regressions from unrelated refactors accidentally block CI.
  → Mitigation: use `criterion`'s `--save-baseline` / `--load-baseline` flags; set a 10%
  tolerance; document the tolerance in `benches/README.md`.
- Risk: Parameter sweep explosion: a grid sweep over 5 parameters × 10 values each = 100,000
  runs. The run queue must not materialize all expanded requests in memory simultaneously.
  → Mitigation: implement lazy sweep expansion using an iterator; dispatch a batch at a time
  bounded by `limits.max_concurrent_runs` from the `RunRequest`.

---

## Milestones

| # | Milestone | Source | Success Signal |
|---|-----------|--------|----------------|
| 1 | `RunQueue` parallel dispatch | COMP-002; ADR-0001 | N concurrent runs complete; results are in submission order |
| 2 | Parameter sweep expansion | DATA-002 §8 | Grid and random sweeps expand correctly; each expanded run has concrete parameter values |
| 3 | Determinism invariant verification | artifact SC-4; FM-3 | Same RunRequest on 1 thread vs. N threads → byte-identical TradeRecord streams |
| 4 | Streaming output mode | DATA-002 §7; COMP-002 | `output.mode = "stream"` emits TradeRecords incrementally before run completes |
| 5 | Benchmarks + CI gate | artifact SC-5 | `bench_single_run_latency` and `bench_hot_loop_throughput` run and produce a stored baseline |

---

## Tasks by Milestone

### Milestone 1: RunQueue parallel dispatch

- `NOT STARTED` Implement `RunQueue` struct: holds `rayon::ThreadPool` configured with
  `num_threads = num_cpus::get()` by default; override via `RunRequest.limits.max_concurrent_runs`
  (→ COMP-002; ADR-0001 work-stealing)
- `NOT STARTED` Implement `RunQueue::submit(requests: Vec<RunRequest>) -> Vec<RunResult>`:
  dispatch each request as an independent Rayon task; collect in submission order
  (→ COMP-002; artifact SC-5)
- `NOT STARTED` Ensure `EngineA` (and all future engines) are `Send + Sync`; add `static_assertions::assert_impl_all!(EngineA: Send, Sync)` in tests
  (→ ADR-0001 GIL-free; artifact SC-5)
- `NOT STARTED` Unit-test: submit 10 identical runs; verify all 10 complete; verify results
  are in submission order (→ COMP-002; artifact SC-4)

### Milestone 2: Parameter sweep expansion

- `NOT STARTED` Implement `SweepSpec` parser: recognize `sweep: grid | range | random` in
  `RunRequest.parameters` (→ DATA-002 §8)
- `NOT STARTED` Implement `GridSweepExpander`: iterate over Cartesian product of all grid/range
  parameters; yield `RunRequest` with concrete `parameters` values
  (→ DATA-002 §8; COMP-002)
- `NOT STARTED` Implement `RandomSweepExpander`: sample within declared parameter bounds using
  `ScopedRng` (→ DATA-002 §8; artifact SC-4 deterministic sweeps)
- `NOT STARTED` Validate that each swept value falls within the strategy's declared parameter
  bounds (→ DATA-002 §10 step 2)
- `NOT STARTED` Implement lazy expansion iterator (not materialized upfront) bounded by
  `limits.max_concurrent_runs` (→ risk mitigation)
- `NOT STARTED` Unit-test grid expansion: 2 params × 3 values each → 9 concrete RunRequests;
  verify all 9 parameter combinations are present (→ DATA-002 §8)

### Milestone 3: Determinism invariant verification

- `NOT STARTED` Run the EMA-crossover example (Plan 0005 fixtures) with `RunQueue` on 1, 2, 4,
  and 8 threads; serialize all `TradeRecord` streams to bytes; assert byte equality across all
  thread counts (→ artifact SC-4; FM-3)
- `NOT STARTED` Verify that two runs of the same sweep with the same seed produce identical
  per-run results in identical order (→ artifact SC-4)
- `NOT STARTED` Add determinism assertion to CI: `cargo test -- --test-threads=8 determinism`
  runs on every push (→ artifact SC-4)

### Milestone 4: Streaming output mode

- `NOT STARTED` Implement `StreamingOutput` via Rust channel (`mpsc::channel`): the `SingleRun`
  sends each `TradeRecord` on fill; the caller receives them in order as they arrive
  (→ DATA-002 §7 `output.mode = "stream"`)
- `NOT STARTED` Expose streaming output to Python via a Python generator / iterator that yields
  `TradeRecord` objects from the channel (→ Plan 0006 pybind; DATA-002 §7)
- `NOT STARTED` Unit-test: streaming run of 100 trades; assert first `TradeRecord` arrives before
  the run completes (→ DATA-002 §7)

### Milestone 5: Benchmarks + CI gate

- `NOT STARTED` Add `benches/single_run_latency.rs` using `criterion`: measure time from
  `RunQueue::submit([request])` to last `TradeRecord` on the EMA-crossover example; report
  P50 and P95 (→ artifact SC-5)
- `NOT STARTED` Add `benches/hot_loop_throughput.rs`: generate a synthetic 10-million-event
  `EventStream`; measure events processed per second; report in CI output
  (→ artifact SC-5; SYS-001 §9)
- `NOT STARTED` Configure `criterion` baseline storage in CI: store baseline in `benches/baselines/`
  (committed); compare against baseline on every PR; flag >10% regression as CI failure
  (→ artifact SC-5)
- `NOT STARTED` Document benchmark targets and tolerance in `benches/README.md`
  (→ artifact SC-5)

---

## Open Questions

- [ ] OD-2 (run-queue boundary) resolution: confirm the following division before Milestone 1:
  "simulator owns parallel dispatch and parameter-sweep expansion; caller owns walk-forward
  window construction, Bayesian search, and distributed execution." Record as resolved in
  `docs/open-questions.md`.
- [ ] Should `RunQueue` expose a `max_concurrent_runs` cap, or always use all available cores?
  Recommend: configurable in `RunRequest.limits`; default to `num_cpus`.
- [ ] Streaming output back-pressure: if the Python consumer is slow, does the engine block or
  drop TradeRecords? Recommend: block (bounded channel); size TBD. Confirm before Milestone 4.

---

## Change Log

| Date | Change | Author |
|------|--------|--------|
| 2026-06-06 | Initial draft | Agent |
