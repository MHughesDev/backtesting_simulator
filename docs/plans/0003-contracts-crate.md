# Plan 0003 — Contracts Crate

**Date:** 2026-06-06
**Type:** Formal
**Author:** Agent
**Status:** Draft
**Derivation Status:** Current

## Goal

`crates/contracts` is fully implemented: every shared cross-boundary type is defined, the
capability-flag bitmask is complete, all 40+ `MarketEvent` payload variants are declared, the
three integration ports (`Account`, `Model`, `Trainer`) are trait-defined, and the `DataManifest`
validator rejects under-specified runs with typed errors. The crate compiles with zero external
dependencies beyond Tier-A serde/decimal types per ADR-0012. Plan 0004 (Core) can begin in
parallel once this plan's Milestone 1 types are stabilized.

---

## Derived From

- Artifact: [docs/artifact.md](../artifact.md) — SC-1 (universal contract spine), SC-3 (loud failures), SC-6 (owns nothing)
- Architecture: [docs/architecture.md](../architecture.md) — §2 Contract Validator, §4 External Dependencies
- Specs:
  - [DATA-003](../specs/DATA-003-instrument-contract.md) — Instrument struct, PriceFormation, capability flags
  - [DATA-004](../specs/DATA-004-market-data-contract.md) — MarketEvent envelope + all payload variants
  - [DATA-005](../specs/DATA-005-signals-contract.md) — SignalEvent, EntityMetric, DocumentSignal, MediaReference
  - [DATA-002](../specs/DATA-002-run-request.md) — DataManifest, validation order §10, error types
  - [INTG-001](../specs/INTG-001-account-ledger-port.md) — Account port trait
  - [INTG-002](../specs/INTG-002-ai-model-inference-port.md) — Model port trait
  - [INTG-003](../specs/INTG-003-training-port.md) — Trainer port trait
- ADRs:
  - [ADR-0003](../adr/0003-capability-based-instrument-model.md) — capability flags, no asset-type switches
  - [ADR-0010](../adr/0010-simulator-does-not-own-portfolio.md) — Account port; simulator owns no portfolio
  - [ADR-0012](../adr/0012-standalone-contracts-kernel.md) — dependency-free kernel

---

## Scope

**In scope:**
- `Instrument` struct with all fields from DATA-003 (identity, `price_formation`, capabilities, quote params, settlement, all capability-gated static metadata)
- `PriceFormation` enum: `CLOB | AMM | NAV | DEALER | CHAIN | OTC | MARKETPLACE | ORACLE`
- `CapabilityFlags` bitmask with all 46 flags from DATA-003
- `AssetClass` enum (informational; 12+ variants per DATA-003)
- All supporting enums: `SettlementType`, `OptionType`, `ExerciseStyle`, `PerpType`, `AmmVariant`, `DayCountConvention`, `OracleType`, `FeeTier`, `TokenInfo`
- `MarketEvent` envelope: `ts_event`, `ts_recv`, `instrument_id`, typed `payload`
- All 40+ payload variant types from DATA-004: `Bar`, `Trade`, `Quote`, `BookSnapshot`, `BookDelta`, `OrderBookOrderEvent`, `AuctionImbalance`, `AuctionResult`, `TradingStatus`, `CorporateAction`, `BorrowRate`, `UniverseMembership`, `FeeScheduleUpdate`, `TokenEvent`, `PoolState`, `SwapEvent`, `GasEvent`, `Nav`, `HoldingsSnapshot`, `CreationRedemptionBasket`, `RollSchedule`, `OpenInterest`, `Funding`, `MarkUpdate`, `IVSurface`, `Greeks`, `ExerciseEvent`, `Mark`, `YieldUpdate`, `YieldCurve`, `CreditSpread`, `CreditRatingEvent`, `Coupon`, `SwapRate`, `NftEvent`, `FloorUpdate`, `NftBidEvent`, `Resolution`, `MarketLifecycleEvent`, `OracleEvent`, `DerivedBar`, `LiquidationEvent`
- Signal-plane types from DATA-005: `SignalEvent`, `EntityMetric`, `DocumentSignal`, `MediaReference`, `ContextBundle`
- Operational/meta envelope: `OperationalEvent` (lineage, warnings, audit)
- `Account` port trait (DATA-002 §5 / INTG-001)
- `Model` port trait (INTG-002)
- `Trainer` port trait (INTG-003)
- `DataManifest` struct and `ManifestViolation` / `DataSufficiencyError` / `ResolutionConflict` / `FidelityWarning` error types (DATA-002 §10)
- `Fill` struct (referenced in Account.apply_fill; needed by engines)
- `TradeRecord` struct skeleton (DATA-002 §7 — full metrics deferred to Plan 0005)
- `InstrumentId`, `VenueId`, `CurrencyCode` newtypes
- `Decimal` money type wrapper (re-export from `rust_decimal` or thin newtype per ADR-0002)

