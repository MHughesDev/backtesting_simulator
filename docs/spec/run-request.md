# Spec: Run Request

A **Run Request** is the per-invocation document that tells the suite *how to execute one run*.
The **Strategy JSON** says *what the strategy is* (reusable, portable); the Run Request binds it
to concrete data, time, parameters, and the injected ports for one execution. The bound
`strategy` may be a single Strategy or a **Plan** (multiple strategies composed by data-flow —
see [contracts/plan.md](contracts/plan.md)); a single Strategy is the degenerate one-node Plan.

The suite **stores nothing** and **assumes no portfolio** (see
[ADR-0005](../adr/0005-strategy-not-stored-suite-is-a-library.md),
[ADR-0010](../adr/0010-suite-does-not-own-portfolio.md)). A Run Request wires in everything the
run needs and is discarded when the run completes.

---

## 1. The per-trade model (read first)

The suite is a **per-trade execution simulator**, not a portfolio manager. For each decision
the strategy makes, the suite produces a **TradeRecord**: the trade setup, the sizing decision,
and the simulated execution information (§7). It does **not** own a cash/position ledger or an
equity curve.

Anything that needs account state reads it from an **injected `Account` port** owned by the
caller:

| Needs account state | Reads from injected `Account` |
|---|---|
| `fixed_fractional` / `volatility_target` sizing | equity, buying power |
| `max_drawdown`, `max_position_pct` risk rules | equity, current positions |
| perp/option margin & liquidation | collateral, margin balance |

The suite owns the **mechanics** (sizing formulas, liquidation math, fill simulation); the
caller owns the **ledger** (cash, positions, settlement, currency). A simple reference `Account`
adapter ships as an *optional* convenience, but the core never assumes one.

---

## 2. Strategy vs. Run Request

| | **Strategy JSON** | **Run Request** |
|---|---|---|
| Answers | *What is the strategy?* | *How do we run it this time?* |
| Lifetime | Reusable, portable (backtest & live) | One invocation |
| Contains | universe, features, models, alpha, sizing, risk, execution; declared *parameter space* | data bindings, time window, parameter *values/sweep*, injected ports, seed, output config |
| Capital / account | Never | Never owned — an `Account` port is injected if needed |

---

## 3. Top-level schema

```jsonc
{
  "schema_version": "1.0",
  "request_id": "caller-correlation-id",      // echoed in results; not persisted

  "strategy": { /* a single Strategy JSON, OR a Plan (multi-strategy), OR a handle the caller resolves */ },

  "time": {
    "start":  "2020-01-01T00:00:00Z",
    "end":    "2024-12-31T23:59:59Z",
    "warmup": "60d"                            // history before `start` for indicator/model warmup
  },

  "instruments": [ /* Instrument definitions (or references) in scope */ ],

  "data":       { /* §4  injected market data bindings (instrument-keyed) */ },
  "reference_bindings": { /* §4b  entity-keyed reference data (issuer/universe/venue, optional) */ },
  "derived_data": { /* §4a  inline bar derivation config (optional) */ },
  "signals":    { /* §4c  exogenous-signal sources (news/social/macro/media, multi-source, optional) */ },
  "cohorts":    { /* §4d  market-wide data sources that materialize instruments dynamically (scanner universe, optional) */ },
  "account":    { /* §5  injected Account port (optional) */ },
  "ai_endpoints": { /* §6  injected AI endpoint bindings */ },
  "components":   { /* §6  injected custom registry components (optional) */ },

  "parameters": { /* §8  concrete values OR a sweep over the strategy's declared space */ },

  "execution_defaults": { /* §9  latency/slippage/fee model defaults */ },

  "determinism": { "seed": 12345 },

  "output": { /* §7  what to emit and how (stream vs batch) */ },

  "limits": { "max_runtime": "30m", "max_memory_mb": 8192 }
}
```

Everything under `data`, `account`, `ai_endpoints`, `components` is an **injected port** —
the caller supplies an implementation; the suite calls it. None of these are owned by the suite.

---

## 4. `data` — injected market data

The suite owns no data. The Run Request binds each instrument's required payload streams to a
source the caller controls (a path, an in-memory Arrow table, or a `DataReader` implementation).

