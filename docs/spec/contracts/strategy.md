# Contract Spec: Strategy

**Type:** Data (JSON schema contract)  
**Status:** ✅ Defined  

A **strategy** is the full, declarative path from data to a trade: universe → features →
alpha/signal → sizing → risk → order placement. It is expressed in **exactly one format:
JSON.** There is no second authoring format, no embedded code, and no monolithic callback.

The suite **never stores strategies.** A strategy JSON is passed in at runtime, validated,
compiled into an internal execution plan, executed, and discarded. Storage, versioning,
user ownership, and selection all live in the trading platform. See
[ADR-0005](../../adr/0005-strategy-not-stored-suite-is-a-library.md) and
[ADR-0004](../../adr/0004-strategy-json-pipeline.md).

---

## 1. Resolving the `on_event` question (decisively)

Earlier design notes floated a "monolithic `on_event` escape hatch for power users." **That
is rejected.** Here is the final rule:

- **`on_event` is an internal engine mechanism, not an authoring surface.** The engine has an
  event loop; strategy authors never write into it directly.
- **The only way to author a strategy is the declarative JSON pipeline.** Every strategy is
  a wiring of typed stages.
- **Extensibility is via the component registry, not via free-form code.** When a built-in
  component is insufficient, you register a *named, reusable, typed component* (an indicator,
  an alpha function, a sizing function, etc.) and reference it by ID from the JSON. The
  registry holds the code; the JSON holds the wiring. **What a "component" is, and the trust
  tiers (built-in / native / WASM sandbox) for running custom ones, are fully defined in
  [component-registry.md](../component-registry.md).**

This gives the user's stated goal exactly: a clean pipeline that hands each stage what it
needs, and an escape hatch that is **controlled and reusable** (a registered component) rather
than an arbitrary tangled blob. A custom component is still a pipeline node — it cannot reach
around the pipeline to mutate global state or read future data.

> **One format. One pipeline. Code lives in registered components, never in the strategy JSON.**

---

## 2. Strategy vs. Run Request (a critical boundary)

These are two different documents. Keeping them separate is what lets the *same* strategy run
unchanged in backtest and live.

| | **Strategy JSON** | **Run Request** ([run-request.md](../run-request.md)) |
|---|---|---|
| Contains | Reusable logic: universe, features, ai_endpoints, alpha, sizing, risk, execution; declared *parameter space* | Concrete run context: which strategy, data source bindings, date range, starting capital, *parameter values or sweep*, RNG seed, accounting currency |
| Lifetime | Portable; identical across backtest and live | Per-invocation |
| Owns sizing logic? | Yes | No |
| Owns parameter *space*? | Yes (declares types + ranges) | Picks concrete *values* or a sweep over the declared space |

A strategy declares *that* `rsi_period` is an int in `[5, 30]`; the run request decides whether
this run uses `14`, or sweeps the whole range across the run queue. The suite receives both at
runtime.

---

## 3. Pipeline stages

Data flows through named stages. Each stage produces **named values**; later stages bind to
those names. This is the "give each stage what it needs" model — explicit bindings, no globals.

```
universe      → the set of instruments in scope (static or a registered dynamic selector)
features      → indicators / derived series from PIT data (vectorizable, pre-computable)
ai_endpoints  → AI inference / agent nodes (emit named outputs + confidence)
alpha         → insights: direction + confidence per instrument, from features/endpoint outputs
sizing        → target positions (fixed, algorithmic, or derived from alpha/endpoint output)
risk          → constraints/stops/kill-switches that can veto or resize targets
execution     → orders: order type, TIF, slicing, slippage tolerance (capability-gated)
```

Evaluation order is fixed: `universe → features → ai_endpoints → alpha → sizing → risk → execution`.
`features` and `ai_endpoints` outputs are addressable by every downstream stage; a later stage may
not read a stage that runs after it.

---

## 4. Top-level schema

```jsonc
{
  "schema_version": "1.0",
  "strategy_id": "user-supplied-id",      // echoed in results; NOT persisted by the suite
  "name": "RSI + News Sentiment Long",
  "description": "Long when oversold and sentiment is positive",

  "parameters": { /* §5  declared parameter SPACE */ },
  "universe":   { /* §6 */ },
  "features":   [ /* §7 */ ],
  "ai_endpoints": [ /* §8  AI inference */ ],
  "alpha":      { /* §9 */ },
  "sizing":     { /* §10  first-class; not always fixed */ },
  "risk":       { /* §11 */ },
  "execution":  { /* §12  capability-gated */ },

  "settings":   { /* §13  warmup, determinism, accounting */ }
}
```

