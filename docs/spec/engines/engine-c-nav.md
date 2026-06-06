# Engine C: NAV

**Status:** ✅ Defined.
**`price_formation`:** `NAV`
**Routes here:** mutual funds (execution); attaches to ETFs as a **valuation layer** alongside
Engine A.

Engine C prices on **Net Asset Value** rather than a live order book. It has two roles: it
*executes* mutual-fund subscriptions/redemptions at NAV, and it *values* basket products (ETFs)
whose execution is handled by Engine A.

---

## 1. The `Engine` trait

```rust
impl Engine for NavEngine {
    fn on_event(&mut self, ev: &MarketEvent, ctx: &mut EngineContext);  // strike NAV, accrue fees, daily reset
    fn submit_order(&mut self, o: Order, ctx: &mut EngineContext) -> OrderResult;  // forward-priced
    fn settle(&mut self, ctx: &mut EngineContext);
    fn supports_order_type(&self, t: OrderType) -> bool;  // market (subscription/redemption) ONLY
}
```

Only market-style subscription/redemption orders are valid (no limit/stop on a NAV product).
Account state is read from the injected `Account`
([ADR-0010](../../adr/0010-suite-does-not-own-portfolio.md)).

---

## 2. Two roles

| Role | Instrument | Behavior |
|---|---|---|
| **Execution engine** | Mutual fund (`price_formation = NAV`) | Orders fill at the next struck NAV (§4) |
| **Valuation layer** | ETF (`HasNAV`, executes via Engine A) | Computes NAV, premium/discount, daily reset; produces marks while Engine A produces fills |

Composition is by capability: an ETF routes execution to Engine A and attaches Engine C for
valuation. They share the `EngineContext` and the injected `Account`.

---

## 3. Forward pricing (the defining mechanic)

Mutual-fund orders are **forward-priced**: the price is **not known when the order is placed**.
An order submitted during the day fills at the **NAV struck at the next valuation point**
(typically the 4:00pm close). The engine therefore:

1. Accepts the order during the day (price unknown).
2. At the next valuation event, strikes NAV and fills the order at that NAV.

This prevents a strategy from "knowing" the fill price at decision time — a real and important
constraint that distinguishes funds from order-book assets.

---

## 4. NAV computation

NAV is **provide-or-derive**:

- **Provided:** a `Nav` payload stream (official end-of-day NAV) — used directly.
- **Derived:** from `HoldingsSnapshot` + underlying prices: `NAV = (Σ holdingᵢ · priceᵢ − liabilities) / shares_outstanding`.
  Requires `HasHoldings` capability and a `HoldingsSnapshot` binding for each holdings disclosure
  (see [contracts/market-data.md](../contracts/market-data.md) §2.18). Each underlying referenced
  in the holdings must itself be bound with price data.
- **iNAV** (intraday indicative) is optional and lower-accuracy; used only for premium/discount
  signals, never as a fund fill price.

The absolute minimum for Engine C is: `Nav` stream **OR** (`HoldingsSnapshot` + underlying price
data for all holdings). Without either, the run is rejected with a `DataSufficiencyError`.

**ETF creation/redemption** (`HasCreationRedemption`): when a `CreationRedemptionBasket` binding
is provided, the engine can model authorized-participant arbitrage mechanics and the basket
composition used for in-kind creation/redemption. This is optional — absence does not affect NAV
computation but limits ETF mechanics modeling.

**Premium/discount** (ETFs): `(market_price − NAV) / NAV`, surfaced for mean-reversion
strategies and tracking-error metrics.

---

## 5. Leveraged / inverse daily reset (path-dependent)

Leveraged/inverse ETFs deliver `N×` the **daily** return, reset each day:

```
daily_index_return = index_close_t / index_close_(t-1) − 1
fund_return        = leverage_factor · daily_index_return − (expense_ratio / 252)
fund_nav_t         = fund_nav_(t-1) · (1 + fund_return)
```

This reproduces **volatility decay** — over multiple days the fund underperforms `N×` the
cumulative index move in choppy markets. The engine simulates each day's reset; it never scales
cumulative returns.

---

## 6. Expense ratio

A daily management-fee drag (`expense_ratio / 252`) is accrued into NAV for all fund types.

---

## 7. Output

A `TradeRecord` per subscription/redemption: `setup` (subscribe/redeem, amount), `sizing`,
`execution` (NAV strike price, shares, fees), and `trigger`. As a valuation layer, Engine C
emits per-event `Mark` records (NAV, premium/discount) without fills.

---

## 8. Determinism & ordering

- NAV is struck deterministically at the valuation event; forward-priced orders fill in
  `(instrument_id, seq)` order at that NAV.
- The daily reset uses only `ts_event ≤ current_ts` index data (no look-ahead).

---

## 9. Open items / parameters

- Valuation-point definition for non-US / 24h reference assets.
- Subscription/redemption fees and minimums; swing pricing (optional).
- Holdings cadence (daily vs. monthly disclosure) and its effect on derived NAV accuracy.
