# ADR-0004: Strategy = a single JSON declarative pipeline

- **Status:** Accepted
- **Date:** 2026-06-04
- **Deciders:** Project owner
- **Informed by:** [spec/contracts/strategy.md](../spec/contracts/strategy.md); resolves MASTER_SPEC OD-5

## Context

A strategy must express the full path to a trade — universe, features, signal/alpha, sizing,
risk, order placement — across every asset class, be authorable by humans *and* AI models,
run fast in a parallel queue, and behave identically in backtest and live. Earlier notes
proposed multiple authoring tiers (declarative / Python SDK / native Rust) and floated a
"monolithic `on_event` escape hatch." Both create problems: multiple formats fragment tooling
and AI generation; a free-form callback mixes concerns and defeats vectorization, determinism,
and capability-gating.

The owner has directed: one declarative format (JSON), a clean pipeline that gives each stage
what it needs, and — if any escape hatch exists — it must be controlled and reusable, not
tangled logic.

## Decision

A strategy is expressed in **exactly one format: JSON**, as a fixed pipeline
`universe → features → models → alpha → sizing → risk → execution`. Stages communicate through
named value bindings. **`on_event` is an internal engine mechanism, not an authoring surface.**
Extensibility is provided exclusively by a **component registry**: custom indicators, alpha
functions, sizing functions, selectors, etc. are registered as named, typed, reusable
components and referenced by ID from the JSON. The JSON holds wiring; the registry holds code.
No code is ever embedded in the strategy JSON.

Sizing is a first-class stage supporting fixed, algorithmic, and alpha/model-derived methods.
The strategy declares a tunable *parameter space*; concrete values/sweeps come from a separate
**Run Request** document.

## Alternatives considered

- **Multiple authoring tiers** — flexible, but fragments tooling and AI codegen, and forks the
  execution path. Rejected in favor of one format with a component registry behind it.
- **Monolithic `on_event` blob** — maximally flexible, but mixes universe/signal/sizing/risk/
  execution, breaks vectorized pre-compute, complicates determinism and capability-gating, and
  is hostile to AI authoring. Rejected.
- **Pure expression DSL with no components** — keeps one format but forces all logic into
  expressions, turning JSON into a programming language. Rejected; components are the escape
  hatch.

## Consequences

- **Positive:** one format to validate, document, and generate; AI emits a typed schema (no
  `eval`, sandboxable); clean separation of concerns; vectorizable features; deterministic,
  capability-gated execution; identical strategy across backtest and live.
- **Negative / accepted tradeoffs:** expressiveness is bounded by built-in components +
  registered components; genuinely novel logic requires registering a component rather than
  inlining code; we must define and police the expression-vs-component boundary.
- **Follow-ups:** component registry trust/sandbox model; expression grammar scope; the Run
  Request schema; declarative fill-reaction model (all tracked in strategy.md §15).