Each binding entry uses a **descriptor object** (not a bare URI string) so the engine knows
what payload class and resolution the data contains before reading any rows:

```jsonc
"data": {
  "reader": "arrow_ipc",                        // arrow_ipc | parquet | injected:<id>
  "bindings": {
    "AAPL@nasdaq.equity": {
      "bars_1d": {
        "uri":             "s3://bucket/aapl_daily.arrow",
        "payload_class":   "Bar",
        "interval":        "1d",
        "adjusted":        false,
        "timestamp_field": "ts_event"
      },
      "bars_1d_adj": {
        "uri":             "s3://bucket/aapl_daily_adj.arrow",
        "payload_class":   "Bar",
        "interval":        "1d",
        "adjusted":        true,
        "timestamp_field": "ts_event"
      },
      "trades": {
        "uri":           "s3://bucket/aapl_trades.arrow",
        "payload_class": "Trade",
        "timestamp_field": "ts_event"
      },
      "corp_actions": {
        "uri":           "s3://bucket/aapl_corp_actions.arrow",
        "payload_class": "CorporateAction"
      }
    },
    "BTC-USD@coinbase.spot": {
      "bars_1m": {
        "uri":           "s3://bucket/btcusd_1m.arrow",
        "payload_class": "Bar",
        "interval":      "1m",
        "adjusted":      false
      },
      "trades": {
        "uri":           "s3://bucket/btcusd_trades.arrow",
        "payload_class": "Trade"
      }
    }
  }
}
```

**Descriptor fields:**

| Field | Required | Description |
|---|---|---|
| `uri` | Yes | Path or reference to the data source |
| `payload_class` | Yes | The `MarketEvent` payload type this data contains (`Bar`, `Trade`, `Quote`, `BookSnapshot`, `BookDelta`, `OrderBookOrderEvent`, `PoolState`, `Funding`, `IVSurface`, `Nav`, `ListingEvent`, `HoldingsSnapshot`, `YieldCurve`, `GasEvent`, etc.) |
| `binding_type` | No | `"event_stream"` (replayed chronologically through the clock) or `"reference"` (loaded once, queried by timestamp). Default `"event_stream"`. See §4b. |
| `interval` | For Bar only | Bar duration: `1s`, `5s`, `1m`, `5m`, `15m`, `30m`, `1h`, `4h`, `1d`, `1w`, `1mo`. Absent for tick-level data. |
| `adjusted` | For Bar only | Whether prices are split/dividend-adjusted (`true`) or unadjusted (`false`). Default `false`. |
| `timestamp_field` | No | The field name in the source data that maps to `ts_event`. Default `"ts_event"`. |
| `ts_unit` | No | `nanoseconds` \| `microseconds` \| `milliseconds` \| `seconds`. Default `nanoseconds`. |

The engine uses the descriptor to know what it has **before reading any data**, determine what
can be derived vs. what is missing, run the Data Sufficiency Check (§10, step 7), and know what
bar intervals can be derived from the provided data.

The suite validates each binding against the instrument's **required-data manifest** (see
[contracts/market-data.md](contracts/market-data.md) §3) and rejects under-specified runs.

### Caller-provided data always wins (precedence rule)

When the caller binds a payload directly, that data is **authoritative** and the engine never
recomputes or overrides it with a derived value. This is a universal rule, not a per-payload
exception:

- If the caller binds `Bar` at a given `interval`, the engine uses those bars for that interval
  and does **not** derive its own bars at that interval (it still derives *other* intervals from
  finer raw data — §4a).
- If the caller binds a pre-computed indicator/feature series the strategy references, the engine
  uses the caller's values rather than recomputing them from derived bars.
- If the caller binds `Greeks`, `Nav`, or any other provide-or-derive payload, the caller's value
  is used and the engine's derivation is skipped for that value.

Derivation is strictly a **fallback** for what the caller did not provide. Anything the engine
derives is flagged `derived: true` with `source_class` lineage (§4a, output §7) so the caller can
persist it post-run and know exactly what it was derived from.

---

## 4b. Reference data vs. event streams

