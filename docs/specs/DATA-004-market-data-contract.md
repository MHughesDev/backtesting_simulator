# Spec: DATA-004 — Market Data Contract

**Spec ID:** DATA-004
**Type:** Data (schema contract)
**Status:** Approved
**Date:** 2026-06-06
**Author:** Agent

The Market Data Contract defines the shape of every data event the caller feeds into the
simulator. It has two layers:
1. A **required envelope** — small, always present, shared by every event.
2. A **typed payload** — one variant per event type; which variants are valid is gated by
   the instrument's capability flags.


---

## 1. The required envelope

Every event, every asset class, every engine:

```
MarketEvent {
  instrument_id: InstrumentId,   // links to an Instrument in the run
  entity_id:     Option<EntityId>, // company/issuer/protocol/collection (mainly for exogenous + reference linkage)
  venue_id:      VenueId,
  ts_event:      i64,            // nanoseconds UTC — when the thing happened
  ts_available:  Option<i64>,   // ns UTC — when a strategy could first know it; defaults to ts_event
  ts_recv:       i64,            // wall-clock time received (for latency modeling; may equal ts_event)
  seq:           u64,            // per-instrument monotonically increasing sequence number
  source_id:     Option<String>, // producing source/processor (reproducibility)
  source_version: Option<String>,// parser/model/processor version
  payload:       Payload,        // exactly one of the variants below
}
```

`ts_event` is the authoritative time source for ordering Market-Data Plane events and enforcing
look-ahead: a strategy or model may not observe market data where `ts_event > current_ts`.

`ts_available` exists for the **Exogenous-Signal Plane** (news, social, fundamentals, media — see
[signals.md](DATA-005-signals-contract.md) and [DATA_TAXONOMY.md](DATA-001-data-taxonomy.md)). External information has a
publication lag, so exogenous look-ahead is enforced by `ts_available ≤ current_ts`, not `ts_event`.
For ordinary market data with no lag, `ts_available == ts_event`.

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

### 2.9 Listing marketplace events (`IsUnique` or `IsFungibleSKU`)

These payloads are valid for any `MARKETPLACE` instrument regardless of whether it represents
an NFT, a physical good, or a commodity SKU.

```
ListingEvent {
  listing_id:    ListingId,         // unique identifier for this specific listing
  event_type:    ListingEventType,
  // ListingEventType =
  //   Created  { asking_price, quantity, condition_tier?, attributes?, shipping_cost? }
  //   Sold     { sale_price, buyer_id?, platform_fee, royalty?, gas_cost?, shipping_cost? }
  //   Removed
  //   PriceChanged { new_price }
  item_id:      Option<ItemId>,     // set for IsUnique items; null for IsFungibleSKU
  sku_id:       Option<SkuId>,      // set for IsFungibleSKU items; null for IsUnique
  venue_id:     VenueId,
  seller_id:    Option<String>,
}

ComparableMarkEvent {
  category_id:   String,
  mark_price:    Decimal,
  mark_type:     Floor | MedianSale | LastSale | ModelEstimate,
  sample_count:  u32,      // number of comparable data points supporting this mark
  lookback_days: u16,
  source:        String,
}
```

`ListingEvent` replaces the former NFT-only `NftEvent`; `ComparableMarkEvent` replaces the
former `FloorUpdate`. The `mark_type` field on `ComparableMarkEvent` distinguishes the nature
and uncertainty of the mark — `Floor` is the lowest active asking price (instantaneous, can
vanish), `MedianSale` and `LastSale` are sale-history derived (more stable), and `ModelEstimate`
is caller-supplied. See [engines/engine-g-marketplace.md](COMP-009-engine-g-marketplace.md) §8
for mark quality and uncertainty disclosure rules.

The `attributes` field on `ListingEvent.Created` carries caller-defined structured metadata
(condition grade, dimensions, material, technical specs, rarity traits) used by Engine G for
item filtering and comparable matching. Richer content (description text, images) travels
through the Exogenous-Signal Plane as `DocumentSignal` / `MediaReference` payloads.

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

