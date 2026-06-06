# Spec: DATA-015 — Asset Spec: Options

**Spec ID:** DATA-015
**Type:** Data (asset schema / requirements)
**Status:** Approved
**Date:** 2026-06-06
**Author:** Agent

**Engine:** E (Derivatives)
**`price_formation`:** `CHAIN`
**Capabilities:** `HasGreeks | HasExpiry | HasIVSurface | HasEarlyExercise (American only)`

---

## 1. What options are

An **option** is a contract giving the buyer the *right, but not the obligation*, to buy
(call) or sell (put) an underlying asset at a specified **strike price (K)** on or before an
**expiry date (T)**.

### Key contract dimensions

| Dimension | Values |
|---|---|
| Type | `Call` (right to buy) / `Put` (right to sell) |
| Exercise style | `American` (exercise any time on or before expiry) / `European` (expiry only) |
| Strike | The agreed price of the underlying |
| Expiry | Standardized dates (3rd Friday of month for US equity options) or custom (crypto) |
| Underlying | A stock, ETF, index, futures contract, or crypto asset |

### Exercise style by market

| Market | Style | Notes |
|---|---|---|
| US equity options (individual stocks) | American | Early exercise possible; dividend risk matters |
| US index options (SPX, VIX) | European | No early exercise |
| US ETF options (SPY, QQQ) | American | SPY is American despite tracking SPX |
| Crypto options (Deribit) | European | Standard |
| Crypto options (some venues) | American | Check per-venue |

This distinction is architecturally important: American options require early-exercise logic
and have higher valuation complexity. Misclassifying them produces wrong greeks and fill behavior.

---

## 2. What a proper backtest requires

### 2.1 Why options backtesting is the hardest

Options are not just another instrument. The option's value depends on:
1. The **underlying price** (non-linear relationship via delta/gamma)
2. **Implied volatility** — market consensus on future vol; the primary "price" in options
3. **Time to expiry** — value decays (theta)
4. **Risk-free rate** — cost of carry (rho)
5. **Dividends** (for American calls on dividend-paying stocks: can trigger early exercise)

A backtest that uses only the option's OHLCV and fills at close price is almost useless —
it misses why the price moved and cannot correctly model the strategy's greek exposure.

### 2.2 The implied volatility surface — central data requirement

The **IV surface** is a 2D grid of implied volatility by (strike, expiry) at each point in time.
It is what options traders "trade" — they buy/sell vol, not just price.

**Why historical IV surface data is required for realistic options backtesting:**
- The surface as of each backtest timestamp is needed to correctly price options and compute greeks.
- Using today's IV surface for historical dates produces massively wrong results.
- The surface changes shape continuously: it can steepen, flatten, skew, or shift level.

Data providers: OptionMetrics IvyDB (institutional standard), SpiderRock, IVolatility,
CME DataMine.

| Field | Notes |
|---|---|
| `ts` | Timestamp of the surface snapshot |
| `underlying_id` | Which asset's surface |
| `strikes[]` | Moneyness or absolute strike values |
| `expiries[]` | Days-to-expiry or absolute dates |
| `iv_grid[][]` | Implied vol at each (strike, expiry) node |
| `surface_type` | `absolute_strike` or `delta_normalized` (e.g. 25Δ, ATM, 10Δ) |

**Provide-or-derive for greeks:** if the IV surface is provided, Engine E computes greeks
using a pricing model (Black-Scholes, Heston, local vol). If greeks are also provided, the
engine uses them directly and validates against its own model.

### 2.3 Option chain data (per instrument)

Each individual option contract is a separate instrument:

| Field | Notes |
|---|---|
| `underlying_id` | The underlying equity/index/crypto |
| `strike` | Strike price |
| `expiry_date` | Expiration date |
| `option_type` | `Call` or `Put` |
| `exercise_style` | `American` or `European` |
| `contract_multiplier` | 100 shares per equity option contract (standard US), 1 for crypto |
| `settlement_type` | `Cash` (index options, VIX) or `Physical` (equity options: delivery of shares) |

### 2.4 Market data for the option

Options often have **wide bid/ask spreads** and **low liquidity** for far-OTM strikes and
distant expiries. The close price of an option may be:
- A real traded price (liquid strikes near ATM)
- An interpolated value from the vol surface (illiquid far-OTM)
- Stale (no trades occurred; last quote may be hours old)

This means the `Mark { price }` payload (derived from IV surface using a pricing model) is
often more reliable than the `Bar.close` for an option contract. The engine uses market data
when available and model-derived marks when market data is stale or absent.

| Required | Field |
|---|---|
| Strongly preferred | IV surface (from which option price can be derived) |
| Preferred | Bid/ask quotes on the option itself |
| Acceptable but weaker | OHLCV bars on the option (if IV surface absent) |

### 2.5 Early exercise (American options)