A binding is one of two kinds, set by `binding_type`:

- **`event_stream`** (default) — replayed chronologically through the simulation clock; the engine
  processes each event when the clock reaches its `ts_event`. Bars, trades, quotes, book events,
  funding, pool states, etc.
- **`reference`** — loaded once at run start into an indexed lookup structure and **queried by
  timestamp** during the run (`lookup(key, ts)`), never replayed as clock events. Used for data
  the engine needs to ask "what was the value as of `ts`?" at any moment, even when no event
  arrives at that moment.

Reference data may still vary over time (it is not "static forever"). S&P 500 membership changes;
credit ratings change. The distinction is *how it is consumed* — replayed vs. queried — not
whether it changes.

**Point-in-time safety for reference lookups (hard rule).** Reference data carries two timestamps
per entry: `effective_ts` (when the value took effect in the world) and `knowable_ts` (when the
value first became publicly knowable). A lookup at `current_ts` may only return entries where
`knowable_ts ≤ current_ts`. This prevents a backdated value — e.g. an index membership change
announced before it took effect, or a rating action timestamped to its effective date but
published later — from leaking future information into the simulation. Where a source provides only
one timestamp, `knowable_ts` defaults to `effective_ts`.

Typical reference bindings: `UniverseMembership`, `RollSchedule`, `ContinuousSeriesMetadata`,
exchange calendars, and (optionally) credit-rating / corporate-action history when supplied as a
lookup table rather than an event stream.

```jsonc
"ES@cme.future": {
  "bars_1d":       { "uri": "...", "payload_class": "Bar", "interval": "1d", "adjusted": false },
  "roll_schedule": { "uri": "...", "payload_class": "RollSchedule", "binding_type": "reference" }
}
```

### Entity-keyed reference data (`reference_bindings`)

Some reference data is keyed by an **entity** that is not a single instrument — an issuer (one
issuer backs many bonds), a universe, or a venue. These are declared in a top-level
`reference_bindings` block, separate from instrument-keyed `data.bindings`, so the data is bound
once and shared across every instrument that maps to that entity. Instruments declare their
`issuer_id` (see [contracts/instrument.md](contracts/instrument.md)); the engine resolves
issuer-level reference data through it.

```jsonc
"reference_bindings": {
  "issuer:FORD":  { "credit_spread":  { "uri": "...", "payload_class": "CreditSpread",       "binding_type": "reference" },
                    "rating_history": { "uri": "...", "payload_class": "CreditRatingEvent",   "binding_type": "reference" } },
  "universe:SP500": { "membership":   { "uri": "...", "payload_class": "UniverseMembership",  "binding_type": "reference" } },
  "venue:NYSE":   { "calendar":       { "uri": "...", "payload_class": "TradingSession",      "binding_type": "reference" } }
}
```

Keys are `"<entity_type>:<entity_id>"` where `entity_type ∈ { issuer, universe, venue }`. An
instrument resolves its entity-keyed data by its declared `issuer_id` / venue / universe membership.

---

## 4a. `derived_data` — inline bar derivation config

The event loop derives bars from raw input data **during** the event loop (not as a pre-pass),
preserving simulation realism. As raw ticks, order book events, and trade events arrive, the
engine accumulates them into in-progress bars at the intervals the run requires (necessity-driven
by default — see below), updating each one continuously until it closes.

**Why inline, not pre-computed:** pre-computing bars before the simulation starts breaks the
time-progression model. A bar derived inline is built only from events the simulation has
already processed — it is never computed from future events.

**Necessity-driven derivation (what gets derived):** the engine does **not** blindly derive every
standard interval. At compile time the suite walks the compiled strategy plan (features → models →
alpha → sizing → risk → execution) and collects every data reference together with the concrete
`(payload_class, interval)` it needs — e.g. `data:close` at the strategy's bar interval, an RSI
feature's `period` over that interval, a model's `input_window` lookback. That set, plus whatever
fidelity the fill model itself needs, is the **required derived-data set**. The engine derives
only those intervals/classes — nothing more. This bounds the memory cost of derivation (a naïve
"derive every interval for every instrument" approach is unbounded for large universes over long
windows).

