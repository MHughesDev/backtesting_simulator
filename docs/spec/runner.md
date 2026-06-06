# Spec: Runner & Run Queue

**Type:** Component (internal run scheduler & executor)  
**Status:** 🔲 Deferred — to be authored during its implementation phase. The architectural
frame below is fixed; this file will be expanded into the full spec.

## Purpose

Defines single-run orchestration and the **parallel run queue** — the primitive for executing
many backtests efficiently (parameter sweeps, multi-asset). The per-invocation inputs are
already specified in [run-request.md](run-request.md); this spec covers *execution*.

## What this spec will define

- Single-run lifecycle: validate → warm up → event loop → emit results.
- The **run queue**: scheduling N runs, work-stealing parallelism (GIL-free Rust).
- **Parameter-sweep search** (grid / random / Bayesian) and where it lives vs. the platform
  (OD-2).
- **Determinism under parallelism** — bit-identical results regardless of thread count.
- Zero-copy sharing of the immutable dataset across parallel runs.
- Cancellation, timeouts, per-run resource caps, progress reporting.

## Fixed constraints (already decided)

- Event-driven, deterministic clock; look-ahead safety system-wide.
- Parallelism via Rust work-stealing; engines are `Send + Sync` ([ADR-0001](../adr/0001-runtime-rust-python-hybrid.md)).
- The suite owns no portfolio; account state via injected `Account` ([ADR-0010](../adr/0010-suite-does-not-own-portfolio.md)).
- Training refits pause-train-resume within a run ([training.md](contracts/training.md)).

## Open items

- OD-2 (run-queue boundary), Q-QUEUE-2 (sweep search ownership), Q-QUEUE-4 (parallel determinism).
