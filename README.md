# backtesting_suite

A headless backtesting engine for testing trading strategies across digital assets —
stocks, ETFs, spot crypto, DEX/AMM pools, futures, perpetual swaps, options, bonds,
NFTs, prediction markets, and more.

> **Status:** early design. No engine code yet. This repository currently holds the
> architecture, data-contract design, decision records, and research that the
> implementation will be built from. See [`docs/spec/MASTER_SPEC.md`](docs/spec/MASTER_SPEC.md).

---

## What this is

A **standalone, headless backtesting library**. It is the *processing engine* that a
separate trading platform (or any tool with the right data contracts) calls to simulate
strategies against historical data.

- **Universal** — one contract spine that spans every tradable asset class.
- **Realistic** — event-driven, path-dependent simulation (order books, AMM slippage,
  funding/liquidation, derivatives valuation), not just vectorized signal replay.
- **Fast** — a Rust core for the hot loop, built to run many backtests concurrently and
  minimize start-to-finish run latency.
- **Embeddable** — exposed to Python (and other hosts) so a trading platform can *utilize*
  it.

## What this is **not**

- **Not** a trading platform, broker, or UI. It executes no real orders.
- **Not** a data vendor. **The suite owns no market data.** The caller supplies data that
  satisfies the contracts; the suite validates, processes, and returns results.
- **Not** an AI-model owner. Models are a dependency the suite *calls* through a contract;
  it does not train or store them.

## Core design ideas

- **Asset ≠ Engine.** The asset's category does not decide how it's simulated. *Price
  formation* does. `BTC` on a central order book and `ETH` in a Uniswap pool are both
  crypto but use different engines. The selector is the instrument's `price_formation`
  field, never a switch on "is this a stock."
- **Capability flags, not type switches.** Instruments advertise capabilities
  (`HasOrderBook`, `HasFunding`, `HasGreeks`, `HasPoolReserves`, …). Engines and strategies
  react to capabilities, which keeps the system genuinely universal and lets one instrument
  compose multiple engines (e.g. an ETF: order-book execution + NAV valuation).
- **Thin required spine, opt-in everything else.** Every event shares a small required
  envelope; the payload is one of many typed variants, gated by capability and required
  only relative to a chosen engine/strategy.
- **Own the contracts.** External dependencies are kept minimal and unopinionated; anything
  that defines what a trade/price/payoff/metric *is* is built in-house. See
  [ADR-0002](docs/adr/0002-minimal-external-dependencies.md).

## Tech stack

- **Rust** core for contracts, engines, and the run queue (safety + speed + fearless
  parallelism). See [ADR-0001](docs/adr/0001-runtime-rust-python-hybrid.md).
- **Python** authoring/integration layer via **PyO3 / maturin** for strategies and AI-model
  adapters.
- **Apache Arrow** as the zero-copy data boundary between Rust and Python (a memory
  *format*, not a trading model).

## Repository layout

```
backtesting_suite/
├── README.md
├── .gitignore
├── Cargo.toml                    # Rust workspace manifest (added when code begins)
├── pyproject.toml                # Python packaging (added when code begins)
│
├── crates/                       # Rust — the performance core (owns engines + contracts)
│   ├── contracts/                #   instrument, market-event, data-manifest types (the IP)
│   ├── core/                     #   clock, event stream, ids, common primitives
│   ├── engines/                  #   execution / price-formation engines A–H
│   ├── strategy/                 #   strategy trait + execution context
│   ├── metrics/                  #   performance & risk metrics
│   ├── runner/                   #   single-run orchestration + the backtest queue
│   └── pybind/                   #   PyO3 bindings (the Rust↔Python boundary)
│
├── python/                       # Python — authoring & integration surface
│   └── btsuite/                  #   strategy SDK, high-level API, AI-model adapters
│
├── docs/                         # all documentation (see docs/ index below)
│   ├── spec/                     #   normative specifications (MASTER_SPEC + per-area)
│   ├── research/                 #   sources, summaries, conclusions (decision evidence)
│   ├── adr/                      #   architecture decision records
│   └── plans/                    #   roadmaps & implementation plans
│
├── examples/                     # runnable example strategies + sample data manifests
├── benches/                      # performance benchmarks (speed is a product goal)
├── tests/                        # cross-crate integration & contract-conformance tests
├── fixtures/                     # tiny SYNTHETIC test data only — the suite owns no real data
├── tools/                        # dev scripts, codegen, schema exporters
└── .github/workflows/            # CI (build, test, lint, bench-on-PR)
```

Names like `btsuite` and the `crates/*` split are provisional and will be confirmed when
implementation begins.

## Documentation index

| Area | Path | Purpose |
|---|---|---|
| Master spec | [`docs/spec/MASTER_SPEC.md`](docs/spec/MASTER_SPEC.md) | High-level, end-to-end system overview |
| Research | [`docs/research/`](docs/research/) | Sources, summaries, and conclusions behind decisions |
| Decisions | [`docs/adr/`](docs/adr/) | Architecture Decision Records (the "why") |
| Plans | [`docs/plans/`](docs/plans/) | Roadmaps and implementation plans |

## License

TBD.
