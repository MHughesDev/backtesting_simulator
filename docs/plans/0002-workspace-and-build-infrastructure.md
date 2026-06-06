# Plan 0002 — Workspace & Build Infrastructure

**Date:** 2026-06-06
**Type:** Formal
**Author:** Agent
**Status:** Draft
**Derivation Status:** Current

## Goal

A valid Rust workspace exists with all seven crates scaffolded (empty but correctly declared and
mutually wired), `cargo check` and `cargo test` pass on the empty skeleton, and a GitHub Actions
CI pipeline runs on every push. This is the foundation every subsequent plan builds on — nothing
compiles until this is done.

---

## Derived From

- Artifact: [docs/artifact.md](../artifact.md) — SC-5 (embeddable and fast; tracked build metrics)
- Architecture: [docs/architecture.md](../architecture.md) — §2 components table (7 crates)
- ADRs:
  - [ADR-0001](../adr/0001-runtime-rust-python-hybrid.md) — Rust core + PyO3/maturin
  - [ADR-0002](../adr/0002-minimal-external-dependencies.md) — minimal external dependencies policy
  - [ADR-0012](../adr/0012-standalone-contracts-kernel.md) — `contracts` is standalone; others depend on it
- Specs: none (this plan produces build infrastructure, not feature code)

---

## Scope

**In scope:**
- Root `Cargo.toml` workspace manifest declaring all 7 crates
- Per-crate `Cargo.toml` with correct inter-crate dependencies and Tier-A external deps declared
- Minimal `lib.rs` skeleton in each crate (no logic — just `#![forbid(unsafe_code)]` where appropriate and a module stub)
- `maturin` / PyO3 build configuration for `crates/pybind`
- `.github/workflows/ci.yml` — `cargo check`, `cargo test`, `cargo clippy --deny warnings`, `cargo fmt --check`
- `.gitignore` updates for Rust build artifacts and Python packaging artifacts

**Out of scope:**
- Any actual feature implementation (that begins in Plans 0003+)
- Python packaging (`pyproject.toml`, wheel publishing) — Plan 0006
- Benchmark harness — Plan 0008
- Integration test harness — Plan 0010

---

## Dependencies

- None — this is the first implementation plan; all prior work is spec/doc only.
- Open question OD-3 (Arrow Tier-A vs bespoke layout) deferred: the workspace declares `arrow`
  as a dependency but the API call sites are added in Plan 0003+. Deferring does not block this plan.

---

## Risks

- Risk: `maturin` / PyO3 version pinning creates subtle compatibility issues with later Python
  packaging. → Mitigation: pin `pyo3` and `maturin` to the latest stable at workspace creation;
  document versions in `Cargo.toml` comments. Re-evaluate in Plan 0006.
- Risk: `cargo clippy --deny warnings` on an empty crate causes spurious dead-code lints in later
  plans. → Mitigation: add `#[allow(dead_code)]` stubs in skeletons only where necessary; remove
  as real code lands.

---

## Milestones

| # | Milestone | Source | Success Signal |
|---|-----------|--------|----------------|
| 1 | Workspace root declared | architecture.md §2 (7 crates); ADR-0001 | `cargo metadata` lists all 7 crates |
| 2 | All 7 crate skeletons compile | architecture.md §2; ADR-0012 (dependency order) | `cargo check --workspace` passes |
| 3 | CI pipeline runs green | artifact SC-5 (tracked build metrics) | GitHub Actions workflow passes on a new push |

---

## Tasks by Milestone

### Milestone 1: Workspace root declared

- `NOT STARTED` Create root `Cargo.toml` as a `[workspace]` manifest listing all 7 members:
  `crates/contracts`, `crates/core`, `crates/engines`, `crates/strategy`, `crates/metrics`,
  `crates/runner`, `crates/pybind` (→ architecture.md §2)
- `NOT STARTED` Add `[workspace.dependencies]` for shared Tier-A dependencies per ADR-0002:
  `serde`, `serde_json`, `arrow`, `rayon`, `rust_decimal`, `uuid`, `chrono`, `thiserror`,
  `pyo3` (→ ADR-0002; ADR-0001)
- `NOT STARTED` Add `[profile.release]` with `lto = "thin"`, `codegen-units = 1` for benchmark
  accuracy (→ artifact SC-5)

