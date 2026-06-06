# Plan 0006 — Python Boundary

**Date:** 2026-06-06
**Type:** Formal
**Author:** Agent
**Status:** Draft
**Derivation Status:** Current

## Goal

A Python user can install `btsuite` (the Python package), construct a strategy JSON and
`RunRequest` in Python, bind Arrow IPC data files, call `btsuite.run()`, and receive a
`RunResult` (TradeRecord stream + metrics) as Python objects. The Rust↔Python data transfer is
zero-copy via Apache Arrow. The GIL is not held during engine execution. A Python integration
test runs the EMA-crossover example from Plan 0005 via Python and asserts the same trades as
the Rust test.

---

## Derived From

- Artifact: [docs/artifact.md](../artifact.md) — SC-5 (embeddable and fast; Python-driven; GIL-free)
- Architecture: [docs/architecture.md](../architecture.md) — §1 (Rust core + Python authoring); §4 (Apache Arrow, PyO3/maturin)
- Specs:
  - [DATA-002](../specs/DATA-002-run-request.md) §4 (Arrow IPC data binding descriptors)
  - [SYS-001](../specs/SYS-001-trading-simulator-overview.md) §9 (performance approach: Arrow zero-copy, GIL-free)
- ADRs:
  - [ADR-0001](../adr/0001-runtime-rust-python-hybrid.md) — Rust core + PyO3/maturin; Arrow zero-copy

---

## Scope

**In scope:**
- `crates/pybind`: thin PyO3 extension module — no trading logic, just wire the Rust API to
  Python types
- `python/btsuite`: Python package providing:
  - `RunRequest` builder (Python dataclasses / TypedDict → JSON serialization)
  - `Instrument` builder
  - Arrow IPC data binding helpers (`bind_arrow_file`, `bind_arrow_table`)
  - `run(request: RunRequest) -> RunResult` — calls through to Rust, releases GIL
  - `RunResult`: Python-friendly container wrapping the `TradeRecord` stream as a
    `pyarrow.RecordBatch` and metrics as a `dict`
  - `.pyi` stub files for IDE completions
- `maturin` build configuration (`pyproject.toml` in `crates/pybind` or root)
- Arrow IPC zero-copy handoff: Rust reads `arrow::RecordBatch` from files or in-memory; PyO3
  converts to `pyarrow.RecordBatch` via the `pyo3-arrow` or Arrow PyCapsule interface
- Python integration test: rerun the `equity_daily_crossover` example end-to-end from Python;
  assert trade count and final equity match the Rust result
- `python/README.md`: installation and quickstart (→ python/ README)

**Out of scope:**
- Python strategy authoring beyond JSON construction (no Python-native `Strategy` class that
  replaces the JSON pipeline — the JSON pipeline is the only strategy format, per ADR-0004)
- `pyproject.toml` wheel publishing / PyPI release — infrastructure for a later release plan
- Python type stubs for all 40+ payload types — generate from Rust types in Plan 0010 (tools)
- Plan / multi-strategy composition from Python — Plan 0007

---

## Dependencies

- Plan 0005 (Engine A + single-run orchestrator) fully complete.
- Open question OD-3 (Arrow Tier-A crate choice) must be resolved before Milestone 2;
  confirm `arrow-rs` (the Apache Arrow Rust implementation) as the chosen crate.

---

## Risks

- Risk: PyO3 `GILPool` and `py.allow_threads` usage is easy to misapply, causing deadlocks if
  any PyO3 object is touched inside `allow_threads`. → Mitigation: all Rust engine objects must
  be Python-free (`#[pyclass]` with `#[pyo3(get)]` is fine but must not call Python callbacks
  during engine execution); add a test that spawns the run on a separate OS thread to verify
  GIL is not held.
- Risk: Arrow version mismatch between `arrow-rs` and the caller's `pyarrow` version causing
  IPC incompatibilities. → Mitigation: document minimum `pyarrow` version in `python/README.md`;
  test against the latest stable `pyarrow`.
- Risk: `maturin develop` vs. `pip install -e` workflows differ for contributors.
  → Mitigation: document both in `python/README.md`; add a `Makefile` target `make dev-install`.

---

## Milestones

| # | Milestone | Source | Success Signal |
|---|-----------|--------|----------------|
| 1 | maturin build configuration | ADR-0001; architecture.md §4 | `maturin develop` in a venv succeeds; `import btsuite` works |
| 2 | Arrow IPC zero-copy data handoff | DATA-002 §4; SYS-001 §9 | Rust reads a `.arrow` file and exposes a `pyarrow.RecordBatch` to Python without copying |
| 3 | Python `btsuite` package API | ADR-0001; artifact SC-5 | `btsuite.run(request)` calls Rust, releases GIL, returns `RunResult` with trades as Arrow |
| 4 | Python integration test | artifact SC-5; SC-4 | Python test produces same trade count and final equity as the Rust unit test from Plan 0005 |

---

## Tasks by Milestone

### Milestone 1: maturin build configuration

