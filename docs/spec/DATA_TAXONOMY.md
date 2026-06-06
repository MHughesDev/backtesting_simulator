# Data Taxonomy — The Complete Venue-Neutral Data Model

This is the master reference for **every kind of data the suite can ingest**, organized into the
three planes that govern how the data behaves. It unifies the market-data contract, the exogenous
signal contract, and the operational/audit records into one map.

It exists because a serious backtest is not just OHLCV. A good meme-coin or NFT strategy may lean
far more on social attention, on-chain flows, and news than on price. The architecture must let a
caller bind **any amount** of that external data — from **any number of separate sources** —
without ever confusing it with the market data the engine fills against.

---

## 1. The governing principle: the suite is venue-neutral

**The core suite recognizes generic, normalized payload types — never venue-specific feed
names.** It does not know what "L2 depth from exchange X" or "social firehose from platform Y" is.
A caller-owned **adapter** translates every vendor/source feed into the suite's normalized
payloads before the suite sees it:

```
vendor/source feed  →  caller-owned adapter/parser  →  normalized payload  →  suite
```

| Bad (vendor dialect) | Good (normalized) |
|---|---|
| "exchange add-order message" | `OrderBookOrderEvent { action: Add, order_id, side, price, size, ts_event, seq }` |
| "exchange match channel" | `Trade` or `OrderBookOrderEvent` (by fidelity) |
| "search-trends raw response" | `SignalEvent { signal_id: "search_attention", value, ts_event, ts_available }` |
| "social firehose post" | `DocumentSignal` / `MediaReference` (PIT, see §4) |

**The suite understands market mechanics, not vendor dialects.** This is what keeps it universal:
adapters absorb the chaos of real feeds; the core stays small and typed.

---

## 2. The three planes

Every data type belongs to exactly one plane, and the plane decides the rules.

| Plane | What it is | Can it be a fill source? | Look-ahead clock | Owner |
|---|---|---|---|---|
| **Market-Data Plane** | Prices, books, pool state, the mechanics an engine simulates fills against | **Yes** | `ts_event` | Caller (bound per instrument) |
| **Exogenous-Signal Plane** | Everything *outside* the market that informs a decision — news, sentiment, fundamentals, macro, on-chain analytics, media | **Never** | `ts_available` | Caller (bound per source) |
| **Operational / Meta Plane** | Records the suite *emits* about its own run — lineage, warnings, decisions, audit | n/a (output) | n/a | Suite |

The single most important consequence: **exogenous data never sets a fill price.** It can change
what the strategy *decides*, never what the market *does*. A news event can make the strategy buy;
the price it buys at still comes only from the Market-Data Plane.

### 2.1 Two clocks: `ts_event` vs. `ts_available`

The Market-Data Plane is governed by `ts_event` (when the thing happened). The Exogenous-Signal
Plane needs a **second timestamp** because external information has a publication lag:

- `ts_event` — when the underlying real-world event occurred (the merger closed; the post was
  written; the filing was dated).
- `ts_available` — when a strategy **could first have known it** (the announcement hit the wire;
  the post was indexed; the filing was published).

**Look-ahead enforcement on the Exogenous-Signal Plane uses `ts_available`, not `ts_event`.** A
merger effective on date Y but announced on date X (X < Y) becomes actionable at X. A filing dated
to a quarter-end but published weeks later becomes visible only at publication. Without
`ts_available`, every external dataset is a look-ahead trap. The suite enforces
`ts_available ≤ current_ts` for all exogenous data; honesty of the timestamps themselves is the
caller's contractual responsibility (the suite cannot know a feed lied about when news broke).

---

## 3. The core event envelope (shared by both data planes)

```
EventEnvelope {
  instrument_id:  Option<InstrumentId>,  // the tradable affected (absent for pure macro/entity signals)
  entity_id:      Option<EntityId>,      // company / issuer / protocol / fund / collection
  venue_id:       Option<VenueId>,
  ts_event:       i64,                   // when it happened (ns UTC)
  ts_available:   Option<i64>,           // when a strategy could know it (ns UTC); defaults to ts_event
  ts_recv:        Option<i64>,           // when the system received it
  seq:            Option<u64>,           // ordering / gap detection
  source_id:      Option<String>,        // which source/processor produced it
  source_version: Option<String>,        // parser/model/processor version (reproducibility)
  payload:        Payload,
}
```

