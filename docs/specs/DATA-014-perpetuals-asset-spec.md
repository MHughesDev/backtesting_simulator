# Spec: DATA-014 — Asset Spec: Perpetual Futures (Perps)

**Spec ID:** DATA-014
**Type:** Data (asset schema / requirements)
**Status:** Approved
**Date:** 2026-06-06
**Author:** Agent

**Engine:** A (Order Book / CLOB)
**`price_formation`:** `CLOB`
**Capabilities:** `HasOrderBook | HasFunding | IsLeveraged | HasMarkPrice | HasLiquidation`

---

## 1. What perpetual futures are

A **perpetual future** (or perpetual swap) is a leveraged derivative with no expiry date.
It provides exposure to an underlying asset's price without ever settling into the underlying.
Instead of convergence at expiry, a **funding rate mechanism** continuously anchors the
perpetual's price to the underlying spot price.

Perps are the highest-volume crypto instrument class. They did not exist in traditional finance
at scale — crypto invented and popularized them.

### How they differ from expiring futures

| Dimension | Expiring Futures | Perpetual Futures |
|---|---|---|
| Expiry | Yes (standardized dates) | None |
| Price tethering | Converges to spot at expiry | Funding rate every 8 hours |
| Roll needed | Yes | Never |
| Settlement | Cash or physical at expiry | Never settles (closed by trader) |
| Primary venues | CME, Binance quarterly, … | Binance, Bybit, OKX, dYdX, Hyperliquid |

### Linear vs. inverse perps

| Type | Collateral | P&L currency | Example |
|---|---|---|---|
| **Linear** | USDT / USDC | USDT | BTC/USDT perp on Binance |
| **Inverse** | BTC | BTC | BTC/USD perp on BitMEX |

In **inverse perps**, gains and losses are denominated in the base asset (BTC), which
introduces non-linear P&L effects because the base asset's value also fluctuates.
A 10% BTC gain on a long inverse perp with BTC collateral = more than 10% return measured
in USD.

---

## 2. What a proper backtest requires

### 2.1 Market data (CLOB — same as futures, with additions)

OHLCV, quotes, trades — same as CEX crypto spot. Additionally:

### 2.2 Funding rate data — the most critical perpetual-specific input

The funding rate is the mechanism by which the perpetual price is anchored to spot. It is
paid between long and short holders every 8 hours (or 1 hour on some venues). If perp price
> spot price, longs pay shorts (funding rate > 0). If perp price < spot price, shorts pay
longs (funding rate < 0).

**Funding rate formula (generalized):**
```
funding_rate = clamp(
    premium_index + clamp(interest_rate - premium_index, -0.05%, 0.05%),
    -cap, +cap
)
where:
  premium_index = (mark_price - spot_index_price) / spot_index_price
  interest_rate = typically fixed at 0.01% per 8h period
  cap = exchange-specific (0.05% or 0.75% for extreme regimes)
```

| Field | Notes |
|---|---|
| `funding_rate` | Rate for the period (e.g. 0.0003 = 0.03% per 8h) |
| `funding_interval_hours` | 8h for most exchanges, 1h for some |
| `next_funding_ts` | Unix timestamp of next funding settlement |
| `mark_price` | Mark price at the moment of funding |

**P&L impact:**
```
funding_payment = position_size × mark_price × funding_rate
```
If long and funding_rate > 0: you pay this amount. If long and funding_rate < 0: you receive it.

Missing funding rates from a backtest can produce P&L errors of 20–50%+ for strategies held
over weeks in high-funding environments.

### 2.3 Mark price — not the last-traded price

The **mark price** is a manipulation-resistant reference price used for:
- Unrealized P&L calculation
- Liquidation triggering
- Funding rate calculation

**Mark price = median of three components:**
1. `index_price × (1 + funding_rate × time_to_funding / 8h)`
2. `index_price + EMA_30min(mid_price - index_price)`
3. `mid_price` (last bid/ask midpoint)

Using the last-traded price for unrealized P&L instead of the mark price causes incorrect
liquidation simulation — a strategy may appear profitable when it would have been liquidated.

| Field | Notes |
|---|---|
| `mark_price` | Published by exchange per second or per funding period |
| `index_price` | Spot reference price (typically multi-exchange median) |

### 2.4 Margin and liquidation

