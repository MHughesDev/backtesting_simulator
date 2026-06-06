# Plan 0009 — AI Model & Training Integration

**Date:** 2026-06-06
**Type:** Formal
**Author:** Agent
**Status:** Draft
**Derivation Status:** Current

## Goal

Three AI/ML integration points are fully operational: (1) the `Model` inference port is callable
from a strategy pipeline's `model` node, with look-ahead safety enforced so models cannot observe
features with `ts_event > current_ts`; (2) the `Trainer` port enables pause-train-resume
walk-forward retraining with a refit cache; (3) untrusted or AI-authored payoff components
(Engine F) and custom indicators run in the WASM sandbox with no I/O, no clock, and no network
access. After this plan, a strategy can inject an ONNX or mock model that receives a
`ContextBundle` of features and signals and returns a directional `ModelOutput`.

---

## Derived From

- Artifact: [docs/artifact.md](../artifact.md) — SC-6 (owns nothing it shouldn't; AI models are injected), FM-4 (look-ahead leakage via model inference)
- Architecture: [docs/architecture.md](../architecture.md) — §2 (Single Run: Model port), §4 (injected Model and Trainer ports)
- Specs:
  - [INTG-002](../specs/INTG-002-ai-model-inference-port.md) — Model inference port: look-ahead safety, multimodal context, `model_id@version`
  - [INTG-003](../specs/INTG-003-training-port.md) — Trainer port: pause-train-resume, PIT data, refit cache, walk-forward
  - [DATA-005](../specs/DATA-005-signals-contract.md) — `MediaReference`, `ContextBundle` assembly
  - [DATA-006](../specs/DATA-006-strategy-contract.md) §8 — strategy `model` node, `context_inputs`, `endpoint_id@version`
  - [COMP-008](../specs/COMP-008-engine-f-synthetic.md) — `PayoffComponent` WASM sandbox
  - [COMP-001](../specs/COMP-001-component-registry.md) — component registry trust tiers (built-in / native / WASM)
- ADRs:
  - [ADR-0006](../adr/0006-model-inference-and-training.md) — inference-only port, injected Trainer, no weights stored
  - [ADR-0007](../adr/0007-shared-training-pipeline-port.md) — training lives in a shared package, not the simulator
  - [ADR-0008](../adr/0008-training-scope-method-visibility-retention.md) — simulator controls when/what; Trainer implements how
  - [ADR-0011](../adr/0011-component-registry-trust-model.md) — WASM sandbox for untrusted components

---

## Scope

**In scope:**
- `Model` inference port wiring in the strategy pipeline:
  - `ContextBundle` assembly: collect features (at `ts_event ≤ current_ts`), signals (at
    `ts_available ≤ current_ts`), and `MediaReference` pointers from the strategy's
    `context_inputs`; assemble into `ContextBundle` (→ DATA-005; DATA-006 §8)
  - Look-ahead guard: assert every feature in `ContextBundle` satisfies `ts_event ≤ current_ts`;
    panic on violation in debug, return `LookAheadViolation` error in release (→ INTG-002; FM-4)
  - Call `model.infer(bundle) -> ModelOutput`; expose `ModelOutput.direction` and
    `ModelOutput.confidence` to downstream pipeline stages (→ INTG-002)
  - ONNX adapter: ship a `OnnxModelAdapter` that implements `Model` for `.onnx` files via
    `ort` crate (→ INTG-002; ADR-0006)
  - Mock adapter: ship a `MockModel` that returns fixed outputs (for testing without ONNX)
- `Trainer` port wiring for walk-forward retraining:
  - Pause-train-resume mechanic: at each refit point declared in the strategy, pause the
    `SimulationClock`, call `trainer.train(config, pit_data_window)`, swap in the new
    `ModelArtifact`, resume the clock (→ INTG-003; ADR-0008)
  - Refit cache: keyed by `(base_version, method, data_window, params, seed)`; skip training
    and return cached artifact when key matches (→ INTG-003; artifact SC-4 determinism)
  - Mock Trainer: ship a `MockTrainer` that returns a fixed `ModelArtifact` without training
    (for testing) (→ ADR-0007)
  - Look-ahead safety for training: the `pit_data_window` passed to `Trainer.train()` covers
    `[warmup_start, current_ts]` only — no future data leaks in (→ INTG-003; FM-4)
- WASM sandbox for untrusted components:
  - Integrate a WASM runtime (e.g. `wasmtime`) in `crates/engines`
  - `WasmPayoffComponent`: implements `PayoffComponent` by calling into a WASM module; the
    WASM instance has no I/O, no clock, no network access (→ COMP-008; ADR-0011)
  - `WasmIndicator`: implements a custom indicator registered in the component registry as
    a WASM module; same purity constraints (→ COMP-001 §WASM trust tier)
  - Confirm `wasmtime` compile-to-WASM path for the two use cases; document in COMP-001/008
- Open question OD-9 (expression-vs-component boundary) resolved here: "simple arithmetic
  expressions inline in JSON; any stateful or complex logic must be a named component" — record
  in open-questions.md before Milestone 1

**Out of scope:**
- Training algorithm implementation (that lives in the shared training package, injected via
  `Trainer` port — the simulator only calls the interface per ADR-0007)
- Model weight storage (the simulator stores nothing — ADR-0006)
- Bayesian hyperparameter search — out of scope per OD-2 (caller's platform responsibility)
- Open question OD-10 (model registry reproducibility over time): defer; `model_id@version`
  resolution is the caller's responsibility (→ INTG-002 §8)
- Open question OD-12 (training artifact validation gating): defer; `MockTrainer` returns
  artifacts unconditionally; gating policy is the caller's responsibility (→ INTG-003 §8)

---

## Dependencies

- Plan 0005 (strategy pipeline, single-run orchestrator) fully complete.
- Plan 0007 (Engine F Synthetic — WASM stub) Milestone 8 must be complete.
- Plan 0008 (run queue) Milestone 1 must be complete.

---

## Risks

- Risk: `ort` (ONNX Runtime Rust bindings) requires the ONNX Runtime shared library to be
  present at link time; CI machines may not have it. → Mitigation: make `OnnxModelAdapter` a
  feature-gated (`"onnx"`) optional module; CI tests run with the `MockModel` by default.
- Risk: WASM sandbox `wasmtime` adds a large compile-time and binary-size dependency.
  → Mitigation: gate it behind a Cargo feature (`"wasm-sandbox"`); default feature is enabled
  in production builds; disabled in unit-test profiles to keep compile times fast.
- Risk: Look-ahead guard false positives — a feature computed as a "provide-or-derive" value
  may have a derived `ts_event` that differs from the strategy's current time.
  → Mitigation: attach `derived: true` + `source_ts` to all derived features; the guard checks
  `source_ts ≤ current_ts` rather than the derived event's registration timestamp.

---

## Milestones

| # | Milestone | Source | Success Signal |
|---|-----------|--------|----------------|
| 1 | `ContextBundle` assembly + look-ahead guard | INTG-002; DATA-005; FM-4 | Bundle contains only features/signals ≤ current_ts; assertion fires on look-ahead violation |
| 2 | `Model` inference port — mock + ONNX | INTG-002; ADR-0006 | Strategy with `model` node calls `MockModel`; `OnnxModelAdapter` loads and runs a test `.onnx` model |
| 3 | `Trainer` port — pause-train-resume | INTG-003; ADR-0008 | Clock pauses at refit point; `MockTrainer` called with correct PIT window; clock resumes; refit cache deduplicates |
| 4 | WASM sandbox | COMP-008; COMP-001; ADR-0011 | A WASM payoff module (compiled from Rust) runs inside `WasmPayoffComponent` with purity enforced |

---

## Tasks by Milestone

### Milestone 1: ContextBundle assembly + look-ahead guard

- `NOT STARTED` Implement `ContextBundleAssembler` in `crates/runner`: given the strategy's
  `context_inputs` spec, collect features from `MarketView` (at `ts_event ≤ current_ts`),
  signals from bound signal sources (at `ts_available ≤ current_ts`), and `MediaReference`
  pointers from signal sources (→ DATA-005; DATA-006 §8)
- `NOT STARTED` Implement look-ahead guard in `ContextBundleAssembler::assemble()`:
  verify every feature's `source_ts ≤ current_ts`; panic in debug builds; return
  `LookAheadViolation { feature_id, source_ts, current_ts }` error in release builds
  (→ INTG-002; artifact FM-4)
- `NOT STARTED` Unit-test: assemble a bundle at `current_ts = T`; verify feature with
  `ts_event = T` is included; verify feature with `ts_event = T+1` is excluded (or triggers error)
  (→ artifact FM-4)

### Milestone 2: Model inference port — mock + ONNX

- `NOT STARTED` Confirm `Model` trait compiles as a `dyn Model` object (→ contracts Plan 0003
  Milestone 3; INTG-002)
- `NOT STARTED` Implement `MockModel`: returns `ModelOutput { direction: Long, confidence: 1.0 }`
  unconditionally; useful for unit tests without loading ONNX (→ INTG-002; ADR-0006)
- `NOT STARTED` Implement `OnnxModelAdapter` (feature-gated `"onnx"`): wraps `ort::Session`;
  `infer(bundle) -> ModelOutput` — flatten `ContextBundle` into ONNX input tensor, run session,
  decode output tensor to `ModelOutput` (→ INTG-002; ADR-0006)
- `NOT STARTED` Integrate model inference into strategy pipeline `model` node:
  call `ContextBundleAssembler::assemble()`, call `model.infer(bundle)`, bind `ModelOutput`
  to the named pipeline output for downstream alpha/sizing stages (→ DATA-006 §8)
- `NOT STARTED` Integration test: run EMA-crossover strategy with an appended `model` node
  using `MockModel`; verify `ModelOutput` is accessible in the alpha stage (→ INTG-002)

### Milestone 3: Trainer port — pause-train-resume

- `NOT STARTED` Implement refit-point detection in `SingleRun`: read `strategy.refit_schedule`
  (if present); detect when `current_ts` crosses a refit boundary; pause clock
  (→ INTG-003; ADR-0008)
- `NOT STARTED` Implement PIT data window construction: snapshot the event stream history
  from `[warmup_start, current_ts]` into a `TrainingDataWindow`; pass to `Trainer.train()`
  (→ INTG-003; artifact FM-4)
- `NOT STARTED` Implement `MockTrainer`: returns a `ModelArtifact { id: "mock", version: 1 }`
  without any computation; records call count for assertions (→ ADR-0007)
- `NOT STARTED` Implement refit cache: key `(base_version, method_id, data_window_hash, params_hash, seed)`;
  if key hit, return cached `ModelArtifact` without calling trainer (→ INTG-003; artifact SC-4)
- `NOT STARTED` After `Trainer.train()` returns, hot-swap the `Model` artifact in the strategy
  pipeline; resume `SimulationClock` (→ INTG-003 §pause-train-resume; ADR-0008)
- `NOT STARTED` Integration test: walk-forward run with 3 refit points; assert `MockTrainer`
  is called exactly 3 times; assert refit cache deduplicates the 2nd run of the same refit
  window (→ INTG-003; artifact SC-4)

### Milestone 4: WASM sandbox

- `NOT STARTED` Add `wasmtime` to `crates/engines` Cargo.toml (feature-gated `"wasm-sandbox"`)
  (→ ADR-0011; COMP-008)
- `NOT STARTED` Implement `WasmPayoffComponent`: loads a WASM module (`.wasm` bytes) into
  `wasmtime::Engine`; exposes `evaluate(ctx: &PayoffContext) -> (Money, PayoffState)` by calling
  the WASM module's exported `evaluate` function; passes only the context as linear-memory
  arguments — no I/O, no clock, no network access (→ COMP-008; ADR-0011)
- `NOT STARTED` Implement `WasmIndicator`: same sandboxing model; called from the component
  registry when an indicator component is registered with `kind: "wasm"` (→ COMP-001 §WASM tier)
- `NOT STARTED` Write a minimal WASM payoff module in `tests/wasm_payoff/`: a simple constant-
  payoff component compiled to WASM (`wasm32-unknown-unknown`); used as a fixture in the WASM
  sandbox test (→ COMP-008)
- `NOT STARTED` Integration test: `WasmPayoffComponent` loaded with the test module; verify
  `evaluate()` returns correct output; verify any attempt to call a host function (file I/O,
  system clock) is trapped by the WASM runtime (→ ADR-0011; COMP-008)
- `NOT STARTED` Verify Engine F example from Plan 0007 now uses `WasmPayoffComponent` for its
  barrier note (replace the in-process stub) (→ COMP-008)

---

## Open Questions

- [ ] OD-9 (expression-vs-component boundary): resolve before Milestone 1: "simple arithmetic
  expressions inline in JSON are permitted; any stateful or complex logic must be a named component
  in the component registry." Record resolution in `docs/open-questions.md`.
- [ ] OD-10 (model registry reproducibility): deferred — `model_id@version` is the caller's
  responsibility per ADR-0006; record as explicitly out-of-scope in INTG-002 §8.
- [ ] OD-12 (training artifact validation gating): deferred to Plan 0010 or the caller's layer;
  the `MockTrainer` always succeeds; a real validation gate is the `Trainer` implementor's
  responsibility per ADR-0007.
- [ ] `wasmtime` vs. `wasmi` for the WASM runtime: `wasmtime` is JIT-compiled (faster but larger
  binary); `wasmi` is interpreted (smaller, more portable). For payoff components (infrequently
  called), `wasmi` may be preferable. Evaluate at Milestone 4 and record choice in COMP-008.

---

## Change Log

| Date | Change | Author |
|------|--------|--------|
| 2026-06-06 | Initial draft | Agent |
