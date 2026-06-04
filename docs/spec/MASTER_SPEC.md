# Master Specification

**Document status:** Draft · living document
**Scope:** High-level, end-to-end overview of the entire backtesting suite.
**Audience:** Anyone building, integrating with, or evaluating the system.

This is the map. It states *what* the system is and how its parts fit together, and links
out to detailed per-area specs, decision records (ADRs), and research. When a section says
"detailed spec TBD," the authoritative detail will live under `docs/spec/` as a separate
document; this file stays high-level.

---

## 1. Purpose

A **standalone, headless backtesting engine** that simulates trading strategies against
historical data across *every* digital asset class. It is the processing core that an
external trading platform (or any caller with conforming data contracts) utilizes — it is
not itself a platform, broker, data vendor, or UI.

### Goals

1. **Universal** — one contract spine spanning order-book spot, AMM pools, funds, debt, FX,
   futures, perpetuals, options, structured products, NFTs, and event markets.
2. **Realistic** — event-driven, path-dependent simulation (order-book matching, AMM price
   impact, funding/liquidation, derivatives valuation), not vectorized signal replay only.
3. **Fast** — minimize start-to-finish run latency; run many backtests concurrently (a
   first-class run queue).
4. **Embeddable** — callable from Python and other hosts so a trading platform can drive it.
5. **Honest** — reject under-specified runs with precise contract errors rather than produce
   plausible-but-wrong results.

### Non-goals

- No live order execution, brokerage, or UI.
- **No data ownership.** The caller provides data; the suite validates and processes it.
- **No model ownership.** AI models are a called dependency, not stored/trained here.

---

## 2. Foundational principles

| Principle | Statement |
|---|---|
| **Asset ≠ Engine** | Asset category does not determine simulation. *Price formation* selects the engine. |
| **Venue + Contract → Engine** | The pairing of where it trades and what it is determines mechanics, expressed via the instrument's `price_formation` field. |
| **Capabilities, not type switches** | Instruments advertise capability flags; engines/strategies react to capabilities. No `match asset_type` in engine logic. |
| **Thin required spine** | A small required event envelope; everything else is a typed, opt-in payload variant. |
| **Required is relative** | "Required" data is defined per (instrument, engine) via a data manifest — not globally. |
| **Provide-or-derive** | Some values (e.g. greeks) may be supplied by the caller or computed by the engine if absent. |
| **Own the IP, rent the infrastructure** | Build anything that defines a trade/price/payoff/metric; depend only on unopinionated formats/runtime. See ADR-0002. |

---

## 3. Architecture hierarchy

The conceptual stack from economics down to agents:

```
Economic Primitive   (ownership, lending, exchange, future-promise, rights,
        │             synthetic, custom-payoff, event-outcome)
        ▼
Venue Mechanics      (CLOB · AMM · NAV · Dealer · Quote-driven · OTC · Marketplace · Oracle)
        ▼
Execution Engine     (A–H — selected by price_formation)
        ▼
Contract Type        (spot, future, perpetual, option, note, pool, bond, NFT, …)
        ▼
Valuation Model      (mark, NAV, cash-flow/yield, greeks/vol, payoff, floor/oracle)
        ▼
Instrument           (a concrete tradable, carrying identity + capabilities + metadata)
        ▼
Strategy             (universal interface; capability-gated asset features)
        ▼
Portfolio  →  Risk  →  AI Agent
```

This ordering is *why* the data contracts are shaped the way they are: the engine is chosen
high in the stack (price formation), so the instrument only needs to carry enough to route
and to feed its engine.

---

## 4. The five contracts

The system's public surface is five contracts. The **input** contracts (1–4) are what a
caller must satisfy; the **output** contract (5) is what the suite returns. Field-level
detail is in dedicated specs (TBD); this is the shape.

### 4.1 Instrument Contract
Identity + classification + capabilities. The `price_formation` field selects the engine;
capability flags gate which data and which order types are valid.

```
Instrument
├── id              # canonical, e.g. "BTC-USD@coinbase.spot", "WETH/USDC@uniswap.v3"
├── venue           # venue_id + VenueMechanics
├── price_formation # CLOB | AMM | NAV | Dealer | Quote | OTC | Marketplace | Oracle  → ENGINE
├── settlement      # cash | physical | onchain | none
├── capabilities    # bitset: HasOrderBook, HasFunding, HasExpiry, HasGreeks,
│                   #         HasPoolReserves, HasCoupon, HasNAV, IsLeveraged, IsUnique, …
├── quote           # base/quote ccy, tick size, lot size, contract multiplier
└── metadata_ref    # → asset-specific static metadata (strike/expiry, coupon schedule, …)
```