**Bar boundary alignment (hard rule):** derived bars are **wall-clock aligned and start at :00**.
A 1m bar covers `[hh:mm:00, hh:mm:59.999…]`; a 1h bar starts on the hour; a 1d bar runs **UTC
midnight to UTC midnight**. Boundaries are never relative to the first observed event.

**Standard bar construction (hard rule):** a derived bar is built from **trade prints** —
`open` = first trade price in the interval, `high` = max, `low` = min, `close` = last,
`volume` = sum of trade sizes. When no `Trade` data exists for the instrument, the fallback source
is the **quote mid** `(bid + ask) / 2` sampled over the interval. The `source_payload_class` in the
config selects the source explicitly; absent that, the engine prefers `Trade`, then `Quote` mid.

**Completion and emission:** when a bar interval closes, the completed bar is emitted as a
`DerivedBar` event (see [contracts/market-data.md](contracts/market-data.md) §2.26) into the
strategy's feature pipeline with `derived: true` and `source_class` set to the raw payload
class it was derived from.

**Derivability direction (hard rule):** you can ALWAYS derive coarser intervals from finer
data (1m bars from ticks; 1h bars from 1m bars). You can NEVER derive finer intervals from
coarser data — you cannot synthesize 1s bars from 1m bars because that would fabricate intrabar
detail that does not exist. A strategy that requires a finer interval than any provided/derivable
source fails the Data Sufficiency Check (§10, step 7) with a `non_derivable_conflict`.

**Warmup gating (hard rule):** the strategy makes **no trading decisions** until, *during the run*,
the minimum required bars/lookback for its features have actually accumulated. Derivation runs
through the warmup window exactly as it does during the measured window — the strategy is simply
held out of decision-making until its first valid feature outputs exist. The first finer-interval
bar cannot be emitted until enough source events to close that interval have been seen.

**Caller-provided bars win:** if the caller provides bars directly for a given interval (e.g. daily
bars), those are used as-is for that interval and the engine does not derive its own at that
interval (precedence rule, §4). Derived bars at finer intervals are still computed from whatever
raw data is available.

**Adjusted-series derivation (Q12):** the split/dividend-adjusted series is provide-or-derive. If
the caller does not bind an adjusted series but binds the unadjusted series plus `CorporateAction`
data, the engine derives the adjusted series by applying corporate actions under a **declared
adjustment method** (`adjustment_method` below; default: multiplicative back-adjustment for splits,
proportional for dividends). The derived adjusted series is flagged `derived: true`. If neither an
adjusted series nor corporate-action data is provided, any signal that needs adjusted data fails
the Data Sufficiency Check.

**Derived-data store:** derived bars are stored in a separate internal store, distinct from the
raw input event stream. They are accessible to the strategy's feature pipeline but are not
mixed back into the raw event replay.

```jsonc
"derived_data": {
  "bar_derivation": {
    "intervals": "necessary",        // "necessary" | "all_standard" | ["1m","5m","1h","1d"] | "none"
    "source_payload_class": "Trade", // Trade | Quote | BookDelta | BookSnapshot — the raw class to derive from
    "adjustment_method": "back_adjust", // back_adjust | proportional | none  (adjusted-series derivation)
    "storage": "memory"              // memory | stream | both
  }
}
```

| Field | Description |
|---|---|
| `intervals` | `"necessary"` (default) derives only the intervals the compiled strategy and fill model actually require (necessity analysis above). `"all_standard"` derives 1s, 1m, 5m, 15m, 30m, 1h, 4h, 1d, 1w regardless of need (higher memory; use for exploratory work). An explicit list derives only the listed intervals. `"none"` disables derivation. |
| `source_payload_class` | Which raw payload class to accumulate bars from. Must be a tick-level class (Trade, Quote, BookDelta, BookSnapshot, OrderBookOrderEvent, PoolState). Absent → prefer `Trade`, then `Quote` mid. |
| `adjustment_method` | How to derive the adjusted series from unadjusted bars + `CorporateAction` data. `back_adjust` = multiplicative back-adjustment for splits, proportional for dividends (default). `proportional` = ratio-adjust all actions. `none` = do not derive an adjusted series. |
| `storage` | `"memory"` keeps derived bars in an internal in-memory store (fastest; discarded after run). `"stream"` emits them to the output stream as auxiliary records. `"both"` does both. Default: `"memory"`. |

