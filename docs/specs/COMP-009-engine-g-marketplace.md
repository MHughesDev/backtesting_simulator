# Spec: COMP-009 — Engine G: Marketplace

**Spec ID:** COMP-009
**Type:** Component (execution engine)
**Status:** Approved
**Date:** 2026-06-06
**Author:** Agent

**`price_formation`:** `MARKETPLACE`
**Routes here:** Any instrument whose price is determined by a discrete transaction between a
willing buyer and a willing seller — not by a continuous order book, AMM formula, or periodic
NAV computation. Includes: NFTs, digital collectibles, on-chain gaming assets, physical goods
on peer-to-peer platforms (Facebook Marketplace, Craigslist, OfferUp, Mercari), fixed-price
and auction listings on e-commerce marketplaces (eBay, Etsy), commodity used goods (electronics
resale, hardware parts, collectibles), estate lots, and any other listing-based market where a
price is observable only at the moment a transaction completes.

Engine G simulates a **listing marketplace**: items appear, get claimed or removed, and
occasionally sell. The engine's core commitment is that **price is observed only when a
transaction occurs** — between transactions, the value of a held item is unknown and any mark
is an estimate. This uncertainty is a first-class property of the engine, reported in every
result rather than papered over.

---

## 1. Price formation: what MARKETPLACE means

Three properties define `MARKETPLACE` price formation and distinguish it from every other
mechanic in this simulator:

1. **Episodic price discovery.** A price exists only when a transaction completes. Between
   transactions there is no bid-ask spread, no NAV, no AMM formula, and no mark-to-market
   price that can be trusted as the actual clearing price for a sale today.

2. **Supply is discrete and enumerated.** Sellers post specific items at specific prices.
   Buyers select from what is actually listed at the time of their decision. A strategy cannot
   fill against a price that has no corresponding listing — the engine enforces this structurally.

3. **Demand is observable only in aggregate and retrospectively.** Bids, offers, and comparable
   sales carry demand information, but none guarantees a fill for any specific item at any
   specific time. The engine defaults to a conservative interpretation of all demand signals.

These properties hold whether the item is a CryptoPunk, a 1930s leather couch, or a box of
DDR5 RAM. The capability flags on each instrument define which additional mechanics apply.

---

## 2. The `Engine` trait

```rust
impl Engine for MarketplaceEngine {
    fn on_event(&mut self, ev: &MarketEvent, ctx: &mut EngineContext);
    // Ingests ListingEvent (created/sold/removed/price-changed), AuctionBidEvent,
    // AuctionCloseEvent, OfferEvent, and ComparableMarkEvent. Advances active-listing state.

    fn submit_order(&mut self, o: Order, ctx: &mut EngineContext) -> OrderResult;
    // Supported order types: BuyAtMaxPrice, ListAtPrice,
    //   Bid (requires HasTimedAuction), SubmitOffer (requires HasOffer)

    fn settle(&mut self, ctx: &mut EngineContext);
    // Force-closes open positions at the most recent ComparableMarkEvent price.

    fn supports_order_type(&self, t: OrderType) -> bool;
}
```

Cash is held in the injected `Account`
([ADR-0010](../adr/0010-simulator-does-not-own-portfolio.md)). `MARKETPLACE` instruments carry
no `Bar` payloads — only `ListingEvent`s and `ComparableMarkEvent`s constitute their market
data stream.

---

## 3. Position model

Position keying depends on the instrument's listing mode, declared via capability flags:

**`IsUnique`** — the item is non-fungible. Positions are keyed by `(category_id, item_id)`.
No two items in the same category are interchangeable. NFTs, individual furniture pieces,
one-of-a-kind collectibles, and serial-numbered goods use this mode. The strategy holds
*specific items*, not quantities of a category. An `IsUnique` instrument may leave `item_id`
unset in the instrument definition when a scanning strategy does not know in advance which
specific item it will acquire; once a fill occurs the engine binds the filled `listing_id` as
the position key for that trade leg.

**`IsFungibleSKU`** — the item is one of many interchangeable units of the same product.
Positions are keyed by `(sku_id, condition_tier)`. Any listing that matches the SKU and
condition tier is a valid candidate. RAM kits, phones, cameras, and commodity used hardware
use this mode. The strategy holds a *quantity* of a position rather than specific serial
numbers.