### Milestone 2: All 7 crate skeletons compile

- `NOT STARTED` Create `crates/contracts/Cargo.toml` with zero intra-workspace dependencies
  (standalone per ADR-0012); declare `serde`, `rust_decimal`, `uuid`, `thiserror` (→ ADR-0012)
- `NOT STARTED` Create `crates/contracts/src/lib.rs` with module stubs:
  `pub mod instrument;`, `pub mod market_event;`, `pub mod ports;`, `pub mod data_manifest;`
  (→ DATA-003, DATA-004, INTG-001/002/003, DATA-002 §10)
- `NOT STARTED` Create `crates/core/Cargo.toml` depending on `contracts`; declare `rayon`,
  `chrono` (→ ADR-0001; architecture.md §2)
- `NOT STARTED` Create `crates/core/src/lib.rs` with module stubs:
  `pub mod clock;`, `pub mod event_stream;`, `pub mod ids;`, `pub mod money;`
  (→ SYS-001 §7; artifact SC-4)
- `NOT STARTED` Create `crates/engines/Cargo.toml` depending on `contracts`, `core`
  (→ architecture.md §2)
- `NOT STARTED` Create `crates/engines/src/lib.rs` with module stubs for each engine:
  `pub mod engine_a;`, `pub mod engine_b;`, …, `pub mod engine_h;`
  (→ architecture.md §2; COMP-003 through COMP-010)
- `NOT STARTED` Create `crates/strategy/Cargo.toml` depending on `contracts`, `core`
  (→ architecture.md §2)
- `NOT STARTED` Create `crates/strategy/src/lib.rs` with module stubs:
  `pub mod strategy_trait;`, `pub mod market_view;`, `pub mod pipeline;`
  (→ DATA-006; SYS-001 §5)
- `NOT STARTED` Create `crates/metrics/Cargo.toml` depending on `contracts`, `core`
  (→ architecture.md §2)
- `NOT STARTED` Create `crates/metrics/src/lib.rs` with module stub `pub mod universal;`
  (→ DATA-008; architecture.md §2)
- `NOT STARTED` Create `crates/runner/Cargo.toml` depending on `contracts`, `core`, `engines`,
  `strategy`, `metrics`, `rayon` (→ architecture.md §2; COMP-002)
- `NOT STARTED` Create `crates/runner/src/lib.rs` with module stubs:
  `pub mod run_queue;`, `pub mod single_run;`, `pub mod validator;`
  (→ COMP-002; DATA-002 §10)
- `NOT STARTED` Create `crates/pybind/Cargo.toml` as a `cdylib` + `rlib` with `pyo3` feature
  `"extension-module"`, depending on `runner`, `contracts` (→ ADR-0001)
- `NOT STARTED` Create `crates/pybind/src/lib.rs` with a bare `#[pymodule]` function stub
  (→ ADR-0001; python/ README)
- `NOT STARTED` Verify `cargo check --workspace` passes with no errors or warnings

### Milestone 3: CI pipeline runs green

- `NOT STARTED` Create `.github/workflows/ci.yml` with jobs:
  `check` (`cargo check --workspace`), `test` (`cargo test --workspace`),
  `clippy` (`cargo clippy --workspace --deny warnings`), `fmt` (`cargo fmt --check`)
  (→ artifact SC-5)
- `NOT STARTED` Pin Rust toolchain in `rust-toolchain.toml` to a specific stable version
  (→ artifact SC-4 determinism; ADR-0002)
- `NOT STARTED` Update `.gitignore` to exclude `target/`, `dist/`, `*.egg-info`, `.venv/`
  (→ ADR-0001 Python packaging)
- `NOT STARTED` Verify CI passes green on the feature branch

---

## Open Questions

- [ ] OD-3: Is `arrow2` or `arrow` (official Apache arrow-rs) the right crate? Evaluate at Plan
  0006 when the Rust↔Python boundary is wired; for now declare `arrow` as a placeholder.
- [ ] Should `crates/pybind` use `pyo3 = { features = ["extension-module"] }` or the newer
  `pyo3-build-config` approach? Confirm at Plan 0006.

---

## Change Log

| Date | Change | Author |
|------|--------|--------|
| 2026-06-06 | Initial draft | Agent |