**Out of scope:**
- Validation *logic* (the `validate()` fn that checks a RunRequest) — Plan 0005 (runner)
- Arrow schema definitions for data transfer — Plan 0006
- Any engine logic — Plans 0005, 0007
- `RunRequest` full parsing — Plan 0005

---

## Dependencies

- Plan 0002 must be complete (workspace scaffold exists, `crates/contracts` has a skeleton).
- DATA-008 (Result/Metrics Contract) is deferred; `TradeRecord` struct is a skeleton with key
  fields only, extended in Plan 0005.

---

## Risks

- Risk: Payload variant count is large (40+); missing one breaks downstream engine conformance.
  → Mitigation: generate the variant list directly from DATA-004 §2 payload table; check off each
  variant when implemented.
- Risk: Capability flag count (46+) and validation rules are complex; contradictory combinations
  may be missed. → Mitigation: unit-test a representative set of valid and invalid flag
  combinations against DATA-003 §2 validation rules.
- Risk: ADR-0012 requires zero engine dependencies in `contracts`; a careless `use engines::*`
  import breaks this. → Mitigation: `#![deny(clippy::wildcard_imports)]` and CI enforces the dep
  graph is acyclic with `contracts` at the root.

---

## Milestones

| # | Milestone | Source | Success Signal |
|---|-----------|--------|----------------|
| 1 | Core identity & routing types | DATA-003; ADR-0003; ADR-0012 | `Instrument` and `PriceFormation` compile; capability flags round-trip via serde |
| 2 | Full MarketEvent payload suite | DATA-004; DATA-005 | Every payload variant compiles; `MarketEvent` is serde-roundtrippable |
| 3 | Integration ports declared | INTG-001/002/003; ADR-0010 | `Account`, `Model`, `Trainer` traits compile; doc-test passes |
| 4 | DataManifest & error types | DATA-002 §10; artifact SC-3 | `DataManifest` constructs and error types serialize/deserialize with typed fields |

---

## Tasks by Milestone

### Milestone 1: Core identity & routing types

- `NOT STARTED` Define `InstrumentId`, `VenueId`, `CurrencyCode` as validated newtypes with
  serde (→ DATA-003 identity block)
- `NOT STARTED` Define `PriceFormation` enum with 8 variants and serde rename attributes
  (`"CLOB"`, `"AMM"`, etc.) (→ DATA-003 routing table)
- `NOT STARTED` Define `CapabilityFlags` as a bitflags struct with all 46 named flags from
  DATA-003 §capability-flags table; add `is_valid_combination()` returning `Result<(), Vec<String>>`
  for contradictory combos (→ DATA-003; ADR-0003)
- `NOT STARTED` Define `AssetClass` enum (→ DATA-003 taxonomy block)
- `NOT STARTED` Define all supporting enums: `SettlementType`, `OptionType`, `ExerciseStyle`,
  `PerpType`, `AmmVariant`, `DayCountConvention`, `OracleType` (→ DATA-003 capability-gated
  metadata block)
- `NOT STARTED` Define `FeeTier` and `TokenInfo` structs (→ DATA-003)
- `NOT STARTED` Define `Instrument` struct with every field from DATA-003, all `Option<T>`
  capability-gated fields included (→ DATA-003 full instrument structure)
- `NOT STARTED` Implement `Instrument::validate()` checking: `price_formation` matches
  capability flags, contradictory flag combos rejected, required fields present for each
  flag (→ DATA-003 §required-data-manifest; ADR-0003)
- `NOT STARTED` Unit-test: equity instrument, BTC-perp, ETH-Uniswap-v3-pool, bond, option,
  NFT — all round-trip through serde and pass validation (→ DATA-009 through DATA-019)

### Milestone 2: Full MarketEvent payload suite

- `NOT STARTED` Define `Timestamp` newtype (nanoseconds UTC) and `Decimal` money alias
  (→ SYS-001 §11 glossary; ADR-0002)
- `NOT STARTED` Define `MarketEvent` envelope: `ts_event: Timestamp`, `ts_recv: Timestamp`,
  `instrument_id: InstrumentId`, `payload: MarketPayload` (→ DATA-004 §1 universal envelope)
- `NOT STARTED` Define `MarketPayload` enum with all Market-Data Plane variant types from
  DATA-004 §2: `Bar`, `Trade`, `Quote`, `BookSnapshot`, `BookDelta`, `OrderBookOrderEvent`,
  `AuctionImbalance`, `AuctionResult`, `TradingStatus`, `CorporateAction`, `BorrowRate`,
  `UniverseMembership`, `FeeScheduleUpdate`, `TokenEvent`, `PoolState`, `SwapEvent`, `GasEvent`,
  `Nav`, `HoldingsSnapshot`, `CreationRedemptionBasket`, `RollSchedule`, `OpenInterest`,
  `Funding`, `MarkUpdate`, `IVSurface`, `Greeks`, `ExerciseEvent`, `Mark`, `YieldUpdate`,
  `YieldCurve`, `CreditSpread`, `CreditRatingEvent`, `Coupon`, `SwapRate`, `NftEvent`,
  `FloorUpdate`, `NftBidEvent`, `Resolution`, `MarketLifecycleEvent`, `OracleEvent`,
  `DerivedBar`, `LiquidationEvent` (→ DATA-004 §2)