`IsUnique` and `IsFungibleSKU` are **mutually exclusive**. Every `MARKETPLACE` instrument must
declare exactly one.

---

## 4. Buy execution (fixed-price listings)

The default buy path, active when `HasTimedAuction` is not set:

```
on buy(max_price, quantity?, item_filter?):
    candidates = active_listings at decision_ts where:
                   asking_price ≤ max_price
                   AND listing_ts ≤ decision_ts
                   AND removal not yet observed at decision_ts
                   AND listing matches item_filter (if provided)
                   [IsUnique: attributes matched against item_filter rules]
                   [IsFungibleSKU: sku_id AND condition_tier must match instrument definition]

    if candidates non-empty:
        [IsUnique]:     fill at lowest qualifying asking_price; record listing_id acquired
        [IsFungibleSKU]: fill quantity units against lowest-priced qualifying listings;
                         each unit consumes one listing (no partial splits on a listing
                         unless the listing explicitly offers quantity > 1)
        cost = asking_price
             + platform_fee
             + royalty?          (HasRoyalty)
             + gas_cost?         (HasGasCost, at chain's gas price at ts_event)
             + shipping_cost?    (HasPhysicalFulfillment, from ListingEvent or fallback model)
    else:
        unfilled — absence of a qualifying listing is real information, not a model failure
```

The engine **never invents a fill**. If no listing existed at or before `decision_ts` at or
below `max_price` matching the filter, the order does not fill. This is the look-ahead
enforcement rule for marketplace assets: the strategy can only buy what was actually offered.

---

## 5. Timed auction execution (`HasTimedAuction`)

When `HasTimedAuction` is set, a listing is a timed auction with a fixed `auction_close_ts`.
Fill mechanics differ entirely from the fixed-price path:

```
on bid(listing_id, bid_price):
    validate bid_price > current_highest_observed_bid (or starting_price if no bids yet)
    validate decision_ts < auction_close_ts
    record bid; strategy is the provisional high bidder

on AuctionCloseEvent(listing_id):
    if strategy's bid is the highest at auction_close_ts:
        if reserve_price unset OR winning_bid ≥ reserve_price:
            fill at winning_bid + platform_fee + shipping?
        else:
            unfilled — reserve not met
    else:
        no fill — strategy was outbid
```

A bid does not guarantee a fill. The engine replays all `AuctionBidEvent`s in `ts_event`
order up to `auction_close_ts`; if any later-timestamped bid exceeds the strategy's bid before
close, the strategy does not win. The engine does not simulate last-second sniping delay beyond
the configured latency model — a bid submitted at `decision_ts` is recorded at
`decision_ts + latency`.

---

## 6. Offer mechanics (`HasOffer`)

When `HasOffer` is set, the strategy may submit an offer below the listed asking price, or
receive and auto-respond to seller-initiated offers.

**Buyer offer path:**
```
on submit_offer(listing_id, offer_price):
    record offer at decision_ts
    await OfferEvent(listing_id, outcome: Accepted | Rejected | Countered | Expired)

fill occurs only when OfferEvent.outcome = Accepted for (listing_id, offer_price)
```

**Conservative default:** an offer fill occurs only when the historical data contains an
`OfferEvent` showing that offer accepted. If no such event exists, the offer does not fill.
The engine does not model acceptance probability — same principle as the conservative sell path.

**Demand-model mode (optional):** a configurable acceptance model estimates the probability
that a seller accepts an offer at a given `offer_price / asking_price` ratio, informed by
historical acceptance rates for the category. Demand-model fills are flagged in results.

**Seller offer path:** the engine ingests seller-initiated `OfferEvent`s. The strategy may
configure an `accept_if` rule (`accept_if: offer_price ≤ threshold`); when a seller offer
arrives and meets the rule, the engine records a fill.

---

## 7. Sell / list execution

A sell order places a listing at a chosen price. The listing is active from the decision
timestamp until filled or explicitly removed.

**Conservative (default):** the listing fills only when a later **observed comparable sale**
occurs at or above the listing price, matched by:
- `IsUnique`: `(category_id, attribute_filter)` — same category, similar traits or condition.
- `IsFungibleSKU`: `(sku_id, condition_tier)` — exact same product and condition grade.

