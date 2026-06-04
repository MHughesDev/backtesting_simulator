# Contract Spec: Market Data

The Market Data Contract defines the shape of every data event the caller feeds into the
suite. It has two layers:
1. A **required envelope** — small, always present, shared by every event.
2. A **typed payload** — one variant per event type; which variants are valid is gated by
   the instrument's capability flags.

---

## 1. The required envelope

Every event, every asset class, every engine:

```
MarketEvent {
  instrument_id: InstrumentId,   // links to an Instrument in the run
  venue_id:      VenueId,
  ts_event:      i64,            // nanoseconds UTC — the canonical clock for look-ahead enforcement
  ts_recv:       i64,            // wall-clock time received (for latency modeling; may equal ts_event)
  seq:           u64,            // per-instrument monotonically increasing sequence number
  payload:       Payload,        // exactly one of the variants below
}
```

`ts_event` is the **only authoritative time source** for ordering events and enforcing
look-ahead safety. A strategy or model may not observe data where `ts_event > current_event.ts_event`.

`seq` enables gap detection (missed events) and intra-timestamp ordering.

---

## 2. Payload variants

### 2.1 Universal variants (all assets)

These four variants carry the ~80% of data common across all instruments.

#### `Mark`
```
Mark {
  price: Decimal,
}
```
A single reference price. Used for: bonds (model-derived), funds (iNAV), NFTs (floor),
prediction markets (implied probability), and any instrument where only a scalar mark is
available. Not used when a full `Bar` is present and meaningful.

#### `Bar`
```
Bar {
  open:     Decimal,
  high:     Decimal,
  low:      Decimal,
  close:    Decimal,
  volume:   Decimal,
  interval: Duration,
  adjusted: bool,        // true = split/dividend-adjusted; false = unadjusted
}
```
All five price fields are **guaranteed present** when a `Bar` is emitted. If a data source
cannot provide a meaningful `high`/`low` (e.g. a bond with one trade per day), it should
emit `Mark` instead of a `Bar` with duplicated fields. The `adjusted` flag distinguishes
series for equities and futures continuous contracts.

#### `Quote` (BBO)
```
Quote {
  bid:      Decimal,
  bid_size: Decimal,
  ask:      Decimal,
  ask_size: Decimal,
}
```
Best bid and offer. Enables spread-aware fill modeling.

#### `Trade`
```
Trade {
  price:          Decimal,
  size:           Decimal,
  aggressor_side: Side,    // Buy | Sell | Unknown
  trade_id:       Option<String>,
}
```
Individual matched trade (tick print).

### 2.2 Order book variants (`HasOrderBook`)

```
BookDelta {
  side:     Side,
  price:    Decimal,
  new_size: Decimal,    // 0 = level removed
  action:   BookAction, // Add | Modify | Delete
}

BookSnapshot {
  bids: Vec<(Decimal, Decimal)>,   // (price, size) sorted best-first
  asks: Vec<(Decimal, Decimal)>,
  depth: u8,
}
```

### 2.3 Perpetual-specific (`HasFunding`, `HasMarkPrice`)

```
Funding {
  rate:            Decimal,   // funding rate for this period (e.g. 0.0003 = 0.03%)
  mark_price:      Decimal,
  index_price:     Option<Decimal>,
  next_funding_ts: i64,       // ns UTC of next funding event
}

MarkUpdate {
  mark_price:  Decimal,
  index_price: Option<Decimal>,
}
```

### 2.4 Derivatives (`HasGreeks`, `HasIVSurface`)

```
Greeks {
  iv:    Decimal,   // implied volatility (annualized, as a fraction, e.g. 0.25 = 25%)
  delta: Decimal,
  gamma: Decimal,
  theta: Decimal,   // per-day time decay
  vega:  Decimal,   // per 1% change in IV
  rho:   Decimal,
}

IVSurface {
  underlying_id: InstrumentId,
  surface_type:  SurfaceType,   // AbsoluteStrike | DeltaNormalized
  strikes:       Vec<Decimal>,  // strike values or delta values depending on surface_type
  expiries:      Vec<u32>,      // days-to-expiry
  iv_grid:       Vec<Vec<Decimal>>,  // [expiry_index][strike_index]
}

ExerciseEvent {
  option_id:       InstrumentId,
  exercise_style:  ExerciseStyle,
  intrinsic_value: Decimal,
  assignment:      bool,         // true if this is an assignment notification to a short holder
}
```