### 2.11 True L3/MBO order book (`HasL3OrderBook`)

```
OrderBookOrderEvent {
  order_id:       String,
  action:         Add | Modify | Cancel | Execute,
  side:           Side,
  price:          Decimal,
  size:           Decimal,
  displayed_size: Option<Decimal>,
  priority_ts:    Option<i64>,
  sequence:       Option<u64>,
}
```

**L2 vs. L3 distinction:** `BookDelta` and `BookSnapshot` carry aggregated depth (price → total
size) — this is L2 data. `OrderBookOrderEvent` carries per-order detail (each individual order
with its own ID and action) — this is true L3/MBO data. Engine A requires `OrderBookOrderEvent`
to enter L3 fidelity mode. If only `BookDelta`/`BookSnapshot` is provided, the engine classifies
the run as L2 fidelity. The capability flag `HasL3OrderBook` gates this payload; `HasOrderBook`
gates L2.

### 2.12 Open interest (`HasOpenInterest`)

```
OpenInterest {
  open_interest:     Decimal,
  volume:            Option<Decimal>,
  settlement_price:  Option<Decimal>,
}
```

### 2.13 Yield curve (`HasYield`)

```
YieldCurve {
  curve_type: Treasury | SOFR | OIS | Credit | Custom(String),
  currency:   CurrencyCode,
  tenors:     Vec<Duration>,
  yields:     Vec<Decimal>,
}
```

**YieldUpdate vs. YieldCurve:** `YieldUpdate` is instrument-specific (carries the YTM of a
single bond). `YieldCurve` is the full curve object (a vector of tenor → yield pairs) used for
pricing bonds and rate-sensitive products from a benchmark curve. Both may be present for the
same run; `YieldCurve` enables curve-derived pricing for instruments without direct price quotes.

### 2.14 Credit data (`HasCreditRisk`)

```
CreditSpread {
  issuer_id:     Option<String>,
  rating_bucket: Option<String>,
  spread_type:   OAS | ZSpread | CDS | TreasurySpread | Custom(String),
  spread_bps:    Decimal,
}

CreditRatingEvent {
  issuer_id:  String,
  agency:     String,
  old_rating: Option<String>,
  new_rating: String,
  outlook:    Option<String>,
}
```

`CreditSpread` supplies the issuer/rating spread used in curve-derived bond pricing. `CreditRatingEvent`
triggers an immediate repricing when a rating changes. Both are gated by `HasCreditRisk`.

### 2.15 Short borrow (`HasBorrowRate`, `HasShortBorrow`)

```
BorrowRate {
  borrow_rate_annualized: Decimal,
  short_availability:     Option<Decimal>,
  hard_to_borrow:         Option<bool>,
}
```

Required when `HasBorrowRate` is set and the strategy can short. The borrow rate accrues daily
on open short positions via `Account`.

### 2.16 Trading sessions and market status (`HasSessions`, `HasTradingStatus`, `HasAuction`)

```
TradingSession {
  session_type:         Regular | PreMarket | AfterHours | Overnight | Asian | London | NewYork | Overlap | OffHours | Custom(String),
  open_ts:              i64,
  close_ts:             i64,
  liquidity_multiplier: Option<Decimal>,
}

TradingStatus {
  status: Open | Closed | Halted | Auction | LimitUpLimitDown | Locked | CancelOnly | PostOnly | Custom(String),
  reason: Option<String>,
}

AuctionImbalance {
  auction_type:     Open | Close | HaltReopen | Custom(String),
  paired_qty:       Decimal,
  imbalance_qty:    Decimal,
  imbalance_side:   Option<Side>,
  indicative_price: Option<Decimal>,
  reference_price:  Option<Decimal>,
}

AuctionResult {
  auction_type:   Open | Close | HaltReopen | Custom(String),
  official_price: Decimal,
  matched_volume: Decimal,
}
```