Standing bids in `OfferEvent` data feed the optional demand model only — they do not produce
an immediate fill in the conservative model. Bids can be withdrawn, may not have applied to
this specific item, and may not have resulted in a real transaction.

**Demand-model (optional):** a configurable sale-arrival model estimates fill probability and
time-to-fill from historical demand for the category. `OfferEvent` standing bids are used as
demand evidence. Demand-model fills are flagged separately in results.

Both paths report the realized sale price net of platform fee, royalty (`HasRoyalty`),
shipping (`HasPhysicalFulfillment`), and gas (`HasGasCost`).

---

## 8. Marking and comparables

Unrealized P&L for a held item is marked to a **comparable mark** — the best available price
estimate for an item of this type in the current market. The mark is always an estimate, never
an executable price, and its quality is explicitly reported.

Marks arrive via `ComparableMarkEvent` (see [market-data.md](DATA-004-market-data-contract.md) §2.27).
The `mark_type` field conveys what the mark represents and what uncertainty it carries:

| `mark_type` | What it is | Uncertainty profile |
|---|---|---|
| `Floor` | Lowest currently-listed asking price in the category | Instantaneous; can vanish when listings are removed |
| `MedianSale` | Median observed sale price over a recent lookback window | Stable; slow to update; may lag a sharp repricing |
| `LastSale` | Most recent comparable sale price | Fresh; single data point; high variance |
| `ModelEstimate` | Caller-supplied model-derived valuation | Highest information content; highest assumption load |

For `IsUnique` items, a specific item may be worth substantially more (desirable traits,
rarity premium) or substantially less (damage, poor condition, illiquidity) than the category
mark. The engine records the `mark_type` and `sample_count` from the source event in all P&L
attribution and uncertainty disclosures.

No interpolation is performed between sparse `ComparableMarkEvent` timestamps. Absence of a
mark update is not a signal of stable price — it means the market for this category was inactive.

---

## 9. Item attributes and AI model integration

**Item attributes are caller-provided metadata.** The engine owns no scoring methodology.
Condition grades, rarity ranks, age estimates, material classifications, technical
specifications, and seller reputation scores are all values the caller supplies — as structured
fields in `ListingEvent` attributes, as reference data attached to the instrument, or as
exogenous signals computed externally and bound via the strategy's signal plane.

The engine uses attributes exclusively for:
1. Filtering buy candidates (`item_filter` in the buy order, matched against listing attributes).
2. Matching comparables for sell fills (e.g. `used_good` sales matched against `used_good` listings).
3. Reporting condition or rarity premium in trade attribution (sale price vs. category mark).

**AI model integration is a strategy-layer concern.** A strategy may bind an injected `Model`
— via `HasExogenousSignals` and the model's `context_inputs` — that processes listing
descriptions, images, or other media and returns scored values that drive the alpha or sizing
stages:

```json
{
  "models": [{
    "id": "listing_scorer",
    "model_id": "marketplace-asset-analyzer",
    "inputs": {
      "list_price":    "data:asking_price",
      "category_comps": "signal:comparable_mark"
    },
    "context_inputs": {
      "images":      { "from": "signal:listing_images",      "lookback": "current" },
      "description": { "from": "signal:listing_description", "lookback": "current" }
    },
    "outputs": {
      "deal_score":      "deal_score",
      "condition_score": "condition_score"
    }
  }],

  "alpha": {
    "when": "model:listing_scorer.deal_score >= 0.90 && data:asking_price <= 300",
    "direction": "long"
  }
}
```

The model's output influences what the strategy decides to do. It cannot influence the fill
price, and it cannot produce a fill against a listing that did not exist. After the strategy
submits a buy order, the engine asks: was this listing present at `decision_ts`? Was the asking
price at or below `max_price`? If either check fails, the order does not fill — regardless of
what the model returned.

Listing descriptions and images travel through the Exogenous-Signal Plane as `DocumentSignal`
and `MediaReference` payloads (see [signals.md](DATA-005-signals-contract.md)), governed by
`ts_available` so publication lag is respected. The model resolves and loads the referenced
content; the engine core never parses raw media.

