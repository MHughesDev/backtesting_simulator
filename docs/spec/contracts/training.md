# Contract Spec: Training (the `Trainer` port)

Some strategies need a model that **changes during the run** — walk-forward refit, periodic
fine-tuning, online adaptation. This spec defines how the suite drives training *without*
owning any ML, and how the **same training pipeline package** serves both this backtester and
the trading platform's live engine.

Companion to [model.md](model.md) (inference). See
[ADR-0006](../adr/0006-model-inference-and-training.md) (opt-in training) and
[ADR-0007](../adr/0007-shared-training-pipeline-port.md) (shared training package via a port).

---

## 1. Core principle: same pipeline, different orchestration

Training logic is identical across environments; only *when and how* it is invoked differs.

| | **Backtest (this suite)** | **Live (trading platform)** |
|---|---|---|
| Clock | Simulated; **frozen** during training | Wall-clock; market keeps moving |
| Invocation | **Synchronous** blocking call | **Asynchronous** background job |
| While training | Sim paused — nothing happens | Keep trading on the **current** model version |
| Model swap | Atomic at sim time `T`, before resuming | **Hot-swap** when ready; record swap time |
| Determinism | **Required** (seeded + point-in-time) | Best-effort; lineage audited |
| Training pipeline | **Same package + method** | **Same package + method** |

The suite owns only the **backtest orchestration** (sync pause-train-resume). The live
orchestration lives in the platform. Both call the same `Trainer`.

---

## 2. Dependency inversion: the suite calls a port, not a package

The suite **does not import a training package.** It defines a `Trainer` **port** and the
caller **injects** a concrete implementation at runtime (the shared `training_pipelines`
package, a remote service, a stub, etc.). The suite never learns what kind of model it is.

```
        ┌──────────────────────┐
        │   *-contracts        │  Model, Trainer, ModelArtifact, FeatureFrame, …
        │  (shared kernel)     │
        └──────────┬───────────┘
        ┌──────────┼────────────────────────┐
        ▼          ▼                         ▼
┌──────────────┐ ┌────────────────────┐ ┌───────────────────────┐
│ backtesting  │ │ training_pipelines │ │   trading_platform    │
│   _suite     │ │ implements Trainer │ │ wires all three:      │
│ calls port   │ │ (depends: contracts│ │  • backtest = sync    │
│ (DI'd impl)  │ │  only)             │ │  • live = async swap  │
└──────────────┘ └────────────────────┘ └───────────────────────┘
```

No circular dependencies. The training package is reused verbatim across backtest and live.

---

## 3. The `Trainer` interface

```rust
trait Trainer {
    /// Produce a new model artifact from a point-in-time training job.
    /// In backtest this is called synchronously; in live, off the hot path.
    /// MUST be deterministic given the same job + seed when ctx.deterministic is set.
    fn train(&self, job: TrainingJob, ctx: &TrainContext) -> Result<ModelArtifact, TrainError>;
}

struct TrainingJob {
    base_model_id: ModelId,            // model to fine-tune from
    base_version:  Option<Version>,    // None = train from scratch
    method:        String,             // registered pipeline, e.g. "lora_finetune", "refit_linear"
    dataset:       FeatureFrame,       // PIT data assembled by the SUITE (ts_event ≤ as_of)
    params:        Params,             // hyper-parameters
    as_of:         Timestamp,          // PIT boundary: sim time (backtest) or wall time (live)
}

struct TrainContext {
    seed:          u64,
    deterministic: bool,               // suite sets true for reproducible backtests
    resource_caps: ResourceCaps,       // optional time/memory limits
    cancel:        CancelToken,
}

struct ModelArtifact {
    model_id: ModelId,
    version:  Version,                 // NEW version produced by this training run
    handle:   ArtifactHandle,          // registry reference — the SUITE STORES NO WEIGHTS
    metrics:  TrainMetrics,            // train/validation metrics, for audit & lineage
}
```

