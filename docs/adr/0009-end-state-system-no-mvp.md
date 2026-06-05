# ADR-0009: Define the end-state system; no MVP scope

- **Status:** Accepted
- **Date:** 2026-06-04
- **Deciders:** Project owner
- **Informed by:** MASTER_SPEC; resolves OD-1

## Context

Earlier framing proposed a phased build (e.g. "design all engines, build the highest-volume
ones first") with an MVP scope. The owner has decided against any MVP, reduced-scope build, or
intermediate "production stasis" target. The specifications and the eventual plan should
describe the **complete, final system** — all asset classes, all eight engines, the full
strategy/model/training/run-request surface — and nothing less.

Planning is sequenced separately: a forthcoming long-term plan will decompose the end-state into
high-level phases, and per-phase plan files will enumerate discrete, atomic tasks. That planning
does **not** reduce the product; it only orders the work toward the same end state.

## Decision

The system is specified and built to its **end state**. There is no MVP, no reduced-scope
milestone, and no intermediate production target in the specs. All eleven asset classes and all
eight engines, plus the full strategy/model/training/run-request/metrics surface, are in scope.

The build will be **organized** into phases via plan files (later), but every phase advances the
single end-state design; no phase ships a deliberately reduced product as the goal.

## Alternatives considered

- **MVP-first (Engine A + E, or core-deep)** — faster to a usable subset, but the owner
  explicitly rejected reduced-scope targets and the "stasis gap" they create. Rejected.
- **Incremental scope creep without a defined end state** — risks an inconsistent architecture.
  Rejected in favor of a fully specified end state up front.

## Consequences

- **Positive:** one coherent target; specs describe the real product; no rework from
  outgrowing an MVP; planning decomposes a known whole rather than guessing at extensions.
- **Negative / accepted tradeoffs:** longer path to first runnable output; the full surface must
  be designed before phase planning; discipline required to keep phases pointed at the end state.
- **Follow-ups:** retire MVP language in `plans/`; produce the long-term phased plan and
  per-phase atomic-task files (sequenced after spec review).
