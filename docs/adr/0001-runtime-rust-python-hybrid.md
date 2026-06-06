# ADR-0001: Runtime — Rust core + Python (hybrid)

- **Status:** Accepted
- **Date:** 2026-06-04
- **Deciders:** Project owner
- **Informed by:** [research/conclusions/0001-runtime-selection.md](../research/conclusions/0001-runtime-selection.md)

## Context

The simulator must run realistic, event-driven, path-dependent simulations (order-book matching,
AMM slippage, funding/liquidation, derivatives valuation) across many asset classes, minimize
start-to-finish run latency, and run many backtests concurrently (a first-class run queue).
At the same time, strategies and AI models are authored by humans *and* LLMs — both far more
productive in Python — and the simulator must be embeddable into a separate trading platform.

These goals pull in two directions: raw per-event speed + safe parallelism (favoring a
systems language) vs. authoring ergonomics + AI/data ecosystem (favoring Python).

## Decision

We will build the core in **Rust** and expose it to **Python** via **PyO3 / maturin**, using
**Apache Arrow** as the zero-copy boundary. The per-event hot loop, engines, and run queue
live in Rust; strategies and model adapters are authored primarily in Python.

## Alternatives considered

- **Pure Python + vectorization** — fastest to prototype, but cannot express realistic
  event-driven multi-engine simulation at speed, and the GIL constrains the run queue
  (free-threading is experimental/optional through 3.13–3.14). Rejected as structurally
  unfit for the realism + concurrency goals.
- **Rust core only** — excellent engine, but no Python authoring layer; strategies/AI would
  pay a steep ergonomics cost or require a custom DSL up front. Rejected for now (a DSL may
  still be added later behind the Python layer).
- **C++ core / C++ + Python** — ties Rust on speed and wins on derivatives math (QuantLib),
  but loses on compile-time memory/data-race safety and on build tooling. Rejected: silent
  miscomputation risk and toolchain friction outweigh the QuantLib advantage, which we can
  capture another way (see ADR-0002).

## Consequences

- **Positive:** top-tier hot-loop speed; GIL-free parallel run queue; compile-time safety on
  the money path; Python authoring + native AI/data ecosystem; clean embeddability;
  precedent (NautilusTrader) for the exact pattern.
- **Negative / accepted tradeoffs:** two toolchains and a build pipeline (maturin); a
  Rust↔Python boundary to design carefully (keep per-tick logic native); Rust's derivatives
  ecosystem is younger than C++'s.
- **Follow-ups:** resolve strategy-authoring form (Python callbacks vs. compiled graph/DSL);
  decide derivatives-math sourcing (see ADR-0002).
