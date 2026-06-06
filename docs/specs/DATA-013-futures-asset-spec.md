# Spec: DATA-013 — Asset Spec: Futures

**Spec ID:** DATA-013
**Type:** Data (asset schema / requirements)
**Status:** Approved
**Date:** 2026-06-06
**Author:** Agent

**Engine:** A (Order Book / CLOB)
**`price_formation`:** `CLOB`
**Capabilities:** `HasOrderBook | HasExpiry | HasOpenInterest | HasRollSchedule | IsLeveraged`

---

## 1. What expiring futures are

A **futures contract** is a standardized agreement to buy or sell an asset at a specified
price on a specified future date (the **expiry**). Multiple contracts with different expiry
dates trade simultaneously. Price converges to the spot price as expiry approaches.

### Why futures need their own capabilities (not just "Engine A with an expiry date")

- Multiple concurrent contracts per underlying (front month, second month, quarterly, etc.)
- **Roll mechanics** — closing an expiring contract and opening the next one; a strategy
  must explicitly manage which contract it holds.
- **Continuous contract construction** — synthesizing a single price series across multiple
  expiries for signal computation.
- **Term structure** — the relationship between contracts of different expiries (contango vs
  backwardation) is itself a signal and a cost.
- **Settlement price** — the final price at expiry (cash or physical delivery) can differ
  from the last traded price.

### Subtypes covered

| Subtype | Examples | Settlement |
|---|---|---|
| Equity index futures | ES (S&P 500), NQ (Nasdaq), DAX | Cash |
| Single stock futures | Rare but exist | Cash or physical |
| Commodity futures | CL (crude oil), GC (gold), ZC (corn) | Physical or cash |
| Energy futures | NG (natural gas), RB (RBOB gasoline) | Physical |
| Rate / bond futures | ZN (10Y Treasury), ZF (5Y Treasury) | Physical delivery of bonds |
| Crypto futures | BTC quarterly (Binance, CME), ETH quarterly | Cash (usually USDT or USD) |
| Volatility futures | VIX futures | Cash (VIX settlement price) |

---

## 2. What a proper backtest requires

### 2.1 Per-contract market data

Each contract is a distinct instrument:
- `ES_2024-12-20` is different from `ES_2025-03-21`.
- OHLCV, quotes, trades — same as equities.
- Must include **open interest (OI)**: the total number of outstanding contracts. OI is a
  proxy for liquidity and market participation; declining OI near expiry signals migration
  to the next contract.

| Field | Notes |
|---|---|
| `open_interest` | Number of outstanding contracts at close |
| `settlement_price` | Final price on expiry date (different from last traded price in some markets) |
| `delivery_date` | Actual date of cash settlement or physical delivery |

### 2.2 Roll mechanics — the single most important modeling decision

**What a roll is:** closing the front-month contract and opening the next-month contract as
expiry approaches, to maintain continuous exposure.

**Roll decision methods:**

| Method | Rule | Pro | Con |
|---|---|---|---|
| Calendar roll | Roll N days before expiry (e.g. N=5) | Simple, predictable | May roll into illiquid period |
| Volume roll | Roll when back-month volume > front-month | Matches market behavior | Depends on OI/volume data |
| User-defined | Strategy explicitly manages contracts | Maximum control | Requires explicit logic |

**Roll cost:** selling the expiring contract and buying the next-month contract at the
current spread between them. This spread is the main source of cost in trend-following strategies.

### 2.3 Continuous contract construction

For signal computation (indicators, ML features), a strategy typically needs a **continuous
price series** rather than individual contracts. There are several methods:

| Method | Description | Bias introduced |
|---|---|---|
| **Unadjusted stitch** | Simply append contracts at roll | Artificial price gaps at roll |
| **Panama Canal (back-adjusted)** | Subtract the roll price gap from all historical prices backward | Prices can go negative for back-adjusted; good for returns, bad for levels |
| **Perpetual / proportional** | Interpolate between front and next by time-to-expiry | More complex; smaller artifacts |
| **Nearby contract** | Always use the front month | Simple; roll gaps remain |