`TradingStatus` events (halts, circuit breakers, auction states) gate fills: no fills may occur
while status is `Halted`, `Locked`, `CancelOnly`, or `PostOnly`. `AuctionImbalance` and
`AuctionResult` are emitted by Engine A when `HasAuction` is set.

**Dynamic overrides static (session state).** The engine maintains a current session/status state
per instrument that **starts from the static exchange calendar** and is **overridden by incoming
`TradingSession` / `TradingStatus` events** from their `ts_event` forward — matching live trading,
where the exchange's real-time messages are the source of truth and the configured calendar is
only the default expectation. The static calendar is the fallback whenever no event has updated
the state.

**Halt behavior (resting orders frozen, not cancelled).** On `TradingStatus: Halted`, resting
limit/stop orders are **frozen** — no fills occur during the halt, and the orders persist and
resume on reopen (participating in the reopening auction if `HasAuction`). `DAY` orders still
expire at session close if the halt runs past it. Detailed in
[engines/engine-a-order-book.md](COMP-003-engine-a-order-book.md) §6.

### 2.17 Futures reference data (`HasRollSchedule`)

```
RollSchedule {
  from_contract: InstrumentId,
  to_contract:   InstrumentId,
  roll_ts:       i64,
  roll_method:   Calendar | Volume | OpenInterest | Custom(String),
}

ContinuousSeriesMetadata {
  method:           Panama | Proportional | Unadjusted | Perpetual | Custom(String),
  source_contracts: Vec<InstrumentId>,
}
```

These are **reference data** payloads, not per-event streaming payloads. They are declared in
`data.bindings` under a dedicated key (e.g. `"roll_schedule"`) and loaded once at run start,
not replayed as events. `RollSchedule` defines when front-month contracts roll to back-month;
`ContinuousSeriesMetadata` declares how the adjusted continuous series was constructed.

### 2.18 ETF / fund basket data (`HasHoldings`, `HasCreationRedemption`)

```
HoldingsSnapshot {
  as_of_ts:           i64,
  holdings:           Vec<HoldingEntry>,
  shares_outstanding: Option<Decimal>,
  liabilities:        Option<Decimal>,
}

HoldingEntry {
  instrument_id: InstrumentId,
  weight:        Option<Decimal>,
  shares:        Option<Decimal>,
  market_value:  Option<Decimal>,
}

CreationRedemptionBasket {
  as_of_ts:           i64,
  creation_unit_size: Decimal,
  components:         Vec<HoldingEntry>,
}
```

`HoldingsSnapshot` enables derived NAV computation in Engine C. `CreationRedemptionBasket`
describes the ETF creation/redemption mechanism for authorized participants. Both are gated by
their respective capability flags.

### 2.19 Gas and on-chain events (`HasGasCost`, `HasSwapEvent`)

```
GasEvent {
  chain_id:           String,
  block_number:       u64,
  base_fee:           Option<Decimal>,
  priority_fee:       Option<Decimal>,
  gas_price:          Decimal,
  native_token_price: Option<Decimal>,
}

SwapEvent {
  pool_address:     Address,
  tx_hash:          String,
  block_number:     u64,
  tx_index:         u32,
  amount_0_in:      Decimal,
  amount_1_in:      Decimal,
  amount_0_out:     Decimal,
  amount_1_out:     Decimal,
  sqrt_price_after: Option<Decimal>,
  gas_used:         Option<Decimal>,
}
```

`GasEvent` carries per-block gas prices; required when `HasGasCost` is set unless a static gas
model is declared in `execution_defaults`. `SwapEvent` carries historical AMM transactions.

**PoolState vs. SwapEvent distinction:** `PoolState` is a state snapshot (the current reserves
or price of the pool at a moment in time). `SwapEvent` is a historical transaction (a specific
swap that occurred, with its inputs, outputs, and gas). They serve different purposes and may
both be bound for the same pool instrument.

