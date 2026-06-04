# Asset Spec: NFTs

**Engine:** G (Marketplace)
**`price_formation`:** `MARKETPLACE`
**Capabilities:** `IsUnique | HasRarity | HasFloor | IsIlliquid`

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

| Payload | Description |
|---|---|
| `NftEvent(Sale) { token_id, price, buyer, seller, marketplace, ts }` | Realized sale |
| `NftEvent(Listing) { token_id, price, marketplace, ts }` | New listing |
| `NftEvent(Delisting) { token_id, marketplace, ts }` | Listing removed |
| `Mark { price }` | Collection floor price (best available mark for unrealized P&L) |

---

## 4. Engine behavior (Engine G)

Engine G simulates a **marketplace**: listings, bids, and sales — not a CLOB.

- **Buy execution:** the strategy submits a buy order specifying a max price. The engine checks
  whether any token is listed at or below that price at the event timestamp. If yes, fill at
  listing price + gas. If no, order is unfilled.
- **Sell (list) execution:** the strategy submits a listing at a specified price. The engine
  records the listing. A sale occurs when a simulated buyer arrives (driven by historical
  demand / price-acceptance model, or conservatively: only at the next observed sale price
  for a similar token).
- **Unrealized P&L mark:** marked to collection floor price as the best available proxy for
  a token's value when not in the process of being sold.

**Important limitation:** because there is no continuous price, the engine cannot simulate
fills at arbitrary prices. Buy orders are matched against actual historical listings only.

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
   individual NFT. The `Bar` payload is never emitted for NFTs. Only `Mark` (floor) at the
   collection level and `NftEvent` (sales/listings) at token level are valid.
2. **Sparse data requires different handling.** Events may be hours or days apart. The engine
   must not interpolate price between sales — absence of a sale is meaningful information.
3. **Positions are token-ID-specific.** The portfolio tracks `{ collection_address, token_id }`
   as the position key, not just `{ collection_address }`.
4. **NFT backtesting has fundamental limits.** Because liquidity is sparse and the buy price
   depends on what is actually listed, simulation results have high uncertainty. Results should
   always be accompanied by a confidence interval or sensitivity analysis. This should be
   disclosed in the result contract.

---

## 7. Sources

- Coinbase: What is an NFT floor price
- Chainlink: What is an NFT floor price
- PMC / Scientific Reports: Heterogeneous rarity patterns drive price dynamics in NFT collections
- Medium (021pulse): Rarity versus floor — premium stability analysis
- Supra Oracles: NFT floor price dynamics