Market-Data Plane payloads typically set `instrument_id` and use `ts_event`. Exogenous-Signal
payloads typically set `entity_id` (and optionally `instrument_id`) and rely on `ts_available`.

---

## 4. The Market-Data Plane (categories 1–20)

These are the payloads an engine consumes to form prices and simulate fills. They are fully
specified in [contracts/market-data.md](contracts/market-data.md); this is the index.

| # | Category | Representative payloads | Engine(s) |
|---|---|---|---|
| 1 | Event spine | `EventEnvelope` | all |
| 2 | Universal price | `Mark`, `Bar`, `Quote`, `Trade`, `SettlementPrice`, `OfficialPrice` | all |
| 3 | Order book / depth | `BookSnapshot`, `BookDelta` (L2), `OrderBookOrderEvent` (L3/MBO), `AuctionImbalance`, `AuctionResult` | A, H |
| 4 | Calendar / session / status | `TradingSession`, `TradingStatus`, `MarketPhase`, `LimitState`, exchange calendar (reference) | A |
| 5 | Instrument reference / security-master | `InstrumentStatic`, `SymbolMapping`, `InstrumentLifecycle`, `ContractSpec`, `TickLotSpec`, `SettlementSpec` | all (reference) |
| 6 | Fees / costs / frictions | `FeeSchedule`, `FeeScheduleUpdate`, `BorrowRate`, `Funding`, `SwapRate`, `GasEvent`, `ExpenseRatio`, `MarketplaceFee`, `RoyaltyFee`, `RepoRate` | all |
| 7 | Order / fill / execution | `OrderRequest`, `OrderState`, `Fill`, `PartialFill`, `ExecutionReport`, `LatencyEvent`, `TradeRecord` | all (suite-emitted) |
| 8 | Account / portfolio / ledger | `AccountSnapshot`, `PositionSnapshot`, `MarginState`, `CollateralState` | injected `Account` |
| 9 | Risk state | `LiquidationEvent`, `InsuranceFundEvent`, `AutoDeleveragingEvent`, `VolatilityEstimate` | A, E + risk stage |
| 10 | Corporate / lifecycle | `CorporateAction` (Dividend/Split/Merger/SpinOff/RightsIssue/Delisting), `UniverseMembership` | A |
| 11 | Futures | `OpenInterest`, `SettlementPrice`, `RollSchedule`, `ContinuousSeriesMetadata`, `TermStructure`, `Basis` | A |
| 12 | Perpetuals | `Funding`, `MarkUpdate`, `IndexPrice`, `LiquidationEvent`, `LeverageTier` | A |
| 13 | Options / derivatives | `IVSurface`, `Greeks`, `OptionChainSnapshot`, `ExerciseEvent`, `AssignmentEvent`, `RiskFreeCurve` | E |
| 14 | Fixed income | `Coupon`, `CleanPrice`/`DirtyPrice`, `YieldUpdate`, `YieldCurve`, `CreditSpread`, `CreditRatingEvent`, `PrepaymentCurve`, `RepoRate` | D |
| 15 | ETF / fund / NAV | `Nav`, `INav`, `HoldingsSnapshot`, `CreationRedemptionBasket`, `PremiumDiscount`, `DailyResetState`, `LeverageFactor` | C |
| 16 | FX | `SwapRate`, `ForwardPoint`, `InterestRateDifferential`, `SessionLiquidity`, `CrossCurrencyRate` | A |
| 17 | Crypto CEX | `TokenEvent` (Fork/Airdrop/Burn/Migration), `ListingEvent`, `DelistingEvent`, `StablecoinDepegEvent` | A |
| 18 | DEX / AMM / on-chain | `PoolState`, `SwapEvent`, `LiquidityEvent`, `TickLiquidity`, `GasEvent`, `MevEvent`, `RouterPath` | B |
| 19 | NFT / marketplace | `NftEvent`, `NftBidEvent`, `FloorUpdate`, `CollectionStats`, `TokenMetadata`, `RarityScore`, `RoyaltyFee` | G |
| 20 | Prediction market | `OutcomeQuote`, `OutcomeBookSnapshot`, `MarketLifecycleEvent`, `Resolution`, `OracleEvent`, `ProbabilityMark` | H |