### 4.2 Market Data Contract
A universal envelope with a tagged-union payload. ~80% of events are the universal four
(`Mark`, `Bar`, `Trade`, `Quote`/`Book`); the rest are capability-gated specializations.

```
MarketEvent {
  instrument_id, venue_id,
  ts_event,   # ns UTC — canonical clock
  ts_recv,    # for latency/realism modeling
  seq,        # per-instrument ordering / gap detection
  payload: Payload
}

Payload =
  | Mark   { price }                                   # single point (sparse assets)
  | Bar    { open, high, low, close, volume, interval } # all five guaranteed present
  | Trade  { price, size, aggressor_side, trade_id }
  | Quote  { bid, bid_size, ask, ask_size }            # BBO
  | BookDelta | BookSnapshot                           # L2/L3
  | Funding { rate, mark_price, next_funding_ts }      # perps
  | OpenInterest { oi }
  | PoolState { reserves[], fee_bps, liquidity, sqrt_price, tick }  # AMM
  | Nav { nav, premium_discount }                      # funds
  | Coupon { rate, accrual, next_payment_ts }          # bonds
  | Greeks { iv, delta, gamma, vega, theta, rho }      # options (provide-or-derive)
  | Resolution { outcome, oracle_id }                  # event markets
  | NftEvent { listing | sale | bid, floor, token_id }
```

**Required-data manifest.** For each `(instrument, engine)` the suite declares the minimum
payloads it needs to produce honest fills, plus optional enrichments. A run that under-feeds
is rejected with a precise error. Per-asset manifests: detailed spec TBD (this answers the
original "what do the data contracts look like per asset" question at field level).

### 4.3 Strategy Contract
One universal interface. Asset-specific power is reached through capability-gated accessors,
so strategies are portable by default and only diverge where they opt in.

