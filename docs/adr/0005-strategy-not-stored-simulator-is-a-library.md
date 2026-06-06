# ADR-0005: The simulator processes strategies but never stores them (library boundary)

- **Status:** Accepted
- **Date:** 2026-06-04
- **Deciders:** Project owner
- **Informed by:** MASTER_SPEC §1, §8; [ADR-0002](0002-minimal-external-dependencies.md)

## Context

The trading simulator is one component of a larger trading platform. The platform stores
strategies, manages users, lets users select strategies, and applies them to live assets for
automated execution — and crucially, the platform's live trading must offer the *same*
buy/sell/order capabilities the simulator simulates. The simulator must be usable as a package/library
the platform calls, not a service that owns product state.

Without a hard boundary, the simulator would accrete strategy storage, user concepts, and
scheduling, duplicating the platform and coupling the two.

## Decision

The simulator is a **library**. It **does not store strategies, users, data, or models.** A
strategy JSON is passed in at runtime, validated, compiled into an execution plan, executed,
and discarded. The simulator echoes the caller-supplied `strategy_id` in results for correlation
but persists nothing.

To guarantee backtest↔live parity, the simulator defines the **order/execution semantics** (order
types, fills, capability-gating) as a shared contract the platform's live engine is expected
to honor. The same strategy JSON therefore drives both backtest (here) and live (in the
platform) with identical behavior; only the data feed differs.

## Alternatives considered

- **Simulator as a stateful service owning strategies/runs** — convenient standalone, but
  duplicates platform responsibilities, couples lifecycles, and contradicts the library intent.
  Rejected.
- **Simulator owning a strategy *cache/registry*** — even a cache implies ownership and
  invalidation concerns. Rejected; the component registry holds *code components*, never
  *strategies*.

## Consequences

- **Positive:** clean separation of concerns; the simulator stays embeddable and stateless per run;
  parity between backtest and live falls out of a shared strategy + order contract; testing is
  simpler (pure function of inputs).
- **Negative / accepted tradeoffs:** the platform must own persistence, versioning, and model/
  data resolution; reproducibility depends on the platform supplying stable `model_id@version`
  and point-in-time data.
- **Follow-ups:** specify the shared order/execution semantics precisely enough that a separate
  live engine can match them (engine specs); the Run Request boundary is now defined in
  [run-request.md](../specs/DATA-002-run-request.md).