---

## 5. The Exogenous-Signal Plane (categories 21–32)

These are **outside information** converted into point-in-time, timestamped inputs. They feed the
strategy's `features` and `models` stages; **they are never a fill source.** Fully specified in
[contracts/signals.md](contracts/signals.md); this is the index.

Everything here normalizes onto a small set of generic payloads (`SignalEvent`, `ExternalEvent`,
`DocumentSignal`, `MediaReference`, `EntityMetric`, `ScheduledEvent`) — the suite never adds a
distinct payload type per data vendor or per category. The categories below are *what kinds of
real-world data map onto those generic payloads*, not 12 separate schemas.

| # | Category | Maps onto | Typical use |
|---|---|---|---|
| 21 | Generic external signal | `SignalEvent`, `ExternalEvent`, `EntityMetric` | any pre-computed numeric/categorical feature |
| 22 | News / document-derived | `DocumentSignal`, `HeadlineEvent`, `FilingSignal`, `TranscriptSignal` | parsed news/filing/transcript features |
| 23 | Sentiment / attention | `SentimentSignal`, `SocialMentionMetric`, `AttentionScore`, `SearchTrendMetric`, `InfluencerMentionEvent` | meme stocks, crypto, NFTs, consumer names |
| 24 | Fundamentals / operating | `FundamentalMetric`, `EarningsEvent`, `GuidanceEvent`, `AnalystEstimate`, `BusinessKpi` | equity fundamentals |
| 25 | Filings / ownership / insider | `RegulatoryFilingEvent`, `InsiderTransactionEvent`, `InstitutionalHolding`, `ShortInterest`, `ShareBuybackEvent` | equity event-driven |
| 26 | Macro / economic | `EconomicRelease`, `MacroMetric`, `CentralBankEvent`, `RateDecisionEvent`, `InflationMetric` | bonds, FX, futures, macro |
| 27 | Scheduled calendar | `ScheduledEvent`, `EarningsCalendarEvent`, `EconomicCalendarEvent`, `TokenUnlockEvent` | the *date itself* as signal (pre-outcome) |
| 28 | Legal / regulatory / policy | `LegalEvent`, `CourtRulingEvent`, `ApprovalDecisionEvent`, `SanctionEvent`, `TariffEvent`, `BankruptcyEvent` | event-driven |
| 29 | Weather / geospatial / physical | `WeatherObservation`, `WeatherForecast`, `DisasterEvent`, `SatelliteObservation`, `FootTrafficMetric`, `ShippingMetric` | commodities, energy, retail, insurance |
| 30 | Supply chain / web / app alt-data | `WebTrafficMetric`, `AppRankMetric`, `ProductReviewMetric`, `JobPostingMetric`, `InventoryAvailabilityMetric` | company-health proxies |
| 31 | Crypto external / on-chain analytics | `OnChainMetric`, `ExchangeFlowMetric`, `TvlMetric`, `ActiveAddressMetric`, `WhaleTransactionEvent`, `ExploitEvent` | crypto strategies (not order-book data) |
| 32 | Model / feature / inference | `FeatureValue`, `FeatureFrame`, `ModelInference`, `ModelConfidence`, `RegimeLabel`, `EmbeddingVector` | strategy/model intermediate values |

### 5.1 Raw media and documents — references, not blobs

The suite still does **not parse or own raw media.** But the architecture must let a model run
inference on the actual post/image/video/news that existed at time *t* (critical for meme coins,
NFTs, and event-driven names). The reconciliation:

- The Exogenous-Signal Plane carries a **`MediaReference`** (or `DocumentSignal` with a `uri`): a
  point-in-time *pointer* to the raw asset — its URI, modality (text/image/video/audio), and
  `ts_available` — optionally alongside pre-extracted features.