---

## 4c. `signals` — exogenous-signal sources (optional, multi-source, unlimited)

The Exogenous-Signal Plane — news, sentiment, social attention, fundamentals, macro, on-chain
analytics, scheduled events, and raw media — enters here. Fully specified in
[contracts/signals.md](contracts/signals.md); this is the binding surface.

Signals are bound **per source**, so different platforms stay cleanly separated and a caller can
attach **as many sources as they want** (an NFT run may use one or two; a meme-coin run a dozen).
All optional — the engine never requires signals; they are additive inputs to `features`/`models`.

```jsonc
"signals": {
  "sources": {
    "social_x":      { "adapter": "injected:x_adapter",
                       "streams": { "attention": { "uri": "...", "payload_class": "SignalEvent",    "signal_id": "social_attention" },
                                    "posts":     { "uri": "...", "payload_class": "MediaReference", "modality": "Text" } } },
    "social_reddit": { "adapter": "injected:reddit_adapter",
                       "streams": { "forum_activity": { "uri": "...", "payload_class": "SignalEvent", "signal_id": "forum_activity" } } },
    "news":          { "adapter": "injected:news_adapter",
                       "streams": { "headlines": { "uri": "...", "payload_class": "DocumentSignal", "doc_type": "news" } } },
    "onchain":       { "adapter": "injected:onchain_adapter",
                       "streams": { "exchange_flows": { "uri": "...", "payload_class": "EntityMetric", "metric_id": "exchange_netflow" } } }
  }
}
```

Two non-negotiables for this plane (detail in [contracts/signals.md](contracts/signals.md)):

- **Never a fill source.** Signals change what the strategy *decides*, never the price the market
  gives it. Fills come only from the Market-Data Plane (§4).
- **`ts_available` look-ahead.** Exogenous look-ahead is enforced by `ts_available` (when a
  strategy could first know the data), not `ts_event` (when it happened). A merger announced at X
  but effective at Y>X is actionable at X. Streams missing `ts_available` default it to `ts_event`.

**Raw media** (`MediaReference`, `DocumentSignal` with a `uri`) is carried as a **point-in-time
reference only** — the suite never loads or parses the bytes. The injected AI endpoint resolves
the URI and performs multimodal inference (see [contracts/strategy.md](contracts/strategy.md) §8
`context_inputs`).

---

## 4d. `cohorts` — market-wide / universe-wide data sources (optional)

A `scanner` universe ([contracts/strategy.md](contracts/strategy.md) §6.3) surveys a **cohort**:
a large, membership-changing universe whose members cannot all be enumerated in advance (every DEX
pair on a chain, every NFT collection in a market, every new equity listing, every new prediction
market). You cannot bind these the per-instrument way in §4 — so a cohort is bound as a **single
market-wide source** that **materializes instruments dynamically** as they appear.

This is engine-agnostic: each materialized instrument still routes to its engine by
`price_formation`. A Solana-DEX cohort yields Engine-B pools; an IPO cohort yields Engine-A
equities; an NFT-mint cohort yields Engine-G collections.

```jsonc
"cohorts": {
  "solana_dex_pairs": {
    "adapter": "injected:solana_pairs_adapter",   // caller-owned; emits new instruments + their data
    "instrument_template": {                        // common fields every member inherits
      "price_formation": "AMM",
      "capabilities": ["HasPoolReserves", "HasConcentratedLiquidity", "HasGasCost", "HasExogenousSignals"],
      "currency": "USD",
      "settlement": "OnChain"
    },
    "streams": {                                    // per-member data the adapter provides
      "pool_states": { "payload_class": "PoolState" },
      "swaps":       { "payload_class": "SwapEvent" },
      "gas":         { "payload_class": "GasEvent" }
    },
    "signals_from": ["onchain", "social_x", "news"]  // exogenous sources (§4c) keyed to each member by entity_id
  }
}
```

**Mechanics:**

