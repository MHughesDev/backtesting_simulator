# REFERENCE DOCUMENT: Engine Deep Dive

**Note:** This is a reference document, not a specification. It duplicates content from individual engine specs (COMP-003 through COMP-010) and the Run Request spec (DATA-002) for quick lookup only. For authoritative specifications, refer to those individual specs.

---

## Engine A — Order Book (`CLOB`)

### How the market and the engine work

A central limit order book is the price-formation mechanism underlying almost every serious
financial exchange on earth — the NYSE, Nasdaq, Binance, CME, FX ECNs, and listed options
markets all run some variant of it. At any moment, the order book is a sorted list of resting
intentions. On the buy side (the bid), participants have submitted limit orders saying "I will
buy X units at price P or better." On the sell side (the ask/offer), participants have
submitted limit orders saying "I will sell X units at price P or better." Bids are sorted
descending — the highest bid is at the top. Asks are sorted ascending — the lowest ask is at
the top. The spread between best bid and best ask is the compensation market makers earn for
providing liquidity and bearing inventory risk.

When you submit a market order to buy, you are saying "fill me immediately at whatever price is
available." The exchange matches you against the lowest available ask first. If that ask is not
large enough to fill your entire order, you consume it completely and move to the next ask level
— which is at a higher price. You keep walking up the ask side until you are filled. This is
walking the book, and the difference between your average fill price and the best ask at
submission time is your market-impact cost. The larger your order relative to the depth at each
level, the worse your average fill price.

When you submit a limit order to buy at price P, if P is at or above the current best ask your
order is marketable — it crosses the spread and executes immediately as a taker. If P is below
the best ask, your order rests in the book at your price level, joining any other orders already
there. You are now in a queue. Under FIFO matching, orders at the same price level fill in
submission order. Your order fills only after every order ahead of you in queue at that price is
consumed. Queue position is economically meaningful: being first in a large queue at an active
level is completely different from being last.

The engine replicates this entire mechanism. The `OrderBookEngineState` maintains a `BookView`
— a reconstruction of the order book from whatever historical data the run provides. The
fidelity of that reconstruction is the most important design variable because historical data
comes in radically different forms, and pretending L1 data is L3 data produces systematically
wrong results.

**The fidelity ladder** is the engine's core honesty mechanism. At L3 you have per-order data:
every individual order submitted to the exchange, with its ID, timestamp, price, size, and
whether it was modified or cancelled. This lets the engine reconstruct the queue exactly as it
existed in history. When your simulated order arrives, the engine knows precisely how many
shares were ahead of you at your price level and fills you only after that volume trades. At L2
you have aggregated depth: total displayed size per price level, but not the individual orders.
The engine walks levels to compute slippage exactly but approximates queue position within a
level. At L1/BBO you only know the best bid, best ask, and their displayed sizes. The engine
fills at the touch up to displayed size and uses a slippage heuristic for anything larger. At
Bar you have only OHLCV — no intrabar detail whatsoever.

The bar-level fill model is the most commonly used and the source of the most subtle backtesting
errors. If you observe a bar close and decide to buy, the price you observed is already in the
past. The earliest you can possibly execute is the next bar's open. The engine enforces this by
default: market orders triggered by bar-close data fill at the next bar's open plus slippage.
Filling at the same bar's close is look-ahead bias — you would have needed to know the close in
order to trade at the close. For limit orders at bar fidelity, a buy limit at price P fills if a
subsequent bar has a low ≤ P; a sell limit fills if a subsequent bar has a high ≥ P. Whether a
touch (low exactly equals your limit) counts as a fill is controlled by `intrabar_fill` in
`execution_defaults`. For stops, the default is pessimistic — stops assume the worst intrabar
price on the adverse side — because assuming you always stop out at the best price of the adverse
move systematically overstates strategy performance.

**The two-series requirement for equities and futures** is a correctness constraint that most
off-the-shelf backtesting tools get wrong. An equity's raw price history is not a clean time
series — companies pay dividends (causing mechanical price drops on ex-date), do stock splits
(halving or thirding the price with no economic change), and undergo corporate restructuring.
For signal computation — moving averages, RSI, momentum — you want the backward-adjusted series
where splits and dividends are applied retroactively so prices are comparable across time. But
for fills and P&L you want the unadjusted series — the actual prices you would have paid. Mixing
them is a category error. Filling at adjusted prices produces meaningless P&L numbers. The engine
enforces the series distinction and rejects any configuration that would apply fills against
adjusted prices. The same logic applies to futures, where contracts expire and roll gaps create
price discontinuities in the raw series. The continuous series construction method — Panama Canal
(back-adjusted by adding/subtracting roll gaps; returns accurate; prices can go negative),
Proportional (ratio-adjusted; levels preserved), or Unadjusted (raw stitch with artificial gaps)
— is a per-instrument parameter that affects only the adjusted signal series. Fills always use
per-contract unadjusted prices.

**Latency** is a first-class concept. An order submitted at timestamp `t` becomes eligible to
match against market data only at `t + latency` (configured via `execution_defaults.latency`).
This simulates the real-world delay between your system generating an order and that order
arriving at the exchange. The practical implication is that the data event that triggered your
decision cannot be the same data event your order matches against. You cannot observe a tick and
fill at that tick's price — you fill at the next event at or after your order's activation time.
This is one of the most commonly violated constraints in naive backtesting systems; the engine
enforces it structurally.

**Capability extensions** are how the engine covers six asset classes without a single
asset-type branch. The engine never has `if asset_type == "futures" { ... }`. Instruments
declare capability flags, and the engine checks those flags to activate additional mechanics.

`HasFunding` activates the perpetual futures funding mechanism. Perpetual swaps are futures
contracts with no expiry date. To keep their price anchored to the underlying spot price,
exchanges run a funding rate mechanism every 8 hours. The funding rate is computed from the
difference between the perp price and the spot (the basis): when the perp trades at a premium,
longs pay shorts; when at a discount, shorts pay longs. The payment is
`position_size × mark_price × funding_rate`. The engine reads `funding_next` from state,
advances past it on `on_event`, computes the payment, reports it to `Account`, and records it
as a separate cash-flow line — not folded into trading P&L. A perp strategy with positive
trading P&L but deeply negative cumulative funding P&L has a completely different risk profile
than one where both are positive, and this separation is what makes that visible.

`HasLiquidation` activates the margin/liquidation loop. Leveraged positions require margin:
collateral posted to cover potential losses. Initial margin is what you need to open the
position (e.g. 10% of notional for 10× leverage). Maintenance margin is the minimum you need
to keep it open (e.g. 5%). If unrealized losses erode collateral below the maintenance margin
threshold, the exchange force-closes the position at or near the bankruptcy price (the price at
which collateral would be exactly zero). The engine checks margin every event: it queries
`Account` for current collateral and position value, computes whether the mark price has crossed
the liquidation price, and if so force-closes and emits a liquidation record. The mark price
(separate from last trade) is used here — it is a manipulation-resistant reference that prevents
a single bad print from triggering a cascade of liquidations.

`HasRollSchedule` handles futures contract rolls. An expiring front-month contract must be
closed and a back-month contract opened to maintain continuous exposure. In contango (back month
more expensive than front), rolling costs money on long positions. In backwardation (back month
cheaper), you earn roll yield. The engine emits two fills at the roll date — close front-month,
open back-month at the observed spread — giving accurate roll-cost attribution.

`HasSwapRates` handles FX overnight rollover. Spot FX trades settle two business days later
(T+2). Holding a position overnight triggers a roll. The cost or benefit is the swap rate: the
overnight interest rate differential between the two currencies in the pair. Long EUR/USD accrues
positive carry if EUR rates exceed USD rates, negative if USD rates exceed EUR rates. Rates are
triple-applied on Wednesday because Wednesday's roll covers the weekend (three days of interest).
The engine applies daily credits/debits via `Account` at the daily rollover event.

`HasCorporateActions` and `HasTokenEvents` handle dividends, splits, mergers, forks, and
airdrops. These are applied in strict `ts_event` order before price matching at that timestamp.
A dividend application adjusts the cash balance and the cost basis. A split adjusts position
size and cost basis. These must happen before the next price comparison so that P&L calculations
remain consistent — if you adjust the stock price for a split but haven't yet adjusted the
position, you'll calculate a false P&L at that moment.

### What is unique about this engine

This is the only engine that has a fidelity ladder with four distinct data levels, each
producing qualitatively different fill realism. It is the only engine that enforces the
two-series constraint (adjusted vs. unadjusted). It is the most general-purpose engine and
the only one where the list of capability extensions is long enough to cover six distinct
asset classes. The `resting` order state (the list of live limit and stop orders awaiting
triggers) is unique to this engine — no other engine rests orders in a queue. The survivorship
bias flag (`universe_survivorship_complete`) is unique to Engine A because it is the only
engine where the universe composition question has this specific meaning (including vs.
excluding delisted instruments from a CLOB universe).

### Data contract requirements

**Instrument fields required for Engine A:**

```jsonc
{
  "id": "AAPL@nasdaq.equity",
  "price_formation": "CLOB",           // routes to Engine A
  "tick_size": 0.01,
  "lot_size": 1,
  "contract_multiplier": 1.0,
  "settlement": "T+2",

  // Always set for equities:
  "capabilities": ["HasOrderBook", "HasCorporateActions", "HasSessions", "HasShortBorrow"],

  // For futures — also add:
  "capabilities": [..., "HasExpiry", "HasRollSchedule", "HasOpenInterest"],
  "expiry_date": "2025-03-21",

  // For perpetuals — also add:
  "capabilities": [..., "HasFunding", "HasMarkPrice", "HasLiquidation", "IsLeveraged"],
  "funding_interval_hours": 8,
  "perp_type": "Linear",               // Linear | Inverse
  "initial_margin_rate": 0.10,
  "maintenance_margin_rate": 0.05,

  // For FX — also add:
  "capabilities": [..., "HasSwapRates", "HasSessionLiquidity"],
  "base_currency": "EUR",              // for EUR/USD: base_currency = EUR, currency = USD

  // For crypto CEX — also add:
  "capabilities": [..., "HasMakerTakerFees", "HasTokenEvents"],
  "fee_schedule": [
    { "volume_30d_usd": 0,       "maker_bps": 10, "taker_bps": 10 },
    { "volume_30d_usd": 1000000, "maker_bps": 8,  "taker_bps": 8  }
  ]
}
```

**Minimum data requirements for Engine A:**

| Level | Requirement | Result if absent |
|---|---|---|
| Absolute minimum | Any one of: `Trade`, `Quote`, `BookSnapshot`, `Bar` | `DataSufficiencyError` — run rejected |
| `HasFunding` set | `Funding` stream | `DataSufficiencyError` |
| `HasCorporateActions` set | `CorporateAction` stream | `DataSufficiencyError` |
| `HasBorrowRate` + strategy shorts | `BorrowRate` stream | `DataSufficiencyError` |
| Strategy uses 1s bars but only 1d bars provided | — | `DataSufficiencyError.non_derivable_conflict` (cannot derive 1s from 1d) |
| No `Quote`/`BookSnapshot`/`Trade` (bar only) | — | Run proceeds; `degraded_fidelity` warning; bar-fidelity fills |

**Market data payload requirements for Engine A:**

Minimum viable (bar fidelity, weakest realism):
- `Bar` events with `adjusted: false` for fills
- `Bar` events with `adjusted: true` for signal computation (equities/futures with rolls)

Upgrades that improve fill realism (fidelity ladder):
- `Trade` events — enables trade-price reference; bar derivation at all intervals ≥ trade frequency
- `Quote` (BBO) — enables L1 spread-aware fill model
- `BookSnapshot` + `BookDelta` (`HasOrderBook`) — enables L2 fill model; exact book walk
- `OrderBookOrderEvent` (`HasL3OrderBook`) — enables true L3/MBO fill model; queue-position reconstruction

