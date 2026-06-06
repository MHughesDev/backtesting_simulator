# Plan 0007 — Engines B through H

**Date:** 2026-06-06
**Type:** Formal
**Author:** Agent
**Status:** Draft
**Derivation Status:** Current

## Goal

All seven remaining execution engines (B AMM, C NAV, D Cash Flow, E Derivatives, F Synthetic,
G Marketplace, H Event Resolution) are implemented in `crates/engines`, each tested with synthetic
fixtures, integrated with the single-run orchestrator from Plan 0005, and verified with a worked
example per engine. After this plan, the simulator supports all eleven asset classes.

Each engine milestone is **independently buildable** in parallel; they share no inter-engine
dependencies. The plan is sequenced by mathematical complexity and risk (simpler first).

---

## Derived From

- Artifact: [docs/artifact.md](../artifact.md) — SC-1 (all 11 asset classes), SC-2 (realistic fills), SC-3 (loud failure), SC-4 (deterministic)
- Architecture: [docs/architecture.md](../architecture.md) — §2 (Engines B–H)
- Specs:
  - [COMP-004](../specs/COMP-004-engine-b-amm.md) — Engine B: AMM
  - [COMP-005](../specs/COMP-005-engine-c-nav.md) — Engine C: NAV
  - [COMP-006](../specs/COMP-006-engine-d-cashflow.md) — Engine D: Cash Flow
  - [COMP-007](../specs/COMP-007-engine-e-derivatives.md) — Engine E: Derivatives
  - [COMP-008](../specs/COMP-008-engine-f-synthetic.md) — Engine F: Synthetic
  - [COMP-009](../specs/COMP-009-engine-g-marketplace.md) — Engine G: Marketplace
  - [COMP-010](../specs/COMP-010-engine-h-event-resolution.md) — Engine H: Event Resolution
  - [DATA-012](../specs/DATA-012-dex-amm-asset-spec.md) — DEX/AMM data
  - [DATA-010](../specs/DATA-010-etfs-and-funds-asset-spec.md) — ETFs & Funds data
  - [DATA-016](../specs/DATA-016-bonds-fixed-income-asset-spec.md) — Bonds data
  - [DATA-015](../specs/DATA-015-options-asset-spec.md) — Options data
  - [DATA-013](../specs/DATA-013-futures-asset-spec.md) — Futures data (Engine A extension)
  - [DATA-014](../specs/DATA-014-perpetuals-asset-spec.md) — Perpetuals data (Engine A extension)
  - [DATA-017](../specs/DATA-017-fx-asset-spec.md) — FX data (Engine A extension)
  - [DATA-018](../specs/DATA-018-nfts-asset-spec.md) — NFTs data
  - [DATA-019](../specs/DATA-019-prediction-markets-asset-spec.md) — Prediction markets data
- ADRs:
  - [ADR-0003](../adr/0003-capability-based-instrument-model.md) — capability flags drive all engine mechanics
  - [ADR-0009](../adr/0009-end-state-system-no-mvp.md) — all asset classes, no MVP

---

## Scope

**In scope:**
- Engine B (AMM): Uniswap v2 constant-product, Uniswap v3 concentrated liquidity (tick crossing),
  Curve StableSwap hybrid; gas cost as first-class P&L; approximate mode when v3 tick data absent
- Engine C (NAV): mutual fund forward pricing at NAV; holdings-derived NAV from basket snapshot;
  leveraged/inverse ETF daily reset (not cumulative); ETF = Engine A CLOB + Engine C valuation layer
- Engine D (Cash Flow): YTM solve via Newton's method; accrued interest by day-count convention;
  dirty-price fills; duration/DV01/convexity; credit-rating repricing; MBS PSA/CPR prepayment model
- Engine E (Derivatives): Black-Scholes-Merton for European options; numerical method (binomial
  tree) for American early exercise; IV surface bilinear interpolation; delta/gamma/vega/theta/rho
  at every fill; exercise/assignment state machine; short option margin