**Pool-state reconstruction (higher fidelity).** When `SwapEvent` flow is bound (`HasSwapEvent`),
Engine B reconstructs the pool's intermediate states by **replaying the real swaps** between
observed `PoolState` snapshots, rather than holding the last snapshot constant until the next one
arrives. This is the more realistic mode — the strategy's own swap is priced against a pool that
has been advanced by all the real swaps that occurred up to that block/tx index. Without
`SwapEvent`, the engine holds the last observed `PoolState` until the next snapshot (lower
fidelity, flagged in results). See [engines/engine-b-amm.md](COMP-004-engine-b-amm.md).

### 2.20 FX swap rates (`HasSwapRates`)

```
SwapRate {
  swap_long:       Decimal,
  swap_short:      Decimal,
  effective_date:  Date,
  triple_swap_day: Option<DayOfWeek>,
}
```

**SwapRate vs. Funding:** `SwapRate` is the FX-specific overnight rollover payload (interest
rate differential between the two currencies in a pair). It must **not** be confused with or
reused as the `Funding` payload (§2.3), which is the perpetual futures funding rate mechanism.
These are distinct payloads with distinct semantics. `HasSwapRates` gates `SwapRate`; `HasFunding`
gates `Funding`.

### 2.21 Fee schedule updates (`HasFeeScheduleUpdates`)

```
FeeScheduleUpdate {
  venue_id:      VenueId,
  instrument_id: Option<InstrumentId>,
  maker_bps:     Option<Decimal>,
  taker_bps:     Option<Decimal>,
  flat_fee:      Option<Decimal>,
  tier_rules:    Option<Vec<FeeTier>>,
  effective_ts:  i64,
}
```

Enables multi-year accurate fee modeling when exchange fee schedules change over a backtest
window. Gated by `HasFeeScheduleUpdates`. **Dynamic overrides static:** when a `FeeScheduleUpdate`
takes effect at `effective_ts`, it supersedes the static `fee_schedule` on the Instrument from
that timestamp forward (the engine maintains a current fee schedule per instrument that starts
from the static value and is updated by each event). The static `fee_schedule` is the fallback
used before the first update and whenever no updates are bound.

### 2.22 Liquidation and insurance events (perpetuals, `HasLiquidation`, `HasLiquidationStream`)

```
LiquidationEvent {
  instrument_id:    InstrumentId,
  side:             Side,
  qty:              Decimal,
  price:            Decimal,
  bankruptcy_price: Option<Decimal>,
  mark_price:       Decimal,
}

InsuranceFundEvent {
  venue_id:  VenueId,
  balance:   Decimal,
  currency:  CurrencyCode,
}

AutoDeleveragingEvent {
  instrument_id: InstrumentId,
  affected_side: Side,
  notional:      Decimal,
}
```

`LiquidationEvent` from an external data stream is optional when `HasLiquidationStream` is set.
`HasLiquidation` (the existing capability) gates the liquidation mechanic; `HasLiquidationStream`
gates the optional external data stream.

**External vs. own liquidation (disjoint).** The external `LiquidationEvent` stream represents
**other market participants'** liquidations and feeds only the market-impact / cascade model
(added slippage, depth shocks). It **never** closes the strategy's own position. The strategy's
own liquidation is always engine-computed from the injected `Account` collateral and the mark
price — never driven by an external event. The two paths do not overlap.

**`AutoDeleveragingEvent` can reach the strategy's position.** On a real exchange, when the
insurance fund is exhausted the exchange auto-deleverages (ADL) the most profitable, highest-
leverage traders on the side opposite the bankrupt position. To stay realistic, when an
`AutoDeleveragingEvent` fires the engine checks whether the strategy holds a qualifying position
on `affected_side`; if so it force-closes a proportional share at the liquidated trader's
bankruptcy price, reports the close to `Account`, and notifies the strategy. `InsuranceFundEvent`
supplies the fund balance context that makes ADL plausible. Both are optional; absent them, ADL is
not modeled.