Required for capability-gated mechanics:
- `Funding` + `MarkUpdate` — required when `HasFunding` + `HasMarkPrice` are set
- `CorporateAction` — required when `HasCorporateActions` is set
- `TokenEvent` — required when `HasTokenEvents` is set
- `BorrowRate` — required when `HasBorrowRate` is set and strategy holds short positions
- `TradingStatus` — required when `HasTradingStatus` is set (halts gate fills)
- `RollSchedule` (reference data) — required when `HasRollSchedule` is set
- `OpenInterest` — when `HasOpenInterest` is set (optional; improves metrics)
- `FeeScheduleUpdate` — optional; overrides static fee schedule when fee schedules change over time
- `UniverseMembership` (reference data) — when `HasUniverseMembership` is set

**data bindings in the Run Request for a perpetual (descriptor format):**

```jsonc
"data": {
  "reader": "arrow_ipc",
  "bindings": {
    "BTC-USD-PERP@binance.perp": {
      "bars_1m": {
        "uri":           "path/to/btcusd_perp_1m.arrow",
        "payload_class": "Bar",
        "interval":      "1m",
        "adjusted":      false
      },
      "trades": {
        "uri":           "path/to/btcusd_perp_trades.arrow",
        "payload_class": "Trade"
      },
      "funding": {
        "uri":           "path/to/btcusd_funding.arrow",
        "payload_class": "Funding"
      },
      "mark_updates": {
        "uri":           "path/to/btcusd_mark.arrow",
        "payload_class": "MarkUpdate"
      }
    }
  }
}
```

**DataSufficiencyError example — strategy needs 1s bars but only daily bars are provided:**

```
DataSufficiencyError {
  instrument_id: "AAPL@nasdaq.equity",
  engine: CLOB,
  missing_required: [],
  non_derivable_conflict: [
    ResolutionConflict { required_interval: "1s", available_interval: "1d" }
  ],
  degraded_fidelity: []
}
```

**Account port requirement:** required if `IsLeveraged`, `HasFunding`, `HasLiquidation`,
`HasShortBorrow`, or any sizing method that references equity/buying power is in use. Optional
for absolute-sizing strategies on non-margined instruments.

---

## Engine B — AMM (`AMM`)

### How the market and the engine work

An automated market maker is a smart contract holding a pool of two or more assets that will
trade with you at any time at a price computed from a mathematical formula over the current pool
reserves. There is no counterparty. There is no order queue. There is no spread in the
traditional sense. You interact directly with the pool's math.

The original and simplest AMM is Uniswap v2's constant product market maker, governed by the
invariant `x · y = k`. The pool holds reserves `R_x` of token X and `R_y` of token Y. The
product `k` must remain constant after any swap (excluding the portion retained as fees). If
you want to buy token Y by selling token X, you add `Δx_eff = Δx · (1 − fee)` of X to the
pool (the fee portion stays, accruing to liquidity providers). The pool's new X reserve is
`R_x + Δx_eff`, and to preserve `k`, the new Y reserve must be `k / (R_x + Δx_eff)`. The
amount of Y you receive is `Δy = R_y − k/(R_x + Δx_eff)`, which simplifies to
`(R_y · Δx_eff) / (R_x + Δx_eff)`. The spot price before the swap is `R_y / R_x`. After the
swap the price of Y is higher because you reduced its supply, so your effective execution price
is worse than the spot price you observed at submission. This gap is price impact — a direct
function of the ratio of your trade size to pool depth. A large trade against a thin pool has
enormous price impact.

For the exact-out case (you want to receive exactly `Δy` of token Y), you solve for the
required input: `Δx = R_x · Δy / ((R_y − Δy) · (1 − fee))`, rounded up by one wei (the
smallest unit of precision) because solidity integer arithmetic always rounds down and the
protocol always requires the pool to be made whole.

The engine computes price impact precisely, compares it to the strategy's `max_slippage_bps`
parameter, and reverts if the tolerance is exceeded — modelling the on-chain `minAmountOut`
argument that every real DEX transaction includes. If the engine's computed slippage exceeds
the tolerance, the swap reverts: no fill, no state change. If `HasRevertGas` is enabled, the
gas cost of the failed transaction is still debited because on Ethereum a reverted transaction
still consumes gas up to the revert point.

**Uniswap v3 — concentrated liquidity** is a substantially more complex system. In v2,
liquidity is distributed uniformly across the entire price range from zero to infinity — most
of it wasted because prices don't trade across most of that range. In v3, liquidity providers
specify a tick range `[P_a, P_b]` within which their capital is active. The pool tracks price
as `√P` in Q64.96 fixed-point arithmetic (a 160-bit integer). Within any tick range where the
current price sits, there is a defined active liquidity `L` — the sum of all LP positions whose
tick range includes the current price. The within-tick swap math is:

```
√P_next = (L · √P_current) / (L + Δin_eff · √P_current)
amount_out = L · (√P_current − √P_next)
```

This is fast and deterministic within a single tick. The problem is that a large enough swap
exhausts the liquidity in the current tick range and the price moves through the tick boundary
into the next range, which has a different value of `L`. This is a tick crossing. The engine
handles it iteratively: swap up to the boundary of the current tick, cross the tick (which
updates `L` by adding or subtracting the `liquidityNet` value stored at that boundary — the net
liquidity that becomes active or inactive when crossing), then continue with remaining input
in the new range. A very large swap crosses multiple ticks, and each crossing requires a loop
iteration. Without the full tick data structure (per-tick liquidity deltas), the engine falls
back to approximate mode — it assumes `L` is constant throughout the swap with no tick
crossings — which is valid for small swaps but increasingly wrong as the swap moves price
significantly. This degradation is flagged explicitly in the `TradeRecord`.

**Curve's StableSwap** is designed for assets that should trade near a fixed ratio —
stablecoin pairs like USDC/USDT, or liquid staking token pairs like stETH/ETH. The CPMM
invariant is shaped for general price discovery and produces enormous slippage near the peg
for assets meant to stay anchored. Curve uses a hybrid invariant:

```
A · nⁿ · Σxᵢ + D = A · D · nⁿ + Dⁿ⁺¹ / (nⁿ · Πxᵢ)
```

where `D` is the total value of the pool and `A` is the amplification coefficient. When `A = 0`
this reduces to the CPMM invariant. When `A → ∞` it behaves like a constant sum with zero
slippage — a stablecoin vending machine. Real pools set `A` to a large finite value (often
100–2000+), producing a curve that is nearly flat (near-zero slippage) near the peg and curves
away for large imbalances. The engine solves for `D` via Newton's method and then solves for
the new output balance given the input.

**The working-copy model** is a backtesting-specific concern with no real-world analog. In
production, a swap actually changes the on-chain pool reserves and every subsequent transaction
sees the updated state. In backtesting, the simulated trade never happened — the pool's real
historical state does not include your hypothetical swap. The engine handles this by maintaining
a working copy of the pool state that diverges from historical state as you apply simulated
swaps. This working copy persists until the next observed `PoolState` event arrives and resets
it to actual historical state. This is correct for isolated trades but can accumulate artifacts
for strategies submitting many closely-spaced swaps before the next observed snapshot. The
engine records this assumption in results.

**Gas** is a first-class P&L line because on EVM chains it can exceed the economic value of
small trades. Gas cost = `gas_used × gas_price` in the chain's native token (ETH on Ethereum,
MATIC on Polygon). Gas prices are volatile — on Ethereum they have ranged from under 5 gwei to
over 500 gwei in the same month. Gas is converted to the accounting currency via the native
token's price at the event timestamp and debited via `Account`. On Solana, transaction fees are
fractions of a cent and negligible. For a strategy that trades small positions on Ethereum, gas
routinely makes the strategy uneconomical — the engine surfaces this rather than burying it in
noise.

### What is unique about this engine

This is the only engine whose pricing is a mathematical formula over pool state rather than a
negotiation against a counterparty or order book. It is the only engine where there is no such
thing as a limit order — every trade is a market swap, period. It is the only engine where the
pricing math differs not just in parameters but in fundamental algorithm depending on the pool
variant (`amm_variant` on the instrument). The working-copy isolation model is unique to this
engine because it is the only engine where the simulated trades would hypothetically have changed
the market state that was being replayed. The gas P&L line exists in Engine G as well (for NFTs
on Ethereum), but Engine B is the primary engine where gas modelling is architecturally central
to correctness.

### Data contract requirements

**Instrument fields required for Engine B:**

```jsonc
{
  "id": "USDC-ETH-0.3@uniswap_v3.dex",
  "price_formation": "AMM",            // routes to Engine B
  "tick_size": 1,                      // tick = 1 (integer ticks in v3)
  "lot_size": 1,
  "contract_multiplier": 1.0,
  "settlement": "OnChain",
  "currency": "USD",

  "capabilities": ["HasPoolReserves", "HasGasCost"],
  // For v3 concentrated liquidity also add:
  "capabilities": [..., "HasConcentratedLiquidity"],

  "amm_variant": "UniswapV3",          // UniswapV2 | UniswapV3 | Curve | Raydium
  "pool_address": "0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8",
  "token_0": { "address": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", "symbol": "USDC", "decimals": 6 },
  "token_1": { "address": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", "symbol": "WETH", "decimals": 18 },
  "fee_bps": 30
}
```

**Minimum data requirements for Engine B:**

| Level | Requirement | Result if absent |
|---|---|---|
| Absolute minimum | `PoolState` stream | `DataSufficiencyError` — run rejected |
| `HasGasCost` set | `GasEvent` stream OR static gas in `execution_defaults` | `DataSufficiencyError` |
| v3 exact fidelity | `tick_data` inside `PoolState` | Run proceeds; `degraded_fidelity` warning; approximate mode |

**Market data payload requirements for Engine B:**

Required:
- `PoolState` events — carries the pool reserves, `sqrt_price_x96`, `current_tick`,
  `liquidity`, and (for v3 exact mode) the `tick_data` vector

Optional but strongly recommended:
- `tick_data` inside `PoolState` for v3 — without it, the engine falls to approximate mode
  and flags results as lower fidelity
- `SwapEvent` stream (`HasSwapEvent`) — historical AMM transactions; distinct from `PoolState`
  snapshots; enables intra-snapshot transaction detail modeling

Gas data (required when `HasGasCost` set):
- A `GasEvent` stream (gas price per block) OR a static gas price in `execution_defaults`

**data bindings in the Run Request for a v3 pool (descriptor format):**

```jsonc
"data": {
  "reader": "arrow_ipc",
  "bindings": {
    "USDC-ETH-0.3@uniswap_v3.dex": {
      "pool_states": {
        "uri":           "path/to/usdc_eth_pool_states.arrow",
        "payload_class": "PoolState"
      },
      "swap_events": {
        "uri":           "path/to/usdc_eth_swaps.arrow",
        "payload_class": "SwapEvent"
      },
      "gas_events": {
        "uri":           "path/to/eth_gas_prices.arrow",
        "payload_class": "GasEvent"
      }
    }
  }
}
```

**Account port requirement:** required for any strategy using non-trivial sizing (anything that
references buying power or equity), and required for gas debiting when `HasGasCost` is set.

---

## Engine C — NAV (`NAV`)

### How the market and the engine work

Net Asset Value pricing is the mechanism underlying mutual funds and it creates a fundamentally
different execution environment from anything order-book based. A mutual fund is not traded on
an exchange. There is no bid-ask spread, no order book, and no intraday price. The fund's price
is computed exactly once per trading day, typically at 4:00pm Eastern time when the US equity
markets close. The NAV is `(total assets − total liabilities) / shares outstanding`. Total assets
are valued at the closing prices of the fund's underlying holdings. When you submit a buy order
(a subscription) to a mutual fund, you do not know the price you will pay. You submit the order,
and if the cutoff time has passed, your order prices at tomorrow's NAV. If you submit before the
cutoff, you get today's NAV — but you only find out the actual price after the market closes and
the fund administrator finishes computation. This is forward pricing, and it is a legal
requirement under the Investment Company Act of 1940. It prevents strategies from timing the
fund based on intraday moves.