### Value references

Stages bind inputs by name using a typed reference grammar (these are JSON string values, not
a separate format):

| Reference | Resolves to |
|---|---|
| `"feature:rsi14"` | a named feature output |
| `"ai:sentiment.value"` | a named AI endpoint output field |
| `"ai:sentiment.confidence"` | an AI endpoint confidence output |
| `"signal:news_volume"` | an exogenous signal stream on the current instrument (see [signals.md](signals.md)) |
| `"data:close"` | a raw market-data field on the **current** instrument |
| `"param:rsi_period"` | a declared parameter's value (bound by the run request) |
| `"portfolio:position"` | current position state for the instrument |

#### Cross-instrument references (analyze one asset, trade another)

By default a reference resolves against the **current** instrument the pipeline is evaluating. To
read **another instrument's** data, qualify the reference with that instrument's id:

| Reference | Resolves to |
|---|---|
| `"data:BTC-USD@coinbase.spot.close"` | a market-data field on a **named reference instrument** |
| `"signal:BTC-USD@coinbase.spot.news_sentiment"` | an exogenous signal on a named reference instrument |
| `"feature:btc_lstm@BTC-USD@coinbase.spot"` | a feature computed on a named reference instrument |

The named instrument must be present in the run (`instruments`) and bound with data, but it does
**not** have to be in the traded `universe` — see **watch-vs-trade** (§6). This is what lets a
strategy trade ETH while a model forecasts BTC: the BTC instrument is watch-only (data + signals
bound, never traded), and the ETH pipeline reads `model:btc_forecast@BTC-USD@...` to decide its
ETH trades. Cross-instrument references obey the same point-in-time rule (`ts_event`/`ts_available`
≤ `current_ts`).

> **Grammar note.** Instrument ids themselves contain `@` and `.` (e.g.
> `BTC-USD@coinbase.spot`), so the qualified form must disambiguate the field from the id. The
> reference grammar resolves this by treating the **trailing known field name** as the field and
> the remainder as the instrument id, and field names are a closed set. The exact delimiter token
> is a finalize-at-implementation detail (Q-STRAT-REF) — a bracketed form like
> `data:[BTC-USD@coinbase.spot].close` is the fallback if the trailing-field rule proves
> ambiguous.

---

## 5. `parameters` — declared search space

The strategy declares *what is tunable*; the run request supplies values or a sweep.

```jsonc
"parameters": {
  "rsi_period":      { "type": "int",   "default": 14, "min": 5, "max": 30, "step": 1 },
  "oversold":        { "type": "float", "default": 30, "min": 10, "max": 40 },
  "sentiment_floor": { "type": "float", "default": 0.6, "min": 0.0, "max": 1.0 }
}
```

Anything elsewhere in the JSON may reference `"param:<name>"`. The run queue sweeps these for
optimization; the strategy structure itself never changes during a sweep.

---

## 6. `universe`

The `universe` is the set of instruments the strategy **trades**. It is a subset of the
instruments in the run.

### 6.0 Watch-vs-trade (the run set vs. the traded set)

- **`instruments`** (Run Request) = **every instrument with data bound** — both the ones being
  traded and any **watch-only reference instruments** used purely for analysis.
- **`universe`** (Strategy) = the subset actually **traded**.
- An instrument in `instruments` but **not** in `universe` is **watch-only**: its market data and
  signals are available to features/models/alpha via cross-instrument references (§4), but the
  strategy never places an order on it. (Trade ETH while forecasting BTC: BTC is watch-only.)

### 6.1 `static`

```jsonc
"universe": {
  "type": "static",
  "instruments": ["AAPL@nasdaq.equity", "BTC-USD@coinbase.spot"]
}
```

### 6.2 `dynamic` (selector over a known set)

A registered selector picks from instruments already known to the run; point-in-time membership
is gated by `UniverseMembership` (see [market-data.md](market-data.md) §2.25):

```jsonc
"universe": {
  "type": "dynamic",
  "selector": { "ref": "top_n_by_volume", "params": { "n": 50, "lookback": "30d" } }
}
```

