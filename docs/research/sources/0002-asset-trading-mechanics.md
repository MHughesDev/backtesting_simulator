# Asset Trading Mechanics: Research Sources

**Research ID:** 0002
**Date:** 2026-06-05
**Scope:** How each of the 11 asset classes is actually traded; what data a platform receives;
real costs and gotchas for simulation accuracy.

---

## 1. Equities (US: NYSE, NASDAQ, ARCA)

### Price formation
Central Limit Order Book (CLOB) with price-time priority. Sessions: pre-market 04:00–09:30 ET
(wide spreads, low volume), regular 09:30–16:00 ET (tight spreads), after-hours 16:00–20:00 ET.
Large-cap spreads: sub-cent. Micro-cap: 5–10 cents.

### Data shape
| Feed | Key fields |
|---|---|
| OHLCV bars | open, high, low, close, volume, interval |
| Quote (BBO) | bid, bid_size, ask, ask_size, ts_event |
| Trade ticks | price, size, aggressor_side |
| Corporate actions | ex_date, action_type, ratio_or_amount |
| Short borrow rates | borrow_rate_annual, as_of_ts, isin |

**Critical two-series requirement:** must maintain *adjusted* prices (backward-adjusted for
splits/dividends, used for signals) and *unadjusted* prices (actual traded prices, used for
fills and P&L) as parallel series. Merging them silently distorts results. This is non-negotiable
for correct backtesting.

### Real costs
- Bid/ask spread: 1 cent (large-cap) to 10 cents (micro-cap)
- Short borrow: 5–200%+ annualized for hard-to-borrow names; daily accrual
- Commission: now $0 at retail brokers; ECN rebate/fee at institutional level

### Gotchas
- **Survivorship bias**: backtesting only currently-listed stocks overstates returns. Full
  historical universe including delisted tickers required; result should flag if universe is
  survivorship-biased.
- **Corporate actions must be events, not post-processing.** Applied in the event stream in
  `ts_event` order before price matching at that timestamp.

---

## 2. ETFs

### Price formation
Execution: CLOB (identical to equities). Valuation: NAV of underlying basket. Authorized
Participants create/redeem shares to keep market price ≤ 1% from NAV (wider in stressed markets).

### Data shape
Same OHLCV/Quote/Trade as equities, plus:
| Feed | Key fields |
|---|---|
| NAV | nav, as_of_date (once daily, post-market) |
| iNAV | indicative_nav, ts (real-time; less accurate) |
| Holdings | { ticker, weight, shares }, as_of_date |
| Premium/discount | (market_price − nav) / nav × 100 |

### Leveraged/inverse daily reset
`fund_return = leverage_factor × daily_index_return − (expense_ratio / 252)`. Cumulative ETF
return ≠ N× cumulative index return due to volatility decay (beta decay). Must simulate each
daily reset; cannot scale returns.

### Mutual funds (NAV-only)
Orders fill at the *next* struck NAV (forward pricing). Route to Engine C exclusively; Engine A
does not apply.

---

## 3. CEX Crypto Spot (Binance, Coinbase, OKX)

### Price formation
CLOB. Market hours: 24/7/365 (no sessions). UTC nanosecond timestamps required.

### Data shape
| Feed | Key fields |
|---|---|
| OHLCV bars | open, high, low, close, volume, interval |
| Quote (BBO) | bid, bid_size, ask, ask_size, ts_event |
| Trade ticks | price, size, aggressor_side, trade_id |
| Fee schedule | volume_tier, maker_bps, taker_bps |
| Token events | event_type (fork/airdrop/burn/migration/delisting), snapshot_ts |

### Real costs
- Maker fee: 0.02–0.10% tiered by 30-day volume (e.g., Binance regular: 0.10%; VIP 5: 0.02%)
- Taker fee: 0.03–0.10%
- Spread: 2–10 bps for major pairs; 50+ bps for altcoins

### Gotchas
- Fee schedule is instrument-level metadata; different exchanges/tiers produce different results.
- No "daily close." Strategies using daily bars must define what "daily" means (typically UTC
  midnight).
- Token events are material (BTC holders received BCH at the 2017 fork).

---

## 4. DEX / AMM (Uniswap v2/v3, Curve, Raydium)

### Price formation
Algorithmic — price derived from pool formula, not an order book. Every trade is a market swap.

