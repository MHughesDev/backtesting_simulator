# Spec: COMP-006 — Engine D: Cash Flow

**Spec ID:** COMP-006
**Type:** Component (execution engine)
**Status:** Approved
**Date:** 2026-06-06
**Author:** Agent

**`price_formation`:** `DEALER`
**Routes here:** bonds, treasuries, municipals, corporates, CDs, MBS.

Engine D is a **cash-flow simulation engine**. A bond's value is the present value of its future
cash flows; returns come from coupon income, price change as yields move, and roll-down. There
is no order book — pricing is yield/curve-derived and execution is a dealer-spread fill.

---

## 1. The `Engine` trait

```rust
impl Engine for CashFlowEngine {
    fn on_event(&mut self, ev: &MarketEvent, ctx: &mut EngineContext);  // accrue, pay coupons, reprice
    fn submit_order(&mut self, o: Order, ctx: &mut EngineContext) -> OrderResult;  // dealer fill
    fn settle(&mut self, ctx: &mut EngineContext);                       // maturity / run end
    fn supports_order_type(&self, t: OrderType) -> bool;  // market (dealer) ONLY
}
```

Cash and positions live in the injected `Account`
([ADR-0010](../adr/0010-simulator-does-not-own-portfolio.md)).

---

## 2. Pricing — provide-or-derive

| Mode | When | How |
|---|---|---|
| **Market** | Liquid bonds with quoted `clean_price` via `Bar` or `Mark` | Use the quote directly |
| **Yield-derived** | A `YieldUpdate` (instrument-specific YTM) is provided | Price = PV of cash flows discounted at YTM |
| **Curve-derived** | A `YieldCurve` + `CreditSpread` for the issuer/rating exists | Discount cash flows at the curve point for each tenor + the credit spread |

**YieldCurve vs. YieldUpdate:** `YieldUpdate` is instrument-specific (carries the YTM of a
single bond). `YieldCurve` is the full benchmark curve (Treasury, SOFR, OIS, etc.) used when
deriving yields for instruments without direct quotes (see [contracts/market-data.md](DATA-004-market-data-contract.md)
§2.13). Both may be bound simultaneously.

**Credit data (`HasCreditRisk`):** `CreditSpread` carries the OAS, Z-spread, or treasury spread
for the issuer/rating bucket used in curve-derived pricing. `CreditRatingEvent` triggers an
immediate repricing when a rating changes — applied in strict `ts_event` order before pricing at
that timestamp. If `HasCreditRisk` is set but neither stream is provided, the run is rejected
with a `DataSufficiencyError`.

Bond price (frequency `f`, periods `N`, coupon `C`, face `F`, yield `y`):

```
P = Σ_{t=1..N} [ C / (1 + y/f)^t ]  +  F / (1 + y/f)^N
```

`YTM` (when only price is known) is solved by root-finding on the equation above.

---

## 3. Accrued interest & dirty price

The market quotes the **clean price** (ex-accrued); the buyer pays the **dirty price**:

```
accrued       = coupon · (days_since_last_coupon / days_in_period)     # per day_count_convention
dirty_price   = clean_price + accrued
```

`day_count_convention` ∈ {`ACT/ACT` (treasuries), `30/360` (corporates), `ACT/360` (bills)}.
Accrued interest is a **running state variable** — it accumulates daily and resets to zero on
each coupon payment.

---

## 4. Cash-flow event stream

The `CouponSchedule` is the bond's event stream. At each `payment_date` the engine credits the
coupon to the `Account`; principal is returned at maturity (or partially for MBS — §7).

---

## 5. Risk measures (provide-or-derive)

Computed from cash flows + yield, or supplied directly:

```
Macaulay duration  = Σ t·PVₜ / P                  (in years)
Modified duration  = Macaulay / (1 + y/f)
DV01               = Modified duration · P · 0.0001
Convexity          = Σ t(t+1)·PVₜ / [P·(1+y/f)²]
ΔP/P ≈ −ModDur·Δy + ½·Convexity·(Δy)²
```

These drive the duration/convexity P&L attribution in metrics.

---

## 6. Dealer fill model

No order book. Fills use the **dirty price** plus a **bid/ask spread calibrated to the bond's
liquidity tier** (treasuries tight; off-the-run corporates wide). Buy at dirty + half-spread,
sell at dirty − half-spread; spread is a configurable per-tier parameter.

---

## 7. Lifecycle

- **Maturity:** at `maturity_date`, `settle` force-closes the position — par value + final
  coupon credited.
- **Credit events:** a `CreditRatingEvent` changes the rating bucket used for `CreditSpread`
  lookup, repricing the bond from that `ts_event` forward.
- **MBS prepayment (extension):** principal can be repaid early per a prepayment model (CPR /
  PSA), shortening effective duration unpredictably and reshaping the cash-flow stream. The most
  complex fixed-income subtype; modeled behind a prepayment-curve input.

---

## 8. Output

A `TradeRecord` per fill: `setup` (buy/sell, face amount), `sizing`, `execution` (clean price,
accrued, dirty price, spread paid, YTM at fill), `trigger`. Coupon and principal cash-flows are
emitted as auxiliary records tagged `coupon` / `principal`.

**Roll-down return:** as a bond ages along the yield curve, its mark-to-market price changes
even with no yield-curve shift (yield converges toward shorter tenors). This roll-down return
is naturally captured in the daily mark series (mark change net of coupon accrual) and does
not require a separate computed field — the mark stream is the record.

---

## 9. Determinism & ordering

- Coupon/principal/credit events apply in `ts_event` order before pricing at that timestamp.
- Model pricing (YTM solve, discounting) is deterministic.
- No curve/quote dated after the decision is used (look-ahead safety).

---

## 10. Open items / parameters

- Curve interpolation method (linear-on-yield vs. spline) and bootstrapping conventions.
- Liquidity-tier spread calibration.
- MBS prepayment model selection (PSA multiples, refinancing incentive).
- Callable/putable bonds (embedded options → may compose with Engine E).
- **Repo financing cost:** leveraged long bond positions are typically financed via repo
  (short-term collateralized borrowing). The repo rate (~Fed funds rate ± spread, currently
  ~4–5% annualized) is a daily carry cost analogous to perpetual funding rates. When
  `IsLeveraged` is set, Engine D should accrue a `repo_rate` charge daily via `Account`,
  reported as an auxiliary record tagged `repo`. The repo rate is a run-config parameter
  (or can be a time-series input for historical accuracy).
