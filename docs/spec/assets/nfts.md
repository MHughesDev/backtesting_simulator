# Asset Spec: NFTs

**Engine:** G (Listing Marketplace)
**`price_formation`:** `MARKETPLACE`
**Capabilities:** `IsUnique | HasRarity | HasFloor | IsIlliquid | HasGasCost | HasRoyalty`

NFTs are a specific form of `ListingAsset` (`asset_class: NFT`). Engine G is a generalized
listing marketplace engine — see [engine-g-marketplace.md](../engines/engine-g-marketplace.md)
for the full spec. This document covers the NFT-specific data requirements and mechanics.

---

## 1. What NFTs are

An **NFT (Non-Fungible Token)** is a unique, non-interchangeable on-chain record of
ownership. Unlike fungible tokens (BTC, ETH, ERC-20) where each unit is identical, each NFT
has a distinct identity. Two NFTs from the same collection are *not* equivalent.

### Why NFTs are categorically different from every other asset

1. **Non-fungible.** You cannot "hold 2.5 CryptoPunks." You hold specific token IDs.
2. **No continuous price discovery.** There is no bid/ask for token #3421 unless a buyer
   and seller agree. Price is observed only at the moment of a sale.
3. **Illiquidity is structural.** A blue-chip NFT might trade once every few days. A
   mid-tier one might trade once a month. Standard OHLCV is mostly meaningless.
4. **Collection-level vs. token-level.** Strategy signals are usually at the *collection*
   level (floor price, volume, trend) while positions are at the *token* level.

### What "trading" NFTs means in practice

- **Buy strategy:** acquire a token at or below floor price, hold for appreciation.
- **Flip strategy:** buy a token, relist at a higher price quickly.
- **Rarity play:** acquire a rare-trait token at floor price; sell at rarity premium.
- **Floor sweep:** buy multiple floor tokens to control collection floor and re-list higher.
- **LP / perpetual NFT strategies:** some protocols (NFTX, NFT perps) offer fungibility layers.

The backtest simulates *listed price discovery* and *sale events*, not a continuous order book.

---

## 2. What a proper backtest requires

### 2.1 Collection-level data

| Field | Notes |
|---|---|
| `collection_address` | Smart contract address |
| `collection_slug` | Human-readable name (e.g. `cryptopunks`, `bayc`) |
| `floor_price` | Lowest listed ask across the collection, in ETH or SOL |
| `volume_24h` | Total notional traded in past 24h |
| `sales_count_24h` | Number of transactions |
| `unique_holders` | Number of distinct wallets holding ≥1 token |
| `total_supply` | Total tokens in the collection |
| `listed_count` | Number of tokens currently listed for sale |

Floor price is the most important signal. It is estimated by convention as the cheapest
currently-listed token — it is **not** a tradeable bid. Buying at floor requires someone
actually listing at that price when you try to buy.

### 2.2 Token-level data

| Field | Notes |
|---|---|
| `token_id` | The specific NFT within the collection |
| `traits` | `[ { trait_type, value } ]` — e.g. `{ "Background": "Blue" }` |
| `rarity_score` | Computed from trait frequencies; multiple methodologies exist |
| `rarity_rank` | Rank within collection (1 = rarest) |
| `last_sale_price` | Most recent realized sale price |
| `last_sale_ts` | Timestamp of that sale |
| `current_listing_price` | Current ask if listed; null if not listed |
| `current_listing_marketplace` | Which platform (OpenSea, Blur, Magic Eden) |

### 2.3 Sales history (the primary time-series data)

Sales events are the only "real" price observations. Between sales, price is unknown.

| Field | Notes |
|---|---|
| `collection_address` | |
| `token_id` | |
| `sale_price` | In native currency (ETH, SOL) and USD equivalent |
| `seller` | Wallet address |
| `buyer` | Wallet address |
| `marketplace` | Which marketplace processed the sale |
| `tx_hash` | On-chain transaction hash |
| `block_number` | For ordering within a block |
| `block_timestamp` | i64 ns UTC |
| `royalty_paid` | Creator royalty (e.g. 5% on OpenSea, often enforced differently on Blur) |
| `marketplace_fee` | Platform fee (typically 0.5–2.5%) |

### 2.4 Listing history

| Field | Notes |
|---|---|
| `token_id` | |
| `listing_price` | |
| `listing_ts` | When listed |
| `delisting_ts` | When delisted (null if still active) |
| `marketplace` | |

The listing history allows simulation of "what was available to buy" at each point in time —
important because a floor strategy can only buy what is actually listed.

### 2.5 Rarity — the most controversial dimension

Rarity score methodologies differ (Rarity Tools, Rarity Sniper, trait count, information content).
The **system does not own a rarity methodology** — the caller provides rarity scores as part
of token metadata. The spec defines the contract slot; the methodology is the caller's.