- The cohort **adapter** (caller-owned, like every other adapter) emits, for each member as it
  appears: the member's **instrument definition** (the `instrument_template` fields plus per-asset
  overrides the feed supplies — symbol, addresses, tick/lot, creator metadata) and the member's
  bound data streams.
- Members are **materialized point-in-time**: a member exists in the run only from the moment its
  first data is knowable (`ts_available`), preventing survivorship bias by construction — you never
  see an asset before it launched.
- The `scanner` universe's filters (§6.3) then decide which materialized members are **admitted**
  (become candidates); the `instrument_template` keeps you from repeating common fields across
  thousands of members, while the feed supplies per-asset specifics.
- Exogenous signals for cohort members come from the normal `signals` sources (§4c), keyed to each
  member by `entity_id`/`instrument_id` — so a per-asset social score or top-10-holder concentration
  is available to the scanner filters and to the strategy.

Cohorts are entirely optional — a run with only named `instruments` (§4) needs none.

---

## 5. `account` — injected ledger (optional)

The caller's portfolio/accounting model. Required **only** if the strategy uses account-relative
sizing, account-relative risk, or margined instruments (perps/options). Omit it for a pure
absolute-sizing strategy.

```jsonc
"account": {
  "port": "injected:my_ledger",      // or "reference" for the shipped convenience adapter
  "config": {
    "base_currency": "USD",
    "starting_balance": 100000,      // lives in the ACCOUNT, not the suite
    "settlement": "T+2",             // caller's accounting rules
    "margin_model": "..."
  }
}
```

The `Account` port the caller implements:

```rust
trait Account {
    fn equity(&self, ccy: CurrencyCode, ts: Timestamp) -> Decimal;
    fn buying_power(&self, ts: Timestamp) -> Decimal;
    fn position(&self, instrument: &InstrumentId) -> Position;
    fn collateral(&self, ts: Timestamp) -> Decimal;       // for margin/liquidation
    fn apply_fill(&mut self, fill: &Fill);                // suite reports simulated fills here
}
```

The suite **queries** it for sizing/risk/margin inputs and **reports** simulated fills to it. It
never defines the accounting internals. The port must be deterministic (no hidden I/O).

---

## 6. Injected ports: ai_endpoints, components

The remaining caller-owned dependencies, wired per run:

```jsonc
"ai_endpoints": {
  "news-sentiment": {
    "adapter":       "onnx",
    "uri":           "...",
    "version":       "3.2.1",
    "endpoint_type": "model",
    "scope":         "data_scoped"
  },
  "listing-scorer": {
    "adapter":       "injected:my_agent_runtime",
    "version":       "1.0.0",
    "endpoint_type": "agent_runtime",
    "scope":         "archived_tools_only"
  }
},
"components": {
  "my_factor_model": { "kind": "wasm", "uri": "..." },    // see Component Registry trust model
  "top_n_by_volume": { "kind": "builtin" }
}
```

- `ai_endpoints` resolve the `endpoint_id@version` referenced in the strategy's `ai_endpoints`
  block (see [contracts/model.md](contracts/model.md)). Each entry declares an `adapter` (how the
  suite calls the endpoint), an `endpoint_type` (informational: `model | agent_runtime | pipeline`),
  and a `scope` (safety-critical: `data_scoped | archived_tools_only | live_external`). Endpoints
  declaring `live_external` are rejected at validation for all backtest runs.
- `components` bind any custom registry components the strategy references (built-in ones need no binding).

---

## 7. `output` — what the suite emits

The suite's primary output is the **TradeRecord stream** plus per-event valuation marks. Portfolio
aggregation and metrics are computed downstream (by the caller, or by an optional analytics layer
over the injected `Account`).

```jsonc
"output": {
  "mode": "stream",                  // stream (incremental) | batch (returned at end)
  "emit": ["trades", "marks", "model_lineage", "warnings", "derived_data", "metrics?"]
}
```

`derived_data` emits every value the engine derived (bars, adjusted series, greeks, NAV, etc.)
tagged `derived: true` with `source_class` lineage, so the caller can persist it and know exactly
what raw data it came from (precedence rule, §4). Anything the caller bound directly is never
re-emitted as derived.