### 2.23 Offer events (`HasOffer`)

```
OfferEvent {
  listing_id:    ListingId,
  offer_price:   Decimal,
  quantity:      Option<Decimal>,
  offerer_id:    Option<String>,
  outcome:       Pending | Accepted | Rejected | Countered | Withdrawn | Expired,
  counter_price: Option<Decimal>,   // set when outcome = Countered
  expiration_ts: Option<i64>,
}
```

`OfferEvent` carries offer-and-response history for `HasOffer` instruments: buyer offers below
asking price, seller counter-offers, acceptances, and rejections. **A standing bid or offer with
outcome `Pending` does not produce an immediate fill in the conservative model.** Engine G's
default sell and offer paths require an observed `OfferEvent.outcome = Accepted` to produce a
fill — an outstanding offer can be withdrawn or rejected before settlement, so treating a pending
offer as a guaranteed fill would be optimistic. Pending offers with stated bid prices feed the
**optional demand model** (higher fidelity, more assumptions) used to estimate fill probability
and time-to-fill. See
[engines/engine-g-marketplace.md](COMP-009-engine-g-marketplace.md) §6.

`OfferEvent` also carries seller-initiated offers (e.g. "best offer" or seller discount
outreach). When a seller offer arrives, the engine checks whether the strategy has configured
an `accept_if` rule and records a fill if the offer meets the threshold.

### 2.24 Prediction market lifecycle (`IsBinary`, `HasResolution`, `HasOracleRisk`)

```
MarketLifecycleEvent {
  market_id: InstrumentId,
  state:     Created | Active | Locked | Resolved | Settled,
}

OracleEvent {
  market_id:        InstrumentId,
  event_type:       Proposal | Dispute | FinalSettlement,
  proposed_outcome: Option<String>,
  final_outcome:    Option<String>,
  dispute_occurred: Option<bool>,
}
```

`MarketLifecycleEvent` drives the Engine H lifecycle state machine (§2 of engine-h). Required
for accurate lifecycle gating — without it, the engine cannot know when to stop accepting fills.
`OracleEvent` carries dispute and settlement detail used for oracle risk modeling when `HasOracleRisk`
is set.

**Event ordering (realistic oracle flow).** The expected sequence is:
`MarketLifecycleEvent: Locked` (event occurred, trading stops) → `OracleEvent: Proposal`
(proposed outcome) → optional dispute window in which `OracleEvent: Dispute` may arrive →
`OracleEvent: FinalSettlement` → `Resolution` (binary payout). A `Dispute` does **not** re-open
trading — once `Locked`, the market never returns to `Active`; the dispute only delays finality.
A `Resolution` **may** arrive with no preceding `OracleEvent` for simple/centralized oracles
(e.g. Kalshi); when `OracleEvent`s are present they gate the dispute/latency/oracle-risk modeling.

### 2.25 Universe membership (reference data, `HasUniverseMembership`)

```
UniverseMembership {
  universe_id:        String,
  instrument_id:      InstrumentId,
  added_ts:           i64,            // effective: when membership began
  added_knowable_ts:  Option<i64>,    // when the addition became publicly knowable (defaults to added_ts)
  removed_ts:         Option<i64>,    // effective: when membership ended
  removed_knowable_ts: Option<i64>,   // when the removal became publicly knowable (defaults to removed_ts)
  reason:             Option<String>,
}
```

**Reference data, not a streaming payload.** `UniverseMembership` is loaded once at run start
from a reference binding (not replayed as events). It declares when each instrument entered and
exited a universe (e.g. an index constituent or a tradeable set), enabling point-in-time universe
construction that prevents survivorship bias. The engine uses `added_ts`/`removed_ts` to gate
whether an instrument is in-universe at each simulation timestamp, subject to the `knowable_ts`
point-in-time rule (§4.5) so a membership change is not visible before it was announced. When a
dynamic universe selector is also in use, `UniverseMembership` **gates** the selector's candidate
pool — the selector may only pick instruments that were in-universe at that timestamp.

