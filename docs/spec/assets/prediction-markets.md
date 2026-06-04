# Asset Spec: Prediction Markets

**Engine:** H (Event Resolution)
**`price_formation`:** `ORACLE`
**Capabilities:** `IsBinary | HasResolution | HasOracleRisk | HasProbabilityPrice`

---

## 1. What prediction markets are

A **prediction market** is a contract that pays a fixed amount if a specified real-world event
occurs (YES = $1) and nothing if it does not (NO = $0). The price at any time is the market's
consensus probability that the event will occur.

Example: "Will the Fed cut rates in September 2025?" traded at 0.62 → market implies 62%
probability of a rate cut.

### How they trade

On modern platforms (Polymarket, since migration to Polygon/CLOB):
- Binary YES and NO shares trade on a CLOB, each priced between $0.01 and $0.99.
- YES + NO always sum to $1.00 (arbitrage enforced).
- The "price" of YES is the implied probability.
- Upon resolution, YES pays $1.00 and NO pays $0.00 (or vice versa).

### How they resolve

Resolution is via an **oracle** that observes the real-world outcome:
- **Polymarket:** uses the UMA Optimistic Oracle — a proposed resolution is submitted; a
  dispute window opens; if unchallenged it settles; if challenged, UMA token holders vote.
- **Augur (v1/v2):** decentralized oracle via REP token holders.
- **Centralized platforms:** operator resolves unilaterally.

**Oracle risk** is a unique feature of prediction markets: the resolution can be disputed,
delayed, or (rarely) resolved incorrectly. A "wrong" oracle resolution is a source of
non-market risk that cannot be backtested away.

### Why prediction markets need their own engine

1. **Binary payoff.** The instrument resolves to exactly $1 or $0 — it has no continuous
   fair value derivable from price action alone. Valuation is probability-based.
2. **Resolution is an event, not a time.** Unlike futures expiry (calendar-driven), a
   prediction market resolves when the event occurs. Time horizon is uncertain.
3. **Primary metric is calibration, not return.** A strategy that correctly predicts event
   probabilities may earn consistent returns even with low per-trade P&L.
4. **Price = probability.** There is no "fundamental value" beyond probability. A bond has a
   cash-flow-derived fair value; a prediction market YES share has exactly the probability of
   YES as its fair value.

---

## 2. What a proper backtest requires

### 2.1 Market data

Prediction markets have a CLOB, so the data structure is similar to other order-book assets.
Key difference: the "price" is bounded between 0 and 1 and represents a probability.

| Field | Notes |
|---|---|
| `market_id` | Unique market identifier |
| `question` | The event text (e.g. "Will X happen before date Y?") |
| `resolution_criteria` | How the outcome will be determined |
| `resolution_deadline` | Latest possible resolution date (some markets have open-ended timelines) |
| `creator` | Market creator address / entity |
| `outcome_tokens` | `[ { outcome: "YES", token_id }, { outcome: "NO", token_id } ]` |

OHLCV for the YES token is valid and useful (price moves as implied probability changes).

### 2.2 Order book data

PolyBackTest provides full historical order book depth at 1-minute resolution — the richest
available data for Polymarket strategies. At minimum:

| Field | Notes |
|---|---|
| `bbo` | Best bid and offer for YES and NO tokens |
| `order_book_depth` | Optional; L2 snapshot |
| `volume` | Total notional traded in interval |
| `spread` | Bid/ask spread (widens for obscure markets; tightens for major events) |

### 2.3 Resolution data

| Field | Notes |
|---|---|
| `resolution_ts` | Actual resolution timestamp |
| `outcome` | `YES` or `NO` |
| `resolution_source` | Oracle that provided the outcome |
| `dispute_occurred: bool` | Whether the resolution was challenged |
| `resolution_note` | Optional description of the decision |

Resolution data is required to compute realized P&L. Without it the backtest cannot close
positions or compute final returns.

### 2.4 Oracle event data

For oracle-aware strategies (e.g. event-driven resolution plays):

| Field | Notes |
|---|---|
| `proposal_ts` | When the resolution was first proposed |
| `dispute_ts` | If a dispute was filed |
| `final_settlement_ts` | When the settlement became final |
| `dispute_outcome` | If disputed, the final ruling |