The engine reproduces this by accepting subscription and redemption orders during the day with
no price, then waiting for the NAV valuation event (typically the 4pm close event in the data),
striking the NAV at that point, and filling all pending orders at that price. A strategy cannot
observe the fill price at decision time because that price does not yet exist. This is the
critical behavioral difference from Engine A, and it matters for any strategy that tries to act
on intraday information.

NAV computation is provide-or-derive. The caller can supply an official `Nav` payload stream —
the actual published daily NAV from the fund administrator — in which case the engine uses it
directly. Alternatively, if the caller provides the fund's current holdings and the prices of
those holdings, the engine computes NAV from first principles: `NAV = (Σ holdingᵢ · priceᵢ −
liabilities) / shares_outstanding`. The `iNAV` (indicative intraday NAV) that some fund
providers publish is a lower-fidelity estimate computed from real-time prices of the holdings.
It is valid only as a signal for premium/discount strategies and is explicitly never used as a
fill price because it is not the official NAV.

**Premium/discount** is the key ETF metric: `(market_price − NAV) / NAV`. The ETF trades on
an exchange via Engine A (at intraday bid/ask prices), while Engine C computes the NAV and
premium/discount in parallel as a valuation layer. Authorized Participants — large institutions
with the right to create and redeem ETF shares at NAV — normally keep the premium/discount near
zero through arbitrage. When it widens (during market stress when the underlying basket is
illiquid), it signals either a mispricing opportunity or a breakdown in the arbitrage mechanism.

**Leveraged and inverse ETF daily reset** is the most misunderstood mechanic in retail investing
and the engine must simulate it correctly. A 2× leveraged ETF does not deliver 2× the cumulative
return of the index over any holding period longer than one day. It delivers 2× the daily return,
reset each day. Consider: the index goes +10%, −10%, +5% over three days. The index ends at
100 × 1.10 × 0.90 × 1.05 = 103.95. The 2× ETF, applying 2× each daily return:

```
Day 1: fund_nav = 100 × (1 + 2 × 0.10)      = 120.00
Day 2: fund_nav = 120 × (1 + 2 × −0.10)     =  96.00
Day 3: fund_nav =  96 × (1 + 2 × 0.05)      = 105.60
```

A naive analyst might expect 2× the cumulative return — `100 × (1 + 2 × 0.0395) = 107.90`.
This is wrong. The product never promised that and does not deliver it. In volatile sideways
markets this daily reset produces volatility decay (also called beta slippage) where the fund
systematically underperforms `N×` the cumulative index return even with a positive trend.
The engine simulates each day's reset individually:
`fund_return_t = leverage_factor × daily_index_return_t − expense_ratio/252`. It never
shortcuts by scaling cumulative returns.

**Expense ratio** drag is accrued daily as `expense_ratio / 252` subtracted from the daily
return for all fund types — not just leveraged ones. For a 10-year backtest, compounding a
0.03% daily drag vs. a 0.20% daily drag produces materially different results.

### What is unique about this engine

This is the only engine with forward pricing — the fill price is unknown at order submission
time, which is a real behavioral constraint with no analog in any other engine. It is the only
engine with two roles in one: it executes trades for mutual funds (as the primary engine) and
provides a valuation layer for ETFs alongside Engine A (as a composition). The leveraged/inverse
daily reset simulation is entirely unique to this engine. The premium/discount computation and
the distinction between iNAV (signal-only) and NAV (fill price) are unique to this engine.

### Data contract requirements

**Instrument fields required for Engine C (mutual fund, pure NAV execution):**

```jsonc
{
  "id": "VTSAX@vanguard.mutualfund",
  "price_formation": "NAV",            // routes to Engine C
  "tick_size": 0.001,
  "lot_size": 1,
  "contract_multiplier": 1.0,
  "settlement": "T+1",
  "currency": "USD",
  "capabilities": ["HasNAV"],
  // If holdings-based NAV derivation is needed:
  "capabilities": [..., "HasBasket"]
}
```

**Instrument fields for Engine A + Engine C composition (ETF):**

```jsonc
{
  "id": "SPY@nyse.etf",
  "price_formation": "CLOB",           // executes via Engine A
  "capabilities": ["HasOrderBook", "HasNAV", "HasBasket", "HasSessions"],
  // For leveraged/inverse ETFs also add:
  "capabilities": [..., "HasDailyReset"],
  "leverage_factor": 2.0,              // 1.0 = standard, +2.0 = 2× long, −1.0 = inverse
  "daily_reset": true
}
```

**Minimum data requirements for Engine C:**

| Level | Requirement | Result if absent |
|---|---|---|
| Absolute minimum | `Nav` stream OR (`HoldingsSnapshot` + underlying price data for all holdings) | `DataSufficiencyError` — run rejected |
| Holdings-derived NAV | `HoldingsSnapshot` binding + each underlying's price data in the same run | `DataSufficiencyError` if holdings reference an unbound underlying |
| ETF premium/discount | `Nav` stream alongside Engine A data | `degraded_fidelity` if absent |

**Market data payload requirements for Engine C:**

For mutual funds:
- `Nav` events (official NAV) — required unless `HasHoldings` is set and `HoldingsSnapshot` is provided
- `HoldingsSnapshot` events — required if NAV is to be derived from basket (`HasHoldings` set)
- `Bar` or `Mark` for each underlying holding — required for holdings-derived NAV

