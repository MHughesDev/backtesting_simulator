# Spec: DATA-016 — Asset Spec: Bonds & Fixed Income

**Spec ID:** DATA-016
**Type:** Data (asset schema / requirements)
**Status:** Approved
**Date:** 2026-06-06
**Author:** Agent

**Engine:** D (Cash Flow)
**`price_formation`:** `DEALER`
**Capabilities:** `HasCoupon | HasYield | HasMaturity | HasCreditRisk | HasAccruedInterest`

---

## 1. What bonds are

A **bond** is a debt instrument: the issuer borrows money from the buyer, agrees to pay
periodic **coupon** payments, and returns the **principal (par value)** at **maturity**.

The bond's market price is the present value of all future cash flows discounted at the
current **yield-to-maturity (YTM)** — the IRR that equates the price to the cash flow stream.

Bond pricing:
```
P = Σ [ C / (1 + y/f)^t ]  +  Face / (1 + y/f)^N
where:
  C   = coupon payment per period
  y   = yield to maturity (annualized)
  f   = coupon frequency (2 for semi-annual, 1 for annual)
  t   = each coupon period
  N   = total periods
  Face = par value (typically $1000)
```

**Key insight:** bond price and yield move in opposite directions. When yield rises, price falls.
Duration quantifies this sensitivity.

### Why bonds need their own engine

1. **Price is yield-derived, not quote-driven.** Bonds rarely trade continuously; price is
   computed from a yield curve, not read off an order book.
2. **Cash flows drive P&L.** A bond strategy's returns come from coupon income + price
   appreciation/depreciation + roll-down along the yield curve.
3. **Accrued interest.** The market quotes a "clean price" (excluding accrued coupon) but the
   buyer actually pays the "dirty price" (clean + accrued). This is specific to fixed income.
4. **Maturity = forced position close.** Unlike a stock, a bond has a contractual end date.

### Subtypes covered

| Subtype | Key features |
|---|---|
| US Treasury bonds/notes/bills | Risk-free; benchmark yield curve; semi-annual coupons |
| Corporate bonds | Credit spread above Treasuries; default risk |
| Municipal bonds | Tax-advantaged; credit risk; state/local issuer |
| Mortgage-backed securities (MBS) | Prepayment risk; irregular cash flows |
| Certificates of deposit (CDs) | Fixed rate; no secondary market |
| Zero-coupon bonds | No coupons; sold at discount; single payment at maturity |
| Floating-rate notes | Coupon resets to a reference rate (SOFR, LIBOR legacy) |

---

## 2. What a proper backtest requires

### 2.1 Bond static data (the contract)