- The **injected `Model` port (caller code) resolves the reference and loads the raw bytes** for
  inference. The suite guarantees only point-in-time correctness and routing; the caller's model
  does the multimodal heavy lifting.

This preserves "the suite owns no data and no models" while enabling true multimodal,
point-in-time inference. See [contracts/signals.md](contracts/signals.md) §4–§5.

---

## 6. The Operational / Meta Plane (category 33)

Records the suite **emits** about its own execution — never ingested, always output. They make a
run debuggable, reproducible, and auditable.

`RunConfig`, `RunWarning`, `ValidationError`, `ManifestViolation`, `DataSufficiencyError`,
`DataQualityWarning`, `SequenceGap`, `SourceLineage`, `ReplayCheckpoint`, `DecisionLog`,
`RiskDecisionLog`, `ModelLineageRecord`, `ComponentLineageRecord`.

These are governed by [run-request.md](run-request.md) §7 (output) and the validation sequence.

---

## 7. Classification axes (how to reason about any data type)

Any payload can be placed on four axes; this is the mental model for deciding how the suite treats
it:

1. **Plane** — Market-Data / Exogenous-Signal / Operational (§2).
2. **Internal vs. external** — internal = produced by the market itself (a trade, a pool swap);
   external = produced outside it (a tweet, a CPI print).
3. **Direct vs. indirect** — direct = acts on price/fills (a quote, a funding rate); indirect =
   informs a decision but never a fill (sentiment, a satellite image).
4. **Provided vs. derived** — provided = bound by the caller; derived = computed by the suite from
   provided data (a `DerivedBar` from ticks, greeks from an IV surface, duration from cash flows).
   Caller-provided always wins; derived is the fallback and is always flagged
   ([run-request.md](run-request.md) §4).

A clean way to read the whole system: **Market-Data Plane = internal + direct. Exogenous-Signal
Plane = external + indirect.** The few exceptions (on-chain analytics are external but describe an
internal market; scheduled-event dates are external and indirect) are exactly the cases the
two-clock model (`ts_event` / `ts_available`) exists to handle.

---

## 8. What the suite derives vs. requires (per plane)

- **Market-Data Plane:** the suite derives coarser bars from finer data, adjusted series from
  unadjusted + corporate actions, greeks from an IV surface, NAV from holdings, duration/convexity
  from cash flows — all flagged `derived`. It requires a per-engine minimum (see
  [run-request.md](run-request.md) §10) and fails loud (`DataSufficiencyError`) otherwise.
- **Exogenous-Signal Plane:** the suite derives **nothing** here by default — exogenous data is
  pre-computed upstream by the caller. It only aligns, windows, and routes it point-in-time (by
  `ts_available`). A strategy that references a signal not bound for any source fails validation.
- **Operational Plane:** entirely suite-produced.

---

## 9. Pointers

| Plane | Contract |
|---|---|
| Market-Data | [contracts/market-data.md](contracts/market-data.md), [contracts/instrument.md](contracts/instrument.md) |
| Exogenous-Signal | [contracts/signals.md](contracts/signals.md) |
| Binding (both) | [run-request.md](run-request.md) §4 (market, instrument-keyed), §4b (reference), §4c (signals), §4d (cohorts) |
| Strategy/model use | [contracts/strategy.md](contracts/strategy.md) §4 (cross-instrument refs), §6 (watch-vs-trade, scanner), §7–§8 |
| Multi-strategy composition | [contracts/plan.md](contracts/plan.md) |
| Operational/output | [run-request.md](run-request.md) §7 |

### How binding differs by strategy topology

- **Single / multi-asset** — bind each instrument's market data in `data.bindings` (§4) and any
  signals in `signals` (§4c). The traded `universe` is a subset of `instruments`; the rest are
  **watch-only** references read via cross-instrument grammar.
- **Universe-wide scan (cohort)** — members cannot be enumerated in advance, so bind a **cohort
  data source** (§4d) that materializes instruments point-in-time as they appear; a `scanner`
  universe screens them and a **Plan** wires the screen to entry/exit strategies.