For American-style options, the holder can exercise at any time. Early exercise is optimal in
specific conditions:
- **Deep ITM puts**: intrinsic value > time value; exercise to receive cash now.
- **American calls on dividend-paying stocks**: exercise just before ex-dividend date to
  receive the dividend (otherwise lose it when stock price drops by dividend amount).
- **Deep ITM calls near expiry**: rarely optimal except to avoid dividend risk.

The engine must evaluate early exercise optimality at each event using the pricing model.
If a short option position is assigned early, the engine must:
- Notify the strategy via a capability-gated event
- Open the resulting underlying position (for physical settlement)
- Adjust cash balance

### 2.6 Survivorship bias for options

Unlike equities where dead companies are excluded, options survivorship bias works differently:
- **Expired worthless options still existed.** If your strategy *could* have been short those
  puts, you can't ignore them.
- **Backtesting only on options that "survived" to be ITM at expiry** overstates premium-selling returns.
- A correct backtest must include all options in the chain, including the ones that expired
  at zero.

---

## 3. Data contract

### Required manifest (Engine E)

```
REQUIRED:
  InstrumentStatic (option) {
    underlying_id,
    strike,
    expiry_date,
    option_type: Call | Put,
    exercise_style: American | European,
    contract_multiplier,
    settlement_type: Cash | Physical
  }
  Underlying price stream (Bar OR Trade OR Quote — any one)
  Volatility input — ONE OF:
    ─ IVSurface stream   (historical surface snapshots; preferred)
    ─ Option Quote stream (bid/ask on the option itself; IV derived by engine)
    ─ Realized vol window (engine uses as proxy; weakest option)

OPTIONAL (enriches simulation):
  Option Quote stream (bid/ask on the contract itself)
  Option Bar stream (OHLCV on the contract)
  Greeks stream (if pre-computed; engine validates against its model)
  OpenInterest stream (per contract)
  Dividend stream for underlying (required for early-exercise American call modeling)
```

### Payload variants used

| Payload | Description |
|---|---|
| `Bar { … }` | Option OHLCV (if available; often stale for illiquid strikes) |
| `Quote { … }` | Option bid/ask |
| `IVSurface { ts, underlying_id, strikes[], expiries[], iv_grid[][] }` | Volatility surface snapshot |
| `Greeks { iv, delta, gamma, theta, vega, rho }` | Provide-or-derive |
| `Mark { price }` | Model-derived option value (used when market data stale) |
| `ExerciseEvent { option_id, style, intrinsic_value }` | Assignment or early exercise notification |

---

## 4. Engine behavior (Engine E)

Engine E is a **valuation engine**, not just a matching engine. It:

1. **Prices options** using Black-Scholes or a configurable model (stochastic vol, local vol) from the IV surface and underlying price.
2. **Computes greeks** at each event (provide-or-derive).
3. **Models fills** against quoted bid/ask when available; against model value with a spread
   estimate when only the surface is provided.
4. **Evaluates early exercise** for American options at each event:
   - If `intrinsic_value > model_value × threshold`, flags early exercise as optimal.
   - Applies assignment to short positions when the engine determines the long holder's optimal
     exercise triggers.
5. **Handles expiry:** on `expiry_date`, applies settlement:
   - OTM options: expire worthless; premium lost.
   - ITM cash options: cash settlement = intrinsic value × multiplier.
   - ITM physical options: underlying shares delivered; short must provide.

---

## 5. Performance metrics (extensions)

| Metric | Notes |
|---|---|
| Greeks P&L attribution | How much P&L came from delta, gamma, theta, vega each |
| Theta decay | Cumulative time-value decay of held options |
| Gamma scalping P&L | P&L from delta-hedging activity |
| IV vs. RV spread | Average difference between IV paid and realized vol (key for vol strategies) |
| Assignment rate | How often short positions were exercised early |
| Premium collected / paid | Total option premium for selling/buying strategies |

---

## 6. Implications for system design

1. **IV surface is a first-class data input, not an annotation.** Engine E cannot function
   without a vol input; the data manifest enforces this strictly.
2. **Options create multi-instrument portfolios implicitly.** A delta-hedged option position
   involves the option AND the underlying. Both must be in the same run with a shared clock.
3. **Pricing model is a simulator-level concern.** The caller provides data; the engine owns the
   pricing model (Black-Scholes baseline). This is one case where the "build it ourselves"
   principle from ADR-0002 matters most — inheriting a pricing model means inheriting its
   assumptions.
4. **American vs. European is not cosmetic.** These are different contracts with different
   fair values. The `exercise_style` field must be validated and used by the engine, not ignored.

---

## 7. Sources

- OptionMetrics IvyDB: industry standard historical options + IV data
- SpiderRock: historical volatility surface datasets
- CME Group: options analytics — greeks and implied volatility
- arXiv 2602.14350: Hidden risks and optionalities in American options
- arXiv 2603.19984: Model risk in optimal exercise of American options
- LuxAlgo: survivorship bias in backtesting
