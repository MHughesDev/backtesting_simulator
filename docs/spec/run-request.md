# Spec: Run Request

A **Run Request** is the per-invocation document that tells the suite *how to execute one run*.
The **Strategy JSON** says *what the strategy is* (reusable, portable); the Run Request binds it
to concrete data, time, parameters, and the injected ports for one execution.

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

  "strategy": { /* inline Strategy JSON, or a handle the caller resolves */ },

  "time": {
    "start":  "2020-01-01T00:00:00Z",
    "end":    "2024-12-31T23:59:59Z",
    "warmup": "60d"                            // history before `start` for indicator/model warmup
  },

  "instruments": [ /* Instrument definitions (or references) in scope */ ],

  "data":       { /* §4  injected data bindings */ },
  "account":    { /* §5  injected Account port (optional) */ },
  "models":     { /* §6  injected Model port bindings */ },
  "trainer":    { /* §6  injected Trainer port (optional) */ },
  "components": { /* §6  injected custom registry components (optional) */ },

  "parameters": { /* §8  concrete values OR a sweep over the strategy's declared space */ },

  "execution_defaults": { /* §9  latency/slippage/fee model defaults */ },

  "determinism": { "seed": 12345 },

  "output": { /* §7  what to emit and how (stream vs batch) */ },

  "limits": { "max_runtime": "30m", "max_memory_mb": 8192 }
}
```

Everything under `data`, `account`, `models`, `trainer`, `components` is an **injected port** —
the caller supplies an implementation; the suite calls it. None of these are owned by the suite.

---

## 4. `data` — injected market data

The suite owns no data. The Run Request binds each instrument's required payload streams to a
source the caller controls (a path, an in-memory Arrow table, or a `DataReader` implementation).

```jsonc
"data": {
  "reader": "arrow_ipc",                        // arrow_ipc | parquet | injected:<id>
  "bindings": {
    "AAPL@nasdaq.equity":   { "bars": "s3://.../aapl_1m.arrow", "corp_actions": "..." },
    "BTC-USD@coinbase.spot":{ "bars": "...", "trades": "..." }
  }
}
```

The suite validates each binding against the instrument's **required-data manifest** (see
[contracts/market-data.md](contracts/market-data.md) §3) and rejects under-specified runs.

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

## 6. Injected ports: models, trainer, components

The remaining caller-owned dependencies, wired per run:

```jsonc
"models": {
  "news-sentiment": { "adapter": "onnx", "uri": "...", "version": "3.2.1" }
},
"trainer": { "port": "injected:training_pipelines" },     // omit if no strategy trains
"components": {
  "my_factor_model": { "kind": "wasm", "uri": "..." },     // see Component Registry trust model
  "top_n_by_volume": { "kind": "builtin" }
}
```

- `models` resolve the `model_id@version` referenced in the strategy (see [contracts/model.md](contracts/model.md)).
- `trainer` is the injected `Trainer` (see [contracts/training.md](contracts/training.md)).
- `components` bind any custom registry components the strategy references (built-in ones need no binding).

---

## 7. `output` — what the suite emits

The suite's primary output is the **TradeRecord stream** plus per-event valuation marks. Portfolio
aggregation and metrics are computed downstream (by the caller, or by an optional analytics layer
over the injected `Account`).

```jsonc
"output": {
  "mode": "stream",                  // stream (incremental) | batch (returned at end)
  "emit": ["trades", "marks", "model_lineage", "warnings", "metrics?"]
}
```

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
5. **Port presence** — if the strategy uses account-relative sizing/risk, margin, models, or
   training, the corresponding injected port is present; else a typed error.
6. **Capability/order-type** — execution order types are valid for each instrument's engine.

Validation is fail-loud: a violation rejects the run with a precise, typed error rather than
producing results.

---

## 11. Invariants

1. **No assumed portfolio.** Account state is injected, never owned (ADR-0010).
2. **Stateless suite.** Nothing persists across runs; `request_id`/`strategy_id` are echoed only.
3. **Deterministic.** Given identical Run Request + injected ports (themselves deterministic),
   results are byte-identical, regardless of thread count.
4. **Point-in-time.** All bindings and ports may only expose `ts_event ≤ current_ts`.
5. **Feed-agnostic strategy.** The same Strategy JSON runs here and live; only the Run Request
   (and the live feed) differ.

---

## 12. Open questions

- **Sweep search ownership (OD-2):** does grid/random/Bayesian search live in the suite's run
  queue or the platform?
- **Reference `Account` adapter scope:** how full-featured is the optional shipped ledger
  (single-currency cash only, or margin/multi-currency)?
- **Streaming result schema:** exact wire format for incremental `output.mode = stream`.
- **Per-run vs per-sweep injected ports:** are ports shared across a sweep's runs or re-created?
