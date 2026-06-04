# Conclusion — Runtime selection

Topic 0001. **Decision: Rust + Python hybrid.**
Feeds: [ADR-0001](../adr/0001-runtime-rust-python-hybrid.md). Date: 2026-06-04.

## Decision

Build the backtesting core in **Rust**, exposed to **Python** via **PyO3 / maturin**, with
**Apache Arrow** as the zero-copy data boundary. Strategies and AI-model adapters are
authored primarily in Python; the per-event hot loop, engines, and run queue live in Rust.

## Why (against the alternatives)

- It scored highest against project-weighted goals (8.4/10 — see
  [summary](../summaries/0001-runtime-language-tradeoffs.md)).
- **Vs. pure Python:** Python can't deliver realistic event-driven multi-engine simulation at
  speed, and the GIL constrains the concurrent run queue (free-threading not yet dependable).
- **Vs. Rust-core-only:** keeping a Python authoring layer is essential because strategies are
  written by humans *and* AI models, both of which are far more productive in Python.
- **Vs. C++ (core or hybrid):** performance is a tie, but Rust wins on compile-time memory/
  data-race safety (wrong P&L from a dangling pointer is unacceptable) and on build tooling.
  NautilusTrader is a direct existence proof of the Rust-core + Python-API pattern in this
  domain.

## Accepted tradeoff

Rust's derivatives-math ecosystem is immature vs. C++'s QuantLib. **Mitigation:** build
derivatives valuation behind our own trait; if needed early, call QuantLib's Python bindings
on the API side; consider an optional native plugin later. We do **not** make C++/QuantLib a
core dependency.

## Follow-ups

- Resolve strategy-authoring form (Python callbacks vs. compiled graph/DSL) — affects hot-loop
  design. Tracked as an open question in the master spec.