### 2.26 DerivedBar (internal — not caller-provided)

```
DerivedBar {
  open:         Decimal,
  high:         Decimal,
  low:          Decimal,
  close:        Decimal,
  volume:       Decimal,
  interval:     Duration,
  adjusted:     bool,
  derived:      true,          // always true for engine-derived bars
  source_class: PayloadClass,  // what raw data this was derived from (Trade, Quote, BookDelta, etc.)
  tick_count:   u64,           // number of source events that contributed
}
```

**Internal payload — callers cannot provide this.** `DerivedBar` is produced by the inline bar
derivation layer during the event loop (see Run Request §DerivedDataConfig) and emitted into the
strategy's feature pipeline alongside caller-provided `Bar` events. It is never a valid binding
in `data.bindings`; any attempt to provide it as caller data is a schema validation error.

The `derived: true` flag and `source_class` field allow the strategy's feature pipeline to
distinguish between caller-provided bars and engine-derived bars.

**Construction standard.** Derived bars are built from **trade prints** (`open` = first trade,
`high` = max, `low` = min, `close` = last, `volume` = Σ sizes); when no trades exist the fallback
is the **quote mid** `(bid+ask)/2`. Boundaries are wall-clock aligned and start at `:00` (a 1d bar
runs UTC-midnight to UTC-midnight). Coarser intervals may be derived from finer data; finer
intervals may **never** be derived from coarser data. Full rules in
[run-request.md](DATA-002-run-request.md) §4a.

**Caller-provided bars are never overridden.** If the caller binds a `Bar` stream at a given
interval, the engine uses it and does not emit a `DerivedBar` for that interval (precedence rule,
[run-request.md](DATA-002-run-request.md) §4).

### 2.27 Timed auction events (`HasTimedAuction`)

```
AuctionBidEvent {
  listing_id:      ListingId,
  bid_price:       Decimal,
  bidder_id:       Option<String>,
  is_reserve_met:  Option<bool>,   // whether this bid meets or exceeds the reserve price
}

AuctionCloseEvent {
  listing_id:      ListingId,
  winning_bid:     Option<Decimal>, // null when reserve was not met
  winner_id:       Option<String>,
  reserve_met:     bool,
}
```

`AuctionBidEvent` records each historical bid on a timed auction listing; required when
`HasTimedAuction` is set. Events replay in `ts_event` order so the engine can determine the
highest bid at any simulation timestamp. `AuctionCloseEvent` fires at `auction_close_ts` and
determines the fill outcome. A `winning_bid` of null (reserve not met) produces no fill.

**Ordering within an auction.** Multiple bids at the same `ts_event` are ordered by `seq`.
The engine considers the strategy outbid if any `AuctionBidEvent` with `ts_event` ≤
`auction_close_ts` carries a `bid_price` strictly greater than the strategy's bid — regardless
of `bidder_id`. The engine does not simulate autobid increment strategies.

---

## 3. Data manifest validation

At run start, before any events are processed, the simulator validates the caller's declared
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
5. **Reference data is queried, not replayed.** Payloads bound as `binding_type: "reference"`
   (e.g. `UniverseMembership`, `RollSchedule`, `ContinuousSeriesMetadata`, exchange calendars)
   are loaded once and queried by timestamp during the run. Each reference entry carries an
   `effective_ts` (when the value took effect) and a `knowable_ts` (when it became publicly
   knowable). A lookup at `current_ts` may only return entries with `knowable_ts ≤ current_ts`,
   preventing backdated values from leaking future information. Where a source supplies only one
   timestamp, `knowable_ts` defaults to `effective_ts`. See
   [run-request.md](DATA-002-run-request.md) §4b.