---

## 10. Transaction costs

Transaction costs are first-class P&L lines. Which apply depends on the instrument's
capability flags:

| Cost component | Capability | Notes |
|---|---|---|
| Platform fee | always | Percentage or flat fee per transaction; always reported |
| Creator / seller royalty | `HasRoyalty` | NFTs, some collectible platforms; enforcement varies by venue |
| Gas (on-chain) | `HasGasCost` | Denominated in native chain token, converted at `ts_event` |
| Shipping / delivery | `HasPhysicalFulfillment` | From `ListingEvent.shipping_cost`; fallback model when absent |
| Pickup friction | `HasPhysicalFulfillment` | Configurable per-trip cost; zero by default for delivery-only listings |
| Inspection failure | `HasPhysicalFulfillment` | Configurable failure rate and associated cost for physical inspection |

When `HasPhysicalFulfillment` is set and no per-listing shipping cost is supplied, the engine
applies a configurable fallback rather than assuming zero. Physical fulfillment cost is
material for categories where shipping is a significant fraction of the transaction value —
treating it as free produces wrong P&L.

---

## 11. Fidelity limits and uncertainty disclosure

Listing marketplace simulation carries inherently higher uncertainty than continuous-market
simulation. The engine attaches an **uncertainty disclosure** to every result:

- **Fill-rate report:** what percentage of buy orders found a qualifying listing at the time.
- **Days-to-fill distribution:** time from listing placement to fill across all sell orders.
- **Mark quality:** `mark_type` used and `sample_count` supporting it at each mark event.
- **Assumption sensitivity:** how results change if the demand model's acceptance rate shifts.
- **Conservative vs. demand-model split:** demand-model fills are reported separately so the
  reader can see how much of the result depends on modeled rather than observed fills.

These disclosures are part of the output contract. A marketplace backtest result without an
attached uncertainty disclosure is incomplete.

---

## 12. Output

A `TradeRecord` per buy fill and per sell fill:

| Field | Content |
|---|---|
| `setup` | `category_id`, `item_id` or `sku_id`, `item_filter` applied, `max_price` or `list_price`, `listing_id` matched |
| `execution` | `fill_price`, `platform_fee`, `royalty`, `gas`, `shipping`, `fill_type` (FixedPrice / AuctionWin / OfferAccepted / ComparableSale) |
| `hold_duration` | ns from buy fill to sell fill |
| `mark_at_buy` / `mark_at_sell` | `ComparableMarkEvent` price and `mark_type` at each fill event |
| `attribute_snapshot` | Caller-provided item attributes at decision time (for attribution) |
| `model_outputs` | Model scores that contributed to the decision (for attribution) |
| `assumption_used` | `Conservative` or `DemandModel` |
| `uncertainty` | Fill-rate, days-to-fill p50/p95, mark sample count (uncertainty disclosure) |

Optional wash-sale filtering can exclude known wash transactions and shill bids from comparables.

---

## 13. Determinism and ordering

- `ListingEvent`s apply in `(category_id, listing_id, seq)` order at each `ts_event`.
- Buy candidates include only listings whose `listing_ts ≤ decision_ts` and whose removal has
  not been observed at or before `decision_ts`.
- Auction bids from `AuctionBidEvent`s replay in `ts_event` order; the strategy's bid position
  at any `decision_ts` reflects all observed bids up to that point.
- Sell fills against comparable sales enforce `ts_fill > ts_listed` strictly (no look-ahead).
- The demand model, if probabilistic, draws from the run seed — deterministic across identical runs.

---

## 14. Open items / parameters

- Condition tier taxonomy: how many tiers, how labeled, whether caller-extensible or a fixed enum.
- Comparable matching radius: how similar must a comparable sale be to count (exact SKU match,
  broad category, trait-distance threshold for `IsUnique`)?
- Demand model prior: base acceptance-rate when historical data for a category is sparse.
- Offer counter-proposal modeling: whether the engine simulates counter-offer sequences.
- Physical fulfillment fallback: default shipping cost when no per-listing value is supplied.
- Wash-sale and shill-bid detection policy for marketplace data.
- Multi-unit `IsFungibleSKU` listings: split-fill rules when a single listing offers more units
  than the strategy requests.