### 6.3 `scanner` (universe-wide screen, engine-agnostic)

A **scanner** surveys a whole **cohort** — a potentially large, membership-changing universe —
and admits the instruments that meet point-in-time filter criteria. It is **not** limited to
"new" assets and **not** crypto-specific: the same mechanism screens DEX pairs on any chain, NFT
collections worldwide, IPOs / new equity listings, new prediction markets — any market where you
survey many assets instead of watching one. Newness is merely one possible filter, not a
requirement.

```jsonc
"universe": {
  "type": "scanner",
  "cohort": "solana_dex_pairs",          // a cohort data source bound in the Run Request (§4d)
  "filters": [                            // point-in-time admission criteria (market data + signals)
    { "field": "data:volume_24h",                "op": ">=", "value": "param:min_vol" },
    { "field": "signal:market_cap",              "op": ">=", "value": "param:min_mcap" },
    { "field": "signal:top10_holder_pct",        "op": "<=", "value": "param:max_concentration" },
    { "field": "signal:social_score",            "op": ">=", "value": "param:min_social" }
  ],
  "max_active": 500,                       // optional cap on simultaneously-tracked instruments
  "rank": { "by": "signal:social_score", "order": "desc" }   // optional cross-sectional ranking
}
```

- Instruments are **materialized from the cohort data source** as they appear (see
  [run-request.md](../run-request.md) §4d); they need not be enumerated in advance.
- Filters are evaluated point-in-time (`ts_available ≤ current_ts` for signal fields) so an asset
  is admitted only once it actually satisfies the criteria with knowable data.
- A scanner **selects candidates; it does not by itself decide entries.** Admission produces a
  candidate set; *when* to actually trade an admitted asset is a separate concern, handled either
  by this strategy's own downstream stages or — preferably for "screen then time the entry" — by a
  separate **entry strategy** wired in a **Plan** (see [plan.md](plan.md)). This is what prevents
  buying the top just because a wide threshold was met.

---

## 7. `features`

Indicators and derived series. Built-in indicators are referenced by `type`; custom ones by a
registry `ref`. Outputs are named for downstream binding. These are designed to be
**pre-computed/vectorized** before the event loop where possible (the speed pattern).

```jsonc
"features": [
  { "id": "rsi14", "type": "RSI", "input": "data:close", "params": { "period": "param:rsi_period" } },
  { "id": "vol20", "type": "STDDEV", "input": "data:close", "params": { "period": 20 } },
  { "id": "embed_news", "ref": "news_embedder", "input": "signal:news_text" }
]
```

---

## 8. `ai_endpoints` — AI inference

AI endpoints are **external by default** — pre-trained models, agent runtimes, or pipelines.
The strategy *calls* an endpoint by ID, passes it the data it needs each time inference runs,
and binds the outputs. The suite never stores weights or agent code. See
[model.md](model.md) and [ADR-0006](../../adr/0006-model-inference-and-training.md).

### 8.1 Inference node

```jsonc
"ai_endpoints": [
  {
    "id": "sentiment",                       // node name; outputs addressed as ai:sentiment.*

    "endpoint_id": "news-sentiment",         // logical endpoint the platform resolves at runtime
    "version":     "3.2.1",                  // pinned for reproducibility (required)
    "endpoint_type": "model",                // model | agent_runtime | pipeline (informational)
    "scope":       "data_scoped",            // data_scoped | archived_tools_only | live_external
                                             //   live_external → rejected at validation for backtests

    "inference_fn": "predict_proba",         // which function/signature on the endpoint to call

    "inputs": {                              // feature bindings: endpoint-input-name → value ref
      "embeddings":    "feature:embed_news",
      "price_context": "feature:vol20"
    },
    "input_window": { "lookback": "7d" },    // PIT window of data fed each inference (≤ current ts)

    "context_inputs": {                      // OPTIONAL — PIT exogenous bundles assembled per call (§8.2)
      "recent_posts": { "from": "social_x.posts", "lookback": "1h", "max_items": 200 },
      "headlines":    { "from": "news.headlines", "lookback": "24h" }
    },

    "frequency": { "every": "1d", "at": "session_open" },
    // alternatives:
    //   { "on_event": "Bar" }          run on every bar
    //   { "every_n_events": 5 }        run every 5th event
    //   { "cron": "0 0 * * MON" }      schedule-based

    "outputs": {
      "value":      "sentiment_score",       // primary output name (→ ai:sentiment.value)
      "confidence": "sentiment_conf"         // confidence output name (→ ai:sentiment.confidence)
    },

    "fallback": {                            // behavior if inference fails / is unavailable / stale
      "policy": "use_last",                  // use_last | use_default | skip_trade | halt
      "default_value": 0.0,
      "max_staleness": "2d"                  // older than this → treat as failure
    },

    "determinism": { "seed": 42 }            // seeded inference for reproducibility
  }
]
```