**Critical:** rarity premium is **not linear with rarity score and is not stable across market
conditions.** In bull markets, 1/1 traits may command 100× floor. In bear markets, the same
trait may trade at 1.2× floor. Any rarity-premium model must be empirically calibrated.

### 2.6 Gas costs

Every on-chain transaction (buy, list, transfer) requires gas. On Ethereum this is material;
on Solana it is negligible.

| Field | Notes |
|---|---|
| `gas_used` | Gas units consumed by the transaction |
| `gas_price_wei` | Price per unit at execution time |
| `total_gas_cost_usd` | For direct P&L accounting |

---

## 3. Data contract

### Required manifest (Engine G)

```
REQUIRED:
  InstrumentStatic (collection) {
    collection_address,
    chain_id,
    total_supply,
    currency: ETH | SOL | … ,
    asset_class = NFT
  }
  SalesHistory stream (per token)
  FloorPrice stream (collection-level)

REQUIRED for rarity strategies:
  TokenMetadata { token_id, traits[], rarity_score, rarity_rank }

OPTIONAL:
  ListingHistory stream
  GasHistory stream
  VolumeHistory stream (collection-level)
```

### Payload variants used

NFTs use the generalized Engine G payload types — see
[market-data.md](../contracts/market-data.md) §2.9 and §2.23 for full field definitions.

| Payload | NFT usage |
|---|---|
| `ListingEvent(Created)` | Token listed for sale at a price on a marketplace |
| `ListingEvent(Sold)` | Token sold; carries sale price, buyer, marketplace, gas, royalty |
| `ListingEvent(Removed)` | Token delisted without a sale |
| `ListingEvent(PriceChanged)` | Seller reduced or changed the asking price |
| `ComparableMarkEvent(Floor)` | Collection floor price — lowest active listing across the collection |
| `OfferEvent` | Standing collection/trait/token bids (`HasOffer` instruments only) |

For NFTs: `item_id = token_id`, `category_id = collection_address`, `chain_id = "eth"` (or
`"sol"`, etc.). The `ListingEvent.Created.attributes` field carries token traits and rarity rank.

---

## 4. Engine behavior (Engine G)

Engine G is the **Listing Marketplace** engine — NFTs are one class of instrument it simulates.
The full mechanics are specified in
[engine-g-marketplace.md](../engines/engine-g-marketplace.md). NFT-specific behavior:

- **Buy execution:** the strategy submits a buy order with a max price and optional trait
  filter. The engine checks whether any matching token is listed at or below that price at
  `decision_ts`. If yes, fill at the lowest qualifying asking price plus gas, marketplace fee,
  and creator royalty. If no qualifying listing exists, the order is unfilled.
- **Sell (list) execution:** the strategy submits a listing at a chosen price. The conservative
  default fills only at the next observed comparable sale at or above the listing price. The
  optional demand model estimates fill probability from `OfferEvent` bid depth and historical
  demand.
- **Unrealized P&L mark:** marked to the collection floor price (`ComparableMarkEvent`,
  `mark_type: Floor`) as the best available proxy for a token's value.

**Core constraint:** because there is no continuous price, the engine cannot simulate fills
at arbitrary prices. Buy orders are matched against actual historical listings only.

---

## 5. Performance metrics (extensions)

| Metric | Notes |
|---|---|
| Floor relative return | Token appreciation vs. floor price change |
| Rarity premium realized | How much above floor each token sold |
| Days to sell | Average time from buy to sell (liquidity metric) |
| Gas cost as % of P&L | Often material on Ethereum |
| Royalty + fee cost | Total creator royalties and marketplace fees paid |
| Wash-trade adjustment | Optionally filter known wash-trade transactions |

---

## 6. Implications for system design

1. **OHLCV does not apply.** There is no `high`, `low`, or continuous `close` for an
   individual NFT. The `Bar` payload is never emitted for NFTs. Only `ComparableMarkEvent`
   (floor/comparable) at the collection level and `ListingEvent` (sales/listings) at token
   level are valid.
2. **Sparse data requires different handling.** Events may be hours or days apart. The engine
   does not interpolate price between sales — absence of a sale is meaningful information.
3. **Positions are token-ID-specific.** The portfolio tracks `(category_id, item_id)` —
   equivalently `(collection_address, token_id)` — not just `collection_address`.
4. **NFT backtesting has fundamental limits.** Because liquidity is sparse and the buy price
   depends on what is actually listed, simulation results have high uncertainty. The engine
   attaches an uncertainty disclosure to every result (fill-rate, days-to-sell distribution,
   mark quality, assumption sensitivity) — see
   [engine-g-marketplace.md](../engines/engine-g-marketplace.md) §11.

---

## 7. Sources

- Coinbase: What is an NFT floor price
- Chainlink: What is an NFT floor price
- PMC / Scientific Reports: Heterogeneous rarity patterns drive price dynamics in NFT collections
- Medium (021pulse): Rarity versus floor — premium stability analysis
- Supra Oracles: NFT floor price dynamics