### 2.5 Market lifecycle

Markets have a complete lifecycle that differs from other instruments:

```
Created → Active (trading) → Locked (awaiting resolution) → Resolved → Settled
```

- **Active:** normal trading.
- **Locked:** event window has passed; no new resolution yet; often illiquid.
- **Resolved:** oracle has set the outcome; contracts paying $1 or $0.
- **Settled:** all token holders have claimed their payouts.

The engine must respect this lifecycle — no fills should be accepted in `Locked` or
`Resolved` states.

---

## 3. Data contract

### Required manifest (Engine H)

```
REQUIRED:
  InstrumentStatic (prediction market) {
    market_id,
    question,
    resolution_criteria,
    resolution_deadline,
    oracle_type: UMA | Augur | Centralized | … ,
    asset_class = PredictionMarket,
    outcomes: [ { outcome_label, token_id } ]
  }
  OutcomeToken Bar or Quote stream (per YES/NO token)
  Resolution stream { outcome, resolution_ts }

OPTIONAL:
  OrderBookDepth stream
  OracleEvent stream (for oracle-risk strategies)
```

### Payload variants used

| Payload | Description |
|---|---|
| `Bar { open, high, low, close, volume, interval }` | YES/NO token price (0–1 range) |
| `Quote { bid, ask, … }` | BBO for YES/NO |
| `Resolution { outcome, oracle_id }` | Event resolved |
| `Mark { price }` | Current implied probability |

---

## 4. Engine behavior (Engine H)

1. **Market lifecycle enforcement.** Rejects fill attempts after the `Locked` state.
2. **Binary payoff at resolution.** On `Resolution` event, open YES positions are worth $1
   (if YES) or $0 (if NO); NO positions are the inverse.
3. **No mark-to-market until resolution** beyond the market price itself. Unrealized P&L
   is `position_value × current_price - cost_basis`.
4. **Oracle risk flag.** If a dispute event is emitted, the engine flags all open positions
   in the market as carrying oracle risk and optionally models a probability of incorrect
   resolution.

---

## 5. Performance metrics (extensions)

| Metric | Notes |
|---|---|
| **Brier score** | Primary calibration metric: `mean((predicted_prob - outcome)²)`. Lower = better calibrated. |
| Log score | Alternative calibration: `mean(log(predicted_prob))` for the correct outcome |
| Accuracy | % of markets predicted correctly (binary) |
| ROI per market | Average return on capital per market entered |
| Resolution timing | How long positions were held before resolution |
| Oracle dispute rate | % of markets with disputed resolutions (as proxy for oracle risk) |

**Note:** raw P&L is a valid metric, but calibration (Brier score) is the primary measure
of strategy quality for prediction-market strategies. A strategy that correctly prices
uncertainty earns steady returns even with a low accuracy rate.

---

## 6. Implications for system design

1. **Resolution replaces expiry.** There is no fixed calendar expiry. The engine must wait
   for a `Resolution` event to close positions, and the resolution timing is uncertain.
2. **Brier score requires per-event probability tracking.** The engine must record the
   strategy's implied probability (= position entry price) at the time of each trade and
   compare to the binary outcome.
3. **Liquidity is market-specific.** Major elections or Fed meeting markets have deep books
   and tight spreads; niche markets may have near-zero liquidity. The fill model must reflect
   the specific market's book depth.
4. **Price bounded [0, 1].** The `tick_size` for YES/NO tokens is effectively $0.01
   (1 cent). No trades can occur at $0.00 or $1.00 until resolution.
5. **Oracle risk is irreducible.** No data model can eliminate the risk of an incorrect
   oracle resolution. The backtest should flag positions with active disputes.

---

## 7. Sources

- PolyBackTest: full Polymarket historical order book at 1-minute resolution
- RockNBlock: UMA Optimistic Oracle and Polymarket resolution mechanics
- arXiv 2604.20421: Datasets for the full lifecycle of prediction markets
- Chainlink: What are prediction market contracts
- MetaMask: Prediction markets in 2026 — trends and regulation overview