### Data shape
| Feed | Key fields |
|---|---|
| Pool state (v2) | reserve_0, reserve_1, fee_bps, block_number, ts |
| Pool state (v3) | sqrt_price_x96, current_tick, liquidity, fee_bps, tick_spacing, tick_data[] |
| Swap history | amount_in, amount_out, gas_used, block_number, tx_index |
| Gas price | base_fee_wei, gas_price_wei, block_number |

### Formulas
**v2 CPMM** (standard): `Δout = (R_out · Δin_eff) / (R_in + Δin_eff)` where `Δin_eff = Δin·(1−f)`.
**v3 CLAMM**: price tracked as `√P`; tick crossings change active liquidity `L`; exact-in swap
within a tick: `√P_next = (L·√P) / (L + Δin_eff·√P)`. Requires per-tick data for full accuracy.
**Curve StableSwap**: lower slippage near peg via amplification parameter `A`.

### Real costs
- Pool fee: 0.01% (v3 stablecoin tier) to 1.00% (low-liquidity pairs); 0.30% = v2 default
- Gas (Ethereum): $5–$100+ per swap; can exceed trade value on small swaps
- Gas (Solana): ~$0.00025 per swap; negligible
- MEV/sandwich: 0–10%+ additional slippage on visible swaps above threshold
- Price impact: superlinear with swap size; modeled exactly from pool formula

### Gotchas
- **No limit orders.** `max_slippage_bps` = on-chain `minAmountOut` guard; exceeded → revert.
- **Failed transaction gas**: EVM reverts still consume gas; material for strategies with high
  revert rates.
- **v3 tick data is required** for full accuracy on large swaps; without ticks, engine must use
  approximate constant-L assumption and flag in results.
- **Pool state cadence**: swaps within a block change reserves; intra-block ordering requires
  full swap history.

---

## 5. Futures (CME, CBOT: ES, NQ, CL, ZB)

### Price formation
CLOB. Multiple contracts per underlying: `ES_2024-12-20` and `ES_2025-03-21` are separate
instruments with separate order books.

### Data shape
| Feed | Key fields |
|---|---|
| OHLCV (per contract) | open, high, low, close, volume, oi, interval |
| Settlement price | price, settlement_date |
| Continuous series | adjusted_price, roll_method, gap_annotation |

**ES contract specifics:** contract value = $50 × index; tick size = 0.25 index points = $12.50.

### Real costs
- Spread: 1 tick ($12.50 for ES) for liquid front-month
- Exchange fee: ~$1.23 per round-trip for ES (CME)
- Margin: ~10–20% of notional (initial); ~5–10% (maintenance)
- Roll cost: spread between front and back contract × position size

### Continuous series construction
For signal computation only (fills use per-contract prices):
- **Panama Canal (back-adjusted)**: subtract roll gap from all prior prices. Prices can go
  negative; returns are accurate; levels are not.
- **Proportional (ratio)**: multiply prior prices by ratio. Levels preserved; rates-of-change
  can distort.
- **Unadjusted stitch**: append at roll date. Creates artificial price gaps.
Engine stores the method used; strategy picks the appropriate series for signals.

### Gotchas
- Contract identity includes expiry date. ES front-month ≠ ES next-month.
- Roll must be explicit: strategy or `HasRollSchedule` config decides *when* to roll; engine
  does not roll silently.
- Two series mandatory: adjusted continuous (signals) and per-contract unadjusted (fills/P&L).
- Basis strategies (long spot / short futures) require both instruments in the same run.

---

## 6. Perpetual Futures (Binance, Bybit, dYdX)

### Price formation
CLOB + periodic funding to anchor price to spot. No expiry. Mark price (not last-traded) is the
reference for unrealized P&L, stops, and liquidation.

### Data shape
| Feed | Key fields |
|---|---|
| OHLCV bars | open, high, low, close, volume |
| Funding rate | rate, mark_price, next_funding_ts |
| Mark price | price, ts (per-second or per-funding-period) |
| Index price | price, ts (spot reference) |
| Liquidation events | price, position_size, side, ts |

### Funding rate formula (Binance standard)
```
premium_index    = (mark_price − index_price) / index_price
funding_rate     = clamp(premium_index + clamp(interest_rate − premium_index, −0.05%, 0.05%),
                         −cap, +cap)
funding_payment  = position_size × mark_price × funding_rate
```
Paid every 8 hours. Longs pay shorts when positive; reversed when negative.
Missing funding rates produces P&L errors of 20–50%+ for positions held over weeks.

### Linear vs. inverse
- **Linear** (BTC/USDT): collateral and P&L in quote (USDT). Standard.
- **Inverse** (BTC/USD on BitMEX): collateral and P&L in base (BTC). Non-linear:
  `pnl_btc = size × (1/entry − 1/exit)`, then `pnl_usd = pnl_btc × exit_btc_price`.

