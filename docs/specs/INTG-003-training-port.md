# Spec: INTG-003 — Training (the `Trainer` port)

**Spec ID:** INTG-003
**Type:** Integration (external training orchestration)
**Status:** Approved
**Date:** 2026-06-06
**Author:** Agent

Some strategies need a model that **changes during the run** — walk-forward refit, periodic
fine-tuning, online adaptation. This spec defines how the backtest simulator drives training
*without* owning any ML.

Companion to [model.md](INTG-002-ai-model-inference-port.md) (inference). See
[ADR-0006](../adr/0006-model-inference-and-training.md) (opt-in training),
[ADR-0007](../adr/0007-shared-training-pipeline-port.md) (training via an injected port), and
[ADR-0008](../adr/0008-training-scope-method-visibility-retention.md) (scope, method selection,
visibility, retention).


---

## 0. Scope of this document (read first)

This repo defines **only two things** about training:

1. The **`Trainer` port** — the interface the simulator calls.
2. The **backtest orchestration** — how the simulator pauses, trains, swaps, and resumes during a
   simulated run.

This repo does **not** define the trading platform, the training system's internals, the model
registry, or any **non-backtest (e.g. live) orchestration**. Those belong to the caller. Where
this document mentions live behavior, it is **interplay context only** — included so the
boundary is unambiguous, not a specification of the platform. (See [ADR-0008](../adr/0008-training-scope-method-visibility-retention.md).)

---

## 1. The method is selected in the strategy, implemented in the caller

The strategy JSON **names the training method by identifier** and supplies its schedule,
window, and hyperparameters (see [strategy.md](DATA-006-strategy-contract.md) §8.2). It **never** contains the
training algorithm. This mirrors how inference works: the strategy names a `model_id`; the
caller resolves the weights. Here the strategy names a `method`; the injected `Trainer`
resolves the implementation.

**Why the method belongs in the strategy:** the strategy JSON is the complete, portable,
reproducible description of behavior. Two strategies identical except for the training method
behave differently and must be distinguishable by their JSON alone. The *selection* is
strategy state; the *implementation* is a caller concern.

```
Strategy JSON  ──names──►  method: "walk_forward_refit"   (the WHAT)
                                   │
Injected Trainer  ──resolves──►   actual fitting pipeline  (the HOW, caller-owned)
```

---

## 2. The `Trainer` port

The simulator calls a `Trainer` it is **given** (dependency injection); it never imports a training
package and never learns the model type.

```rust
trait Trainer {
    /// Produce a new model artifact from a point-in-time training job.
    /// Called synchronously by the backtest orchestrator.
    /// MUST be deterministic given the same job + seed when ctx.deterministic is set.
    fn train(&self, job: TrainingJob, ctx: &TrainContext) -> Result<ModelArtifact, TrainError>;
}

struct TrainingJob {
    base_model_id: ModelId,            // model to fine-tune from
    base_version:  Option<Version>,    // None = train from scratch
    method:        String,             // the identifier named in the strategy JSON
    dataset:       FeatureFrame,       // PIT data assembled by the SIMULATOR from STRAT-BOUND features
    params:        Params,             // hyper-parameters from the strategy JSON
    as_of:         Timestamp,          // PIT boundary = current sim time
}

struct TrainContext {
    seed:          u64,
    deterministic: bool,               // simulator sets true for reproducible backtests
    resource_caps: ResourceCaps,       // optional time/memory limits
    cancel:        CancelToken,
}

struct ModelArtifact {
    model_id: ModelId,
    version:  Version,                 // NEW version produced by this training run
    handle:   ArtifactHandle,          // registry reference — the SIMULATOR STORES NO WEIGHTS
    metrics:  TrainMetrics,            // train/validation metrics, for lineage
}
```

**Division of labor:** the simulator assembles the PIT `dataset` (from strat-bound features),
decides *when* to call `train`, enforces determinism, and swaps the model. The injected
`Trainer` performs the fitting and returns a new version. The simulator stores no weights.

---

## 3. Backtest orchestration — pause, train, resume

Triggered by a strategy's `models[].training` block (see [strategy.md](DATA-006-strategy-contract.md) §8.2).
Within a backtest, time is *simulated*, so "stop and wait for training" is simply a
**synchronous blocking call** — the sim clock does not advance while training runs.

```
sim loop reaches refit point T (per training.schedule)
  1. finish current event; snapshot strategy + model state
  2. assemble PIT dataset:  ts_event ≤ T,  window = training.train_data_window,
        SOURCE = the strategy's bound features only (§5)
  3. check REFIT CACHE keyed by (base_version, method, data_window, params, seed)
        ├─ hit  → reuse cached ModelArtifact   (NO training)
        └─ miss → Trainer.train(job)           (BLOCKING; sim clock frozen)
  4. swap active model → new version;  retain {initial, current} only (§6);
        append a lightweight record to the model-lineage timeline
  5. resume event loop at T with the new model
```

