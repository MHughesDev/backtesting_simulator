# ADR-0012: Contracts are a standalone, dependency-free shared kernel

- **Status:** Accepted
- **Date:** 2026-06-04
- **Deciders:** Project owner
- **Informed by:** [ADR-0007](0007-shared-training-pipeline-port.md); resolves OD-11 / Q-REPO-1

## Context

The cross-boundary types — instrument, market-data envelope, the strategy JSON schema, the
`Model` / `Trainer` / `Account` ports, and the **order/execution semantics** — are consumed by
three peer systems: this **backtest suite**, the **training-pipelines** package, and the
**trading platform** (including its live engine). They are a **shared kernel** — the common
language those peers speak. The question is whether to keep them inside the backtest suite (and
have others depend on the suite) or to treat them as a standalone package.

The owner has chosen to design the **end-state** system (ADR-0009) and requires strict
**backtest↔live parity**, which is only guaranteed if both speak the *same* contract definitions
rather than two copies that can drift.

## Decision

Treat the contracts as a **standalone, dependency-free, independently-publishable shared
kernel**. Concretely:

1. Author `crates/contracts` with **zero dependencies on the rest of the suite** — only types,
   traits (ports), and the order/execution semantics.
2. **Outside systems depend on `contracts`, never on the backtest engine.** The training package
   and the platform's live engine consume the kernel directly.
3. Physically **extract** it into its own package the moment a second consumer exists; until
   then it lives in this repo as a self-contained crate. Whether it sits in this repo or a
   separate one is a packaging detail — the dependency rule is what matters.

## Alternatives considered

- **Contracts live in the suite; others depend on the suite** — fewer moving parts now, but
  semantically wrong (why would a live engine depend on the backtester?), risks pulling in
  engine code, and makes parity drift-prone. Rejected as the end-state.
- **Duplicate contracts per system** — guarantees drift between backtest and live. Rejected
  outright.
- **Extract a separate repo immediately** — clean, but premature ceremony before a second
  consumer exists. Deferred: build dependency-free now, extract when needed.

## Consequences

- **Positive:** one source of truth for everything that crosses a boundary; backtest↔live parity
  by construction; peers depend on a neutral kernel, not on the backtester; extraction later is a
  trivial mechanical move.
- **Negative / accepted tradeoffs:** the `contracts` crate must be kept rigorously dependency-free
  (a discipline, enforced in review/CI); eventually an extra package to version and release.
- **Follow-ups:** enforce "contracts has no intra-suite deps" in CI; decide repo vs. monorepo
  topology when the training package / platform materialize (Q-REPO-2).
