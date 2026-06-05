# Engine G: Marketplace

**Status:** ✅ Defined.
**`price_formation`:** `MARKETPLACE`
**Routes here:** NFTs, digital collectibles.

Engine G simulates a **marketplace** of unique items — listings, bids, and sales — not a
continuous order book. Price is observed only when a sale occurs; between sales it is unknown.
This engine is honest about a hard limit: **NFT simulation carries high uncertainty**, which it
discloses in results.

---

## 1. The `Engine` trait

```rust
impl Engine for MarketplaceEngine {
    fn on_event(&mut self, ev: &MarketEvent, ctx: &mut EngineContext);  // ingest listings/sales/floor
    fn submit_order(&mut self, o: Order, ctx: &mut EngineContext) -> OrderResult;  // buy / list
    fn settle(&mut self, ctx: &mut EngineContext);
    fn supports_order_type(&self, t: OrderType) -> bool;  // market (buy), listing (sell)
}
```

Positions are keyed by **`{collection_address, token_id}`** (not a fungible quantity). Cash is in
the injected `Account` ([ADR-0010](../../adr/0010-suite-does-not-own-portfolio.md)). For
`IsUnique` instruments there are **no `Bar` payloads** — only `Mark` (floor) and `NftEvent`
(listing/sale/bid).

---

## 2. No order book

There is no continuous bid/ask. The market is a stream of discrete `NftEvent`s: listings appear,
listings are removed, sales occur. The engine matches the strategy's intent against **what was
actually available** at each timestamp.

---

## 3. Buy execution

A buy order specifies a collection (and optionally trait filters) and a **max price**:

```
on buy(max_price):
    candidates = tokens listed at ts with listing_price ≤ max_price (matching filters)
    if candidates non-empty:
        fill at the lowest qualifying listing_price
        cost = listing_price + gas + marketplace_fee + creator_royalty
    else:
        unfilled  (you cannot buy what is not listed)
```

The engine **never invents a fill at an arbitrary price** — it can only buy against observed
listings. Absence of a qualifying listing is a real (and informative) non-fill.

---

## 4. Sell / list execution

A sell is a **listing** at a chosen price; a sale happens when a buyer arrives:

- **Conservative (default):** the listing fills only when a later **observed comparable sale**
  occurs at or above the listing price (matched by collection and, if available, trait/rarity).
- **Demand-model (optional):** a configurable acceptance model estimates fill probability/timing
  from historical demand — higher fidelity, more assumptions.

Either way the engine reports the realized sale price net of marketplace fee, royalty, and gas.

---

## 5. Marking

Unrealized P&L for a held token is marked to the **collection floor price** (`Mark` /
`FloorUpdate`) as the best available proxy — with the explicit caveat that a specific token may
be worth far more (rarity) or be unsellable at floor. No interpolation between sparse events.

---

## 6. Rarity

Rarity scores are **caller-provided** token metadata; the suite owns **no rarity methodology**.
The engine uses them only to (a) match trait-filtered buys and comparable sales and (b) attribute
realized **rarity premium** (sale price vs. floor). It does not *model* a rarity-premium curve —
that premium is non-linear and unstable across market regimes
([assets/nfts.md](../assets/nfts.md) §2.5).

---

## 7. Costs

Three real P&L lines, always populated:

- **Marketplace fee** (e.g. 0.5–2.5%),
- **Creator royalty** (e.g. 0–10%, enforcement varies by marketplace),
- **Gas** (material on Ethereum, negligible on Solana).

---

## 8. Fidelity limit & uncertainty disclosure

Because liquidity is sparse and buy fills depend on what is actually listed, results have high
variance. The engine attaches an **uncertainty disclosure** to NFT results (e.g. days-to-sell
distribution, fill-rate, sensitivity to the sale-arrival assumption) so a backtest is never
mistaken for a precise expectation.

---

## 9. Output

A `TradeRecord` per buy/sale: `setup` (collection/token/max-price or list price), `sizing`,
`execution` (price, marketplace fee, royalty, gas, days-to-sell, assumption used), `trigger`.
Optional wash-trade filtering can exclude known wash transactions from comparables.

---

## 10. Determinism & ordering

- `NftEvent`s apply in `(collection, token_id, seq)` order at each timestamp.
- Buys match only listings observed at-or-before the decision; sells fill only on later observed
  demand (look-ahead safety).
- The sale-arrival/demand model draws from the run seed if probabilistic.

---

## 11. Open items / parameters

- Default sale-arrival model (conservative observed-sale vs. demand model) and its parameters.
- Comparable-selection rule for trait/rarity matching.
- Wash-trade detection source.
- Floor-price source reconciliation across marketplaces.
