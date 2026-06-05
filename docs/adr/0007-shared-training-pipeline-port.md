# ADR-0007: Training is a shared pipeline invoked through a port

- **Status:** Proposed — refined by [ADR-0008](0008-training-scope-method-visibility-retention.md) (repo topology still to confirm; live-orchestration framing narrowed to context-only per ADR-0008 §4)
- **Date:** 2026-06-04
- **Deciders:** Project owner
- **Informed by:** [spec/contracts/training.md](../spec/contracts/training.md),
  [ADR-0006](0006-model-inference-and-training.md), [ADR-0005](0005-strategy-not-stored-suite-is-a-library.md)

## Context

Some strategies require a model that changes during the run (walk-forward refit, periodic
fine-tuning). In a backtest the simulation must pause at each refit point, (re)train on
point-in-time data, swap the model, and resume. The same training capability is also needed by
the trading platform's **live** engine. The owner wants a single automated training-pipeline
system reused by both backtest and live, rather than two implementations that can drift apart.

Two forces must be reconciled: (1) the suite must not own ML or store weights
([ADR-0006](0006-model-inference-and-training.md)); (2) backtest training is synchronous in
*simulated* time while live training is asynchronous in *wall-clock* time.

## Decision

1. **Define training behind a `Trainer` port** in the shared contracts. The suite calls the
   port; a concrete implementation (the `training_pipelines` package) is **injected** at
   runtime via dependency inversion. The suite never imports the training package and never
   learns the model type.
2. **The same training pipeline serves backtest and live**; only orchestration differs — the
   suite drives it **synchronously** (pause-train-resume), the platform drives it
   **asynchronously** (background train + hot-swap). This asymmetry is explicit in
   [training.md](../spec/contracts/training.md) §1.
3. **The suite owns orchestration and PIT data assembly**, never the training method. It
   decides *when* to refit, assembles `ts_event ≤ as_of` data, enforces determinism, caches
   refits, and records model lineage. The injected `Trainer` does the fitting and returns a
   new versioned `ModelArtifact` handle (weights live in the caller's registry).

## Alternatives considered

- **Bake training into the suite** — convenient, but turns the backtester into an ML platform
  and violates ADR-0006. Rejected.
- **Two separate training implementations (backtest vs. live)** — they would drift; a strategy
  validated in backtest would not match live. Rejected; defeats the parity goal.
- **Suite imports the training package directly** — couples the suite to a specific ML stack
  and creates dependency tangles. Rejected in favor of a port + dependency injection.

## Consequences

- **Positive:** one training pipeline reused across backtest and live (parity extended to
  training); suite stays ML-agnostic and stores no weights; deterministic, leak-free
  walk-forward by construction; trainer is swappable/mockable for tests.
- **Negative / accepted tradeoffs:** mid-run training breaks vectorized pre-compute (mitigated
  by refit caching, `freeze_after_first`, coarse schedules); reproducibility depends on the
  injected trainer honoring determinism; the run queue must budget for training cost.
- **Open — repo topology (to confirm):** either (a) extract a standalone `*-contracts` package
  that `backtesting_suite`, `training_pipelines`, and the platform all depend on; or (b) keep
  the contracts in this repo's `crates/contracts` and have the training package depend on it.
  (a) maximizes reuse/parity; (b) is fewer moving parts. Tracked as OD-11.