| Field | Notes |
|---|---|
| `initial_margin_rate` | Fraction of notional required to open (e.g. 0.02 = 2% for 50× leverage) |
| `maintenance_margin_rate` | Minimum to avoid liquidation (e.g. 0.5% for BTC on Binance) |
| `liquidation_price` | Calculated per position: `entry_price × (1 ± maintenance_margin_rate / leverage)` |
| `insurance_fund_balance` | Optional: size of exchange insurance fund (affects socialized loss) |

**Liquidation mechanics:**
1. Mark price reaches the liquidation price.
2. Engine forcibly closes the position at the bankruptcy price (or mark price, depending on exchange).
3. If the position has residual value, it goes to the insurance fund.
4. If the position has negative equity (underwater), the insurance fund absorbs the loss; if
   the fund is depleted, socialized losses (auto-deleveraging, ADL) apply to counterparties.

**Cascade liquidation risk:** A large liquidation drops the mark price → triggers other
positions' liquidation prices → further price drop. This feedback loop is a real risk that
affects P&L for *other* strategies in the same market. The backtest must model cascades if
simulating a portfolio at scale.

### 2.5 Inverse perp P&L formula

For inverse perps (BTC-margined BTC/USD):
```
pnl_in_btc = position_contracts × (1/entry_price - 1/exit_price)
pnl_in_usd = pnl_in_btc × exit_btc_price
```
The non-linearity means a 10% BTC price rise on a 1× long inverse perp produces 9.09% return
in BTC but 10% in USD (the extra P&L comes from the BTC appreciation itself).

---

## 3. Data contract

### Required manifest (Engine A + funding, bar-level)

```
REQUIRED:
  InstrumentStatic {
    underlying_id,
    perp_type: Linear | Inverse,
    quote_currency,         # USDT for linear, BTC for inverse
    contract_size,
    tick_size,
    initial_margin_rate,
    maintenance_margin_rate,
    funding_interval_hours,
    exchange
  }
  Bar stream (OHLCV)
  FundingRate stream { rate, mark_price, next_funding_ts }
  MarkPrice stream (required if funding stream doesn't include per-period mark)

OPTIONAL:
  IndexPrice stream (for mark price recomputation)
  LiquidationEvent stream (for cascade simulation)
  Quote stream
```

### Payload variants used

| Payload | Description |
|---|---|
| `Bar { … }` | OHLCV of the perpetual contract |
| `Funding { rate, mark_price, next_funding_ts }` | Per-funding-period funding data |
| `Mark { price }` | Intra-period mark price updates |
| `Quote { … }` | BBO for spread-aware fills |

---

## 4. Engine behavior (Engine A with funding)

- Opens with margin check: `position_notional × initial_margin_rate ≤ available_balance`.
- Every funding period (`next_funding_ts`), the engine applies:
  ```
  funding_payment = size × mark_price × funding_rate  (sign depends on direction)
  ```
- Continuously monitors: if `mark_price ≤ liquidation_price` (long) or `mark_price ≥ liquidation_price`
  (short), force-close and settle.
- For inverse perps, all P&L is computed in base-currency before converting to USD for reporting.

---

## 5. Performance metrics (extensions)

| Metric | Notes |
|---|---|
| Funding P&L | Total funding payments received or paid |
| Funding P&L as % of total | How much of strategy returns came from funding carry |
| Liquidation events | Number and P&L impact of positions that hit liquidation |
| Effective leverage used | Average leverage over the backtest period |
| Mark vs. last-price slippage | Difference between simulated fill and mark price |

---

## 6. Implications for system design

1. **Funding is a scheduled event, not a bar event.** The engine must fire a `Funding` payload
   at each funding timestamp, independent of bar cadence.
2. **Mark price is the reference, not last trade.** All unrealized P&L and liquidation checks
   use `mark_price`. The engine must track this separately from bar close price.
3. **Linear and inverse require different P&L math.** Inverse perps require denomination
   conversion. This must be a first-class configuration on the instrument, not a calculation
   done ad-hoc in the strategy.
4. **Funding P&L is separate from trading P&L.** The metrics contract must always break these
   out separately — conflating them hides the nature of the returns.

---

## 7. Sources

- Cube Exchange: Funding rate mechanics
- MetaMask: Perpetual futures liquidation mechanics
- MetaMask: Funding frequency and trading strategies
- arXiv 2506.08573: Designing funding rates for perpetual futures in cryptocurrency markets
- CoinAPI: Historical data for perpetual futures