**Required fields:** `id`, `endpoint_id`, `version`, `inference_fn`, `inputs`, `frequency`,
`outputs.value`, `fallback.policy`. Everything else is optional with defaults.

**Look-ahead safety:** `inputs` and `input_window` may only reference data with
`ts_event ≤ current_ts`. The suite enforces this; an endpoint literally cannot be handed future
data.

---

### 8.2 `context_inputs` — point-in-time exogenous bundles (multimodal inference)

Beyond structured feature `inputs`, an endpoint node may request **point-in-time context
bundles** of exogenous data assembled fresh at each inference call. This is how an endpoint
runs over the news, social posts, images, and videos that existed at time *t* — essential for
meme-coin, NFT, listing-marketplace, and event-driven strategies.

Each `context_inputs` entry names a bound signal source/stream (from the Run Request `signals`
block, see [run-request.md](../run-request.md) §4c) and a `lookback` window:

```jsonc
"context_inputs": {
  "recent_posts": { "from": "social_x.posts",   "lookback": "1h", "max_items": 200 },
  "headlines":    { "from": "news.headlines",    "lookback": "24h" },
  "flows":        { "from": "onchain.exchange_flows", "lookback": "24h" }
}
```

At each inference call the suite collects, from each named stream, every record with
`ts_available ≤ current_ts` inside the window (capped by `max_items`), and hands the bundle to
the injected AI endpoint alongside the structured `inputs`. For `MediaReference`/`DocumentSignal`
items the bundle carries **references** (URIs + modality + any pre-extracted features); the
**injected endpoint resolves the URIs and loads the raw bytes** — the suite never parses media.
Look-ahead is enforced for the bundle (nothing with `ts_available > current_ts` appears), and the
bundle is assembled deterministically. Full contract: [signals.md](signals.md) §5.

---

## 9. `alpha` — signal generation

Turns features and model outputs into **insights**: a direction and confidence per instrument.
Rules are typed expressions over named values (comparisons/arithmetic/boolean) — *not* code.
Complex logic belongs in a registered alpha component (`ref`), not a giant expression.

```jsonc
"alpha": {
  "insights": [
    {
      "id": "long_oversold_positive",
      "when": "feature:rsi14 < param:oversold && ai:sentiment.value > param:sentiment_floor",
      "direction": "long",
      "confidence": "ai:sentiment.confidence",      // expression or a bound value
      "horizon": "5d"
    },
    {
      "id": "exit_overbought",
      "when": "feature:rsi14 > 70",
      "direction": "flat"
    }
  ]
}
```
Registered component form:
```jsonc
"alpha": { "ref": "my_factor_model", "params": { "...": "..." }, "inputs": { "...": "..." } }
```

---

## 10. `sizing` — first-class, not always fixed

Sizing converts insights into **target positions**. It is a full stage, not an afterthought,
and supports fixed, algorithmic, and alpha/model-derived methods.

```jsonc
"sizing": {
  "method": "volatility_target",   // see table below
  "params": { "target_vol": 0.15, "vol_input": "feature:vol20", "max_position_pct": 10 }
}
```

| `method` | Sizing comes from |
|---|---|
| `fixed_units` | A constant quantity |
| `fixed_fractional` | A fixed % of equity per position |
| `volatility_target` | Algorithmic: scale so position vol ≈ target |
| `from_alpha` | Proportional to insight confidence/score |
| `from_model` | Directly from an AI endpoint output (e.g. `ai:position_sizer.value`) |
| `kelly` | Kelly fraction from win/loss estimates |
| `equal_weight` | Split capital across active insights |
| `ref:<id>` | A registered custom sizing component |

`from_alpha` and `from_model` are the explicit answer to "sizing may derive from alpha/model
output." A position sizer can itself be a model node in §8.

---

## 11. `risk`