```
trait Strategy {
  fn on_event(&mut self, view: &MarketView, ctx: &mut Context);
}
```
- `MarketView` always exposes universal fields (price, bid/ask, indicators, **and other
  instruments' values** — enabling cross-asset thresholds).
- `view.funding()` / `view.greeks()` return `Option` — `Some` only when the instrument has
  the capability.
- `ctx.submit(order)` is capability-checked (you cannot send a limit order to an AMM
  instrument; the engine rejects at contract level).

Strategies authored in Python (primary) via the SDK; hot reusable pieces may move to Rust.

### 4.4 Model Contract
AI/ML models plug in as a pure inference dependency the suite *calls*:
```
trait Model { fn infer(&self, features: FeatureFrame) -> Prediction; }
```
The suite owns no weights and does no training. Adapters (ONNX, TorchScript/LibTorch via
`tch`, or a remote endpoint) live behind this trait. Determinism and look-ahead safety
(features may only use data at-or-before `ts_event`) are contract requirements. Detailed
spec TBD.

### 4.5 Result / Metrics Contract
What a run returns. A universal metrics core plus per-asset extensions.
- **Universal:** total/period returns, volatility, Sharpe/Sortino, max drawdown, exposure,
  turnover, hit rate, fees paid, fill quality (slippage vs. arrival).
- **Per-asset extensions:** funding P&L (perps), slippage/price-impact and gas (AMM), greeks
  attribution / theta decay (options), yield/duration/convexity P&L (bonds), tracking error
  (funds), floor/illiquidity metrics (NFTs), Brier score (event markets).

Metrics selection follows capabilities: the suite reports the universal set always and adds
extensions for the capabilities present. Detailed spec TBD.

---

## 5. Engines

Engines are selected by `price_formation`. Build the rule: **if price formation changes,
build a new engine; if it stays the same, extend the existing one.**

| Engine | Name | Price formation | Covers |
|---|---|---|---|
| **A** | Order Book | CLOB | stocks, ETFs (exec), CEX spot crypto, futures, perpetuals, listed-option execution |
| **B** | AMM | Liquidity pool | DEX tokens, memecoins, stablecoin pools |
| **C** | NAV | End-of-day NAV | mutual funds, some index funds, ETF valuation leg |
| **D** | Cash Flow | Dealer / accrual | bonds, treasuries, CDs |
| **E** | Derivatives | Model valuation | options, warrants, futures/perp valuation, greeks |
| **F** | Synthetic | OTC / payoff | CFDs, swaps, structured & barrier notes |
| **G** | Marketplace | Auction / floor | NFTs, collectibles |
| **H** | Event Resolution | Oracle / probability | prediction & binary markets |

**Composition:** an instrument may use more than one engine via capabilities (ETF = A for
execution + C for valuation). Engines are not a flat menu; they compose.

**MVP scope — OPEN DECISION.** Recommended posture: *design all 8 interfaces now so nothing
is architecturally blocked, build the highest-volume engines deep first* (A, then B and E).
Final scope to be recorded in `docs/plans/` and an ADR. See open questions §10.

---

## 6. Asset taxonomy → engine routing

| Asset class | Engine(s) | Key extra capabilities / data |
|---|---|---|
| Order-book spot (stocks, CEX crypto, REITs, ADRs) | A | HasOrderBook; corporate actions, borrow rate |
| DEX / AMM | B | HasPoolReserves; `PoolState`, fee tier, slippage, gas |
| Funds / ETFs | A + C | HasNAV; `Nav`, holdings, tracking error |
| Debt / bonds | D | HasCoupon; `Coupon`, yield curve, duration, credit |
| FX | A′ (quote-driven) | rate differential, session liquidity |
| Futures (expiring) | A | HasExpiry; `OpenInterest`, expiry, term structure |
| Perpetuals | A | HasFunding, IsLeveraged; `Funding`, mark price, liquidations |
| Options | E | HasGreeks, HasExpiry; IV surface, strike, exercise |
| Synthetic / structured | F | payoff formula, financing, barriers |
| Marketplace (NFT) | G | IsUnique; `NftEvent`, floor, rarity |
| Event / prediction | H | HasResolution; `Resolution`, oracle, rules |

---

## 7. Execution model & the run queue

- **Event-driven core.** A deterministic clock replays `MarketEvent`s in `ts_event` order;
  engines mutate state and produce fills; strategies react via `on_event`.
- **Look-ahead safety.** Strategies and models may only observe data at-or-before the
  current `ts_event`. Enforced by the contract, not by convention.
- **Vectorized pre-compute, event-driven execution.** Indicators and model features may be
  computed in bulk up front (fast), then replayed event-by-event for realistic fills — the
  pattern that gives both speed and fidelity.
- **Run queue.** Submitting and scheduling *many* backtests (parameter sweeps, multi-asset)
  is a first-class concern of the suite (`crates/runner`), exploiting Rust's GIL-free
  parallelism. The *trading platform* may have its own higher-level job orchestration, but
  the primitive — "run N backtests efficiently" — lives here. (Open question §10 settles the
  exact boundary.)

---

## 8. Performance approach

- Rust core for the per-event hot loop; Python only at the authoring/orchestration edges.
- Apache Arrow columnar data crosses the Rust↔Python boundary zero-copy.
- Parallel run execution via work-stealing (e.g. `rayon`); engines are `Send`/`Sync`.
- Benchmarks tracked in `benches/`; start-to-finish latency is a tracked product metric.

See research: [runtime selection](../research/conclusions/0001-runtime-selection.md).

---

## 9. Integration boundary (who owns what)

| Concern | Owner |
|---|---|
| Market data (historical feeds) | **Caller / trading platform** |
| AI model weights & training | **Caller** |
| Job orchestration / UI / live trading | **Caller** |
| Instrument & market-data contracts | **Suite** |
| Engines, valuation, fills, metrics | **Suite** |
| Strategy & model *interfaces* | **Suite** |
| Run queue primitive | **Suite** (platform may wrap it) |

The suite validates everything the caller provides against the contracts and fails loudly
on under-specification.

---

## 10. Open questions / decisions pending

1. **MVP engine scope** — design-all-build-core (recommended) vs. all-8 vs. A+E only.
2. **Run-queue boundary** — how much orchestration lives in the suite vs. the platform.
3. **Dependency line** — confirm Arrow in Tier A (ADR-0002), or make even Arrow earn its
   place with a bespoke columnar layout.
4. **Derivatives math** — build in Rust vs. optional QuantLib plugin behind our trait.
5. **Strategy authoring form** — Python callbacks vs. a compiled strategy graph/DSL (affects
   both AI-model authoring and hot-loop speed).

---

## 11. Related documents

- Decisions: [`docs/adr/`](../adr/) — start with
  [ADR-0001 (runtime)](../adr/0001-runtime-rust-python-hybrid.md),
  [ADR-0002 (dependencies)](../adr/0002-minimal-external-dependencies.md),
  [ADR-0003 (capability model)](../adr/0003-capability-based-instrument-model.md).
- Research: [`docs/research/`](../research/).
- Plans: [`docs/plans/`](../plans/).

---

## 12. Glossary

- **CLOB** — Central Limit Order Book.
- **AMM** — Automated Market Maker (liquidity-pool pricing).
- **NAV** — Net Asset Value.
- **Capability** — a flag on an instrument enabling specific data/order types.
- **Price formation** — how an instrument's price is determined; selects the engine.
- **Data manifest** — the declared minimum + optional data for an (instrument, engine).
- **Provide-or-derive** — a value the caller may supply or the engine may compute if absent.
- **Look-ahead** — illegally using data dated after the current event time.