- Engine F (Synthetic): payoff-component model for CFDs, barrier notes, autocalls; WASM sandbox
  for untrusted/AI-authored payoff components; barrier and autocall state machines; CFD overnight
  financing
- Engine G (Marketplace): per-token/per-item position accounting; listing-based buy fills; floor
  price marking with uncertainty disclosure; comparable-sale sell fills; gas cost P&L; royalty;
  physical fulfillment costs
- Engine H (Event Resolution): lifecycle state machine (Created→Active→Locked→Resolved→Settled);
  probability-bounded trading; oracle-driven settlement; Brier-score metric; oracle risk modeling
- Engine A capability extensions (completing what was deferred in Plan 0005):
  - `HasFunding`: perpetual swap funding accrual (DATA-014)
  - `HasLiquidation`: mark-price margin check and force liquidation (DATA-014)
  - `HasRollSchedule`: continuous futures series construction (DATA-013)
  - `HasSwapRates`: FX overnight rollover (DATA-017)
  - `HasCorporateActions`: equity dividend/split adjustment (DATA-009)
  - `HasMakerTakerFees`: tiered crypto fee schedule (DATA-011)
- Synthetic fixtures per engine in `fixtures/`
- One worked example per engine in `examples/`
- `DataSufficiencyError` checks completed for all 8 engines (→ DATA-002 §10 per-engine minimums)

**Out of scope:**
- Parallel run queue — Plan 0008
- WASM runtime integration for Engine F — Plan 0009 (Engine F stub uses an in-process payoff
  function that implements the payoff component interface; WASM sandbox added in Plan 0009)
- Full metrics contract (DATA-008) — Plan 0010
- Python examples — can be added after Plan 0006

---

## Dependencies