For ETFs (in addition to Engine A's bar/quote/book data):
- `Nav` events — provides the official NAV for premium/discount computation
- `HoldingsSnapshot` (`HasHoldings`) — optional; enables derived NAV computation
- `CreationRedemptionBasket` (`HasCreationRedemption`) — optional; enables authorized-participant basket modeling

**data bindings in the Run Request (descriptor format):**

```jsonc
"data": {
  "bindings": {
    "VTSAX@vanguard.mutualfund": {
      "nav_events": {
        "uri":           "path/to/vtsax_nav.arrow",
        "payload_class": "Nav"
      }
    },
    "SPY@nyse.etf": {
      "bars_1d": {
        "uri":           "path/to/spy_1d.arrow",
        "payload_class": "Bar",
        "interval":      "1d",
        "adjusted":      false
      },
      "nav_events": {
        "uri":           "path/to/spy_nav.arrow",
        "payload_class": "Nav"
      },
      "holdings": {
        "uri":           "path/to/spy_holdings.arrow",
        "payload_class": "HoldingsSnapshot"
      }
    }
  }
}
```

---

## Engine D — Cash Flow (`DEALER`)

### How the market and the engine work

Fixed income is a completely different mental model from equity. When you buy a bond you are
making a loan to the issuer — a government, municipality, or corporation. The economics are not
primarily about price appreciation. They are about the present value of a contractually defined
cash-flow stream: periodic coupon payments over the life of the bond, plus the return of face
value (par) at maturity. The price of a bond is the market's current consensus on what that
future stream of payments is worth, discounted at the prevailing yield.

Bond pricing from first principles: a bond with face value `F`, annual coupon rate `c`, coupon
frequency `f` (semiannual = 2), maturity in `N` periods, and yield-to-maturity `y` has a
present value:

```
P = Σ_{t=1}^{N} [ (c·F/f) / (1 + y/f)^t ]  +  F / (1 + y/f)^N
```

When `y = c`, `P = F` — the bond prices at par. When `y > c`, the bond prices below par (at a
discount) because the required yield exceeds what the coupon pays. When `y < c`, the bond prices
above par (at a premium). This inverse relationship — yields up means prices down, yields down
means prices up — is the central truth of fixed income and every risk measure in this engine
flows from it.

YTM (yield-to-maturity) is solved by root-finding on the same equation when only the price is
known. Newton's method converges in a few iterations for non-pathological bonds.

**Duration and DV01** quantify price sensitivity to yield changes. Macaulay duration is the
weighted average time to receive the bond's cash flows, where each flow is weighted by its PV
as a fraction of the total price — effectively the bond's "average maturity" in present-value
terms. Modified duration adjusts for the discounting frequency: `ModDur = MacDur / (1 + y/f)`.
DV01 (Dollar Value of 01) is the price change for a one-basis-point yield move:
`DV01 = ModDur × P × 0.0001`. Convexity is the second-order term — the curvature of the
price-yield relationship. Because the relationship is convex (price falls less per unit yield
rise than it gains per unit yield fall), long bonds benefit asymmetrically from yield moves.
The full price change estimate: `ΔP/P ≈ −ModDur × Δy + ½ × Convexity × (Δy)²`. All of these
are provide-or-derive: if the data includes pre-computed risk measures, the engine uses them;
if absent, it derives them from the cash flows.

**Accrued interest and the clean/dirty price distinction** is a bookkeeping convention
fundamental to correct P&L computation. Bonds pay coupons at discrete intervals, but interest
accrues continuously in between. The clean price is what's quoted in the market — the bond's
price excluding accrued interest. The dirty price (also called the full price or invoice price)
is what the buyer actually pays: `dirty = clean + accrued`. Accrued interest is
`coupon × (days_since_last_coupon / days_in_period)`, computed under the instrument's
day-count convention. US Treasuries use ACT/ACT (actual calendar days). US corporate bonds use
30/360 (every month treated as 30 days, every year as 360 days). T-bills and money market
instruments use ACT/360. These conventions affect the accrued interest computation and getting
them wrong produces small but systematic errors in P&L. The engine maintains accrued interest
as a running state variable, incrementing it daily and resetting it to zero on each coupon
payment (which is also credited to `Account` as a cash event).

**Yield derivation and curve pricing** handle the cases where a direct price quote is absent.
For illiquid bonds, you may only have the current Treasury yield curve and a credit spread. The
yield curve maps maturity (6m, 1y, 2y, 5y, 10y, 30y) to yield. For a 7-year A-rated corporate
bond, you look up the 7-year Treasury yield, add the credit spread for A-rated 7-year
corporates, and discount the bond's cash flows at that composite yield. A `CreditRating`
downgrade widens the credit spread, causing an immediate repricing — a real and significant P&L
event for a bond holder. The engine applies rating changes in strict `ts_event` order before
pricing at that timestamp so you don't compute the new (worse) price before the downgrade has
been announced.

**The dealer fill model** reflects actual bond market microstructure. Unlike equities, most
bonds do not trade on an exchange with a public order book. They trade OTC via electronic
platforms (Bloomberg TSOX, MarketAxess, Tradeweb) where dealers quote bid and offer. You
request a quote, a dealer responds, and you either take it or don't. The spread varies
enormously by liquidity tier: on-the-run Treasuries trade at fractions of a basis point;
illiquid corporate bonds may have spreads of 50–100bps. The engine models this as:
buy at `dirty_price + half_spread`, sell at `dirty_price − half_spread`, with spread calibrated
to the bond's liquidity tier as a configurable parameter.

**MBS prepayment** is the most complex fixed-income mechanic. A mortgage-backed security pools
thousands of mortgages. Homeowners have the option to prepay at any time — when rates fall,
they refinance, paying off the old mortgage early. This is an embedded call option held by the
borrower, not the MBS investor. When rates fall and prepayments accelerate, the MBS investor
receives principal back right when they would most want to keep it (because now they'd reinvest
at lower rates). When rates rise, prepayments slow and the investor is stuck with a lower-yielding
security longer than expected. This makes MBS negatively convex — price appreciation during
falling rates is damped by the prepayment option. The engine models this with a prepayment curve
(PSA model or CPR curve), which specifies the expected annualized prepayment rate as a function
of the rate environment. Principal scheduled for prepayment at each period is returned early,
shortening effective duration and reshaping the remaining cash-flow stream.

Roll-down return — the gain from a bond aging along the yield curve over time without any
yield-curve shift — is naturally captured in the daily mark series (mark change net of coupon
accrual) and requires no separate computed field.

### What is unique about this engine

This is the only engine built around a scheduled cash-flow stream as the primary value driver
rather than continuous trading. It is the only engine that carries accrued interest as running
state. It is the only engine where the pricing computation is a root-finding problem (YTM solve).
It is the only engine where day-count conventions matter and affect fill prices. It is the only
engine with a prepayment model for path-dependent principal repayment. Duration, convexity,
and DV01 as provide-or-derive outputs are unique to this engine.

### Data contract requirements

**Instrument fields required for Engine D:**

```jsonc
{
  "id": "US10Y@treasury.bond",
  "price_formation": "DEALER",         // routes to Engine D
  "tick_size": 0.0001,
  "lot_size": 1000,                    // minimum face value
  "contract_multiplier": 1.0,
  "settlement": "T+1",
  "currency": "USD",

  "capabilities": ["HasCoupon", "HasYield", "HasMaturity"],
  // For credit risk sensitivity also add:
  "capabilities": [..., "HasCreditRisk"],
  // For leveraged bond positions (repo financing) also add:
  "capabilities": [..., "IsLeveraged"],

  "coupon_rate": 0.04,                 // 4% annual
  "coupon_frequency": 2,               // semiannual
  "day_count": "ACT/ACT",             // ACT/ACT | 30/360 | ACT/360
  "par_value": 1000,
  "maturity_date": "2034-11-15",
  "credit_rating": "AAA"               // used for spread lookup
}
```

**Minimum data requirements for Engine D:**

| Level | Requirement | Result if absent |
|---|---|---|
| Absolute minimum | Any one of: `Bar`/`Mark` with clean prices, `YieldUpdate`, `YieldCurve` | `DataSufficiencyError` — run rejected |
| `HasCoupon` set | `Coupon` stream | `DataSufficiencyError` |
| `HasCreditRisk` set | `CreditSpread` or `CreditRatingEvent` stream | `DataSufficiencyError` |
| Curve-derived mode | `YieldCurve` + `CreditSpread` for the issuer/rating | `DataSufficiencyError` if `YieldCurve` absent and no direct price |

**Market data payload requirements for Engine D:**

For liquid bonds with direct price quotes:
- `Bar` or `Mark` with clean prices (the engine converts to dirty price internally)

For yield-derived pricing:
- `YieldUpdate` events carrying current instrument-specific YTM and optional spread-over-treasury

For curve-derived pricing:
- `YieldCurve` events — full benchmark curve (Treasury, SOFR, OIS) at each timestamp
- `CreditSpread` events — issuer/rating spread over the benchmark curve (`HasCreditRisk`)
- `CreditRatingEvent` events — rating changes triggering immediate repricing (`HasCreditRisk`)

Always required when `HasCoupon` is set:
- `Coupon` events — carries next payment timestamp, payment amount, accrual

**data bindings in the Run Request (descriptor format):**

```jsonc
"data": {
  "bindings": {
    "US10Y@treasury.bond": {
      "bars_1d": {
        "uri":           "path/to/us10y_daily.arrow",
        "payload_class": "Bar",
        "interval":      "1d",
        "adjusted":      false
      },
      "yield_updates": {
        "uri":           "path/to/us10y_yields.arrow",
        "payload_class": "YieldUpdate"
      },
      "coupon_events": {
        "uri":           "path/to/us10y_coupons.arrow",
        "payload_class": "Coupon"
      },
      "yield_curves": {
        "uri":           "path/to/treasury_curves.arrow",
        "payload_class": "YieldCurve"
      },
      "credit_spreads": {
        "uri":           "path/to/corp_spreads.arrow",
        "payload_class": "CreditSpread"
      },
      "credit_ratings": {
        "uri":           "path/to/rating_changes.arrow",
        "payload_class": "CreditRatingEvent"
      }
    }
  }
}
```

**Account port requirement:** required for coupon credit events (`apply_fill` called with coupon
cash), for maturity principal repayment, and for leveraged bond positions (repo financing).

---

## Engine E — Derivatives (`CHAIN`)

### How the market and the engine work

Options are contracts that give the holder the right but not the obligation to buy (call) or
sell (put) an underlying asset at a specified price (the strike) on or before a specified date
(expiry). The seller receives the premium upfront and accepts the obligation. This asymmetric
payoff — unlimited upside for the buyer, capped at premium for the seller — means that pricing
an option is fundamentally an exercise in probabilistic expectation under a risk-neutral measure.

The core inputs to option pricing are: the current underlying price `S`, the strike `K`,
time to expiry `T` (in years), the risk-free rate `r`, the dividend/carry yield `q`, and the
implied volatility `σ`. Of these, `σ` is the only one that cannot be observed directly — it is
backed out from the market price of the option. If you know all other inputs and the option's
market price, you solve the pricing formula backwards to find the `σ` that produces that price.
This is the implied volatility, and it is the market's consensus expectation of future realized
volatility of the underlying over the option's remaining life.

**Black-Scholes-Merton** is the foundation. It assumes the underlying follows geometric
Brownian motion, continuous time, no transaction costs, constant volatility, and lognormal
returns. Under these assumptions:

```
d1 = [ln(S/K) + (r − q + σ²/2)·T] / (σ·√T)
d2 = d1 − σ·√T
Call = S·e^(−qT)·N(d1) − K·e^(−rT)·N(d2)
Put  = K·e^(−rT)·N(−d2) − S·e^(−qT)·N(−d1)
```

`N(d2)` is the risk-neutral probability that the option expires in-the-money. The engine
computes these exactly with the continuous dividend yield `q` modifying the forward price.

**American options** require different treatment because early exercise can be optimal. For a
deep-in-the-money put with high interest rates, exercising now and investing the proceeds may
be worth more than holding the put for its remaining optionality. For a call on a
high-dividend stock just before the ex-dividend date, exercising early to capture the dividend
can be optimal. BSM gives a lower bound on the American price. The correct American price
requires a numerical method: either a binomial tree (discretize time to expiry into N steps,
build the stock price tree, solve backwards applying `max(intrinsic, continuation)` at each
node) or a PDE solver (finite differences on the Black-Scholes PDE with the early exercise free
boundary). The Barone-Adesi-Whaley approximation is a fast analytic alternative. The pricing
model is behind a pluggable trait: `model(S, K, T, r, q, σ, style) → (price, greeks,
exercise_boundary)`. Which numerical method is behind the trait is the open decision OD-4.

**Greeks** are the sensitivities of the option price to each input variable and are the primary
risk management language for options portfolios:

- **Delta (Δ):** `∂price/∂S` — how much the option price changes per $1 move in the
  underlying. Call delta: 0 to 1. Put delta: −1 to 0. ATM options have delta ≈ ±0.5.
  Used for delta hedging — trading the underlying in the delta-equivalent quantity to make
  the position insensitive to small moves.
- **Gamma (Γ):** `∂²price/∂S²` = `∂Δ/∂S` — how fast delta changes per $1 move. High gamma
  means frequent rebalancing is needed. Highest for near-the-money options near expiry.
- **Vega (ν):** `∂price/∂σ` — price change per 1% change in implied volatility. Long options
  are long vega (benefit from rising vol). Short options are short vega. Reported per 1% vol.
- **Theta (Θ):** `∂price/∂T` — daily time decay. Long options lose value daily (negative
  theta). Short options earn daily (positive theta).
- **Rho (ρ):** sensitivity to the risk-free rate — small except for long-dated options.

The engine computes all greeks at each fill and records them in the `TradeRecord`. This enables
greek P&L attribution in the metrics layer: decomposing option P&L into delta (directional),
gamma (convexity), vega (vol moves), and theta (time decay) components.

**The IV surface and its usage.** Options on the same underlying trade at different implied vols
depending on strike (the volatility smile or skew) and expiry (the term structure). Deep OTM
puts on equities typically trade at much higher implied vol than ATM options — this is the put
skew, reflecting demand for downside insurance. The IV surface encodes all of this as a 2D grid
of `σ(K, T)` values. At each event the engine looks up `σ(K, T)` by interpolating bilinearly
across the log-moneyness axis `log(K/F)` (where `F` is the forward price) and the expiry axis.
The surface is taken as-of the current event timestamp — using a later surface for an earlier
date is look-ahead and is structurally prevented.

**Early exercise and assignment** require the engine to check, every event for every open
American position, whether exercising is optimal and whether the short side would be assigned.
The exercise boundary — the underlying price above/below which exercise is optimal — is an
output of the numerical pricing model. When the underlying crosses that boundary, the engine
either flags optimal exercise to the strategy (for long positions) or triggers assignment (for
short positions). Assignment on a short call results in the engine opening a short position in
the underlying against `Account` at the strike price. Assignment on a short put results in
buying the underlying at the strike price.

**Fill fidelity** for options is a serious practical concern. Option quotes can be extremely
wide (5–10% of mid for illiquid strikes), stale, or completely absent for deep OTM or far-dated
contracts. When live quotes are available, the engine fills against bid/ask exactly. When only
the IV surface is available, it fills at model price ± a spread estimate that widens with
OTM-ness and illiquidity. When only OHLCV bars are available with no IV surface, the engine
flags the result as low accuracy — this is not a usable path because option OHLCV bars conflate
underlying price moves with IV moves, and without separating them you cannot know whether a
price change was directional or a vol regime shift.

**Margin mechanics for short options:** long options require only the premium paid upfront.
Short options require margin — the engine queries collateral from `Account` every event and
force-liquidates if collateral is insufficient against the marked short option value. This is
the same margin mechanic as Engine A's `HasLiquidation`.

### What is unique about this engine

This is the only engine that is explicitly a valuation engine rather than purely a matcher —
it prices contracts from the IV surface on every single event regardless of whether a trade
is being made. It is the only engine where the data manifest has a hard requirement (IV surface
— not optional, not provide-or-derive — it is required or the run is rejected). It is the only
engine where greek computation is a core output of every event cycle rather than a derived
post-processing metric. The early-exercise/assignment state machine is unique to this engine.
The pluggable pricing model trait (the OD-4 decision) means the engine is the only one where
the core pricing computation is explicitly designed to be swapped out.

### Data contract requirements

**Instrument fields required for Engine E:**

```jsonc
{
  "id": "AAPL-2025-01-17-C200@cboe.option",
  "price_formation": "CHAIN",          // routes to Engine E
  "tick_size": 0.01,
  "lot_size": 1,
  "contract_multiplier": 100,          // 100 shares per US equity option contract
  "settlement": "Physical",            // Physical | Cash
  "currency": "USD",

  "capabilities": ["HasGreeks", "HasIVSurface", "HasExpiry"],
  // For American-style options also add:
  "capabilities": [..., "HasEarlyExercise"],
  // For short option margin also add:
  "capabilities": [..., "HasLiquidation", "IsLeveraged"],

  "expiry_date": "2025-01-17",
  "strike": 200.00,
  "option_type": "Call",               // Call | Put
  "exercise_style": "American",        // American | European

  // IsLeveraged (for short option margin):
  "initial_margin_rate": 0.20,
  "maintenance_margin_rate": 0.15
}
```

**The underlying instrument must also be in the run** with its own data bindings. The engine
reads `S` (the underlying price) at every event — it cannot price the option without the
underlying being on the shared event clock.

**Minimum data requirements for Engine E:**

| Level | Requirement | Result if absent |
|---|---|---|
| Absolute minimum (both required) | `IVSurface` for the underlying | `DataSufficiencyError` — run rejected |
| Absolute minimum (both required) | Underlying price data (`Bar`/`Quote`/`Trade`) | `DataSufficiencyError` — run rejected |
| High-fidelity fills | Option `Quote` events | Run proceeds; `degraded_fidelity`; surface-only fill model |
| `HasEarlyExercise` set | `ExerciseEvent` stream | `degraded_fidelity`; assignment modeled from pricing model only |

**Market data payload requirements for Engine E:**

Hard required (run rejected without either):
- `IVSurface` events for the underlying — the grid of `σ(K, T)` values at each timestamp
- `Bar`, `Quote`, or `Trade` events for the underlying price `S`

Strongly recommended (improves fill fidelity from surface-only to quotes):
- `Quote` events for the option itself — enables bid/ask fill model

Optional (provide-or-derive):
- `Greeks` events — if provided, used directly; if absent, derived from BSM

Option-specific lifecycle events:
- `ExerciseEvent` — when `HasEarlyExercise` is set

**data bindings in the Run Request (descriptor format):**

```jsonc
"data": {
  "bindings": {
    "AAPL@nasdaq.equity": {
      "bars_1d": {
        "uri":           "path/to/aapl_1d.arrow",
        "payload_class": "Bar",
        "interval":      "1d",
        "adjusted":      false
      }
    },
    "AAPL-2025-01-17-C200@cboe.option": {
      "bars_1d": {
        "uri":           "path/to/aapl_c200_bars.arrow",
        "payload_class": "Bar",
        "interval":      "1d",
        "adjusted":      false
      },
      "quotes": {
        "uri":           "path/to/aapl_c200_quotes.arrow",
        "payload_class": "Quote"
      },
      "iv_surface": {
        "uri":           "path/to/aapl_iv_surface.arrow",
        "payload_class": "IVSurface"
      }
    }
  }
}
```

---

## Engine F — Synthetic (`OTC`)

### How the market and the engine work

OTC derivatives are contracts negotiated bilaterally between two parties — typically an
institution and a dealer bank — with terms customized to the parties' exact needs rather than
standardized by an exchange. CFDs, equity swaps, barrier notes, structured products, and
autocalls all live here. What unifies them is that their value is defined by a rule over one
or more underlyings, the rule is often path-dependent, and there is no public exchange listing
their price.

**The payoff-component model** is the central architectural decision for this engine. Rather
than hard-coding payoff logic in the engine or allowing strategies to embed arbitrary logic in
their JSON, the engine demands that every synthetic instrument reference a named, registered
payoff component from the component registry. This component is a pure deterministic function:

```
payoff_value = payoff_component(underlyings_PIT, contract_params, prior_state) → (value, new_state)
```

The function takes the current values of the underlyings (all point-in-time — only values at
or before `current_ts`), the contract parameters, and whatever path-dependent state has
accumulated so far, and returns the current mark value plus the updated state. The engine is
completely payoff-agnostic — it calls this function at every event and uses the return value as
the mark. The purity requirement is load-bearing: given the same inputs the component always
returns the same output, which makes backtests deterministic, prevents look-ahead (the component
sees only PIT underlying values), and enables sandboxing of untrusted or AI-generated payoff
logic in a WASM runtime where it has no I/O, no clock, and no network access.

**A CFD** is conceptually simple: you and the dealer agree to exchange the difference between
the entry price and exit price of an underlying asset. You never own the underlying. P&L is
exactly the mark-to-market change in the underlying times your notional, minus the overnight
financing cost. The payoff component for a vanilla CFD is `mark = direction × notional ×
(current_underlying_price / entry_price − 1)`. The overnight financing — typically LIBOR/SOFR
+ a dealer spread, applied to the full notional — accrues via `Account` daily and is reported
as a separate P&L line from the directional payoff.

**A barrier note** is a structured product where the payoff depends on whether the underlying
has touched a certain price level at any point during the product's life. A knock-out note
terminates if the underlying touches the barrier — the investor receives a recovery amount and
the product ends. A knock-in note is dormant until the underlying touches the barrier, at which
point it activates. The engine maintains a state machine for each barrier product. On every
`on_event`, it checks whether the current underlying mark has breached the barrier (for
continuous observation) or whether a scheduled observation date has arrived and the observation
price is beyond the barrier (for discrete observation). On breach, the payoff component
transitions state — the engine is executing a schedule of checks and state-transition callbacks.
Path dependence lives in the component's state, not in the engine.

**An autocall** is a structured note that automatically redeems early at scheduled observation
dates if the underlying is above a recall level. It pays a coupon for holding. If it never
triggers the recall, it may have a barrier on the downside (investor loses principal if the
underlying falls below the barrier at maturity). The engine drives the observation schedule:
at each observation date, it evaluates the recall condition via the payoff component. If
triggered, it forces settlement — the payoff component returns the early-redemption amount,
credited to `Account`. If not triggered, the periodic coupon accrues. If all observation dates
pass, the product goes to maturity and the component evaluates the final barrier condition.

**A convertible bond** composes Engine D and Engine E inside a single payoff component. It has
a bond floor computed exactly as Engine D would compute it (PV of fixed income payments). It
also has a conversion option — the holder can convert the bond into a fixed number of equity
shares. The convertible's value is `max(bond_floor, conversion_value) + time_value_of_option`.
The payoff component internally calls the same BSM/PDE pricing primitives Engine E exposes for
option valuation. The engine itself knows none of this — it calls `payoff_component(...)` and
gets back a mark.

**OTC fill execution** has no order book and no price discovery. You execute at the marked
value (as computed by the payoff component) plus a bid-ask spread and any financing setup cost,
both specified in the contract's metadata as counterparty terms.

### What is unique about this engine

This is the only engine where the pricing computation is entirely delegated to an external
registered component. It is the only engine where the concept of "what is the payoff of this
contract" is not built into the engine at all — the engine is intentionally payoff-agnostic.
It is the only engine with a barrier state machine. It is the only engine that composes with
both Engine D (cash flow) and Engine E (options math) through the payoff component layer. The
WASM sandbox for untrusted payoff logic is architecturally unique to this engine because this
is the only context where truly arbitrary financial contracts might be authored outside of the
core team.

### Data contract requirements

**Instrument fields required for Engine F:**

```jsonc
{
  "id": "AAPL-CFD-LONG@db.synthetic",
  "price_formation": "OTC",            // routes to Engine F
  "tick_size": 0.01,
  "lot_size": 1,
  "contract_multiplier": 1.0,
  "settlement": "Cash",
  "currency": "USD",

  // No standard capability flags uniquely gate Engine F;
  // the payoff component handles all instrument-specific mechanics.
  // The underlying(s) must be in the same run with their own instrument definitions.
  "capabilities": []
}
```

The `payoff_component_id` (a registered component ID) is resolved at runtime through the
`components` section of the Run Request — not stored in the instrument definition, because the
instrument definition is caller-supplied static metadata and the component lives in the registry.

**Minimum data requirements for Engine F:**

| Level | Requirement | Result if absent |
|---|---|---|
| Absolute minimum | All underlyings referenced by the payoff component must have price data bound | `DataSufficiencyError` — run rejected |
| Absolute minimum | Payoff component registered in `components` | `DataSufficiencyError` — run rejected |

**Market data payload requirements for Engine F:**

There are no Engine-F-specific payload variants. The engine requires whatever the payoff
component requires, which means:

- All underlyings referenced by the payoff component must be in the run with their own data
  bindings (their `instrument_id` appears in the `instruments` array of the Run Request)
- The `PoolState`, `Bar`, `Quote`, or `IVSurface` data appropriate for each underlying is
  bound in `data.bindings` under that underlying's instrument ID, using the descriptor format

**Run Request `components` binding — required:**

```jsonc
"components": {
  "aapl_cfd_payoff": {
    "kind": "builtin",            // builtin = ships with the suite
    "ref": "vanilla_cfd"
  },
  "barrier_note_payoff": {
    "kind": "wasm",               // untrusted/custom payoff runs in WASM sandbox
    "uri": "path/to/barrier_note.wasm"
  }
}
```

---

## Engine G — Marketplace (`MARKETPLACE`)

### How the market and the engine work

NFT markets are structurally unlike any other financial market the suite simulates. NFTs are
non-fungible: each token is unique. One unit of AAPL is perfectly interchangeable with any other
unit of AAPL. But CryptoPunk #7804 is not interchangeable with CryptoPunk #1 — they are distinct
objects with potentially very different values based on their traits. This means there is no
single "price" for an NFT the way there is a price for a stock. There is a floor price — the
lowest currently-listed price for any token in the collection — but this is a lower bound on
value, not the price of any specific token.

There is no continuous order book. The market structure consists of listings (sellers placing
tokens at fixed prices on marketplaces like OpenSea, Blur, or Magic Eden) and bids (buyers
specifying a maximum price for any qualifying token in a collection). When a buyer purchases,
they select a specific listed token and pay the listing price plus fees. When a seller decides
to sell, they either list at a price and wait for a buyer or accept an outstanding collection
bid. Prices are observed only when sales actually occur. Between sales, the token has no
verifiable market price.

**Buy execution** in the engine matches this reality exactly. A buy order specifies a
collection, optional trait filters, and a maximum price. The engine looks at what was actually
listed at the decision timestamp — not what could theoretically be listed, not an interpolated
price, but actual observed listings from historical data. If any listing exists at or below the
max price matching the trait filters, the engine fills at the lowest qualifying listing price.
Total cost: `listing_price + gas + marketplace_fee + creator_royalty`. If no qualifying listing
exists, the order is simply unfilled. This is not a failure state or an error — it is a
realistic and informative outcome. A strategy that bids aggressively on high-rarity tokens at
floor prices will have very low fill rates, and the engine shows that honestly rather than
inventing fills.

**Sell execution** is the harder modeling problem. In a real NFT sale, the seller lists a token
at a price and waits. The waiting time depends on demand for the collection, the listing price
relative to floor, the token's rarity, and the current market regime. There is no historical
order book showing you what bids existed at each moment in time. What you have is a stream of
actual completed sales. The engine uses conservative matching by default: a listing fills only
when an actual observed comparable sale occurs in the historical data at or above the listing
price for the same collection (and, if available, comparable trait profile). If no such sale is
ever observed in the run window, the token goes unsold. The demand model extension estimates
fill probability and timing distribution from historical demand patterns — higher fidelity, more
assumptions, all disclosed.

**Marking to floor** is the only practical approach to unrealized P&L but is explicitly
imprecise. If you hold a token that cost 5 ETH to buy and the current floor price is 3 ETH,
the engine marks your position at 3 ETH. But your token might have rare traits commanding a 2×
premium over floor (making it worth ~6 ETH), or you might have a token so generic it cannot
attract even floor-price buyers. The engine cannot model individual token rarity premiums because
there is no stable relationship between trait rarity and price premium across market regimes —
bull markets produce wild rarity premiums; bear markets collapse them entirely. The engine
records the floor-price mark source and its limitations explicitly in every result.

Rarity scores are caller-provided metadata. The suite owns no rarity methodology — multiple
calculation methods exist (simple rarity score, information-theoretic, harmonic mean) and
they produce meaningfully different rankings for the same collection. The engine uses provided
scores only for trait-filtered buy matching and for attributing realized rarity premium
(sale price vs. floor). It does not model a rarity-premium curve.

**Costs** are three always-populated P&L lines: marketplace fee (0.5–2.5%), creator royalty
(0–10%, enforcement varies by marketplace), and gas. On Ethereum Mainnet during normal
conditions, gas for a single NFT purchase is $20–100. During high-congestion periods (major
mint events, bull runs) it has been $500+. For strategies trading floor-priced tokens in the
$500–2000 range, gas is 5–20% of trade value. The engine treats it as a first-class P&L line.

**Positions are keyed by `{collection_address, token_id}`** — not a fungible quantity. This is
architecturally significant: the engine's position accounting is per-token, not per-collection.
Owning three tokens in the same collection at different prices is three distinct positions, not
one position of size three.

**Uncertainty disclosure** is mandatory. NFT backtest results have inherently high variance:
data is sparse (a given token may trade once per week at best), the fill model makes assumptions
about sale arrival, floor price is a rough proxy for individual value, and NFT market regimes
are extremely unstable. The engine attaches an uncertainty report to results: days-to-sell
distribution, fill rate, sensitivity to the sale-arrival assumption.

### What is unique about this engine

This is the only engine where positions are tracked by unique token ID rather than by fungible
quantity. It is the only engine where fill availability depends on whether specific tokens were
actually listed at your price at the specific historical timestamp (not just whether the asset
was "tradeable"). It is the only engine with a mandatory uncertainty disclosure attached to
results — not as a caveat but as a structured output field. It is the only engine that uses
floor price as the mark and explicitly documents that this mark is a lower bound on individual
token value, not a true mark. It is the only engine where there are no `Bar` payloads for the
individual token — only `NftEvent` and `FloorUpdate`.

### Data contract requirements

**Instrument fields required for Engine G:**

```jsonc
{
  "id": "BAYC-7804@opensea.nft",
  "price_formation": "MARKETPLACE",   // routes to Engine G
  "tick_size": 0.001,                 // ETH precision
  "lot_size": 1,                      // always 1 (non-fungible)
  "contract_multiplier": 1.0,
  "settlement": "OnChain",
  "currency": "ETH",

  "capabilities": ["IsUnique", "HasFloor", "HasGasCost"],
  // If rarity-filtered buying is needed:
  "capabilities": [..., "HasRarity"],
  // IsIlliquid is always set for NFTs:
  "capabilities": [..., "IsIlliquid"],

  "collection_address": "0xBC4CA0EdA7647A8aB7C2061c2E118A18a936f13D",
  "token_id": "7804"
}
```

**Minimum data requirements for Engine G:**

| Level | Requirement | Result if absent |
|---|---|---|
| Absolute minimum | `NftEvent` stream | `DataSufficiencyError` — run rejected |
| `HasFloor` set + strategy holds positions | `FloorUpdate` stream | `DataSufficiencyError` (positions cannot be marked) |
| `HasGasCost` set | `GasEvent` stream OR static gas in `execution_defaults` | `DataSufficiencyError` |
| Sell-side bid liquidity | `NftBidEvent` stream | `degraded_fidelity`; sell fills only on observed comparable sales |

**Market data payload requirements for Engine G:**

Required:
- `NftEvent` events — listings, delistings, and completed sales for the collection

Required when `HasFloor` is set and strategy holds positions:
- `FloorUpdate` events — collection floor price updates for marking

Optional (improves sell fill accuracy and buy-side matching):
- `NftBidEvent` events — active collection bids; enables immediate sell-side fill against bids
- Trait metadata for each token (provided as static metadata, not a streaming payload)

Gas data (required when `HasGasCost` set):
- `GasEvent` stream or static gas price in `execution_defaults`

**data bindings in the Run Request (descriptor format):**

```jsonc
"data": {
  "bindings": {
    "BAYC-7804@opensea.nft": {
      "nft_events": {
        "uri":           "path/to/bayc_events.arrow",
        "payload_class": "NftEvent"
      },
      "floor_updates": {
        "uri":           "path/to/bayc_floor.arrow",
        "payload_class": "FloorUpdate"
      },
      "nft_bids": {
        "uri":           "path/to/bayc_bids.arrow",
        "payload_class": "NftBidEvent"
      },
      "gas_events": {
        "uri":           "path/to/eth_gas.arrow",
        "payload_class": "GasEvent"
      }
    }
  }
}
```

---

## Engine H — Event Resolution (`ORACLE`)

### How the market and the engine work

Prediction markets are markets where you bet on the outcomes of real-world events — elections,
sports, economic data releases, regulatory decisions. The tradeable unit is a binary contract:
you buy YES shares (which pay $1 if the event occurs) or NO shares (which pay $1 if the event
does not occur). The price of a YES share at any time is the market's implied probability of
the event occurring. If YES trades at $0.68, the market collectively believes there is a 68%
chance the event resolves YES. The price is the probability — not a proxy for it, not an
indicator of it. It is the probability, in dollar form.

This is a fundamentally different type of instrument. It has no underlying asset whose price
determines value, no yield, no cash flow, no greeks, no leverage mechanics in the traditional
sense. Its entire value is probabilistic and resolves to a binary outcome. The engine is
designed around this reality.

**The lifecycle state machine** is architecturally central. A market is `Created` when the
contract is deployed. It becomes `Active` when trading opens — YES/NO shares can be freely
bought and sold. At some point the underlying event either occurs or reaches a deadline: the
market becomes `Locked` — trading stops because information asymmetry is too extreme (some
participants may have advance knowledge of the outcome). The oracle reports the outcome, the
market becomes `Resolved`, and finally `Settled` when payouts are credited. The engine enforces
that fills are rejected once the market is `Locked`. This is critical for backtest honesty — a
strategy must not be able to buy YES at 10¢ in the hours before a known outcome is announced.

**Probability-bounded trading** uses Engine A's matching primitives. YES and NO tokens on
venues like Polymarket have actual CLOBs — resting limit orders at specific probability prices.
The fill model for market orders walks the order book exactly as Engine A does. The
`price(YES) + price(NO) = $1` constraint means you can equivalently express any position as
long YES or short YES (= long NO). A trade cannot occur at exactly $0.00 or $1.00 before
resolution because those are degenerate — a contract known with certainty to resolve one way
is not a market.

**Resolution and binary payoff:**
- YES wins → YES holders receive $1/contract, NO holders receive $0
- NO wins → NO holders receive $1/contract, YES holders receive $0
Positions are closed at the binary payoff via `Account`. Until resolution, unrealized P&L is
simply `position × current_price − cost_basis`.

**Resolution timing uncertainty** distinguishes prediction markets from every other
time-bounded derivative in the suite. A futures contract expires on a known date. An option
has a fixed expiry. But "will the Fed cut rates in March?" resolves when the FOMC meeting
occurs — which is scheduled — and the outcome becomes definitively known at a specific moment
that cannot be anticipated. More importantly, the oracle reporting that outcome has its own
latency and dispute mechanism. On Polymarket via UMA, a resolution can be disputed by
bond-posting challengers and takes up to 48 hours to finalize. The engine models this by
waiting for the actual `Resolution` event in the data, not by expiring at a calendar date.

**Liquidity degradation near the lock date** is an irreducible modeling challenge. As the event
approaches and the outcome becomes more certain based on public information, the order book
becomes increasingly one-sided. By the day before resolution, you might only be able to buy YES
at $0.95 or sell at $0.60 — spreads that reflect extreme adverse selection risk. The engine
fills against whatever historical book data exists, which will reflect actual historical
liquidity (or lack thereof). The result reports the `days_held_before_lock` distribution so
analysts can assess how much of the strategy's activity fell in this high-uncertainty window.

**Oracle risk** has no analog in traditional markets. Resolution is determined by a human or
governance process interpreting a real-world event. This process can be wrong (a controversial
outcome disputed by multiple data sources), delayed (natural disasters interfering with data
collection), or manipulated (oracle dispute mechanisms being gamed). The engine flags positions
affected by dispute events and optionally applies a configured probability of incorrect
resolution — not to predict it, but to let the analyst see result sensitivity to oracle failure.

**Brier score and calibration metrics** are the primary performance evaluation framework for
prediction market strategies. A strategy that consistently buys YES at 70¢ on events that
resolve YES 70% of the time is well-calibrated — its probability estimates match actual
frequencies. The Brier score `mean((p − outcome)²)` measures this directly: `p` is the entry
price (the implied probability you paid) and `outcome` is 0 or 1. A perfect predictor has
Brier = 0. A random predictor has Brier = 0.25. The engine records the entry price at each
trade specifically so this metric is computable downstream.

**CLOB extension (highest-priority open item for this engine):** Polymarket runs a full CLOB
for major markets on Polygon; Kalshi is an SEC-regulated exchange with standard CLOB mechanics.
Resting limit orders on YES/NO outcome tokens are the normal trading mechanic for liquid
prediction markets, not a fringe extension. The order-type matrix currently marks limit orders
as unsupported for Engine H. This should be promoted to fully supported when `HasClob` is set
on the instrument.

### What is unique about this engine

This is the only engine where the tradeable value is a probability bounded strictly to (0, 1)
by definition. It is the only engine with a lifecycle state machine that prevents fills after a
specific lifecycle event (`Locked`) rather than after a calendar expiry date. It is the only
engine where the settlement event is data-driven (a `Resolution` payload) rather than
calendar-driven. It is the only engine where calibration metrics (Brier score, log score) are
the primary performance framework rather than P&L-based metrics. It is the only engine where
oracle risk — the possibility that the resolution mechanism itself fails — is a modeled risk
factor with an explicit disclosure in results.

### Data contract requirements

**Instrument fields required for Engine H:**

```jsonc
{
  "id": "US-ELECTION-2024-TRUMP@polymarket.prediction",
  "price_formation": "ORACLE",         // routes to Engine H
  "tick_size": 0.01,                   // $0.01 probability increment
  "lot_size": 1,                       // 1 share minimum
  "contract_multiplier": 1.0,
  "settlement": "Cash",
  "currency": "USD",

  "capabilities": ["IsBinary", "HasResolution", "HasProbabilityPrice"],
  // If oracle dispute is possible (most markets):
  "capabilities": [..., "HasOracleRisk"],

  "question": "Will Donald Trump win the 2024 US Presidential Election?",
  "resolution_criteria": "Resolves YES if Donald Trump wins the electoral college...",
  "oracle_type": "UMA"                 // UMA | Centralized | Kleros | ...
}
```

**Minimum data requirements for Engine H:**

| Level | Requirement | Result if absent |
|---|---|---|
| Absolute minimum | YES/NO price stream (`Bar` or `Quote`) | `DataSufficiencyError` — run rejected |
| Absolute minimum | `Resolution` event stream | `DataSufficiencyError` — run rejected (cannot simulate settlement) |
| Lifecycle accuracy | `MarketLifecycleEvent` stream | `degraded_fidelity`; engine cannot gate fills at `Locked` accurately |
| Oracle risk modeling | `OracleEvent` stream (when `HasOracleRisk` set) | Falls back to `dispute_occurred` on `Resolution` payload |
| CLOB fills (`HasClob`) | `BookSnapshot` + `BookDelta` for YES/NO book | `degraded_fidelity`; bar-based fill model used instead |

**Market data payload requirements for Engine H:**

Required:
- `Bar` or `Quote` events for YES/NO token prices during the `Active` lifecycle phase
- `Resolution` event — carries the outcome, oracle ID, and whether a dispute occurred

Strongly recommended:
- `MarketLifecycleEvent` — drives the Created → Active → Locked → Resolved → Settled state machine
  accurately; without it, lifecycle gating is approximated from `Resolution` event timing

Optional:
- `OracleEvent` — proposal, dispute, and final settlement detail for oracle risk modeling
- `BookSnapshot` and `BookDelta` for YES/NO token order book — enables Engine A matching
  primitives when `HasClob` is set (promotes limit order support)

**data bindings in the Run Request (descriptor format):**

```jsonc
"data": {
  "bindings": {
    "US-ELECTION-2024-TRUMP@polymarket.prediction": {
      "bars_1d": {
        "uri":           "path/to/trump_yes_daily.arrow",
        "payload_class": "Bar",
        "interval":      "1d",
        "adjusted":      false
      },
      "resolution": {
        "uri":           "path/to/trump_resolution.arrow",
        "payload_class": "Resolution"
      },
      "lifecycle": {
        "uri":           "path/to/trump_lifecycle.arrow",
        "payload_class": "MarketLifecycleEvent"
      },
      "oracle_events": {
        "uri":           "path/to/trump_oracle.arrow",
        "payload_class": "OracleEvent"
      }
    }
  }
}
```

---

## The Run Request — Line by Line

The Run Request is the per-invocation document that tells the suite how to execute one run.
The Strategy JSON says what the strategy is. The Run Request says how to run it this time —
binding it to concrete data, a time window, parameter values, and all injected ports.

The suite stores nothing. There is no session, no database, no persistence between invocations.
A Run Request wires in everything the run needs and is discarded when the run completes.

---

### Top-level envelope

```jsonc
{
  "schema_version": "1.0",
```
The version of the Run Request schema itself. The suite uses this to decide which parser to
invoke. Increment this when breaking changes are made to the top-level structure. This is not
the strategy version or the data version — it is strictly the schema version of this document.

```jsonc
  "request_id": "caller-correlation-id",
```
An opaque string the caller supplies. The suite echoes it in every result and TradeRecord for
the run. It is never stored, never interpreted, never used for routing. It exists purely so the
caller can correlate results back to the specific invocation that produced them — useful when
running many sweeps concurrently and receiving results asynchronously.

```jsonc
  "strategy": { /* inline Strategy JSON, or a handle the caller resolves */ },
```
The full strategy definition for this run. This is either the complete Strategy JSON object
inlined here, or a handle (an opaque reference the caller's infrastructure resolves to a
strategy JSON before the suite sees it). The suite never stores strategies — it receives one
per invocation, validates it, compiles it into an internal execution plan, runs it, and
discards it. The strategy defines the declarative pipeline: universe, features, models, alpha,
sizing, risk, and execution stages. It declares the parameter space (what is tunable and the
bounds); the Run Request provides the concrete values.

```jsonc
  "time": {
    "start":  "2020-01-01T00:00:00Z",
    "end":    "2024-12-31T23:59:59Z",
    "warmup": "60d"
  },
```
`start` and `end` define the window over which the backtest produces results — trades are
evaluated, metrics are computed, and TradeRecords are emitted. `warmup` is the period before
`start` for which market data must be available but during which no trades are evaluated. It
exists to allow indicators and model features to reach steady state before the measurement
period begins. A 20-period moving average needs 20 bars of history before its first valid
output — without warmup, the first 20 results of the measurement period would be based on an
uninitialized indicator. The warmup period is data you need to supply but whose trades and
metrics do not appear in the output.

```jsonc
  "instruments": [ /* Instrument definitions (or references) in scope */ ],
```
The complete set of tradeable instruments available in this run. Each entry is a full Instrument
object as defined in the Instrument Contract spec — containing `id`, `price_formation`,
`capabilities`, `tick_size`, `lot_size`, `contract_multiplier`, and all the
capability-gated static metadata (expiry dates, strikes, coupon schedules, pool addresses, etc.)
that the selected engine needs. The `price_formation` field on each instrument is the routing
key — it tells the suite which engine to instantiate for that instrument. No other field
determines engine selection.

---

### `data` — injected market data bindings

```jsonc
  "data": {
    "reader": "arrow_ipc",
```
The data reader type — the format and access mechanism the suite uses to load market data.
`arrow_ipc` means Apache Arrow IPC files (the primary format — zero-copy, columnar, extremely
fast to deserialize into the Rust hot loop). `parquet` means Parquet files (slower to load but
widely available). `injected:<id>` means a custom `DataReader` implementation the caller
provides at runtime — used when data is sourced from an in-memory store, a streaming feed, or
any proprietary source. The reader is specified once for the entire run; per-instrument format
overrides are not supported at the top level (but an `injected` reader can implement per-instrument
logic internally).

```jsonc
    "bindings": {
      "AAPL@nasdaq.equity": {
        "bars":         "s3://bucket/aapl_1m.arrow",
        "corp_actions": "s3://bucket/aapl_corp_actions.arrow"
      },
      "BTC-USD@coinbase.spot": {
        "bars":   "s3://bucket/btcusd_1m.arrow",
        "trades": "s3://bucket/btcusd_trades.arrow"
      }
    }
```
A map of instrument ID to its data sources. Each key must match an instrument ID in the
`instruments` array. Each value specifies which data streams are bound for that instrument
and where to find them. The keys inside the per-instrument object (`bars`, `trades`,
`quotes`, `funding`, `pool_states`, `iv_surface`, etc.) map to the payload class names
defined in the Market Data Contract. The suite validates each binding against the instrument's
required-data manifest at run start: if a required payload class is missing, the run is
rejected with a `ManifestViolation` error naming the specific missing class. Optional payload
classes produce warnings but allow the run to proceed at reduced fidelity.

---

### `account` — injected ledger

```jsonc
  "account": {
    "port": "injected:my_ledger",
```
Identifies which `Account` implementation to use. `"injected:my_ledger"` means the caller
provides a concrete implementation of the `Account` trait at runtime. `"reference"` means
use the optional reference adapter the suite ships as a convenience (single-currency cash
ledger, basic margin support). The `Account` port is the caller's portfolio and accounting
model — the suite queries it for equity, buying power, positions, and collateral, and reports
simulated fills to it. The suite never defines the accounting internals; it only defines the
interface (the `Account` trait).

```jsonc
    "config": {
      "base_currency": "USD",
      "starting_balance": 100000,
```
When using the reference adapter, `starting_balance` sets the initial cash in the account.
This value lives in the Account, not in the suite — the suite sees it only by querying
`account.equity(...)`. For the injected port, this config object is passed through to the
caller's implementation without interpretation.

```jsonc
      "settlement": "T+2",
```
The caller's settlement cycle. T+2 means equity trades settle two business days after
execution. This affects when buying power is actually updated in the account after a fill.
The suite reports fills immediately; the account decides when those fills are settled into
cash and positions. The settlement mechanics are entirely the account's responsibility.

```jsonc
      "margin_model": "..."
    }
  },
```
The margin model the reference account adapter uses when `IsLeveraged` or `HasLiquidation`
capabilities are active. For the injected port, this is caller-defined.

The `account` section is optional. It is required only if the strategy uses account-relative
sizing (e.g. `fixed_fractional` sizing needs `equity`), account-relative risk rules (e.g.
`max_drawdown_pct` needs an equity curve), or margined instruments (perps, short options, or
any `IsLeveraged` instrument needs `collateral` and `maintenance_margin`). For a pure
absolute-sizing strategy on non-margined instruments, it may be omitted entirely.

---

### `models` — injected model port bindings

```jsonc
  "models": {
    "news-sentiment": {
      "adapter": "onnx",
      "uri":     "s3://bucket/news_sentiment_v3.onnx",
      "version": "3.2.1"
    }
  },
```
Resolves the `model_id@version` references declared in the strategy's `models` stage. The key
(`"news-sentiment"`) matches the `model_id` in the strategy JSON. `adapter` specifies the
runtime format — `onnx` for ONNX Runtime (the primary cross-platform model format), `pytorch`
for TorchScript, `custom` for a caller-injected inference function. `uri` is the path or
address of the model artifact. `version` must exactly match the version pinned in the strategy
JSON — a version mismatch is a validation error, not a warning, because mismatched versions
can silently change model behavior and invalidate reproducibility. The suite never stores model
weights and never manages model artifacts — the caller resolves model handles to artifacts and
provides them here.

---

### `trainer` — injected Trainer port

```jsonc
  "trainer": {
    "port": "injected:training_pipelines"
  },
```
The injected `Trainer` implementation. Required only if the strategy's `models` section
includes a `training` block with `"enabled": true`. The `Trainer` port is the caller's
training algorithm infrastructure — the suite orchestrates *when* to retrain (based on the
strategy's declared rolling window and step schedule) and *with what point-in-time data* (it
assembles the PIT training dataset from the bound features at the current simulation clock).
But the training algorithm itself — the actual model fitting, hyperparameter optimization,
gradient computation — is the caller's responsibility. The suite calls the Trainer with the
PIT dataset and the training method name (declared in the strategy JSON) and receives back a
new model artifact handle. If no strategy in this run uses training, this section is omitted.

---

### `components` — injected custom registry components

```jsonc
  "components": {
    "my_factor_model": {
      "kind": "wasm",
      "uri":  "s3://bucket/factor_model.wasm"
    },
    "top_n_by_volume": {
      "kind": "builtin"
    }
  },
```
Binds any custom components referenced in the strategy JSON by ID. Built-in components
(indicators, alpha functions, sizing methods, and payoff functions that ship with the suite
itself) need no binding — they are resolved automatically. `kind: "wasm"` deploys the component
into the WASM sandbox — a sealed WebAssembly runtime with no I/O, no clock, no network, and no
mutable global state. This is the trust tier for untrusted or AI-generated components. `kind:
"native"` loads a shared library compiled against the component ABI — trusted, fast, but
requires the caller to vouch for its safety. `uri` resolves to the component artifact. Components
referenced in the strategy but not bound here produce a `ComponentNotFound` validation error.

---

### `parameters` — concrete values or a sweep

```jsonc
  "parameters": {
    "rsi_period": 14,
    "oversold": 30
  }
```
Single-run form: each parameter name maps to a concrete value. The strategy JSON declared the
parameter space (type, min, max, default, step). The Run Request supplies the specific value
to use for this invocation. Every supplied value must fall within the declared bounds or the
run is rejected with a `ParameterBoundsViolation` error.

```jsonc
  "parameters": {
    "rsi_period": { "sweep": "grid",  "values": [10, 14, 20] },
    "oversold":   { "sweep": "range", "min": 20, "max": 35, "step": 5 }
  }
```
Sweep form: instead of a single value, each parameter declares a sweep space. The run queue
expands this into multiple single-run invocations — one per combination of parameter values
(for grid search) or as many as configured (for random or Bayesian search). Each expanded
invocation is a fully independent run with its own `request_id` (derived from the parent by
appending the parameter combination). The sweep search method and run queue orchestration are
governed by the runner spec (TBD) and open decision OD-2.

---

### `execution_defaults`

```jsonc
  "execution_defaults": {
    "latency": "100ms",
```
The default submission-to-fill latency for all instruments in this run. An order submitted at
time `t` becomes eligible to match against market data only at `t + 100ms`. This simulates
network and processing delay. Can be overridden per instrument in the Instrument definition.
For daily-bar backtests, latency is less meaningful (the resolution is one day) but for
intraday tick-level backtests it prevents fills on the data that triggered the decision.

```jsonc
    "slippage_model": "size_vs_volume",
```
The default slippage model used when book depth is absent (L1 or bar fidelity). `size_vs_volume`
is a square-root market impact model: `slippage_bps = k × √(order_notional / interval_volume)`,
where `k` is a calibration constant and `interval_volume` is the volume during the bar interval.
Alternative presets include `zero` (no slippage — optimistic, for research purposes only),
`fixed_bps` (a constant flat slippage regardless of size), and `conservative_crypto` (higher
constants calibrated to crypto liquidity conditions). When L2/L3 book data is available, this
model is not used — exact book walk slippage is computed directly.

```jsonc
    "intrabar_fill": "pessimistic"
  },
```
Governs the ambiguous case of whether an intrabar price touch (low touching a buy limit, high
touching a sell limit, or high/low triggering a stop) counts as a fill. `pessimistic` does not
count a touch as a fill — the price must clearly trade through. `optimistic` counts any touch
as a fill. `mid` uses the midpoint assumption. The default is pessimistic for all stop triggers
and for limit orders, because assuming best-case intrabar execution systematically overstates
strategy performance. The specific cases where this matters most: buy stops assumed to trigger
at exactly the stop price, and sell stops assumed to trigger at the worst intrabar low.

---

### `determinism`

```jsonc
  "determinism": {
    "seed": 12345
  },
```
The global random seed for the run. Every source of randomness in the suite — probabilistic
queue tie-breaks in the order book, the optional sandwich MEV model in Engine B, the optional
incorrect-resolution model in Engine H, the demand model in Engine G, and any seeded inference
in injected models — draws from a deterministic PRNG seeded by this value. Given an identical
Run Request with all identical injected ports (themselves deterministic), the same seed
guarantees byte-identical results regardless of thread count or hardware. This is a hard
invariant of the suite. Sweeps use derived seeds (`seed XOR parameter_hash`) so each swept
run is independently reproducible.

---

### `output`

```jsonc
  "output": {
    "mode": "stream",
```
`stream` emits TradeRecords and marks incrementally as they are produced during the run —
useful for long runs where the caller wants to process results progressively or display a live
equity curve. `batch` returns all results at the end of the run as a single collection — simpler
to consume but requires buffering the entire run's output in memory.

```jsonc
    "emit": ["trades", "marks", "model_lineage", "warnings", "metrics?"]
```
Selects which record types to emit. `trades` is the primary TradeRecord stream (always
recommended). `marks` is per-event valuation marks — useful for computing an equity curve but
can be very large for long runs with many instruments. `model_lineage` emits a record for each
model inference and, when training is active, a record for each refit (which model artifact
was active at each point in time — critical for reproducibility auditing). `warnings` emits
non-fatal contract warnings (missing optional payload classes, survivorship bias flag,
fidelity downgrade notifications). `metrics?` is annotated with `?` because aggregate metrics
(Sharpe, drawdown, Brier score) are not yet specified in the metrics contract (TBD in
`contracts/metrics.md`).

---

### `limits`

```jsonc
  "limits": {
    "max_runtime": "30m",
    "max_memory_mb": 8192
  }
}
```
Resource caps for the run. `max_runtime` causes the run to be terminated if it exceeds the
specified wall-clock duration — a safety guard for parameter sweeps where a poorly-formed
strategy or unexpectedly large dataset might cause a run to spin indefinitely.
`max_memory_mb` sets a resident memory cap. If either limit is exceeded, the run terminates
with a `ResourceLimitExceeded` error and emits whatever results were produced up to that point
(in stream mode) or no results (in batch mode). These limits are enforced by the runner and are
optional — omitting them means no cap, which is appropriate for trusted internal invocations
but dangerous for untrusted user-submitted runs.

---

## Inline bar derivation

The event loop derives bars from raw input data **during** the event loop (not as a pre-pass).
As raw ticks, order book events, and trade events arrive, the engine accumulates them into bars
and, when an interval closes, emits a `DerivedBar` into the strategy's feature pipeline with
`derived: true` and `source_class` set to the raw payload class it was derived from.

- **What is derived (necessity-driven):** the engine derives only the `(payload_class, interval)`
  pairs the compiled strategy plan and the fill model actually require — found by static analysis
  of the plan — not every standard interval. This bounds memory.
- **Boundaries:** wall-clock aligned, start at `:00`; daily bars run UTC-midnight to UTC-midnight.
- **Construction:** from trade prints (O=first, H=max, L=min, C=last, V=Σsize); fallback = quote
  mid `(bid+ask)/2` when no trades exist.
- **Direction rule:** coarser-from-finer always (1m from ticks; 1h from 1m); finer-from-coarser
  never — attempting it is a `DataSufficiencyError.non_derivable_conflict` (validation step 7).
- **Warmup gating:** the strategy makes no decisions until the minimum lookback bars have
  accumulated *during* the run; derivation runs through warmup.
- **Adjusted series:** derivable from unadjusted bars + `CorporateAction` under a declared
  `adjustment_method`; flagged derived; else signals needing it fail step 7.

Configuration lives in `derived_data.bar_derivation` in the Run Request (see run-request.md §4a).

---

## Cross-cutting data-handling rules

These resolved rules apply across every engine and complement the per-engine sections above.

- **Caller-provided data always wins.** Directly bound data (bars, indicators, greeks, NAV, …) is
  authoritative. Derivation is strictly a fallback for what was not provided. Anything the engine
  derives is flagged `derived: true` with `source_class` lineage and can be emitted for the caller
  to persist (`output.emit: ["derived_data"]`).
- **Reference data vs. event streams.** A binding is `event_stream` (replayed through the clock)
  or `reference` (loaded once, queried by timestamp). Reference entries carry `effective_ts` and
  `knowable_ts`; lookups enforce `knowable_ts ≤ current_ts` so a backdated value cannot leak future
  information. Entity-keyed reference data (issuer/universe/venue) is bound once in
  `reference_bindings`; instruments resolve it via their declared `issuer_id`.
- **Dynamic overrides static.** Live events override static configuration from their `ts_event`
  forward, with the static value as fallback: `TradingSession`/`TradingStatus` override the
  exchange calendar; `FeeScheduleUpdate` overrides the static `fee_schedule`.
- **Halt behavior.** On `TradingStatus: Halted`, resting orders freeze (not cancel) and resume at
  reopen (reopening auction if `HasAuction`); DAY orders still expire at session close.
- **Liquidation paths are disjoint.** The strategy's own liquidation is always engine-computed from
  Account + mark price. The external `LiquidationEvent` stream is other participants' liquidations
  only (cascade/market-impact). `AutoDeleveragingEvent` is the one external event that *can* close
  the strategy's own qualifying position (proportional, at the bankrupt trader's bankruptcy price).