Constraints and stops that can veto or resize targets after sizing. Evaluated every event.

```jsonc
"risk": {
  "constraints": [
    { "type": "max_position_pct",    "value": 10 },
    { "type": "max_gross_exposure",  "value": 1.5 },
    { "type": "stop_loss_pct",       "value": 8,  "scope": "per_position" },
    { "type": "take_profit_pct",     "value": 25, "scope": "per_position" },
    { "type": "max_drawdown_pct",    "value": 20, "action": "flatten_and_halt" }
  ],
  "kill_switch": { "on": "max_drawdown_pct", "action": "flatten_and_halt" }
}
```

---

## 12. `execution` — order placement (capability-gated)

Turns risk-approved targets into orders. Order types are validated against each instrument's
capabilities — e.g. a `limit` order on an AMM instrument is a contract error, caught at
compile time. See [engines/README.md](../engines/README.md) order-type matrix.

```jsonc
"execution": {
  "default_order_type": "limit",       // market | limit | stop | stop_limit | swap
  "limit_offset_bps": 5,
  "time_in_force": "DAY",              // GTC | IOC | FOK | DAY
  "slicing": { "method": "twap", "duration": "5m" },   // none | twap | vwap
  "max_slippage_bps": 50,             // AMM swaps
  "per_asset_overrides": {
    "AMM": { "default_order_type": "swap", "max_slippage_bps": 30 }
  }
}
```

### Reacting to fills

Even declaratively, strategies need to respond to fills, partial fills, and rejections.
These are expressed as risk/execution rules over `portfolio:*` state (e.g. a `stop_loss_pct`
constraint reacts to the filled position), *not* as imperative callbacks. The precise
declarative fill-reaction model is an open question (§16).

---

## 13. `settings`

```jsonc
"settings": {
  "warmup": "60d",                 // history needed before the official start (indicator warmup)
  "rng_seed": 12345,               // deterministic randomness (or supplied by run request)
  "accounting_currency": "USD"     // base currency for P&L aggregation
}
```

---

## 14. Invariants (enforced for every strategy, all stages)

1. **Declarative only.** No code in the JSON. Logic beyond typed expressions must be a
   registered component referenced by ID.
2. **Pure & deterministic.** No wall-clock, no I/O, no unseeded randomness. Same inputs →
   same outputs. This enables reproducibility and parallel sweeps.
3. **Point-in-time.** Every binding (features, model inputs, training data) may only read
   `ts_event ≤ current_ts`.
4. **Capability-gated actions.** Execution order types are validated against instrument
   capabilities at compile time.
5. **Feed-agnostic.** A strategy never knows whether data is historical replay or live. Same
   JSON runs in backtest and (in the platform) live.
6. **Stateless authoring, stateful execution.** The JSON is static; the compiled plan holds
   serializable state (indicator buffers, positions, model context) for determinism/resume.

---

## 15. Composition: multiple strategies and the Plan

A single Strategy is always **one flat pipeline** — it is never nested inside another strategy
(nesting would turn the JSON into a recursive program with control flow, which §14.1 forbids).
When you need several strategies — a screen that hands candidates to one or more entry strategies,
an exit manager, or simply two unrelated strategies running at once — they are composed in a
**Plan**: a container that holds multiple strategies and wires them by **data-flow**
(one strategy's output becomes another's `universe`/signal input), not by nesting or function
calls. The Run Request's `strategy` field may be a single Strategy or a Plan; a single Strategy is
the degenerate one-node Plan. Full spec: [plan.md](plan.md).

---

## 16. Open questions (carried to design discussion)

- **Expression vs. component boundary:** how much logic is allowed in `when`/expressions
  before it must become a registered component? (Risk: JSON becoming a programming language.)
- **Declarative fill reactions:** the precise model for reacting to partial fills / rejections
  without imperative callbacks.
- **AI endpoint registry & reproducibility:** how the platform guarantees `endpoint_id@version`
  resolves to identical weights/code at backtest time and months later.

> **Resolved since first draft:** multi-strategy portfolios → the **Plan** layer ([plan.md](plan.md),
> OD-6); component-registry trust model → tiered built-in/native/WASM ([component-registry.md](../component-registry.md),
> ADR-0011); Run Request schema → [run-request.md](../run-request.md); dynamic-universe
> survivorship → point-in-time `UniverseMembership` ([market-data.md](market-data.md) §2.25).
