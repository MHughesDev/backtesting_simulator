# Asset Spec: Equities

**Engine:** A (Order Book / CLOB)
**`price_formation`:** `CLOB`
**Capabilities:** `HasOrderBook | HasCorporateActions | HasShortBorrow | HasSessions`

---

## 1. What equities are

An equity represents direct ownership of a fraction of a company. It trades on a **central
limit order book (CLOB)** — a resting queue of bids and offers matched by price-time priority.

### Subtypes covered

| Subtype | Notes |
|---|---|
| Common stock | Voting shares, e.g. AAPL, TSLA |
| Preferred stock | Fixed dividend, senior to common in liquidation |
| REIT | Real estate investment trust; required to distribute ≥90% earnings |
| ADR / GDR | Depositary receipts for foreign companies (trade in USD on US exchanges) |
| Tokenized stock | On-chain representation of an underlying equity (e.g. Backed Finance tokens) |

### Key mechanics

- **Sessions.** US equities: pre-market (04:00–09:30 ET), regular (09:30–16:00 ET), after-hours
  (16:00–20:00 ET). Liquidity and spreads differ dramatically between sessions.
- **CLOB matching.** Market orders execute against the best available resting order; limit
  orders rest until filled or cancelled.
- **Short selling.** Borrowing shares to sell short incurs a **borrow rate**; hard-to-borrow
  names can carry annualized rates of 20–200%+.
- **Corporate actions.** External events that change the quantity or value of shares owned (see §3).

---

## 2. What a proper backtest requires

### 2.1 Price data

| Field | Type | Notes |
|---|---|---|
| `open` | decimal | First trade of interval |
| `high` | decimal | Highest trade |
| `low` | decimal | Lowest trade |
| `close` | decimal | Last trade of interval |
| `volume` | u64 | Shares traded |
| `vwap` | decimal | Volume-weighted average price (optional but improves fill modeling) |
| `interval` | duration | e.g. 1m, 5m, 1d |

**Critical:** the backtesting suite must maintain **two price series** in parallel:
- **Adjusted prices** — split- and dividend-adjusted backward; used for signal computation and
  indicator calculation.
- **Unadjusted prices** — actual traded prices at each date; used for fill modeling and P&L
  calculation.

Using only adjusted prices for fills causes silently overstated returns because the adjusted
series artificially smooths corporate-action price gaps.

### 2.2 Quote data (for realistic fill modeling)

| Field | Type |
|---|---|
| `bid` | decimal |
| `bid_size` | u64 |
| `ask` | decimal |
| `ask_size` | u64 |
| `ts_event` | i64 (ns UTC) |

Without quotes a backtest uses the close price for fills; this overstates execution quality
because the spread is invisible.

### 2.3 Trade data (tick-level, optional)

Individual prints — price, size, aggressor side — enable realistic queue-position modeling.
Required only if the strategy models microstructure (e.g. market-making, TWAP execution).

### 2.4 Corporate actions (REQUIRED — the most common source of silent bugs)

Corporate actions are discrete events that must be applied in the correct order relative to
the event timestamps. Missing any of them produces phantom price discontinuities or wrong
share counts.

| Action | Effect on backtest | Required fields |
|---|---|---|
| **Cash dividend** | Stock price drops by dividend amount on ex-date; cash credited to account | `ex_date`, `pay_date`, `amount_per_share`, `currency` |
| **Stock split** | Share count multiplies, price divides by split ratio | `ex_date`, `ratio` (e.g. 4:1) |
| **Reverse split** | Share count divides, price multiplies | `ex_date`, `ratio` (e.g. 1:10) |
| **Spin-off** | New shares created for subsidiary; cost-basis allocation | `ex_date`, `new_ticker`, `ratio`, `cost_basis_allocation` |
| **Merger / acquisition** | Target delisted; shares exchanged for cash, acquirer shares, or mix | `effective_date`, `consideration_type`, `cash_per_share`, `share_ratio` |
| **Rights issue** | Shareholders can buy new shares at a discount | `ex_date`, `subscription_price`, `ratio` |
| **Delisting** | Stock removed from exchange; forced close at last price or zero | `date`, `final_price` |

**Survivorship bias** is the result of backtesting only on currently-listed stocks and
ignoring companies that were delisted, went bankrupt, or were acquired. It systematically
overstates strategy performance. The data provider must supply the full historical universe
including dead tickers.

### 2.5 Short borrow data

Required if the strategy can go short:

| Field | Notes |
|---|---|
| `borrow_rate` | Annualized %, changes daily for HTB (hard-to-borrow) names |
| `short_availability` | Estimated shares available to borrow (optional; used to reject fills) |

### 2.6 Session metadata

| Field | Notes |
|---|---|
| `exchange_calendar` | Market open/close times, half-days, holidays by date |
| `session_type` | `pre_market`, `regular`, `after_hours` per event |

---

## 3. Data contract

### Required manifest (Engine A, bar-level)

```
REQUIRED:
  InstrumentStatic {
    ticker, exchange, currency,
    lot_size, tick_size,
    asset_class = Equity
  }
  Bar stream (adjusted + unadjusted pairs)  OR  Trade stream
  ExchangeCalendar

REQUIRED IF short selling is enabled:
  BorrowRate stream

REQUIRED IF the strategy uses corporate-action events:
  CorporateAction stream

OPTIONAL (improves fill modeling):
  Quote stream (BBO)
  Trade stream (if using bar-level data)
```

### Payload variants used

| Payload | When used |
|---|---|
| `Bar { open, high, low, close, volume, interval, adjusted: bool }` | Standard; must indicate which series |
| `Quote { bid, bid_size, ask, ask_size }` | Spread-aware fill modeling |
| `Trade { price, size, aggressor_side }` | Tick-level |
| `CorporateAction { action_type, … }` | Dividends, splits, mergers, etc. |

---

## 4. Engine behavior (Engine A)

- Fills against quotes if present; against bar midpoint ± half-spread estimate if absent.
- Corporate actions are applied as the event stream progresses; adjusted/unadjusted prices are
  maintained separately by the engine.
- Borrow cost is accrued daily for open short positions.
- Session gating: the engine can be configured to reject orders outside regular session.

---

## 5. Performance metrics (extensions to universal metrics)

| Metric | Notes |
|---|---|
| Dividend income | Cash received from dividends while holding the position |
| Borrow cost | Total interest paid on short positions |
| Gross vs. net returns | Separates price appreciation from dividend income |
| Adjustment P&L | Shows effect of using adjusted vs. unadjusted prices |
| Survivorship-bias flag | Warning if the instrument universe excludes dead tickers |

---

## 6. Implications for system design

1. **Two price series is non-negotiable.** The adjusted/unadjusted distinction must be a
   first-class field on `Bar`, not a post-processing step.
2. **Corporate actions are events, not static metadata.** They must flow through the same
   event stream as price data so the engine processes them in strict time order.
3. **Survivorship bias cannot be enforced by the suite** (we own no data), but the result
   contract should include a `universe_has_dead_tickers: bool` flag the caller sets to
   document this choice.
4. **Sessions change behavior, not just liquidity.** The engine must know whether to accept
   orders in pre/after-hours based on configuration.

---

## 7. Sources

- QuantConnect corporate actions documentation (types and handling)
- QuantStart survivorship bias series
- FUTU HK backtest corporate actions documentation
