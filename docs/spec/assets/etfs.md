# Asset Spec: ETFs & Funds

**Engine:** A (Order Book) for execution + C (NAV) for valuation
**`price_formation`:** `CLOB` (execution); NAV valuation is a parallel model, not a
separate routing — see §4
**Capabilities:** `HasOrderBook | HasNAV | HasBasket | HasCorporateActions`
For leveraged/inverse: add `IsLeveraged | HasDailyReset`

---

## 1. What ETFs and funds are

An **ETF (Exchange-Traded Fund)** is a basket of underlying securities wrapped into a single
share that trades on an exchange like a stock. Its price is set by the CLOB, but its *fair
value* is the **Net Asset Value (NAV)** of the underlying basket.

### Key mechanism: creation / redemption

**Authorized Participants (APs)** — large financial institutions — can exchange a basket of
the underlying securities for newly created ETF shares (creation), or return ETF shares for
the underlying basket (redemption). This arbitrage mechanism keeps the market price closely
anchored to NAV.

- ETF market price > NAV → AP creates shares (buy basket, receive ETF shares, sell ETF) → price falls
- ETF market price < NAV → AP redeems shares (buy ETF, redeem for basket, sell basket) → price rises

This is why ETFs rarely deviate materially from NAV in liquid markets — but they **do deviate**
in illiquid or stressed conditions.

### Subtypes covered

| Subtype | Notes |
|---|---|
| Standard ETF | Tracks an index (equity, bond, commodity) |
| Actively managed ETF | PM selects holdings; NAV updated daily |
| Leveraged ETF (e.g. TQQQ 3×) | Daily reset; 3× daily return of index |
| Inverse ETF (e.g. SQQQ) | Daily reset; −1× or −3× daily return of index |
| Bond ETF | Holds fixed-income instruments; NAV uses bond pricing |
| Commodity ETF | May hold physical commodity or futures contracts |
| Crypto ETF | Holds crypto spot or futures |
| Mutual fund | Not exchange-traded; priced once daily at 4:00pm NAV |
| ETN (Exchange-Traded Note) | Debt instrument, not a basket — counterparty risk differs |

---

## 2. What a proper backtest requires

### 2.1 Standard market data (same as equities)

OHLCV, bid/ask quotes, trades, exchange calendar — see [equities.md](equities.md) §2.

### 2.2 NAV data

| Field | Notes |
|---|---|
| `nav` | Official end-of-day NAV (published after market close) |
| `inav` | Intraday indicative NAV — computed real-time from basket prices; less accurate |
| `premium_discount_pct` | `(market_price - nav) / nav × 100` |

The **premium/discount** is often the signal itself in ETF strategies (mean reversion to NAV).
Without NAV data, this signal cannot be computed.

### 2.3 Holdings / basket data

| Field | Notes |
|---|---|
| `holdings[]` | `{ ticker, weight, shares }` — composition of the basket |
| `as_of_date` | Effective date of holdings (typically monthly or daily disclosure) |
| `creation_unit_size` | Minimum shares for AP creation/redemption (e.g. 50,000) |

Required for:
- Computing NAV independently from holdings + underlying prices
- Basket arbitrage strategies
- Understanding sector/factor exposure changes

### 2.4 Leveraged / inverse ETF specifics

**Critical:** leveraged/inverse ETFs use a **daily reset** mechanism. They deliver N× the
*daily* return of the index, not N× the *cumulative* return.

- Over multiple days, **volatility decay** (also called beta decay) causes the fund to
  underperform N× the index on a cumulative basis in volatile, directionless markets.
- P&L computation must model this: each day's return = N × index_return_that_day.
- Do not assume the 3× fund tracks 3× the underlying over multi-day periods.

| Field | Notes |
|---|---|
| `leverage_factor` | e.g. 3.0, -1.0, -3.0 |
| `daily_reset: bool` | True for all leveraged/inverse ETFs |
| `expense_ratio` | Annualized management fee (reduces return daily) |

### 2.5 Corporate actions

Same as equities. ETF distributions (dividends from underlying holdings) are passed through
to shareholders as cash dividends on the ex-date.

---

## 3. Data contract

### Required manifest (Engine A + C, bar-level)

```
REQUIRED:
  InstrumentStatic {
    ticker, exchange, currency,
    lot_size, tick_size,
    asset_class = ETF,
    leverage_factor,      # 1.0 for standard, ±N for leveraged/inverse
    daily_reset           # true for leveraged/inverse
  }
  Bar stream (market price, unadjusted)
  Nav stream { nav, as_of_date }

REQUIRED for basket strategies:
  Holdings stream { holdings[], as_of_date }

OPTIONAL:
  iNAV stream (intraday)
  Quote stream (BBO)
  CorporateAction stream (distributions)
```

### Payload variants used

| Payload | Description |
|---|---|
| `Bar { … }` | Market price OHLCV |
| `Nav { nav, premium_discount }` | Daily NAV + premium/discount |
| `Quote { … }` | BBO for spread-aware fills |
| `CorporateAction { … }` | Dividend distributions |

---

## 4. Engine behavior

### Execution layer (Engine A)
Fills use the CLOB market price. Identical to equities.

### Valuation layer (Engine C)
Engine C computes and tracks NAV alongside market price. It does **not** route fills —
it is a valuation model attached to the instrument, not a separate execution path.

For leveraged/inverse ETFs, Engine C applies the daily reset:
```
daily_return = index_close_t / index_close_t-1 - 1
fund_return  = leverage_factor × daily_return - (expense_ratio / 252)
fund_price_t = fund_price_t-1 × (1 + fund_return)
```

This is why the Engine selection rule is: *same execution price formation → extend Engine A;
different valuation model → attach Engine C*. The routing decision is on execution, not valuation.

---

## 5. Performance metrics (extensions)

| Metric | Notes |
|---|---|
| Tracking error | `std_dev(ETF_return - index_return)` over the period |
| Premium/discount mean and range | How much did the ETF trade above/below NAV on average |
| Volatility decay cost | Cumulative underperformance vs. N× index (leveraged/inverse) |
| Expense ratio drag | Cost of management fee on position over holding period |

---

## 6. Implications for system design

1. **Engine composition.** One instrument can use two engines simultaneously. The capability
   flag `HasNAV` is what triggers Engine C attachment; `price_formation = CLOB` still routes
   execution to Engine A.
2. **NAV is a different clock.** NAV updates once daily (or intraday for iNAV), while market
   price updates tick-by-tick. The event stream must handle payloads on different cadences for
   the same instrument.
3. **Leveraged ETF path-dependency.** Daily reset means the strategy cannot be correctly
   evaluated by simply scaling underlying returns. The engine must simulate each day's reset.
4. **Mutual funds** have a degenerate CLOB — one fill per day at the 4:00pm NAV, no bid/ask.
   They route to Engine C exclusively, not Engine A.

---

## 7. Sources

- Schwab Asset Management: ETF creation/redemption mechanism
- CFA Institute: ETF mechanics, tracking, authorized participants
- AnalystPrep: ETF creation/redemption and authorized participants
- BIS Quarterly Review (March 2021): Bond ETF arbitrage anatomy