- `NOT STARTED` Define each payload struct with all fields from DATA-004 §2; mark
  `derived: bool` and `source_class: Option<PayloadClass>` on `DerivedBar` (→ DATA-004; DATA-002 §4a)
- `NOT STARTED` Define Signal-plane types from DATA-005: `SignalEvent`, `EntityMetric`,
  `DocumentSignal`, `MediaReference`, `ContextBundle` — each carrying `ts_available`
  (→ DATA-005; SYS-001 §11 `ts_available`)
- `NOT STARTED` Define `OperationalEvent` for lineage/warning/audit records emitted by the
  simulator (→ DATA-001 Operational/Meta Plane; DATA-002 §7 `emit`)
- `NOT STARTED` Unit-test: every `MarketPayload` variant round-trips through serde; `ts_event`
  ordering is consistent in a sorted Vec (→ artifact SC-4)

### Milestone 3: Integration ports declared

- `NOT STARTED` Define `Fill` struct: `instrument_id`, `side`, `fill_price: Decimal`,
  `fill_qty: Decimal`, `fees: Decimal`, `ts_fill: Timestamp` (→ DATA-002 §7 TradeRecord
  execution block; needed by Account.apply_fill)
- `NOT STARTED` Define `Position` struct: `instrument_id`, `quantity: Decimal`,
  `avg_entry_price: Decimal`, `unrealized_pnl: Decimal` (→ DATA-002 §5 Account port)
- `NOT STARTED` Define `Account` trait with methods: `equity`, `buying_power`, `position`,
  `collateral`, `apply_fill` exactly as in DATA-002 §5 (→ DATA-002 §5; ADR-0010)
- `NOT STARTED` Define `ModelInput` and `ModelOutput` types; define `Model` trait with
  `infer(context: ContextBundle) -> ModelOutput` (→ INTG-002)
- `NOT STARTED` Define `TrainingConfig` and `ModelArtifact` types; define `Trainer` trait with
  `train(config: TrainingConfig, data_window: TimeRange) -> ModelArtifact` (→ INTG-003)
- `NOT STARTED` Unit-test: each trait compiles as a `dyn Trait` object; mock impl can be
  constructed for test harness (→ artifact SC-6)

### Milestone 4: DataManifest & error types

- `NOT STARTED` Define `PayloadClass` enum listing every data class by name
  (→ DATA-003 required-data-manifest; DATA-002 §10)
- `NOT STARTED` Define `DataManifest` struct: `instrument_id`, `engine: EngineType`,
  `required: Vec<PayloadClass>`, `optional: Vec<PayloadClass>`,
  `provide_or_derive: Vec<PayloadClass>` (→ DATA-003 §required-data-manifest)
- `NOT STARTED` Define `EngineType` enum: `CLOB | AMM | NAV | DEALER | CHAIN | OTC | MARKETPLACE | ORACLE`
  (→ DATA-002 §10 per-engine minimums)
- `NOT STARTED` Define `ManifestViolation` error type with `instrument_id` and
  `missing_required: Vec<PayloadClass>` (→ DATA-002 §10 validation order step 4)
- `NOT STARTED` Define `DataSufficiencyError` with `missing_required`, `non_derivable_conflict:
  Vec<ResolutionConflict>`, `degraded_fidelity: Vec<FidelityWarning>` (→ DATA-002 §10)
- `NOT STARTED` Define `ResolutionConflict` struct: `required_interval`, `available_interval`
  (→ DATA-002 §10)
- `NOT STARTED` Define `FidelityWarning` struct: `missing_payload_class`, `fidelity_impact`
  (→ DATA-002 §10)
- `NOT STARTED` Define `TradeRecord` struct skeleton: `ts_decision`, `ts_fill`,
  `instrument_id`, `setup`, `sizing`, `execution`, `trigger`, `account_context` (→ DATA-002 §7;
  full metrics in Plan 0005)
- `NOT STARTED` Verify `cargo test --package contracts` passes with no warnings (→ artifact SC-3)

---

## Open Questions

- [ ] Should `Decimal` be a thin newtype over `rust_decimal::Decimal` or a direct re-export?
  (ADR-0002 says build in-house for correctness; a re-export is likely sufficient if we pin the
  version — decide before Milestone 1.)
- [ ] `CapabilityFlags` as `bitflags!` macro vs. a plain `u64` with named constants?
  `bitflags` is cleaner but is an external dep. If ADR-0002 restricts it, use `u64`.
- [ ] Where exactly does `ContextBundle` assembly logic live — `contracts` (struct definition
  only) vs. `runner` (assembly logic)? Plan 0009 will clarify; for now, struct in `contracts`.

---

## Change Log

| Date | Change | Author |
|------|--------|--------|
| 2026-06-06 | Initial draft | Agent |