### TradeRecord

```
TradeRecord {
  ts_decision, ts_fill,
  instrument_id,

  setup: {                 // the trade setup
    side, order_type, time_in_force, limit_price, max_slippage_bps, slicing
  },
  sizing: {                // the sizing decision + provenance
    method, size_units, notional, inputs_used   // e.g. equity, alpha confidence, model output
  },
  execution: {             // the execution information
    fill_price, fill_qty, fees, slippage, gas, partial, rejected_reason, queue_info
  },
  trigger: {               // explainability: what fired this trade
    insight_id, feature_values, model_outputs
  },
  account_context: {       // present only if an Account port was injected
    equity, position_before, position_after
  }
}
```

This is exactly "sizing and trading setup and information for each time the strategy goes to make
a trade." Metrics (Sharpe, drawdown, equity curve) are derived from these records + the injected
account, in [contracts/metrics.md](contracts/metrics.md) (TBD) — not assumed by the core.

---

## 8. `parameters` — values or sweep

The strategy declares the tunable *space*; the Run Request supplies concrete values, or a sweep
the run queue expands.

```jsonc
// single run
"parameters": { "rsi_period": 14, "oversold": 30 }

// sweep — the queue expands this into many runs
"parameters": {
  "rsi_period": { "sweep": "grid",   "values": [10, 14, 20] },
  "oversold":   { "sweep": "range",  "min": 20, "max": 35, "step": 5 }
}
```

Sweep search method (grid / random / Bayesian) and queue behavior: see `runner.md` (TBD) and
OD-2. Every swept value must fall within the strategy's declared parameter bounds.

---

## 9. `execution_defaults`

Run-wide defaults for the realism knobs, overridable per instrument in the Instrument contract:

```jsonc
"execution_defaults": {
  "latency": "100ms",                 // submission → fill delay
  "slippage_model": "size_vs_volume", // default when no book depth
  "intrabar_fill": "pessimistic"      // optimistic | pessimistic | mid  (stop/limit triggers)
}
```

---

## 10. Validation order

At run start, before any event is processed:

1. **Schema** — Run Request and Strategy JSON parse and type-check.
2. **Parameter bounds** — supplied values/sweeps lie within the strategy's declared space.
3. **Instruments** — capabilities valid; `price_formation` routes to a real engine.
4. **Data manifest** — each instrument's required payloads are bound (§4); else `ManifestViolation`.
5. **Port presence** — if the strategy uses account-relative sizing/risk, margin, or
   `ai_endpoints`, the corresponding injected port is present; else a typed error. If the strategy
   references a `signal:<id>` or an endpoint `context_inputs.from` source, the corresponding
   `signals` source/stream is bound (§4c); else `SignalNotBound`. Endpoints declaring
   `live_external` scope are rejected at this step with `ScopeViolation`.
6. **Capability/order-type** — execution order types are valid for each instrument's engine.
7. **Data sufficiency** — per-engine minimum data requirements are checked against the declared
   descriptors; else `DataSufficiencyError` (see below).
8. **Plan wiring** — if the `strategy` is a Plan, every `universe.from` names an existing strategy
   of a compatible role, the wiring is acyclic (a DAG), and `account_mode`/`conflict_policy` are
   resolvable; else a typed `PlanWiringError` (see [contracts/plan.md](contracts/plan.md) §8).
9. **Cohort/scanner** — if a `scanner` universe is used, its `cohort` names a bound cohort source
   (§4d) and its filter `field`s resolve to bound market-data/signal classes; else a typed error.

Validation is fail-loud: a violation rejects the run with a precise, typed error rather than
producing results.

### `DataSufficiencyError`

Produced by step 7 when the bound data does not meet per-engine minimum requirements:

```
DataSufficiencyError {
  instrument_id:        InstrumentId,
  engine:               EngineType,
  missing_required:     Vec<PayloadClass>,   // absent payload classes that cannot be derived
  non_derivable_conflict: Vec<ResolutionConflict>, // strategy needs finer resolution than provided
  degraded_fidelity:    Vec<FidelityWarning>, // run can proceed but fidelity is lower than optimal
}

ResolutionConflict {
  required_interval:  Duration,   // strategy feature requires this bar interval
  available_interval: Duration,   // only this (coarser) interval was provided
  // Derivation direction: finer CANNOT be derived from coarser.
  // e.g. 1s bars needed but only 1d bars provided → non_derivable_conflict
}

FidelityWarning {
  missing_payload_class: PayloadClass,
  fidelity_impact:       String,  // human-readable description of fidelity reduction
}
```

**Derivability rule (enforced in step 7):** coarser intervals can always be derived from finer
data (1d bars from 1m bars). Finer intervals can NEVER be derived from coarser data (1s bars
from 1m bars). A `non_derivable_conflict` is a hard rejection — the run cannot proceed.
`degraded_fidelity` entries are warnings; the run proceeds at reduced fidelity, disclosed in
results.

**Per-engine absolute minimums** (absence of these is a hard `DataSufficiencyError`):

| Engine | Absolute minimum to run |
|---|---|
| A (CLOB) | Any one of: `Trade`, `Quote`, `BookSnapshot`, `Bar` with timestamps. Without at least one price source, rejected. |
| B (AMM) | `PoolState` stream with timestamps. |
| C (NAV) | `Nav` stream OR (`HoldingsSnapshot` + underlying price data for all holdings). |
| D (DEALER) | Any one of: `Bar`/`Mark` with clean prices, `YieldUpdate`, `YieldCurve`. |
| E (CHAIN) | `IVSurface` for the underlying AND underlying price data (`Bar`/`Quote`/`Trade`). Both required — either alone is rejected. |
| F (OTC) | All underlyings referenced by the payoff component must have price data bound AND the payoff component must be registered in `components`. |
| G (MARKETPLACE) | `ListingEvent` stream with timestamps. |
| H (ORACLE) | Price/probability stream (`Bar` or `Quote` for YES/NO tokens) AND `Resolution` event stream. `MarketLifecycleEvent` strongly recommended. |

**Capability-gated data that triggers `DataSufficiencyError` when missing:**

- Engine A, `HasCorporateActions` set but no `CorporateAction` stream → error.
- Engine A, `HasFunding` set but no `Funding` stream → error.
- Engine A, `HasBorrowRate` set and strategy shorts but no `BorrowRate` stream → error.
- Engine B, `HasGasCost` set but no `GasEvent` stream and no static gas config in `execution_defaults` → error.
- Engine D, `HasCoupon` set but no `Coupon` stream → error.
- Engine D, `HasCreditRisk` set but no `CreditSpread` or `CreditRatingEvent` stream → error.
- Engine G, `HasFloor` set but no `ComparableMarkEvent` stream and strategy holds positions → error.
- Engine G, `HasGasCost` set but no `GasEvent` stream and no static gas config → error.
- Engine H, no `Resolution` stream → error (cannot simulate settlement).

---

## 11. Invariants

1. **No assumed portfolio.** Account state is injected, never owned (ADR-0010).
2. **Stateless suite.** Nothing persists across runs; `request_id`/`strategy_id` are echoed only.
3. **Deterministic.** Given identical Run Request + injected ports (themselves deterministic),
   results are byte-identical, regardless of thread count.
4. **Point-in-time.** All bindings and ports may only expose `ts_event ≤ current_ts`. For
   `reference` bindings the constraint is `knowable_ts ≤ current_ts` (§4b) — a backdated value
   may not leak in before it was publicly knowable.
5. **Feed-agnostic strategy.** The same Strategy JSON runs here and live; only the Run Request
   (and the live feed) differ.
6. **Caller-provided data wins.** Directly bound data is authoritative; the engine derives only
   what was not provided, and flags everything it derives (§4).

---

## 12. Open questions

- **Sweep search ownership (OD-2):** does grid/random/Bayesian search live in the suite's run
  queue or the platform?
- **Reference `Account` adapter scope:** how full-featured is the optional shipped ledger
  (single-currency cash only, or margin/multi-currency)?
- **Streaming result schema:** exact wire format for incremental `output.mode = stream`.
- **Per-run vs per-sweep injected ports:** are ports shared across a sweep's runs or re-created?