- **AMM pool-state reconstruction.** With `SwapEvent` flow, Engine B replays real swaps between
  `PoolState` snapshots (higher fidelity) rather than holding the last snapshot constant.
- **NFT sells stay conservative.** Sell fills only on an observed comparable sale; `NftBidEvent`
  feeds the optional demand model, never an immediate fill.
- **Oracle ordering.** Locked → Proposal → (Dispute never re-opens trading) → FinalSettlement →
  Resolution; a `Resolution` may arrive with no `OracleEvent` for centralized oracles.
- **Universe gating.** A dynamic universe selector may only pick instruments that were in-universe
  (per point-in-time `UniverseMembership`) at that timestamp.
- **Watch-vs-trade.** The run's `instruments` is the full data set; the strategy's `universe` is the
  traded subset. Instruments in `instruments` but not `universe` are **watch-only** — read via
  cross-instrument references (`data:<instrument>.field`) but never traded (trade ETH, forecast BTC).
- **Universe-wide scan (cohort).** A `scanner` universe screens a **cohort data source** that
  materializes instruments point-in-time as they appear (engine-agnostic: DEX pairs → Engine B,
  IPOs → Engine A, NFT mints → Engine G). Each member still routes by `price_formation`.
- **Plans, not nested strategies.** Multiple strategies compose in a **Plan** by data-flow
  (screen `selector` → `entry` → `exit`, or concurrent independents), never by nesting. Capital is
  shared or isolated per `account_mode`; opposing intents resolve per `conflict_policy`. See
  [contracts/plan.md](contracts/plan.md).

