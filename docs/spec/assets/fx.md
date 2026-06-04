# Asset Spec: FX (Foreign Exchange)

**Engine:** A (Order Book / CLOB, quote-driven variant)
**`price_formation`:** `CLOB`
**Capabilities:** `HasOrderBook | HasSwapRates | HasSessionLiquidity`

---

## 1. What FX is

FX (foreign exchange) is the simultaneous buying of one currency and selling of another.
A currency pair like `EUR/USD` represents the price of the base currency (EUR) in units of
the quote currency (USD). Price = 1.10 means 1 EUR costs 1.10 USD.

### Why FX is Engine A (not a new engine)

FX spot trades on an **interbank dealer market** — effectively a quote-driven version of
a CLOB where dealers post bid/ask spreads and traders take liquidity. The price-formation
mechanism is bid/ask matching, same as equities and CEX crypto. The unique features
(carry, swap rates, sessions) are implemented as **capability extensions** on Engine A, not
a new price-formation mechanic.

### Key differences from equities

| Dimension | Equities | FX Spot |
|---|---|---|
| What you own | Fraction of a company | Relative claim: one currency vs. another |
| "Value" anchor | Earnings, book value | Interest rate differential, purchasing power parity |
| Overnight cost | Short borrow rate (if short) | Swap points / rollover (for any open position) |
| Sessions | Regular hours + pre/after | 24/5 (Sun 5pm ET – Fri 5pm ET); 3 overlapping sessions |
| Corporate actions | Yes | None |
| Tick size | $0.01 | 1 pip (0.0001 for most pairs, 0.01 for JPY pairs) |
| Primary fee | Commission | Bid/ask spread (commission-free at most brokers) |

### Subtypes covered

| Subtype | Examples |
|---|---|
| Major pairs | EUR/USD, GBP/USD, USD/JPY, USD/CHF, AUD/USD, USD/CAD |
| Minor / cross pairs | EUR/GBP, EUR/JPY, GBP/JPY |
| Exotic pairs | USD/TRY, USD/ZAR, USD/MXN |
| Crypto pairs (spot) | BTC/USD — handled by CEX crypto spec; listed here for reference |

---

## 2. What a proper backtest requires

### 2.1 Market data

Same envelope as equities: OHLCV bars, quotes (BBO), individual trades.

FX-specific differences:
- **Spread is the primary cost.** Most retail/institutional FX has no explicit commission;
  the bid/ask spread is the complete transaction cost.
- **Pips.** The minimum price movement is a pip (0.0001 for EUR/USD). Fill modeling must
  respect pip granularity.

### 2.2 Swap / rollover rates — the carry cost

Holding an FX position overnight incurs an **overnight rollover** (or earns income, if the
carry is positive). This is the interest rate differential between the two currencies.

**Why it matters:** a carry trade (borrow low-rate currency, buy high-rate currency) can
earn or lose significant P&L from swap rates alone. Backtests ignoring rollover overstate
returns for carry-negative pairs and understate them for carry-positive pairs.

Swap rate formula (simplified):
```
forward_rate = spot × (1 + rate_quote) / (1 + rate_base)
swap_points = forward_rate - spot
daily_swap = position_size × swap_points / spot (approximately position × rate_differential / 365)
```

| Field | Notes |
|---|---|
| `swap_long` | Pips (or points) credited/debited per day for long positions |
| `swap_short` | Pips (or points) credited/debited per day for short positions |
| `swap_date` | Date this rate was effective |
| `triple_swap_day` | Wednesday (for most pairs) — 3× swap to account for weekend |

Swap rates change over time as central bank rates change; a correct backtest uses the
historical swap rate stream, not a static rate.

### 2.3 Session liquidity

The FX market is global and continuous 5 days a week, but liquidity is not uniform.

| Session | Times (UTC) | Characteristics |
|---|---|---|
| **Asian / Tokyo** | 00:00 – 09:00 | Lower volatility; JPY pairs most active |
| **London** | 07:00 – 16:00 | Highest volume; EUR pairs most active |
| **New York** | 12:00 – 21:00 | Second highest; USD pairs most active |
| **London / NY overlap** | 12:00 – 16:00 | Peak liquidity; tightest spreads |

Spread is not constant — it widens significantly outside peak hours and especially at the
market open on Sunday and during news events.

| Field | Notes |
|---|---|
| `session_type` | `asian`, `london`, `new_york`, `overlap`, `off_hours` |
| `spread_multiplier` | Relative spread vs. normal (e.g. 3.0 during off-hours) |

### 2.4 No overnight gap (almost)

FX is nearly continuous, but there is a **weekend gap** (Friday 5pm ET to Sunday 5pm ET
when markets are closed). Strategies holding positions over the weekend are exposed to gap
risk. The backtest must model the gap — a position held Friday close at 1.1000 might open
Monday at 1.0950 with no fills available at in-between prices.

---

## 3. Data contract

### Required manifest (Engine A, bar-level)

```
REQUIRED:
  InstrumentStatic {
    base_currency,         # e.g. EUR
    quote_currency,        # e.g. USD
    pip_size,              # 0.0001 for most, 0.01 for JPY pairs
    lot_size,              # standard lot = 100,000 units of base currency
    asset_class = FX
  }
  Bar stream (OHLCV in quote currency)

REQUIRED if the strategy holds overnight:
  SwapRate stream { swap_long, swap_short, swap_date }

OPTIONAL:
  Quote stream (tight spreads; required for spread-cost modeling)
  SessionMetadata stream
```

### Payload variants used

| Payload | Description |
|---|---|
| `Bar { open, high, low, close, volume, interval }` | OHLCV |
| `Quote { bid, bid_size, ask, ask_size }` | BBO; spread is the key cost signal |
| `SwapRate { swap_long, swap_short }` | Daily rollover rate |

---

## 4. Engine behavior (Engine A, FX mode)

- Fills against quoted bid (for sell) or ask (for buy) when quotes are available.
- At each daily close (or at user-configured rollover time), applies swap credits/debits to
  open positions using the `SwapRate` for that date.
- Triple swap on Wednesdays: `swap_amount × 3`.
- Weekend gap: no fills are generated between Friday 5pm ET and Sunday 5pm ET; positions are
  marked at Friday's last close during this gap.
- Session metadata used optionally to widen modeled spread for off-hours periods.

---

## 5. Performance metrics (extensions)

| Metric | Notes |
|---|---|
| Carry P&L | Total income / cost from swap rates |
| Spread cost | Total bid/ask spread paid on fills |
| Gap risk P&L | P&L impact from weekend / holiday gaps |
| Pip P&L | Returns expressed in pips for interpretability |
| Session breakdown | Performance by trading session (useful for session-based strategies) |

---

## 6. Implications for system design

1. **Swap rates are time-varying and must flow through the event stream.** A fixed carry
   assumption is a common backtest error; correct modeling requires historical daily swap rates.
2. **FX "volume" is notional traded, not share count.** FX bars report notional volume in
   the base currency or in tick count. The interpretation differs from equity volume.
3. **Pip granularity must be enforced at fill time.** An order that would fill at 1.10003 must
   round to 1.1000 — prices outside pip resolution are not valid.
4. **Cross-currency P&L conversion.** A EUR/USD trade's P&L is naturally in USD. A EUR/JPY
   trade's P&L is in JPY and must be converted to the portfolio's base currency using the
   JPY/USD rate at the time of the trade.

---

## 7. Sources

- Forextraders.com: Computing swap points and forward rates
- CME Group: covered interest parity, forward FX swaps
- Capital.com: What is a forex swap
- ThinkMarkets: Forex backtesting guide
