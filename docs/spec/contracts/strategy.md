# Contract Spec: Strategy

A **strategy** is the full, declarative path from data to a trade: universe → features →
alpha/signal → sizing → risk → order placement. It is expressed in **exactly one format:
JSON.** There is no second authoring format, no embedded code, and no monolithic callback.

The suite **never stores strategies.** A strategy JSON is passed in at runtime, validated,
compiled into an internal execution plan, executed, and discarded. Storage, versioning,
user ownership, and selection all live in the trading platform. See
[ADR-0005](../adr/0005-strategy-not-stored-suite-is-a-library.md) and
[ADR-0004](../adr/0004-strategy-json-pipeline.md).

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
  registry holds the code; the JSON holds the wiring.

This gives the user's stated goal exactly: a clean pipeline that hands each stage what it
needs, and an escape hatch that is **controlled and reusable** (a registered component) rather
than an arbitrary tangled blob. A custom component is still a pipeline node — it cannot reach
around the pipeline to mutate global state or read future data.

> **One format. One pipeline. Code lives in registered components, never in the strategy JSON.**

---

## 2. Strategy vs. Run Request (a critical boundary)

These are two different documents. Keeping them separate is what lets the *same* strategy run
unchanged in backtest and live.

| | **Strategy JSON** | **Run Request** *(separate spec, TBD)* |
|---|---|---|
| Contains | Reusable logic: universe, features, models, alpha, sizing, risk, execution; declared *parameter space* | Concrete run context: which strategy, data source bindings, date range, starting capital, *parameter values or sweep*, RNG seed, accounting currency |
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
universe   → the set of instruments in scope (static or a registered dynamic selector)
features   → indicators / derived series from PIT data (vectorizable, pre-computable)
models     → AI model inference nodes (emit named outputs + confidence)
alpha      → insights: direction + confidence per instrument, from features/model outputs
sizing     → target positions (fixed, algorithmic, or derived from alpha/model output)
risk       → constraints/stops/kill-switches that can veto or resize targets
execution  → orders: order type, TIF, slicing, slippage tolerance (capability-gated)
```

Evaluation order is fixed: `universe → features → models → alpha → sizing → risk → execution`.
`features` and `models` outputs are addressable by every downstream stage; a later stage may
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
  "models":     [ /* §8  AI inference (+ optional training) */ ],
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
| `"model:sentiment.value"` | a named model output field |
| `"model:sentiment.confidence"` | a model confidence output |
| `"signal:news_volume"` | an exogenous signal stream (see [signals.md](signals.md), TBD) |
| `"data:close"` | a raw market-data field on the current instrument |
| `"param:rsi_period"` | a declared parameter's value (bound by the run request) |
| `"portfolio:position"` | current position state for the instrument |

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

```jsonc
"universe": {
  "type": "static",
  "instruments": ["AAPL@nasdaq.equity", "BTC-USD@coinbase.spot"]
}
```
or dynamic, via a registered selector component (point-in-time membership is the caller's
responsibility — see open questions):
```jsonc
"universe": {
  "type": "dynamic",
  "selector": { "ref": "top_n_by_volume", "params": { "n": 50, "lookback": "30d" } }
}
```

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

## 8. `models` — AI inference (and optional training)

Models are **pre-trained and external by default**. The strategy *calls* a model by ID,
passes it the data it needs each time inference runs, and binds the outputs. The suite never
stores weights and never trains unless a strategy explicitly opts in (§8.2). See
[model.md](model.md) and [ADR-0006](../adr/0006-model-inference-and-training.md).

### 8.1 Inference node

```jsonc
"models": [
  {
    "id": "sentiment",                       // node name; outputs addressed as model:sentiment.*

    "model_id": "news-sentiment",            // logical model the platform resolves at runtime
    "model_version": "3.2.1",                // pinned for reproducibility (required)
    "inference_fn": "predict_proba",         // which function/signature on the model to call

    "inputs": {                              // feature bindings: model-input-name → value ref
      "embeddings":    "feature:embed_news",
      "price_context": "feature:vol20"
    },
    "input_window": { "lookback": "7d" },    // PIT window of data fed each inference (≤ current ts)

    "frequency": { "every": "1d", "at": "session_open" },
    // alternatives:
    //   { "on_event": "Bar" }          run on every bar
    //   { "every_n_events": 5 }        run every 5th event
    //   { "cron": "0 0 * * MON" }      schedule-based

    "outputs": {
      "value":      "sentiment_score",       // primary output name (→ model:sentiment.value)
      "confidence": "sentiment_conf"         // confidence output name (→ model:sentiment.confidence)
    },

    "fallback": {                            // behavior if inference fails / is unavailable / stale
      "policy": "use_last",                  // use_last | use_default | skip_trade | halt
      "default_value": 0.0,
      "max_staleness": "2d"                  // older than this → treat as failure
    },

    "determinism": { "seed": 42 },           // seeded inference for reproducibility

    "training": { /* §8.2 — optional, omitted = pure inference */ }
  }
]
```

**Required fields:** `id`, `model_id`, `model_version`, `inference_fn`, `inputs`, `frequency`,
`outputs.value`, `fallback.policy`. Everything else is optional with defaults.

**Look-ahead safety:** `inputs` and `input_window` may only reference data with
`ts_event ≤ current_ts`. The suite enforces this; a model literally cannot be handed future
data.

### 8.2 Optional training / fitting

Off by default. When present and `enabled`, the suite orchestrates *when* to (re)fit and *with
what point-in-time data*, but the **training method itself is a registered, preconfigured
routine owned by the caller** — the suite does not own training algorithms.

```jsonc
"training": {
  "enabled": true,
  "method": "walk_forward_refit",            // a registered, preconfigured training method
  "schedule": { "rolling_window": "1y", "step": "1mo" },
  "train_data_window": "2y",                 // data range used to fit; strictly ≤ current ts
  "params": { "learning_rate": 0.001, "epochs": 5 },
  "freeze_after_first": false                // true = fit once at warmup, then inference-only
}
```

**Invariants:** training data is strictly point-in-time (`ts_event ≤ current_ts`) — this is
how walk-forward avoids look-ahead. Training is deterministic (seeded). Refit points are
cached by `(method, data_window, params)` so a parameter sweep does not retrain redundantly.

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
      "when": "feature:rsi14 < param:oversold && model:sentiment.value > param:sentiment_floor",
      "direction": "long",
      "confidence": "model:sentiment.confidence",   // expression or a bound value
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
| `from_model` | Directly from a model output (e.g. `model:position_sizer.value`) |
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
declarative fill-reaction model is an open question (§15).

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

## 15. Open questions (carried to design discussion)

- **Multi-strategy portfolios:** one run = one strategy = one portfolio, or can several
  strategy JSONs share one capital pool with portfolio-level netting and risk?
- **Expression vs. component boundary:** how much logic is allowed in `when`/expressions
  before it must become a registered component? (Risk: JSON becoming a programming language.)
- **Component registry trust model:** are custom components Rust (compiled), Python (PyO3), or
  WASM? How are they sandboxed to preserve determinism and look-ahead safety?
- **Declarative fill reactions:** the precise model for reacting to partial fills / rejections
  without imperative callbacks.
- **Run Request schema:** the separate document binding data, dates, capital, seeds, sweeps.
- **Model registry & reproducibility:** how the platform guarantees `model_id@version`
  resolves to identical weights at backtest time and months later.
- **Dynamic-universe survivorship:** point-in-time index membership as a data contract.