### Gotchas
- Funding is a scheduled event at each `next_funding_ts`, independent of bar cadence.
- Mark price, not last-traded, drives all margin/liquidation checks.
- Funding P&L and trading P&L must be separated in output; conflating them obscures return source.

---

## 7. Options (US Equity, Index, Crypto)

### Price formation
Valuation via pricing model (Black-Scholes baseline) + CLOB execution against the option chain.
Option price is fundamentally *derived* from underlying, IV surface, time, rate, and dividends.

### Data shape
| Feed | Key fields |
|---|---|
| Underlying price | OHLCV or Quote on underlying_id |
| IV surface | ts, underlying_id, strikes[], expiries[], iv_grid[][] |
| Option quotes | bid, ask, bid_size, ask_size, ts |
| Greeks | delta, gamma, theta, vega, rho |
| Dividend stream | ex_date, amount_per_share, underlying_id |

**IV surface is mandatory.** Using today's surface for historical dates produces grossly wrong
prices. Surface shape changes continuously (steepness, skew, term structure).

### Real costs
- Bid/ask spread: 1–5 cents (ATM liquid) to 50+ cents (far-OTM)
- Commission: ~$0.65 per contract
- IV premium: `entry_IV − realized_vol` (the primary P&L driver for vol strategies)

### Gotchas
- OHLCV-only is almost useless for options; heavy reliance on IV surface or option quotes.
- American vs. European style changes fair value; field must be validated and respected.
- Dividend stream required for American call early-exercise detection.
- Survivorship bias: must include chains that expired worthless, not only ITM-at-expiry.
- Delta-hedged strategies require the underlying in the same run with a shared clock.

---

## 8. Bonds / Fixed Income (US Treasuries, Corporates, Municipals)

### Price formation
OTC dealer market; no central exchange or order book. Price is yield-derived. Dealers quote
bid/ask; trader accepts or walks away.

### Data shape
| Feed | Key fields |
|---|---|
| Static metadata | cusip/isin, issuer, par_value, coupon_rate, coupon_freq, day_count, maturity_date, credit_rating |
| Coupon schedule | payment_date, coupon_amount, principal_payment |
| Clean price | price, ts |
| YTM | ytm, ts |
| Yield curve | tenors[], yields[], curve_type (Treasury/SOFR/Credit), ts |
| Credit spread (OAS) | spread_bps, ts |
| Duration/convexity | macaulay_dur, modified_dur, dv01, convexity |

### Day count conventions (matter for accrued interest)
- ACT/ACT: US Treasuries
- 30/360: corporate bonds
- ACT/360: T-bills

Using the wrong convention produces incorrect daily accrued interest.

### Real costs
- Bid/ask spread: 0.5–5 bps (on-the-run Treasuries) to 50+ bps (illiquid municipals)
- Repo financing: ~4–5% annualized cost to finance a leveraged long bond position
- Accrued interest: part of dirty price (not a separate fee)

### Gotchas
- Most bonds trade infrequently. Mark is model-derived (yield curve + credit spread + discounting)
  most of the time, not observed.
- Repo cost matters for leveraged bond strategies; long bonds financed on repo carry a daily
  financing cost analogous to funding rates on perpetuals.
- Roll-down return: as bond approaches maturity, yield converges to the curve → price
  appreciation if curve slopes up. This is a source of return distinct from coupon income.
- MBS prepayment changes effective duration unpredictably; requires a prepayment model.

---

## 9. FX (EUR/USD, GBP/USD, USDJPY)

### Price formation
Interbank dealer market (ECN/EBS/Reuters). No central exchange. Conceptually a CLOB of dealer
quotes, but continuous executable prices come from dealer streams, not a public resting book.

### Data shape
| Feed | Key fields |
|---|---|
| OHLCV bars | open, high, low, close, volume (notional) |
| Quote (BBO) | bid, ask, bid_size, ask_size |
| Swap rates | swap_long, swap_short, swap_date, currency_pair |

Sessions: Asian 00:00–09:00 UTC, London 07:00–16:00 UTC, NY 12:00–21:00 UTC.
Market hours: Sunday 21:00 UTC to Friday 21:00 UTC. Weekend gap.

### Real costs
- Spread: 0.5–2 pips major pairs / 1–5 pips minors / 10+ pips off-hours.
  London–NY overlap is tightest; off-hours 3–10× wider.
