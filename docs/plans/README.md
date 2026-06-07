# Plans

Roadmaps and implementation plans. Plans are forward-looking and *do* change — unlike ADRs
(which are immutable history), a plan is updated as reality moves. Each plan should link to
the spec sections and ADRs it implements.

## Convention

`NNNN-plan-slug.md`. Keep a short status line at the top (Draft / Active / Done / Abandoned).

## Execution order

Plans 0003 and 0004 may begin in parallel once Plan 0002 is complete. All other plans are
sequenced as listed — each plan depends on the ones before it. See the Dependencies section
of each plan for exact gates.

```
0002 → 0003 ──┐
         0004 ──┴→ 0005 → 0006 → 0007 → 0008 → 0009 → 0010
```

## Index

| # | Plan | Type | Status | Summary |
|---|------|------|--------|---------|
| [0001](0001-mvp-roadmap.md) | MVP roadmap | Formal | **Superseded** | Replaced by end-state phased plans below per ADR-0009 |
| [0002](0002-workspace-and-build-infrastructure.md) | Workspace & Build Infrastructure | Formal | Draft | Rust workspace, all 7 crate skeletons, GitHub Actions CI skeleton |
| [0003](0003-contracts-crate.md) | Contracts Crate | Formal | Draft | `crates/contracts`: Instrument, capability flags, all 40+ MarketEvent payloads, ports, DataManifest |
| [0004](0004-core-crate.md) | Core Crate | Formal | Draft | `crates/core`: deterministic clock, event stream, IDs, Money type, scoped RNG |
| [0005](0005-engine-a-and-first-run.md) | Engine A & First End-to-End Run | Formal | Draft | Engine A (CLOB), Strategy trait, RunRequest validation, single-run orchestrator, first worked example |
| [0006](0006-python-boundary.md) | Python Boundary | Formal | Draft | `crates/pybind` + `python/btsuite`: PyO3/maturin, Arrow zero-copy, `btsuite.run()` |
| [0007](0007-engines-b-through-h.md) | Engines B through H | Formal | Draft | All 7 remaining engines + Engine A capability extensions; completes all 11 asset classes |
| [0008](0008-run-queue-and-concurrency.md) | Run Queue & Concurrency | Formal | Draft | Parallel `RunQueue`, parameter sweep expansion, determinism tests, benchmarks |
| [0009](0009-ai-model-and-training.md) | AI Model & Training Integration | Formal | Draft | Model inference port, Trainer pause-train-resume, WASM sandbox |
| [0010](0010-integration-tests-examples-ci.md) | Integration Tests, Examples & CI/CD | Formal | Draft | Full test suite, 11 worked examples, DATA-008 finalization, CI/CD complete |