**Division of labor:** the suite assembles the PIT `dataset` and decides *when* to call
`train`; the injected `Trainer` does the actual fitting and returns a new version. The suite
then swaps the inference `Model` (see [model.md](model.md)) to that version.

---

## 4. Backtest orchestration — pause, train, resume

Triggered by a strategy's `models[].training` block (see [strategy.md](strategy.md) §8.2).

```
sim loop reaches refit point T (per training.schedule)
  1. finish current event; snapshot strategy + model state
  2. assemble PIT dataset:  ts_event ≤ T,  window = training.train_data_window
  3. check REFIT CACHE keyed by (base_version, method, data_window, params, seed)
        ├─ hit  → reuse cached ModelArtifact   (NO training)
        └─ miss → Trainer.train(job)           (BLOCKING; sim clock frozen)
  4. hot-swap inference Model → new version; append to model-lineage timeline
  5. resume event loop at T with the new model
```

### Invariants

- **Point-in-time.** `dataset` contains only `ts_event ≤ T`. This is what makes walk-forward
  leak-free; enforced by the suite, not the trainer.
- **Determinism.** `deterministic = true` in backtest. Same seed + same PIT data + same method
  ⇒ same artifact ⇒ reproducible backtest. The trainer must honor this.
- **Refit caching.** Identical `(base_version, method, data_window, params, seed)` is trained
  once and reused — across refit points *and across a parameter sweep*. Without this, an
  N-run sweep retrains N× for nothing (see §6).
- **Lineage recorded.** Every swap is logged (sim time, old→new version, training metrics) and
  returned in results, so you can see exactly which model produced which trades.

---

## 5. Live orchestration (platform-owned, documented here for parity)

Out of scope for this repo, but the contract is designed so the platform can:

- Run `Trainer.train` **asynchronously** on a schedule/trigger with live-accumulated PIT data.
- Continue trading on the **current** version while training runs.
- **Hot-swap** atomically when the new artifact is validated, recording the swap time.
- Decide gap behavior (the same `fallback` policy from [model.md](model.md) applies if a model
  is mid-swap or unavailable).

Because both environments call the *same* `Trainer` with the *same* `method`, a strategy that
fine-tunes over time behaves consistently in backtest and live — the **parity** guarantee
(ADR-0005) extended to training.

---

## 6. Performance: training breaks the fast path — contain it

The vectorize-then-replay speed pattern assumes features are pre-computed before the loop.
Mid-run training interrupts that. Mitigations the suite applies:

- **Refit caching** (§4) — the single biggest lever for sweeps.
- **`freeze_after_first`** — fit once during warmup, then inference-only (no further pauses).
- **Coarse schedules** — monthly/quarterly refits, not per-bar.
- **Sweep-aware sharing** — parameters that do not affect the training job (e.g. an execution
  slippage parameter) share one trained artifact across the whole sweep dimension.

Training is often the dominant cost of a model-using backtest; the run queue must budget for
it and surface it in benchmarks.

---

## 7. What the suite owns vs. does not

| Concern | Owner |
|---|---|
| *When* to refit (schedule) and assembling PIT training data | **Suite** |
| Pause-train-resume orchestration + refit cache + lineage | **Suite** |
| The `Trainer` / `Model` interfaces | **Suite** |
| The training **method/pipeline** (the actual fitting code) | **Caller** (`training_pipelines`) |
| Model **weights/artifacts** and the registry they live in | **Caller** |
| Live async orchestration + hot-swap | **Caller** (platform) |

---

## 8. Open questions

- **Validation gating:** may a freshly trained artifact be rejected (e.g. validation metric
  worse than incumbent) and the old version retained? Where is that policy declared?
- **Artifact lifetime in a run:** are intermediate artifacts kept for the whole run (memory)
  or only the active + previous version?
- **Cross-run artifact reuse:** can the refit cache persist across separate backtest runs, or
  is it per-run only? (Persistence implies the suite touching storage — tension with ADR-0005.)
- **Repo topology:** is `*-contracts` extracted as a standalone package both repos depend on,
  or does the training package depend on this repo's `crates/contracts` directly? (See
  ADR-0007.)
