# crates/ — Rust workspace

The performance core. A Cargo workspace; each subdirectory is a crate with a single
responsibility. No code yet — this lists intended crates (provisional names).

| Crate | Responsibility |
|---|---|
| `contracts` | The IP: `Instrument`, capability flags, `MarketEvent` envelope + payload variants, required-data manifests. Depends on nothing trading-opinionated. |
| `core` | Shared primitives: deterministic clock, event stream, ids, fixed-point/decimal money types. |
| `engines` | Price-formation engines A–H (order book, AMM, NAV, cash-flow, derivatives, synthetic, marketplace, event). Selected by `price_formation`. |
| `strategy` | `Strategy` trait, `MarketView`, `Context`, capability-gated accessors. |
| `metrics` | Universal performance/risk metrics + per-asset extensions. |
| `runner` | Single-run orchestration and the parallel backtest **queue**. |
| `pybind` | PyO3 bindings — the Rust↔Python boundary. Thin; no logic. |

Dependency rule per [ADR-0002](../docs/adr/0002-minimal-external-dependencies.md): Tier-A
infrastructure only (serde, Arrow, rayon, …); everything trading-opinionated is built here.