---

## Validation order at run start

Before the first event is processed, the suite runs nine validation passes in strict order.
All nine must pass or the run is rejected with a precise, typed error. There is no partial
execution. (This mirrors the authoritative list in [run-request.md](run-request.md) §10; if the
two ever drift, the Run Request spec wins.)

**1. Schema:** the Run Request and Strategy JSON are parsed and type-checked against their
schemas. Any malformed JSON, missing required field, or type mismatch produces a
`SchemaValidationError` naming the specific field.

**2. Parameter bounds:** every value (or sweep range) supplied in `parameters` is checked
against the bounds declared in the strategy's `parameters` section. A value outside its
declared `[min, max]` range produces a `ParameterBoundsViolation`.

**3. Instruments:** each instrument's `price_formation` value is checked against the set of
known engines. Each capability set is checked for internal consistency (e.g. you cannot set
`HasFunding` without `HasMarkPrice` because funding uses the mark price). Each instrument's
`tick_size` and `lot_size` are checked for positive non-zero values.

**4. Data manifest:** for each `(instrument_id, engine)` pair, the suite checks that all
`required` payload classes declared in the instrument's data manifest are bound in
`data.bindings`. A single missing required payload class produces a `ManifestViolation`
naming the instrument and the missing class. Optional missing classes produce warnings but
do not block the run.