| Field | Notes |
|---|---|
| `cusip` / `isin` | Identifier |
| `issuer` | Entity that issued the bond |
| `par_value` | Face value (typically $1000 per bond) |
| `coupon_rate` | Annual coupon as % of par (e.g. 0.05 = 5%) |
| `coupon_frequency` | Semi-annual (2), annual (1), quarterly (4), zero-coupon (0) |
| `issue_date` | Original issue date |
| `maturity_date` | Date principal is returned |
| `day_count_convention` | `ACT/ACT` (Treasuries), `30/360` (corps), `ACT/360` (T-bills) |
| `currency` | USD, EUR, etc. |
| `credit_rating` | AAA, AA, … (from Moody's / S&P / Fitch) |
| `seniority` | Senior secured, senior unsecured, subordinated |

**Day count convention** determines how accrued interest is calculated. Using the wrong
convention produces incorrect cash-flow timing.

### 2.2 Coupon schedule

All cash flows, pre-computed from static data:

| Field | Notes |
|---|---|
| `payment_date` | Exact date of each coupon payment |
| `coupon_amount` | Cash per bond per period |
| `record_date` | Must hold bond on this date to receive payment |
| `principal_payment` | Non-zero only at maturity (or for MBS partial prepayments) |

The coupon schedule is the "event stream" for a bond. The backtest applies each payment
as a cash credit to the portfolio at the `payment_date`.

### 2.3 Price / yield data

Bonds trade in a **dealer market** (OTC), not on an exchange. Quoted prices are from
broker-dealers. Pricing models:

| Approach | When used |
|---|---|
| **Market price** (if available) | Actively traded Treasuries and investment-grade corps |
| **Model price from yield curve** | Illiquid bonds; computed by discounting cash flows at appropriate yield |
| **Yield** | Often quoted instead of price (Treasury market quotes yield, not price) |

| Field | Notes |
|---|---|
| `clean_price` | Quoted market price, excluding accrued interest; per $100 face value |
| `dirty_price` | `clean_price + accrued_interest`; what buyer actually pays |
| `yield_to_maturity` | Annualized yield at which price = PV of cash flows |
| `accrued_interest` | Coupon earned since last payment date = `coupon_rate × days_since_last / days_in_period` |

### 2.4 Yield curve data

Required for model pricing of illiquid bonds and for yield-curve strategies:

| Field | Notes |
|---|---|
| `curve_date` | Date the yield curve was observed |
| `tenors[]` | e.g. 0.25Y, 0.5Y, 1Y, 2Y, 3Y, 5Y, 7Y, 10Y, 20Y, 30Y |
| `yields[]` | Yield at each tenor |
| `curve_type` | `Treasury` / `SOFR` / `OIS` / `Credit(rating)` |
| `credit_spread` | Spread over Treasury for a given rating / issuer |

### 2.5 Duration and convexity

These are not input data — they are **provide-or-derive**: Engine D can compute them from
the bond's cash flows and current yield, or the caller can provide them directly.

| Metric | Formula / meaning |
|---|---|
| **Macaulay duration** | Weighted-average time to cash flows (in years) |
| **Modified duration** | `Macaulay / (1 + y/f)` — % price change per 1% yield change |
| **DV01** | Dollar change in price per 1bp (0.01%) yield change |
| **Convexity** | Second-order sensitivity; corrects for non-linearity of price-yield curve |

Price change approximation:
```
ΔP/P ≈ -ModDuration × Δy  +  0.5 × Convexity × (Δy)²
```

### 2.6 Credit risk data

| Field | Notes |
|---|---|
| `credit_rating_history` | Time-series of ratings (for modeling rating migration) |
| `credit_spread_history` | OAS or Z-spread over the risk-free rate |
| `default_probability` | Optional; from CDS market or model |
| `recovery_rate` | What fraction of par is recovered in default (typically 40% for senior unsecured) |

---

## 3. Data contract

### Required manifest (Engine D)

```
REQUIRED:
  InstrumentStatic (bond) {
    cusip/isin, issuer, par_value,
    coupon_rate, coupon_frequency, day_count_convention,
    issue_date, maturity_date,
    currency, credit_rating, seniority
  }
  CouponSchedule { payment_date, coupon_amount, principal_payment }[]
  Yield or Price stream — ONE OF:
    ─ CleanPrice stream  (if market-quoted)
    ─ YTM stream         (engine derives price)
    ─ YieldCurve stream  (engine prices from curve + spread)

REQUIRED for credit strategies:
  CreditSpread stream
  CreditRatingHistory stream

OPTIONAL:
  DirtyPrice stream (or engine computes from clean + accrued)
  Duration/Convexity (provide-or-derive)
```

### Payload variants used

| Payload | Description |
|---|---|
| `Coupon { rate, accrual, next_payment_ts }` | Coupon payment event |
| `Mark { price }` | Clean price |
| `Yield { ytm, spread_over_treasury }` | Yield-based quote |
| `CreditRating { rating, outlook, ts }` | Rating change event |

---

## 4. Engine behavior (Engine D)

Engine D is a **cash-flow simulation engine**:

1. **Maintains a coupon schedule** for each held bond.
2. **Applies coupon payments** as cash credits at each `payment_date`.
3. **Computes dirty price** = clean_price + accrued_interest at any point.
4. **Computes yield-derived clean price** if price data is absent (from yield curve + credit spread + bond math).
5. **Applies maturity:** at `maturity_date`, position is closed; par value + final coupon credited.
6. **Applies credit events:** rating change events may trigger repricing of the credit spread.
7. **Computes duration and convexity** provide-or-derive for risk metrics.

Unlike Engine A, Engine D does **not** simulate an order book. Fill modeling is based on the
dirty price (what the buyer actually pays) plus a bid/ask spread calibrated to the bond's
liquidity tier.

---

## 5. Performance metrics (extensions)

| Metric | Notes |
|---|---|
| Coupon income | Cash received from coupon payments |
| Price appreciation / depreciation | Change in clean price |
| Accrued interest income | Included in total return but separated for clarity |
| Duration P&L | Return attributed to parallel yield-curve shift |
| Convexity P&L | Return from non-linear yield-sensitivity (favorable asymmetry) |
| Roll-down return | Price appreciation as bond "rolls down" the yield curve as time passes |
| Credit spread P&L | Return from spread tightening/widening |
| YTM at entry vs. exit | Shows if you bought at a good or bad yield |

---

## 6. Implications for system design

1. **Price is model-derived by default.** Most bonds don't trade continuously. Engine D must
   be capable of computing a mark from yield curve + credit spread + cash-flow discounting at
   every event timestamp.
2. **Accrued interest is a running state variable.** It accumulates continuously between
   coupon dates and resets to zero on the payment date. The engine tracks this per position.
3. **The "close" of a bond is its maturity.** There is no need to model delisting risk; the
   engine handles maturity as a scheduled forced-close event.
4. **MBS is a special case.** Mortgage-backed securities have **prepayment risk** — borrowers
   can pay off their mortgages early, which shortens the effective duration unpredictably.
   This requires a prepayment model (CPR, PSA) and is the most complex fixed-income subtype.

---

## 7. Sources

- Raymond James: Duration and convexity bond basics
- AnalystPrep CFA Level 1: Duration, convexity, and P&L estimation
- CFA Institute: Yield curve strategies
- BIS Quarterly Review (March 2021): Bond ETF arbitrage (for bond ETF cross-reference)