- Overnight swap: `position_size × rate_differential / 365` daily. Triple on Wednesday.
- Swap rate changes over time with central bank policy; must use historical rates.

### Gotchas
- Swap rates are time-varying; a static carry assumption is a common and material error.
- "Volume" is notional, not share count.
- Weekend gap: no fills from Friday close to Sunday open; positions marked at Friday's last price.
- Session liquidity: off-hours strategies face materially wider spreads; fixed slippage assumption
  produces overoptimistic results.
- Cross-currency P&L: EUR/JPY P&L is in JPY; must convert to base currency using JPY/USD.

---

## 10. NFTs (OpenSea, Blur, Magic Eden)

### Price formation
Marketplace listings and sales events. No continuous order book. Price is observed only at
moment of sale. Floor price = cheapest current listing in a collection (not a mid-market quote).

### Data shape
| Feed | Key fields |
|---|---|
| Collection metadata | collection_address, name, total_supply, unique_holders |
| Floor updates | collection_address, floor_price, listed_count, ts |
| Listing history | collection_address, token_id, listing_price, listing_ts, delisting_ts |
| Sales history | collection_address, token_id, sale_price, marketplace_fee, royalty, gas_used, ts |
| Token metadata | token_id, traits[], rarity_score, rarity_rank |

No Bar payloads for `IsUnique` instruments.

### Real costs
- Marketplace fee: 0.5–2.5%
- Creator royalty: 0–10% (enforcement varies by marketplace)
- Gas: $20–$500+ per transaction (Ethereum); negligible on Solana

### Rarity premium
Rarity methodology is caller-provided; engine owns none. Premium is highly unstable:
1/1 traits command 100× floor in bull markets, 1.2× in bear markets. No universal formula.

### Gotchas
- Positions keyed by `{collection_address, token_id}`; not fungible.
- Buy fills only against actual observed historical listings; engine cannot invent fills.
- Sparse data: events hours or days apart; no interpolation between sales.
- Uncertainty disclosure is mandatory; sparse liquidity makes results high-variance.
- Gas can dominate P&L for Ethereum; small trades may be structurally loss-making.

---

## 11. Prediction Markets (Polymarket, Kalshi, Manifold)

### Price formation
**Polymarket:** CLOB on Polygon for major markets (elections, Fed). Less-liquid markets may use
lighter AMM-like liquidity. Price = implied probability ∈ [0, 1].
**Kalshi:** US-regulated exchange; CLOB.
**Binary:** YES pays $1 if event resolves YES, $0 if NO. NO is inverse.

### Data shape
| Feed | Key fields |
|---|---|
| Market metadata | market_id, question, resolution_criteria, resolution_deadline, oracle_type |
| Outcome token OHLCV | open, high, low, close, volume (price range [0, 1]) |
| Order book depth | bbo, depth snapshots |
| Resolution event | outcome, resolution_ts, oracle_source |
| Oracle dispute stream | proposal_ts, dispute_ts, final_settlement_ts, dispute_outcome |

### Real costs
- Spread: 1–50 cents depending on liquidity
- Platform fee: ~2% (Polymarket; deducted from winnings)
- Oracle risk: resolution can be disputed or wrong; not a cost but a risk factor

### Gotchas
- Resolution is an event, not a date. Timing uncertain; engine waits for `Resolution` payload.
- Price bounded strictly (0, 1) until resolution; no trades at exactly $0 or $1.
- Liquidity dries up as resolution approaches; positions become effectively illiquid.
- Polymarket's Polygon-based CLOB for major markets is functionally identical to Engine A;
  confirming that the "promote limit orders to first-class" open item is a real-world necessity
  for major prediction markets.
- Brier score = primary calibration metric; raw P&L is secondary.

---

## Sources
- QuantConnect documentation (equity corporate actions, continuous futures)
- CME Group product specifications (ES contract, settlement, margin)
- Binance / Bybit API documentation (funding rate formula, maker/taker tiers)
- OptionMetrics IvyDB documentation (IV surface standards)
- Uniswap v2/v3 whitepaper (pool math, tick structure)
- arXiv 2410.09983 (Uniswap v3 concentrated liquidity)
- arXiv 2309.13648 (MEV costs on DEX)
- arXiv 2506.08573 (perpetual funding rate design)
- arXiv 2602.14350, 2603.19984 (options backtesting)
- Raymond James / CFA Institute fixed income references
- Polymarket / Kalshi product documentation
- BIS Quarterly Review March 2021 (ETF mechanics)
