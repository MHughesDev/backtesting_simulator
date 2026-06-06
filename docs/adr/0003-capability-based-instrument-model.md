# ADR-0003: Capability-based instrument model (asset ≠ engine)

- **Status:** Accepted
- **Date:** 2026-06-04
- **Deciders:** Project owner
- **Informed by:** MASTER_SPEC §2–§4, §6

## Context

A universal system must avoid hard-coding behavior per asset category. The naive approach —
`match asset_type { Stock => …, Crypto => …, Option => … }` scattered through engines and
strategies — does not scale to "every digital asset," produces duplicated logic, and breaks
on instruments that don't fit one box (an ETF that executes on an order book but is valued by
NAV; spot crypto on a CEX vs. the same token in an AMM pool).

The key observation: **asset category does not determine simulation — price formation does.**
`BTC` on a central limit order book and `ETH` in a Uniswap pool are both crypto yet require
different engines.

## Decision

We will model every tradable as an **Instrument** that advertises:

1. a **`price_formation`** field (CLOB | AMM | NAV | Dealer | Quote | OTC | Marketplace |
   Oracle) — the *only* thing that selects an engine, and
2. a set of **capability flags** (`HasOrderBook`, `HasFunding`, `HasExpiry`, `HasGreeks`,
   `HasPoolReserves`, `HasCoupon`, `HasNAV`, `IsLeveraged`, `IsUnique`, …).

Engines and strategies react to **capabilities**, never to an asset-type enum. Data payloads
and order types are gated by capabilities; valid data per (instrument, engine) is declared by
a **required-data manifest**. Instruments may compose multiple engines (e.g. ETF = order-book
execution + NAV valuation).

## Alternatives considered

- **Asset-type enum + per-type branches** — simple at first, but duplicates logic, can't
  express composite instruments, and doesn't scale to arbitrary new assets. Rejected.
- **One engine per asset class (flat menu)** — closer, but still conflates "what it is" with
  "how its price forms," and can't reuse an engine across classes or compose two. Rejected in
  favor of price-formation-selects-engine + capabilities.

## Consequences

- **Positive:** genuinely universal; new assets are added by declaring capabilities, not by
  editing engines; composite instruments work; strategies are portable by default and only
  diverge where they opt into a capability.
- **Negative / accepted tradeoffs:** more upfront modeling rigor (capability taxonomy,
  per-(instrument,engine) manifests); capability/engine compatibility must be validated.
- **Follow-ups:** finalize the capability taxonomy and the field-level required-data manifests
  per asset class (detailed spec under `docs/specs/`).
