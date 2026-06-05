# Engine A: Order Book (CLOB)

**Status:** ✅ Defined.
**`price_formation`:** `CLOB`
**Routes here:** equities, ETFs (execution), CEX crypto spot, futures, perpetuals, FX,
listed-option execution.

Engine A simulates execution against a **central limit order book** — the most widely used
price-formation mechanic. It spans many asset classes through **capability-gated extensions**
(sessions, fees, funding, liquidation, expiry, roll, swap rates), never through asset-type
branches.

---

## 1. The `Engine` trait

```rust
impl Engine for OrderBookEngine {
    fn on_event(&mut self, ev: &MarketEvent, ctx: &mut EngineContext);
    fn submit_order(&mut self, o: Order, ctx: &mut EngineContext) -> OrderResult;
    fn settle(&mut self, ctx: &mut EngineContext);
    fn supports_order_type(&self, t: OrderType) -> bool;  // market, limit, stop, stop_limit
}
```

- `on_event` advances book state, fires scheduled events (funding, expiry, sessions), checks
  resting orders for fills, and checks margin/liquidation.
- `submit_order` validates the order against capabilities, applies latency, and rests or
  attempts immediate execution.
- `settle` force-closes open positions at end of run (or contract expiry).
- The engine **owns no portfolio**: equity, buying power, positions, and collateral are read
  from the injected `Account` port; simulated fills are reported back to it
  ([ADR-0010](../../adr/0010-suite-does-not-own-portfolio.md)).

---

## 2. Internal state

```
OrderBookEngineState {
  book:          BookView,            // reconstructed to the best available fidelity (§3)
  resting:       Vec<RestingOrder>,   // live limit/stop orders with queue metadata
  last_trade:    Decimal,
  mark_price:    Option<Decimal>,     // perps/derivatives; separate from last_trade
  funding_next:  Option<i64>,         // next funding ts (perps)
  expiry:        Option<Date>,        // futures/options
  session:       SessionState,        // equities/FX
  fee_schedule:  FeeSchedule,
}
```

The book is a *view* derived from whatever market data the run provides — see fidelity ladder.

---

## 3. Data-fidelity ladder (the core design)

The engine must produce honest fills from whatever data exists. Matching adapts to fidelity;
**richer data ⇒ more realistic fills**. The mode is detected from the payloads bound for the
instrument (see [contracts/market-data.md](../contracts/market-data.md)).

**Two price series (equities and futures):** for assets affected by corporate actions or futures
rolls, two series must be available:
- **Unadjusted** (actual traded prices): used for fills, P&L, and the book. Engine A always
  fills against unadjusted prices.
- **Adjusted / continuous** (backward-adjusted for splits, dividends, or roll gaps): used by
  the strategy's feature pipeline for signal computation. Engine A does not own signal
  computation but must preserve the series distinction so fills are not applied to adjusted
  prices.

The run's data manifest declares which series are bound per instrument; the engine rejects a
configuration that would apply fills against adjusted prices.

| Fidelity | Data available | Matching behavior |
|---|---|---|
| **L3** | `BookDelta`/`BookSnapshot` with per-order detail | True queue-position priority; partial fills as volume trades through your level |
| **L2** | Aggregated depth (price→size) | Walk the book levels; exact book-walk slippage; queue approximated by size-ahead |
| **L1 / BBO** | `Quote` (best bid/ask + sizes) | Fill at touch up to displayed size; remainder via slippage heuristic (§5) |
| **Bar** | `Bar` OHLCV only | Synthetic fill model with intrabar assumptions (§5); no queue |

The same engine, the same order semantics — only the realism changes. The run records the
fidelity used so results disclose it.

---

## 4. Order types & lifecycle

Supported: `market`, `limit`, `stop`, `stop_limit`. TIF: `GTC`, `IOC`, `FOK`, `DAY`.

```
submit → validate(caps, tick/lot) → apply latency → 
    market      : execute immediately against opposing side (§5)
    limit       : marketable? execute; else rest with queue position
    stop        : rest as trigger; on trigger → becomes market
    stop_limit  : rest as trigger; on trigger → becomes limit
```

- **Latency:** an order submitted at `t` becomes active at `t + latency` (from
  `execution_defaults.latency`); it can only match data at-or-after that time — never the data
  that triggered it (look-ahead safety).
- **TIF:** `IOC` fills what it can immediately, cancels the rest; `FOK` fills fully or cancels;
  `DAY` cancels at session close; `GTC` persists.
- **Tick/lot:** prices snap to `tick_size`; quantities to `lot_size`; violations are rejected.

---

## 5. Fill model

### 5.1 Market orders
- **L2/L3:** walk the opposing side, consuming `(price, size)` levels until filled or exhausted.
  `avg_fill = Σ(price_i · qty_i) / total_qty`. Remaining unfilled → partial (or rejected for FOK).
- **L1:** fill up to displayed `ask_size`/`bid_size` at the touch; remainder gets the slippage
  heuristic applied to a synthetic deeper book.
- **Bar:** fill at the **next bar's open** by default (a decision on bar *t* cannot see inside
  bar *t*; the earliest executable price is *t+1* open), plus slippage. Configurable to
  same-bar close for coarser studies (flagged as weaker).

