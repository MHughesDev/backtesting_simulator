# The Artifact

Fill this in before anything else. It does not need to be complete — it needs to be honest.
The goal is to get the most important things out of your head and into a form that can be designed against.

Each section feeds a downstream artifact: research briefs, plans, specs, or ADRs.

---

## What Are We Building?

A standalone, headless **backtesting engine** for simulating trading strategies against historical
data across every tradable asset class. It is the processing core a trading platform (or any
conforming caller) drives — not itself a platform, broker, data vendor, or UI. Strategies are
passed in at runtime as declarative JSON, validated against data contracts, executed by an
event-driven simulation, and returned as per-trade records and metrics.

---

## Who Uses It?

The primary user is a **trading platform** that embeds the engine to run backtests on behalf of
its users — and, secondarily, any tool or researcher that can satisfy the engine's input contracts
(instrument definitions, market data, a strategy JSON). The caller owns all product state (data,
model weights, strategy storage, the portfolio ledger, the UI, and a live execution engine that
honors the *same* order semantics the engine simulates, so a strategy behaves identically in
backtest and live).

---

## What Problem Does It Actually Solve?

Realistic, multi-asset backtesting is hard to do honestly. Most backtesters either (a) cover one
asset class with bespoke logic that does not generalize, or (b) vectorize signal replay and quietly
produce plausible-but-wrong P&L when the data is insufficient. This engine exists to provide a
single, **universal contract spine** spanning equities, crypto, derivatives, DEX/AMM, bonds, NFTs,
and prediction markets, with **event-driven, path-dependent** simulation (order matching, AMM
pricing, funding/liquidation, derivatives valuation) — and to **fail loudly** when a run is
under-specified rather than emitting garbage. Without it, every asset class needs its own
backtester and every result is suspect.

---

## What Does Good Look Like?

- SC-1: **Universal contract spine.** A single instrument/market-data/strategy contract set
  expresses all eleven asset classes, with engine selection driven solely by an instrument's
  `price_formation` field — no `match asset_type` branching anywhere in engine logic.
- SC-2: **Realistic, event-driven fills.** Simulation is path-dependent: order matching at the
  best fidelity the supplied data allows (L1→L2→L3, trades, bars), AMM pricing, funding,
  liquidation, and derivatives valuation — not vectorized signal replay.
- SC-3: **Fails loudly on under-specification.** A run whose data does not satisfy the declared
  per-`(instrument, engine)` data manifest is rejected with a precise contract error; it never
  silently produces plausible-but-wrong P&L.
- SC-4: **Deterministic & reproducible.** The same inputs produce bit-identical results regardless
  of thread count (scoped RNG, documented tie-break ordering for simultaneous events).
- SC-5: **Embeddable and fast.** Exposed to Python so a platform can drive it, with a first-class
  parallel run queue for many concurrent backtests; start-to-finish latency and hot-loop throughput
  are tracked product metrics.
- SC-6: **Owns nothing it shouldn't.** The engine stores no strategies, no data, no model weights,
  and assumes no portfolio — equity/positions/collateral are read through an injected `Account`
  port; AI models are an injected inference dependency.

---

## What Would Make This Fail?

- FM-1: **Silent wrong P&L.** The engine fills against the wrong price series (e.g. adjusted
  instead of unadjusted), or proceeds on insufficient data, and returns confident but incorrect
  results — [technical, product].
- FM-2: **Asset-type branching leaks in.** Engine logic accumulates `match asset_type` special
  cases instead of reacting to capability flags, collapsing the universal-spine design under its
  own complexity — [technical].
- FM-3: **Non-determinism under parallelism.** Results vary with thread count or RNG scoping,
  destroying reproducibility and the backtest/live parity guarantee — [technical].
- FM-4: **Look-ahead leakage.** Signals or model inferences read data not yet knowable at the
  simulated timestamp (`ts_available`/`knowable_ts` not enforced), inflating returns — [technical, product].
- FM-5: **Performance misses make it unusable.** Per-event hot-loop overhead or copying the dataset
  across parallel runs makes large sweeps too slow to be worth embedding — [operational].

---

## What Don't We Know Yet?

The full design backlog — blocking and exploratory — is tracked in
[open-questions.md](./open-questions.md) (stable `Q-AREA-n` IDs). The highest-leverage open items
carried into build/planning phases include: the run-queue orchestration boundary (simulator vs. caller),
the expression-vs-component boundary in strategy JSON, model-registry reproducibility over time,
training-artifact validation gating, and the first end-to-end vertical slice (which asset class to
prove first).

---

## What Are We Deliberately Not Building?

- **No live order execution, brokerage, or UI.** The engine simulates and returns; the caller's
  live engine honors the same semantics.
- **No data ownership.** The caller supplies all market and reference data satisfying the contracts.
- **No model ownership.** AI models are an injected, called dependency — never trained, stored, or
  owned by the engine (training is opt-in orchestration over an injected `Trainer` port).
- **No portfolio/ledger ownership.** Equity, positions, and collateral live behind an injected
  `Account` port; portfolio-level analytics are the caller's layer.
- **No MVP subset.** The system is specified and built to its end state across all eleven asset
  classes (see ADR-0009), not as a minimal first slice.

---

## What Are We Working Within?

- **Stack:** A Rust core for the per-event hot loop (GIL-free parallelism; engines `Send + Sync`),
  exposed to Python at the authoring/orchestration edges; Apache Arrow crosses the Rust↔Python
  boundary zero-copy. See [ADR-0001](./adr/0001-runtime-rust-python-hybrid.md).
- **Minimal external dependencies:** anything defining a trade, price, payoff, or metric is built
  in-house to avoid library drift and preserve determinism. See
  [ADR-0002](./adr/0002-minimal-external-dependencies.md).
- **Standalone contracts kernel:** the shared contracts are a dependency-free kernel that external
  systems depend on, never the engine itself. See [ADR-0012](./adr/0012-standalone-contracts-kernel.md).
- **Reproducibility is non-negotiable:** decimal/fixed-point money types and scoped RNG so results
  are deterministic and parity with a live engine is verifiable.