### Invariants

- **Point-in-time.** `dataset` contains only `ts_event ≤ T`. This is what makes walk-forward
  leak-free; enforced by the simulator.
- **Determinism.** `deterministic = true` in backtest. Same seed + same PIT data + same method
  ⇒ same artifact ⇒ reproducible backtest. The injected trainer must honor this.
- **Refit caching.** Identical `(base_version, method, data_window, params, seed)` is trained
  once and reused across refit points and across a parameter sweep (see §7).
- **Lineage recorded.** Every swap appends a lightweight record (sim time, old→new version,
  training metrics, active interval) to the results, even though only two *artifacts* are
  retained (§6).

---

## 4. Inputs visibility — strat-bound features only

Training sees **exactly the data the strategy has bound as features** — the same surface the
model sees at inference time. It may **not** reach into arbitrary instruments in the universe,
raw streams the strategy never referenced, or anything outside the strategy's declared feature
set.

Rationale: bounding training visibility to the inference surface (a) keeps the leakage surface
small and auditable, (b) guarantees train/serve consistency (the model is fit on the same kind
of inputs it will be scored on), and (c) preserves separation of concerns — the simulator hands the
trainer a `FeatureFrame` built from the strategy's bindings, nothing more.

---

## 5. Artifact retention — initial + current only

During a run the simulator retains **exactly two model artifacts**:

| Slot | Meaning |
|---|---|
| **initial** | The model exactly as passed in at run start (immutable for the whole run) |
| **current** | The most recently trained model (the active one) |

On each refit, the freshly trained artifact becomes **current**; the previously-current
artifact is **discarded** (superseded). The **initial** artifact is preserved for the entire
run. At run end you therefore hold the initial model and the final trained model — useful for
baseline-vs-adapted comparison — and nothing in between.

Note: only the heavy **artifacts** (weights/handles) are bounded to two. The lightweight
**lineage records** (version ids, metrics, active intervals) are kept for the full timeline so
the run remains auditable.

---

## 6. Determinism & reproducibility

A training-enabled backtest is reproducible only if:

- the method, schedule, window, params, and seed are fixed (all in the strategy JSON);
- `base_model_id@version` resolves to identical weights each time (caller's responsibility);
- the injected `Trainer` is deterministic under the seed;
- training data is strictly point-in-time (simulator-enforced).

---

## 7. Performance: training breaks the fast path — contain it

The vectorize-then-replay speed pattern assumes features are pre-computed before the loop.
Mid-run training interrupts that. Mitigations the simulator applies:

- **Refit caching** (§3) — the single biggest lever for sweeps.
- **`freeze_after_first`** — fit once during warmup, then inference-only (no further pauses).
- **Coarse schedules** — monthly/quarterly refits, not per-bar.
- **Sweep-aware sharing** — parameters that do not affect the training job share one trained
  artifact across that sweep dimension.

Training is often the dominant cost of a model-using backtest; the run queue must budget for it
and surface it in benchmarks.

---

## 8. Interplay with non-backtest systems (context only — not specified here)

The same `Trainer` port is intended to be reusable by the caller's other systems (e.g. a live
engine), so a strategy that fine-tunes over time behaves consistently wherever it runs —
the **parity** intent (ADR-0005) extended to training. **How** any non-backtest system
orchestrates training (synchronously, asynchronously, with hot-swaps, with maintenance
windows) is entirely the caller's decision and is **out of scope for this repo.** This section
exists only to make the boundary explicit:

- **The simulator provides:** the `Trainer` port, the `Model` port, the strategy `training` schema,
  and the backtest orchestration that uses them.
- **The caller provides:** the `Trainer` implementation, the model registry, and any
  non-backtest orchestration.

---

## 9. Ownership

| Concern | Owner |
|---|---|
| *When* to refit (schedule) + assembling PIT training data from strat-bound features | **Simulator (backtest)** |
| Pause-train-resume orchestration + refit cache + lineage + 2-artifact retention | **Simulator (backtest)** |
| The `Trainer` / `Model` interfaces (ports) | **Simulator** |
| The training **method/pipeline** (the actual fitting code) | **Caller** |
| Model **weights/artifacts** and the registry they live in | **Caller** |
| Any non-backtest (e.g. live) training orchestration | **Caller (out of scope here)** |

---

## 10. Open questions

- **Validation gating (OD-12):** may a freshly trained artifact be rejected (e.g. validation
  metric worse than the incumbent) and the previous `current` retained instead? Where is that
  policy declared?
- **Cross-run refit cache:** per-run only, or persisted across runs? Persistence implies the
  simulator touching storage (tension with ADR-0005); could be a caller-injected cache interface.
- **Repo topology (OD-11):** standalone `*-contracts` package vs. depending on this repo's
  `crates/contracts` (see [ADR-0007](../adr/0007-shared-training-pipeline-port.md)).