### 5.2 Limit orders
- Rest at the limit price. **Fill condition:**
  - L3: when the volume ahead in queue trades through, then your order fills (partials allowed).
  - L2/L1: when the market trades at or through your price.
  - Bar: a buy limit fills if a later bar's `low ≤ limit` (sell: `high ≥ limit`), at the limit
    price; whether a touch counts uses `execution_defaults.intrabar_fill`
    (optimistic/pessimistic/mid).
- A marketable limit (crosses the spread on arrival) executes immediately as a taker.

### 5.3 Stop / stop-limit
- Trigger reference is the **mark price** if present (perps/derivatives), else last trade.
- Buy-stop triggers when price `≥ stop`; sell-stop when `≤ stop`. On bars, trigger if
  `high ≥ stop` / `low ≤ stop`. On trigger, convert to market (stop) or limit (stop-limit).
- Intrabar trigger fills use the `intrabar_fill` assumption (default **pessimistic** — stops
  assume the worse side, avoiding optimistic bias).

### 5.4 Slippage (when depth is absent)
- **Book-walk** (L2/L3): exact, from consumed levels — no heuristic needed.
- **Heuristic** (L1/bar): `slippage_bps = impact(order_notional / interval_volume)` via a
  configurable curve. Default is a square-root impact model `k · √(size/ADV)` with per-liquidity
  calibration; the conservative tables in [assets/crypto-spot-cex.md](../assets/crypto-spot-cex.md)
  are an alternative preset.

### 5.5 Fees
- **Taker** (market / marketable limit) vs **maker** (resting limit that provides liquidity),
  per the instrument's `fee_schedule`. Crypto uses maker/taker bps by volume tier; equities use
  commission + optional ECN fee/rebate.
- The active tier (volume-dependent) is supplied via instrument config or the injected `Account`;
  the engine applies the resolved rate to each fill.

---

## 6. Capability extensions (gated; never asset-type branches)

| Capability | Behavior |
|---|---|
| `HasSessions` | Orders gated to allowed sessions; pre/after-hours optional; weekend/holiday gaps from the exchange calendar; `DAY` orders expire at close. |
| `HasFunding` | At each `next_funding_ts`, `funding_payment = position · mark · rate`; applied via `Account`; recorded separately from trading P&L. |
| `HasMarkPrice` | Maintains `mark_price` distinct from last trade; used for unrealized P&L, stops, and liquidation. |
| `HasLiquidation` / `IsLeveraged` | Each event: query collateral & maintenance margin from `Account`; if `mark` crosses the liquidation price, force-close at bankruptcy/mark price and report. Cascade modeling optional if `LiquidationEvent` data is provided. |
| `HasExpiry` (futures) | At `expiry_date`, settle open positions at `settlement_price` (cash) or warn/forced-close (physical). |
| `HasRollSchedule` | At a roll date, emit two fills: close front-month + open back-month at the observed spread. The **continuous series construction method** — Panama Canal (back-adjusted; prices can go negative; returns accurate), Proportional (ratio-adjusted; levels preserved), or Unadjusted (raw stitch; artificial gaps) — is a per-instrument config parameter. Method selected affects only the adjusted series used for signals; fills always use per-contract unadjusted prices. |
| `HasSwapRates` (FX) | At the daily rollover, apply `swap_long/short`; triple on Wednesdays; mark across weekend gaps. |
| `HasShortBorrow` | Accrue borrow cost daily on open short positions via `Account`. |
| `HasCorporateActions` / `HasTokenEvents` | Apply dividends/splits/mergers/forks/airdrops in strict `ts_event` order before price matching at that timestamp. |

Inverse perps (`perp_type = Inverse`) compute P&L in the base currency before USD conversion
(see [assets/perpetuals.md](../assets/perpetuals.md) §2.5).

---

## 7. Output

Each fill (or rejection) produces a `TradeRecord` (see [run-request.md](../run-request.md) §7):
`setup` (side, order type, TIF, limit/stop, slippage tol), `sizing` (from the strategy stage),
`execution` (fill price/qty, fees, slippage, partial flag, reject reason, queue info, fidelity
used), `trigger` (the insight/feature/model values), and `account_context` if an `Account` is
injected. Funding/borrow/roll cash-flows are emitted as auxiliary records tagged by source.

**Survivorship bias flag:** the run result includes `universe_survivorship_complete: bool` — set
`true` only if the instrument universe includes all historically-active instruments for the
period (including delisted). When `false`, results carry an implicit survivorship bias; the
consuming platform should surface this to the analyst.

---

## 8. Determinism & ordering

- Events at one `ts_event` are processed in `(instrument_id, seq)` order; lifecycle events
  (corporate actions, funding, expiry) before price matching at that timestamp.
- No order matches data dated at or before its own submission (latency rule).
- All randomness (e.g. probabilistic queue tie-breaks, if enabled) draws from the run seed.

---

## 9. Open items / parameters

- Default slippage curve calibration constants per liquidity tier.
- L3 queue model detail (FIFO vs. pro-rata venues).
- Liquidation cascade fidelity (self-impact modeling) when only marks are available.
- Same-bar vs. next-bar market-fill default (currently next-bar open).
- **Session-aware spread multiplier for FX:** FX spreads widen 3–10× outside the London–NY
  overlap session. The slippage heuristic should accept a per-session spread multiplier for
  `HasSwapRates` instruments so that off-hours strategies are not systematically over-optimistic.