**5. Port presence:** if the strategy's sizing uses `fixed_fractional` or `volatility_target`,
the `account` port must be present. If the strategy uses margined instruments
(`IsLeveraged`, `HasLiquidation`), the `account` port must be present. If the strategy's
`models` section has training enabled, the `trainer` port must be present. If any strategy
stage references a component by ID, that component must be bound in `components`. Missing
required ports produce a `PortMissingError` naming the port and the strategy stage that
requires it.

**6. Capability/order-type:** the order types declared in the strategy's `execution` stage are
checked against the order-type matrix for each instrument's engine. A `limit` order on an
`AMM` instrument (`price_formation: "AMM"`) is a contract error, caught here before any event
is processed. A `swap_exact_in` order on a `CLOB` instrument is similarly caught. These
mismatches produce a `CapabilityOrderTypeError` naming the instrument, the engine, and the
invalid order type.

**7. Data sufficiency:** for each `(instrument_id, engine)` pair, the suite checks the bound
data descriptors against the per-engine minimum requirements. This pass uses the `payload_class`
and `interval` fields from each descriptor — it does not read any data files. A missing required
payload class that cannot be derived produces a `DataSufficiencyError` (hard rejection). A
non-derivable conflict (strategy feature requires finer resolution than provided) also produces
a `DataSufficiencyError.non_derivable_conflict` (hard rejection). Data that is sufficient to
run but at reduced fidelity produces `DataSufficiencyError.degraded_fidelity` warnings (run
proceeds, disclosed in results).