- `NOT STARTED` Create `pyproject.toml` at the repo root (or inside `crates/pybind`) declaring
  `maturin` as the build backend, module name `btsuite_core`, minimum Python 3.10
  (→ ADR-0001; architecture.md §4)
- `NOT STARTED` Configure `crates/pybind/Cargo.toml` as `crate-type = ["cdylib"]` with
  `pyo3 = { features = ["extension-module"] }`; confirm `pyo3` version matches installed
  `maturin` (→ ADR-0001)
- `NOT STARTED` Expose a bare `#[pymodule]` called `btsuite_core` in `crates/pybind/src/lib.rs`
  with a version-check function `btsuite_core.version() -> str` (→ ADR-0001)
- `NOT STARTED` Create `python/btsuite/__init__.py` that imports from `btsuite_core` and
  re-exports the public API (→ python/ README)
- `NOT STARTED` Test `maturin develop && python -c "import btsuite; print(btsuite.version())"` exits 0
  (→ artifact SC-5)

### Milestone 2: Arrow IPC zero-copy data handoff

- `NOT STARTED` Confirm `arrow-rs` as the Arrow crate (resolving OD-3); add to workspace
  dependencies alongside `pyo3-arrow` (or the PyCapsule bridge) (→ OD-3; ADR-0002)
- `NOT STARTED` Implement `ArrowDataReader` in `crates/runner`: given a file URI or in-memory
  buffer, read an Arrow IPC file into `arrow::RecordBatch`; validate schema against declared
  `payload_class` in the binding descriptor (→ DATA-002 §4 descriptor fields)
- `NOT STARTED` Implement PyO3 conversion: expose `RecordBatch` from Rust to Python as
  `pyarrow.RecordBatch` via the Arrow PyCapsule interface or `pyo3-arrow`; verify no copy of
  row data occurs (→ SYS-001 §9; ADR-0001)
- `NOT STARTED` Implement `bind_arrow_file(path: str, payload_class: str) -> DataBinding`
  Python helper in `btsuite` (→ DATA-002 §4 descriptor)
- `NOT STARTED` Unit-test: write a small synthetic Arrow IPC file in Python; read it in Rust;
  expose back to Python; assert content matches (→ SYS-001 §9)

### Milestone 3: Python btsuite package API

- `NOT STARTED` Expose `ContractValidator`, `SingleRun`, `MetricsCollector` to Python via
  `#[pyclass]` wrappers in `crates/pybind` — no logic, just delegation (→ ADR-0001 thin binding)
- `NOT STARTED` Implement `run(request: RunRequest) -> RunResult` in Python: serialize
  `RunRequest` to JSON → pass to Rust `SingleRun` → collect output → wrap in `RunResult`
  (→ DATA-002 §3; artifact SC-5)
- `NOT STARTED` Release GIL during Rust engine execution:
  `py.allow_threads(|| single_run.execute())` — verify no Python objects are touched during run
  (→ ADR-0001 GIL-free; artifact SC-5)
- `NOT STARTED` Return `TradeRecord` stream as `pyarrow.RecordBatch` in `RunResult.trades`;
  return metrics as `dict[str, float]` in `RunResult.metrics` (→ DATA-002 §7; DATA-008 skeleton)
- `NOT STARTED` Generate `.pyi` stub files for `btsuite_core` module (→ python/ README; SC-5)
- `NOT STARTED` Document `python/README.md` with installation, quickstart, and data format guide
  (→ python/ README)

### Milestone 4: Python integration test

- `NOT STARTED` Create `python/tests/test_equity_crossover.py`: construct `Instrument` and
  `RunRequest` using `btsuite` builders pointing to `fixtures/equity_daily.arrow`; call
  `btsuite.run()`; assert `len(result.trades) > 0`, final equity is positive
  (→ artifact SC-5; SC-4)
- `NOT STARTED` Assert trade-level determinism: run twice with the same seed; compare
  `result.trades` as DataFrames; assert exact equality (→ artifact SC-4; FM-3)
- `NOT STARTED` Assert GIL is not held during run: spawn a Python thread that does work
  concurrently; assert it runs without blocking while `btsuite.run()` executes in another thread
  (→ ADR-0001; artifact SC-5)
- `NOT STARTED` Add Python tests to the GitHub Actions CI pipeline: `pytest python/tests/`
  after `maturin develop` (→ Plan 0002 CI; artifact SC-5)

---

## Open Questions

- [ ] OD-3 confirmed here: use `arrow-rs` (the Apache Arrow Rust crate). Record this resolution
  in `docs/open-questions.md` before Milestone 2 begins.
- [ ] Should `btsuite` expose a high-level `Strategy` class that wraps the JSON pipeline (e.g.
  `strategy = EMAStrategy(fast=10, slow=30)`), or should Python users always hand-write JSON?
  ADR-0004 says JSON is the only format, but a Python builder that emits JSON is consistent with
  it. Defer decision to Plan 0007 strategy authoring phase; note here as out-of-scope for now.
- [ ] Minimum `pyarrow` version: confirm compatibility range with `arrow-rs` IPC format version.

---

## Change Log

| Date | Change | Author |
|------|--------|--------|
| 2026-06-06 | Initial draft | Agent |