- Plan 0005 (Engine A + single-run orchestrator) fully complete.
- Plan 0004 (core crate) fully complete.
- Open decision OD-4 (Derivatives math: build in Rust vs. optional QuantLib plugin): resolved in
  favor of "build in Rust" for the binomial-tree model (avoid QuantLib's C++ FFI complexity);
  record in open-questions.md before Milestone 4 (Engine E).

---

## Risks

- Risk: Engine E (Derivatives) requires an IV surface for every run — a hard requirement, not
  degraded-fidelity. Synthetic IV surface fixtures must be realistic enough to exercise the
  interpolation code. → Mitigation: generate a synthetic `IVSurface` with plausible term structure
  and smile in the fixture generator.
- Risk: Engine D (Cash Flow) YTM Newton's method may fail to converge for edge-case bonds.
  → Mitigation: bound the iteration to 100 rounds; return `ConversionError` with a diagnostic
  if no root is found within tolerance; unit-test extreme yield cases.
- Risk: Engine F payoff components must be pure (no I/O). WASM integration is deferred but the
  in-process payoff trait must already enforce purity by convention. → Mitigation: the `PayoffComponent`
  trait takes only a `&PayoffContext` (no I/O, no timestamps beyond the snapshot) and has no
  mutable global access; document the purity requirement in the trait doc-comment.
- Risk: Engine B Uniswap v3 tick-crossing is algorithmically complex; an off-by-one in tick
  indexing produces incorrect fill prices. → Mitigation: port the reference tick math from the
  Uniswap v3 whitepaper; unit-test against known on-chain swap amounts from fixture data.

---

## Milestones

| # | Milestone | Source | Success Signal |
|---|-----------|--------|----------------|
| 1 | Engine H (Event Resolution) | COMP-010; DATA-019 | Prediction market lifecycle runs; Brier score computed on settled market |
| 2 | Engine G (Marketplace) | COMP-009; DATA-018 | NFT buy fills only when listing existed; floor-price mark attached |
| 3 | Engine B (AMM) | COMP-004; DATA-012 | Uniswap v2 trade fills at correct constant-product price; gas debited |
| 4 | Engine C (NAV) | COMP-005; DATA-010 | Mutual fund fills at end-of-day NAV; leveraged ETF daily reset correct |
| 5 | Engine D (Cash Flow) | COMP-006; DATA-016 | Bond fills at dirty price; YTM computed; coupon accrual is correct |
| 6 | Engine A extensions | COMP-003; DATA-013/014/017 | Perpetual funding accrual and liquidation work; FX rollover debits correctly |
| 7 | Engine E (Derivatives) | COMP-007; DATA-015 | BSM European option priced; IV interpolated; greeks at every fill; American early exercise triggers |
| 8 | Engine F (Synthetic) | COMP-008 | CFD overnight financing correct; barrier state machine triggers knock-out; WASM stub compiles |

---

## Tasks by Milestone

### Milestone 1: Engine H — Event Resolution

- `NOT STARTED` Implement `EngineH` with `MarketLifecycleStateMachine`:
  `Created → Active → Locked → Resolved → Settled` (→ COMP-010 lifecycle)
- `NOT STARTED` Implement fill gating: reject orders when state is `Locked` or `Resolved`
  (→ COMP-010; artifact SC-2)
- `NOT STARTED` Implement probability price bounds: fill price must be in `(0.0, 1.0)`;
  reject orders outside bounds (→ COMP-010; DATA-019)
- `NOT STARTED` Implement oracle-triggered settlement: on `Resolution` event, mark all open
  positions to 0 or 1; compute P&L (→ COMP-010; DATA-002 §10 Engine H minimum: Resolution stream)
- `NOT STARTED` Compute Brier score per trade: `(entry_price − outcome)²`; attach to `TradeRecord`
  (→ COMP-010 Brier score; README §Engine H)
- `NOT STARTED` Create `fixtures/prediction_market_binary.arrow`; example `examples/prediction_market_resolution.rs`
  (→ artifact SC-1)

### Milestone 2: Engine G — Marketplace

- `NOT STARTED` Implement `EngineG` with per-item position tracking keyed by
  `(category_id, item_id)` for `IsUnique` instruments and `(sku_id, condition_tier)` for
  `IsFungibleSKU` instruments (→ COMP-009; DATA-003 `IsUnique`, `IsFungibleSKU`)
- `NOT STARTED` Implement listing-based buy fill: fill only when a qualifying `ListingEvent`
  (at or below max price) existed at or before decision time; reject otherwise — never invent a fill
  (→ COMP-009; README §Engine G)
- `NOT STARTED` Implement comparable-sale sell fill: match against observed historical `NftEvent`
  sales; attach `UncertaintyReport` to `TradeRecord` with fill rate and days-to-sell distribution
  (→ COMP-009)
- `NOT STARTED` Implement floor-price marking from `FloorUpdate` events; tag mark as `FloorPrice`
  type with lower-bound disclaimer (→ COMP-009; DATA-003 `HasFloor`)
- `NOT STARTED` Implement gas cost P&L debit when `HasGasCost` and `GasEvent` stream present
  (→ DATA-003 `HasGasCost`; COMP-009)
- `NOT STARTED` Create `fixtures/nft_collection.arrow`; example `examples/nft_collection_sweep.rs`
  (→ artifact SC-1)

### Milestone 3: Engine B — AMM

- `NOT STARTED` Implement `EngineB` with Uniswap v2 constant-product swap:
  `delta_x = (x * delta_y) / (y - delta_y)` for exact-output; apply fee bps
  (→ COMP-004 §UniswapV2; README §Engine B)
- `NOT STARTED` Implement Uniswap v3 concentrated liquidity: tick-range iteration,
  `√P` computation, partial fill at tick boundaries; fall back to approximate constant-liquidity
  mode and flag `FidelityWarning` when tick data absent (→ COMP-004 §UniswapV3)
- `NOT STARTED` Implement Curve StableSwap invariant swap for stablecoin pools (→ COMP-004 §Curve)
- `NOT STARTED` Implement gas cost as first-class P&L line: debit in chain-native token
  converted at `GasEvent.price_usd` (→ COMP-004; DATA-003 `HasGasCost`)
- `NOT STARTED` Create `fixtures/uniswap_v2_pool.arrow` and `fixtures/uniswap_v3_pool.arrow`;
  example `examples/dex_amm_arb.rs` (→ artifact SC-1)

### Milestone 4: Engine C — NAV

- `NOT STARTED` Implement `EngineC` mutual fund forward pricing: accept order at submission
  time, fill at end-of-day `Nav` event — fill price unknown at submission (→ COMP-005; README §Engine C)
- `NOT STARTED` Implement holdings-derived NAV: sum `HoldingsSnapshot` positions weighted by
  closing prices of underlyings; tag as `derived: true` (→ COMP-005 §NAV derivation)
- `NOT STARTED` Implement ETF Engine A + Engine C composition: CLOB execution via Engine A for
  intraday fills; NAV valuation layer via Engine C for daily mark (→ COMP-005; DATA-010)
- `NOT STARTED` Implement leveraged/inverse ETF daily reset: apply leverage factor to daily
  index return, reset leverage daily (never cumulatively) (→ COMP-005; README §Engine C; DATA-010)
- `NOT STARTED` Create `fixtures/mutual_fund_nav.arrow`, `fixtures/leveraged_etf.arrow`;
  example `examples/etf_daily_reset.rs` (→ artifact SC-1)

### Milestone 5: Engine D — Cash Flow

- `NOT STARTED` Implement `EngineD` with YTM Newton's method solve: given clean price, solve
  for YTM; handle convergence failure with `ConversionError` (→ COMP-006; README §Engine D)
- `NOT STARTED` Implement day-count accrued interest: `ACT/ACT` for Treasuries, `30/360` for
  corporates, `ACT/360` for T-bills; dirty price = clean + accrued (→ COMP-006; DATA-016)
- `NOT STARTED` Compute duration, modified duration, DV01, convexity from cash flows if not
  supplied by caller (→ COMP-006; DATA-003 `provide_or_derive`)
- `NOT STARTED` Implement credit-rating repricing: on `CreditRatingEvent`, immediately reprice
  open position at new yield spread in event-timestamp order (→ COMP-006; DATA-003 `HasCreditRisk`)
- `NOT STARTED` Implement MBS prepayment PSA/CPR curve: shorten remaining cash-flow stream
  when rates change (→ COMP-006; DATA-016)
- `NOT STARTED` Create `fixtures/us_treasury_10yr.arrow`, `fixtures/corporate_bond.arrow`;
  example `examples/bond_yield_curve.rs` (→ artifact SC-1)

### Milestone 6: Engine A capability extensions

- `NOT STARTED` Implement `HasFunding` perpetual swap funding accrual:
  debit/credit `funding_rate * position_value` every `funding_interval_hours`
  (→ COMP-003 §HasFunding; DATA-014; README §Engine A)
- `NOT STARTED` Implement `HasLiquidation` mark-price margin check: compute
  maintenance margin from `maintenance_margin_rate * position_notional`; force liquidation
  if unrealized loss erodes collateral below threshold; never use last-trade price for liquidation
  (→ COMP-003 §HasLiquidation; DATA-014; README §Engine A look-ahead safety)
- `NOT STARTED` Implement `HasRollSchedule` continuous futures: construct continuous price series
  from `RollSchedule` reference data; flag roll yield in `TradeRecord` (→ COMP-003; DATA-013)
- `NOT STARTED` Implement `HasSwapRates` FX overnight rollover: debit/credit based on
  interest rate differential from `SwapRate` stream; triple rollover on Wednesday
  (→ COMP-003 §HasSwapRates; DATA-017; README §FX)
- `NOT STARTED` Implement `HasCorporateActions` equity adjustments: on `CorporateAction` event
  (dividend, split, merger), update the unadjusted price series and derive adjusted series
  (→ COMP-003 §HasCorporateActions; DATA-009)
- `NOT STARTED` Implement `HasMakerTakerFees` tiered crypto fees: resolve fee tier from
  30-day rolling volume; apply maker/taker rate from `FeeScheduleUpdate` events
  (→ COMP-003 §HasMakerTakerFees; DATA-011)

### Milestone 7: Engine E — Derivatives

- `NOT STARTED` Implement `EngineE` Black-Scholes-Merton pricing for European options:
  `C = S·N(d1) − K·e^(-rT)·N(d2)` (→ COMP-007; README §Engine E)
- `NOT STARTED` Implement IV surface bilinear interpolation on `(log_moneyness, expiry)` axes
  from `IVSurface` payload (→ COMP-007; DATA-003 `HasIVSurface`)
- `NOT STARTED` Compute delta, gamma, vega, theta, rho at every fill; attach to `TradeRecord`
  for P&L attribution (→ COMP-007; DATA-003 `HasGreeks`)
- `NOT STARTED` Implement binomial tree for American option pricing and early exercise boundary;
  check `HasEarlyExercise` flag to activate (→ COMP-007; OD-4 resolved: binomial tree)
- `NOT STARTED` Implement exercise/assignment state machine for short option positions:
  track whether position is in-the-money; trigger assignment at expiry
  (→ COMP-007; DATA-003 `HasEarlyExercise`)
- `NOT STARTED` Create `fixtures/options_iv_surface.arrow`, `fixtures/equity_option_chain.arrow`;
  example `examples/options_iv_surface.rs` (→ artifact SC-1)

### Milestone 8: Engine F — Synthetic

- `NOT STARTED` Define `PayoffComponent` trait: `evaluate(ctx: &PayoffContext) -> (Money, PayoffState)`;
  `PayoffContext` contains current underlying prices, contract params, accumulated path state
  (→ COMP-008; ADR-0011 trust tiers)
- `NOT STARTED` Implement in-process `PayoffComponent` for a vanilla CFD: `pnl = (current_price − entry_price) * qty * leverage`
  (→ COMP-008 §CFD)
- `NOT STARTED` Implement CFD overnight financing charge: debit `position_notional * financing_rate / 365`
  per calendar day (→ COMP-008 §CFD financing)
- `NOT STARTED` Implement barrier state machine: `KnockIn` / `KnockOut` at barrier level;
  path-dependent (checks every event) (→ COMP-008 §barrier notes)
- `NOT STARTED` Implement autocall state machine: check autocall observation date; trigger
  early redemption if underlying at or above call level (→ COMP-008 §autocall)
- `NOT STARTED` Implement WASM sandbox stub: `WasmPayoffComponent` that satisfies the
  `PayoffComponent` trait interface but panics with `WasmNotYetIntegrated` — WASM runtime
  added in Plan 0009 (→ COMP-008; ADR-0011)
- `NOT STARTED` Create `fixtures/cfd_underlying.arrow`, `fixtures/barrier_note.arrow`;
  example `examples/synthetic_barrier_note.rs` (→ artifact SC-1)

---

## Open Questions

- [ ] OD-4 (Derivatives math) resolved here: use binomial tree for American options (not
  QuantLib). Record this resolution in `docs/open-questions.md` before Milestone 7 begins.
- [ ] Engine B approximate mode flag: when Uniswap v3 tick data is absent, the engine falls
  back to approximate constant-liquidity mode. Is this a `FidelityWarning` (run proceeds) or a
  `DataSufficiencyError` (run rejects)? COMP-004 says "flag the result" → treat as `FidelityWarning`.
  Confirm before Milestone 3.
- [ ] Engine G `UncertaintyReport` fields: what quantiles and window sizes for the days-to-sell
  distribution? Specify in COMP-009 before Milestone 2.

---

## Change Log

| Date | Change | Author |
|------|--------|--------|
| 2026-06-06 | Initial draft | Agent |
