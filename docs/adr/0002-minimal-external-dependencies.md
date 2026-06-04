# ADR-0002: Minimal external dependencies — own the contracts

- **Status:** Accepted (Tier-A membership of Apache Arrow: to confirm)
- **Date:** 2026-06-04
- **Deciders:** Project owner
- **Informed by:** MASTER_SPEC §2, §9

## Context

The data contracts *are* the product. Adopting an existing backtesting framework, exchange
SDK, or quant library means inheriting its data model and assumptions, at which point the
"universal" contract silently degrades into "whatever library X already supported." The owner
has explicitly chosen to be slow to adopt external open-source projects. But a blanket "no
dependencies" rule is also wrong — reimplementing serialization or columnar memory layout is
wasted effort and risk.

We need a concrete, durable rule for where the line sits.

## Decision

We will distinguish **infrastructure/format** (unopinionated, freely used) from **opinion**
(anything that defines a trade/price/payoff/metric — built in-house), via three tiers:

- **Tier A — Allowed (infrastructure):** stable, ubiquitous, encodes no trading opinion.
  Examples: `serde`, **Apache Arrow** (columnar memory format *and* the zero-copy Rust↔Python
  bridge), `rayon` (parallelism), `time`/`chrono`, PyO3 + maturin.
- **Tier B — Evaluate per-case (numeric leaves):** low-opinion primitives we could build but
  shouldn't. Adopt only the leaf function, never the domain model. Examples: `statrs`,
  `ndarray`, a Parquet reader.
- **Tier C — Build ourselves (the IP):** anything defining an instrument, fill, price-
  formation rule, payoff, or metric. The instrument/market-data contracts, all engines,
  order-book matching, AMM math, funding/liquidation, the strategy interface, metrics.
  **QuantLib stays out of the core** — at most a later optional plugin behind our own
  derivatives-valuation trait, never a hard dependency.

The governing line: *a dependency is acceptable only if it is a format or runtime primitive,
never if it encodes what a trade or price is.*

## Alternatives considered

- **No external dependencies at all** — maximal control, but reinvents serialization and
  columnar layout for no real benefit; also forfeits Arrow's zero-copy boundary. Rejected as
  over-strict. (Can be revisited specifically for Arrow — see open item below.)
- **Adopt a mature framework (e.g. an existing backtester / QuantLib) as foundation** —
  fastest start, but surrenders the contract design that is the whole point. Rejected.

## Consequences

- **Positive:** the contracts and engines remain fully ours and exact; small, auditable
  dependency surface; Arrow gives a zero-copy boundary "for free."
- **Negative / accepted tradeoffs:** we build more ourselves (notably derivatives math) and
  carry that maintenance; every Tier-B adoption needs a deliberate review.
- **Open item:** confirm Arrow belongs in Tier A. If the owner wants even Arrow to earn its
  place, we design a bespoke columnar layout instead (more work, total control).
- **Follow-ups:** when proposing any Tier-B dependency, record the leaf function used and why
  building it is not worthwhile.