**Implication:** the simulator must maintain both the continuous (adjusted) series for signal
computation *and* the individual contract series for fill modeling and P&L. This is the same
adjusted/unadjusted split as equities.

### 2.4 Contango and backwardation

**Contango:** futures price > spot price (most common; cost-of-carry model explains it).
- Rolling in contango = selling cheap / buying expensive → **negative roll yield**.
- Long-only strategies are penalized in persistent contango (e.g. VIX futures, many commodity ETFs).

**Backwardation:** futures price < spot price (common in energy, agricultural markets with
supply constraints).
- Rolling in backwardation → **positive roll yield**.
- Long strategies are aided by backwardation.

The term structure slope (basis = futures - spot) must be available in the data for basis
strategies and roll-yield computation.

| Field | Notes |
|---|---|
| `spot_reference_price` | The underlying spot price at the time of the futures price observation |
| `basis` | `futures_price - spot_price` |
| `roll_yield_annualized` | Annualized gain/loss from rolling at current term structure |

---

## 3. Data contract

### Required manifest (Engine A, bar-level, per individual contract)

```
REQUIRED:
  InstrumentStatic {
    underlying_id,       # e.g. "ES_INDEX", "BTC_SPOT"
    expiry_date,
    contract_size,       # e.g. 50 for ES (50 × index value = notional)
    tick_size,
    initial_margin,      # as fraction of notional
    maintenance_margin,
    settlement_type:  Cash | Physical,
    exchange
  }
  Bar stream (per individual contract, unadjusted)
  OpenInterest stream

REQUIRED for continuous-contract strategies:
  ContinuousBar stream (adjusted, with roll method annotated)
  RollSchedule { roll_dates: [ { from_contract, to_contract, roll_date } ] }

OPTIONAL:
  SpotReference stream (for basis computation)
  Quote stream
```

### Payload variants used

| Payload | Description |
|---|---|
| `Bar { …, adjusted: bool }` | OHLCV; adjusted = true for continuous series |
| `OpenInterest { oi }` | Daily open interest |
| `Mark { price }` | Settlement price on expiry date |
| `Quote { … }` | BBO for spread-aware fills |

---

## 4. Engine behavior (Engine A with expiry handling)

- At the contract's `expiry_date`, open positions are settled:
  - Cash settlement: position closed at `settlement_price`; P&L credited/debited.
  - Physical delivery: warning / forced close (physical delivery is not simulated).
- If the strategy holds an expiring contract and no roll logic has triggered, the engine
  forcibly closes the position at settlement price and generates a `ContractExpiredEvent`.
- Roll events trigger two fills: close front-month + open back-month.
- Margin is tracked; initial margin is required to open; maintenance margin must be
  maintained or the position is liquidated.

---

## 5. Performance metrics (extensions)

| Metric | Notes |
|---|---|
| Roll cost | Total P&L impact from rolling contracts (positive in backwardation, negative in contango) |
| Basis P&L | Returns from basis trade (long spot / short futures or vice versa) |
| Roll yield annualized | Annualized contribution from term-structure slope |
| Contract slippage at roll | Spread paid at each roll event |
| Expiry P&L | P&L from settlement vs. last-traded price |

---

## 6. Implications for system design

1. **Contract identity includes expiry.** `ES_2024-12-20` and `ES_2025-03-21` are different
   instruments with different `instrument_id`s, even though they share an underlying.
2. **Continuous series is provide-or-derive.** The simulator can construct a continuous series
   from individual contracts using a specified roll method, or the caller can provide a
   pre-built continuous series. Both paths must be supported.
3. **Basis strategies need spot.** If a strategy trades the basis between futures and spot,
   both instruments must be in the same run with a shared clock.
4. **Roll decisions must be explicit.** The engine does not roll automatically — it forcibly
   closes expired positions. Roll logic belongs in the strategy or in the roll schedule
   the caller provides.

---

## 7. Sources

- QuantPedia: Continuous futures contracts methodology for backtesting
- QuantStart: Continuous futures contracts for backtesting purposes
- Hudson & Thames ArbitrageLab: Futures rollover documentation
- QuantVPS: Continuous futures contracts for CME traders (best practices)