### 2.5 AMM / DEX (`HasPoolReserves`)

```
PoolState {
  // Uniswap v2 / CPMM fields
  reserve_0:      Option<u128>,
  reserve_1:      Option<u128>,
  // Uniswap v3 / CLAMM fields
  sqrt_price_x96: Option<u160>,
  current_tick:   Option<i32>,
  liquidity:      Option<u128>,
  tick_data:      Option<Vec<TickEntry>>,
  // Common
  fee_bps:        u16,
  block_number:   u64,
}
```

### 2.6 Fixed income (`HasCoupon`)

```
Coupon {
  rate:            Decimal,    // annualized coupon rate
  accrual:         Decimal,    // accrued interest since last payment
  next_payment_ts: i64,        // ns UTC
  payment_amount:  Decimal,    // actual cash per bond for this period
}

YieldUpdate {
  ytm:                  Decimal,
  spread_over_treasury: Option<Decimal>,
  credit_rating:        Option<String>,
}
```

### 2.7 NAV / funds (`HasNAV`)

```
Nav {
  nav:                Decimal,
  premium_discount:   Decimal,   // (market_price - nav) / nav
  inav:               Option<Decimal>,  // intraday indicative NAV
}
```

### 2.8 Corporate / token events (`HasCorporateActions`, `HasTokenEvents`)

```
CorporateAction {
  action_type: CorporateActionType,
  // CorporateActionType = Dividend { amount_per_share, currency, ex_date, pay_date }
  //                     | Split    { ratio, ex_date }
  //                     | Merger   { consideration, effective_date }
  //                     | SpinOff  { new_instrument_id, ratio, ex_date }
  //                     | Delisting { final_price, date }
  //                     | RightsIssue { subscription_price, ratio, ex_date }
}

TokenEvent {
  event_type: TokenEventType,
  // TokenEventType = Fork    { new_asset_id, ratio, fork_date }
  //               | Airdrop  { airdrop_asset_id, amount_formula, snapshot_date }
  //               | Burn     { amount_burned, date }
  //               | Migration { new_asset_id, ratio, migration_date }
  //               | Delisting { final_price, date }
}
```

### 2.9 NFT events (`IsUnique`)

```
NftEvent {
  token_id:    TokenId,
  event_type:  NftEventType,
  // NftEventType = Sale   { price, buyer, seller, marketplace, gas_cost }
  //             | Listing { price, marketplace }
  //             | Delisting { marketplace }
}

FloorUpdate {
  collection_address: Address,
  floor_price:        Decimal,
  listed_count:       u32,
  volume_24h:         Decimal,
}
```

### 2.10 Prediction market events (`IsBinary`, `HasResolution`)

```
Resolution {
  market_id:         InstrumentId,
  outcome:           String,         // "YES" or "NO"
  oracle_id:         String,
  resolution_ts:     i64,
  dispute_occurred:  bool,
}
```

---

## 3. Data manifest validation

At run start, before any events are processed, the suite validates the caller's declared
data feeds against the required manifest for each `(instrument, engine)` pair.

```
DataManifestViolation {
  instrument_id:    InstrumentId,
  engine:           EngineType,
  missing_required: Vec<PayloadClass>,
  warning_optional: Vec<PayloadClass>,    // present = better fidelity
}
```

A single missing required payload class causes the run to be rejected with this error.
Optional missing payloads produce warnings but allow the run to proceed.

---

## 4. Clock and ordering invariants

1. All events in a single run share one monotonically non-decreasing `ts_event` clock.
2. Events at the same `ts_event` are ordered by `(instrument_id, seq)`.
3. No event may be processed by a strategy or model before all events at earlier `ts_event`
   values are processed — look-ahead safety.
4. `ts_recv ≥ ts_event` always. If the caller doesn't distinguish receipt time from event
   time, `ts_recv == ts_event` is acceptable.
