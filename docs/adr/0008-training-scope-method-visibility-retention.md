# ADR-0008: Training — method selection, visibility, retention, and scope

- **Status:** Accepted
- **Date:** 2026-06-04
- **Deciders:** Project owner
- **Informed by:** [spec/contracts/training.md](../spec/contracts/training.md);
  refines [ADR-0007](0007-shared-training-pipeline-port.md)

## Context

Following [ADR-0006](0006-model-inference-and-training.md) (opt-in training) and
[ADR-0007](0007-shared-training-pipeline-port.md) (training via an injected `Trainer` port),
four concrete questions remained: (1) does the *training method* belong in the strategy JSON?
(2) what data may training see? (3) how many trained artifacts does a run keep? and (4) how far
should this repo go in describing live/platform training behavior? ADR-0007 had described live
orchestration as "async hot-swap," which over-stepped this repo's remit.

## Decision

1. **Method selection lives in the strategy JSON — the identifier, not the algorithm.** The
   strategy names the training `method` (plus schedule, window, hyper-parameters); the injected
   `Trainer` resolves the implementation. This keeps the strategy a complete, portable,
   reproducible description of behavior (the same JSON must not yield different results
   depending on what the platform chose to train with), while the training code stays
   caller-owned. Mirrors how `model_id` works for inference.

2. **Training visibility is limited to the strategy's bound features.** The suite builds the
   training `FeatureFrame` from the strategy's declared feature bindings only — the same surface
   the model sees at inference. Training may not reach into arbitrary universe instruments or
   unreferenced streams. This bounds the leakage surface and guarantees train/serve consistency.

3. **A run retains exactly two artifacts: `initial` and `current`.** `initial` is the model as
   passed in (immutable for the run); `current` is the most recently trained model. Each refit
   replaces `current` and discards the superseded artifact; `initial` is kept throughout.
   Lightweight lineage *records* (version ids, metrics, active intervals) are kept for the full
   timeline for auditability, but only two heavy *artifacts* are held.

4. **Scope: this repo defines only the `Trainer` port and the backtest orchestration.** It does
   not specify the trading platform, the training system internals, the model registry, or any
   non-backtest (e.g. live) orchestration. Such systems are described only as *interplay
   context* to make the boundary explicit. This **refines ADR-0007**, which is updated to stop
   prescribing live async behavior.

## Alternatives considered

- **Method chosen by the platform, not the strategy** — would make the same strategy JSON
  non-reproducible and break backtest/live parity. Rejected.
- **Training sees the whole universe** — more flexible, but enlarges the leakage surface and
  invites train/serve skew. Rejected.
- **Retain every intermediate artifact** — full lineage of weights, but unbounded memory for
  long walk-forward runs and unnecessary given the lightweight lineage records. Rejected.
- **Fully specify live training here** — convenient context, but violates separation of
  concerns and this repo's library remit. Rejected; live is the caller's.

## Consequences

- **Positive:** strategies stay self-contained and reproducible; leakage surface is small and
  auditable; memory is bounded to two artifacts; the repo's scope is crisp (port + backtest
  orchestration only), reinforcing separation of concerns.
- **Negative / accepted tradeoffs:** intermediate trained models are not recoverable after a run
  (only their lineage records remain); strategies must declare every feature training needs as a
  binding; the platform carries all non-backtest orchestration.
- **Follow-ups:** validation gating policy (OD-12); cross-run refit-cache persistence; repo
  topology for shared contracts (OD-11).