**8. Plan wiring:** if the bound `strategy` is a Plan (multiple strategies), every
`universe.from` names a strategy that exists in the Plan and has a compatible role, the wiring is
acyclic (a DAG of strategies), and `account_mode` / `conflict_policy` are resolvable. Violations
produce a `PlanWiringError` (see [contracts/plan.md](contracts/plan.md) §8). A single Strategy is
the degenerate one-node Plan and passes this trivially.

**9. Cohort / scanner:** if a `scanner` universe is used, its `cohort` names a bound cohort data
source (Run Request §4d) and every filter `field` resolves to a bound market-data or signal class.
A reference to an unbound cohort or an unresolvable filter field is a typed error.

---

## Run invariants

These hold for every run, unconditionally.

1. **No assumed portfolio.** The suite never owns cash, positions, or an equity curve. Account
   state is injected via the `Account` port. The suite owns the mechanics (sizing formulas,
   fill simulation, liquidation math). The caller owns the ledger.

2. **Stateless suite.** Nothing persists across runs. `request_id` and `strategy_id` are
   echoed in results but never stored. Running the same Run Request twice produces two
   independent, identical outputs.

3. **Deterministic.** Given identical Run Request + injected ports (themselves deterministic),
   results are byte-identical regardless of thread count, hardware, or OS. The global seed
   is the single source of randomness.

4. **Point-in-time.** Every binding, port query, model inference input, and training data
   window may only expose `ts_event ≤ current_ts`. The suite enforces this at the contract
   boundary. A strategy or model literally cannot be handed future data.

5. **Feed-agnostic strategy.** The Strategy JSON is identical for backtest and live execution.
   Only the Run Request differs (data bindings for backtest; a live feed adapter for live).
   A strategy that behaves differently in backtest vs. live indicates a contract violation,
   not a property of the strategy.

6. **Fail loud.** An under-specified run is rejected with a precise error before any event is
   processed. The suite never silently produces plausible-but-wrong results from incomplete
   data or missing ports.
