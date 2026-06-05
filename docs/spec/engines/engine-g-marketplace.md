# Engine G: Marketplace

**Status:** 🔲 Deferred — authored during its implementation phase. Architectural frame is fixed.

**`price_formation`:** `MARKETPLACE` · **Routes here:** NFTs, digital collectibles.

## What this spec will define
- Listing/bid/sale simulation against observed historical listings (no continuous order book).
- Floor-price marks; token-ID-keyed positions; rarity premium; royalties + marketplace fees; gas.
- The fundamental fidelity limit (sparse liquidity) and how uncertainty is disclosed in results.

## Fixed constraints (already decided)
- Implements the `Engine` trait; **no `Bar` payloads** for `IsUnique` instruments — only `Mark`
  (floor) and `NftEvent` ([assets/nfts.md](../assets/nfts.md)).
- Positions keyed by `{collection, token_id}`; account state via injected `Account`
  ([ADR-0010](../../adr/0010-suite-does-not-own-portfolio.md)).
- Deterministic, point-in-time; no price interpolation between sparse events.

## Source asset spec
[nfts](../assets/nfts.md)
