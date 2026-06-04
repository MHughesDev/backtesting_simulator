# Asset Spec: Crypto Spot (CEX)

**Engine:** A (Order Book / CLOB)
**`price_formation`:** `CLOB`
**Capabilities:** `HasOrderBook | HasMakerTakerFees | HasTokenEvents`

---

## 1. What CEX crypto spot is

Spot cryptocurrency on a centralized exchange (CEX) — Binance, Coinbase, OKX, Kraken —
is direct ownership of the token traded on a central limit order book. Mechanically identical
to equities execution, but with critical differences in fees, market hours, and lifecycle events.

### Key differences from equities

| Dimension | Equities | CEX Crypto Spot |
|---|---|---|
| Market hours | Session-based (9:30–4pm ET + pre/after) | 24/7/365, no close |
| Corporate actions | Yes (dividends, splits, mergers) | No corporate actions per se |
| Token events | N/A | Hard forks, airdrops, burns, chain migrations |
| Fee structure | Commission + ECN rebate/fee (often flat %) | Maker/taker tiered by 30-day volume |
| Minimum tick | Often $0.01 | Varies widely (e.g. $0.00001 for micro-caps) |
| Short selling | Via borrow market | Via margin lending on exchange (separate product) |
| Listing / delisting | Rare, regulated | Common, unannounced |

### Subtypes covered

- Major pairs: BTC/USDT, ETH/USDT, BTC/USD
- Altcoin pairs: any ERC-20, SPL, or native-chain token on a CEX
- Stablecoin pairs: USDC/USDT, BUSD/USDT (low vol, unique spread dynamics)

---

## 2. What a proper backtest requires

### 2.1 Market data

Same envelope as equities: OHLCV bars, quotes (BBO), individual trades. Key differences:

- **No session concept.** The event stream never gaps for overnight / weekend.
- **Timestamps must be UTC nanoseconds.** Crypto exchanges use high-precision timestamps;
  millisecond resolution is the minimum, nanoseconds preferred.
- **Pair base/quote matter.** `BTC/USDT` is a different instrument from `BTC/USD`. The quote
  currency determines the P&L denomination.

### 2.2 Fee structure — the most commonly mismodeled factor

Crypto exchanges use a **tiered maker/taker model** where the fee rate depends on the trader's
30-day rolling volume (and sometimes native-token holdings).

| Role | Definition | Effect |
|---|---|---|
| **Maker** | Adds liquidity — resting limit order that is later filled | Lower fee or rebate |
| **Taker** | Removes liquidity — market order or immediately-matching limit | Higher fee |

Example Binance spot tiers (approximate, changes over time):

| Tier | 30d Volume (BTC) | Maker | Taker |
|---|---|---|---|
| Regular | < 50 | 0.10% | 0.10% |
| VIP 1 | ≥ 50 | 0.09% | 0.10% |
| VIP 5 | ≥ 4,000 | 0.02% | 0.04% |

**Implication:** a strategy that is profitable at 0.10%/0.10% may be unprofitable at 0.04%/0.04%
and vice versa. The backtest must model the *effective* fee the strategy would have paid.
BNB (exchange token) discounts are an additional multiplier. The data contract must carry the
fee schedule as static metadata on the instrument.

A correct model also requires understanding **queue priority**: if the strategy places limit
orders, the fill model must account for resting orders ahead of it in the queue. Without
order-book depth this is estimated via size-to-volume ratio.

### 2.3 Slippage model

For CEX crypto, empirical slippage as a function of order size relative to daily volume:

| Size vs. daily volume | Estimated slippage |
|---|---|
| < 0.01% | Negligible (~0.01–0.05%) |
| 0.01–0.1% | 0.05–0.1% |
| 0.1–1% | 0.1–0.5% |
| > 1% | 0.5–5%+ (market-moving) |

These are approximations; the actual model uses order-book depth if available.

### 2.4 Token events

| Event | Effect | Required fields |
|---|---|---|
| **Hard fork** | Chain splits; holder receives new-chain tokens at 1:1 ratio | `fork_date`, `new_asset_id`, `ratio` |
| **Airdrop** | Free tokens credited to holders at a snapshot | `snapshot_date`, `airdrop_asset_id`, `amount_formula` |
| **Token burn** | Supply decreases; typically price-positive | `date`, `amount_burned` |
| **Chain migration** | Token swapped to new contract at fixed ratio (e.g. ERC-20 → mainnet) | `migration_date`, `new_asset_id`, `ratio` |
| **Exchange delisting** | Forced close at last available price | `date`, `final_price` |

Token events are less frequent and less formalized than equity corporate actions, but ignoring
them for positions held through forks or airdrops produces wrong P&L.

### 2.5 No borrow market on the engine

CEX crypto **spot** has no native borrow market for shorting — that lives in margin/futures
products (separate instruments). Engine A for spot treats all positions as long-only unless
the exchange's margin product is modeled separately.

---

## 3. Data contract

### Required manifest (Engine A, bar-level)

```
REQUIRED:
  InstrumentStatic {
    base_asset, quote_asset, exchange,
    tick_size, lot_size (min order qty),
    fee_schedule: [ { volume_tier, maker_bps, taker_bps } ],
    asset_class = CryptoSpot
  }
  Bar stream (UTC nanosecond timestamps)

OPTIONAL (improves fill modeling):
  Quote stream (BBO)
  Trade stream
  TokenEvent stream
```

### Payload variants used

| Payload | Description |
|---|---|
| `Bar { open, high, low, close, volume, interval }` | OHLCV |
| `Quote { bid, bid_size, ask, ask_size }` | BBO |
| `Trade { price, size, aggressor_side, trade_id }` | Tick print |
| `TokenEvent { event_type, … }` | Fork, airdrop, burn, migration |

---

## 4. Engine behavior (Engine A)

Same CLOB matching as equities with these overrides:
- No session gating — all orders accepted at any hour.
- Fee calculation uses the maker/taker schedule from `InstrumentStatic`.
- `TokenEvent` payloads are applied to open positions in time order.

---

## 5. Performance metrics (extensions)

| Metric | Notes |
|---|---|
| Effective fee rate | Average fee paid as % of notional; reveals if tier modeling is correct |
| Token event P&L | P&L contribution from forks and airdrops |
| BTC-denominated returns | Optional alongside quote-currency returns |

---

## 6. Implications for system design

1. **Fee schedule is instrument-level metadata, not a global constant.** Different exchanges
   charge differently; the same exchange charges differently per volume tier. The `InstrumentStatic`
   must carry the full schedule so the engine can compute the effective rate per fill.
2. **No concept of "market close."** Strategies that rely on daily bar events (e.g. close-to-open
   momentum) must explicitly define what "daily" means for a 24/7 market (typically UTC midnight).
3. **Token events are rare but material.** A Bitcoin Cash hard fork in 2017 credited BTC holders
   with BCH. A backtest holding BTC through that date and ignoring the airdrop understates returns.
4. **Slippage tables must be calibrated per asset.** A `BTC/USDT` backtest with a $50M order
   would apply very different slippage than a micro-cap altcoin with $100K daily volume.

---

## 7. Sources

- Paybis: how to backtest crypto bot with realistic fees and slippage
- DolphinDB: best practices for high-frequency backtesting of market-making strategies
- StratBase.ai: backtest with realistic fees and commissions
- Binance fee schedule (reference for tier structure, not a data contract)
