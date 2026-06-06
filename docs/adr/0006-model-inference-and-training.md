# ADR-0006: AI models — inference by default, opt-in point-in-time training

- **Status:** Accepted
- **Date:** 2026-06-04
- **Deciders:** Project owner
- **Informed by:** [spec/contracts/model.md](../specs/INTG-002-ai-model-inference-port.md), [spec/contracts/strategy.md](../specs/DATA-006-strategy-contract.md) §8

## Context

Strategies will call AI/alpha models. These models are usually **pre-trained** before
execution and should not be trained during backtesting or live trading. However, some
scenarios warrant fitting or refitting a model during a run (e.g. walk-forward refit on a
rolling window). The simulator must support inference cleanly, allow opt-in training without
becoming a training framework, and keep both leak-free and reproducible.

## Decision

Models are an **external dependency invoked through the Model Contract**; the simulator stores no
weights. **Inference is the default and only behavior unless a strategy explicitly opts in to
training.** When opted in, the simulator **orchestrates** training (decides *when* to refit and
assembles strictly point-in-time `ts_event ≤ current_ts` training data) but the **training
method is a registered, preconfigured routine owned by the caller** — the simulator does not own
training algorithms.

A strategy calls a model by ID: it passes `model_id` + pinned `model_version`, an
`inference_fn`, the bound point-in-time `inputs`, an inference `frequency`, named `outputs`
(value + optional confidence), and a `fallback` policy. The engine enforces point-in-time
inputs, determinism (seeded), and the fallback policy.

## Alternatives considered

- **No training at all (inference-only)** — simplest, but blocks legitimate walk-forward and
  scenario-specific fitting. Rejected in favor of opt-in.
- **Simulator owns a training framework** — powerful, but turns the backtester into an ML platform,
  violates "owns no models," and risks look-ahead via careless data windows. Rejected.
- **Implicit/always-on retraining** — convenient, but expensive, non-obvious, and a leakage
  hazard. Rejected; training must be explicit per strategy.

## Consequences

- **Positive:** clean inference path; walk-forward and scenario fitting available without the
  simulator owning ML; leak-free by construction (PIT-enforced); reproducible via version pinning +
  seeds + refit caching.
- **Negative / accepted tradeoffs:** opt-in training breaks full pre-loop vectorization at
  refit points (mitigated by caching refits by data-window/params); reproducibility depends on
  the platform resolving `model_id@version` to identical weights and providing deterministic
  methods.
- **Follow-ups:** model registry / version-resolution contract; refit caching strategy in the
  runner; remote-adapter determinism guidance.
