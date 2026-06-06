# Open Questions

A living backlog of unresolved design questions for the backtesting suite. This is the
exploratory layer *above* the decisions: when a question here is answered, it graduates into an
**ADR** (the durable decision) and/or a **spec** edit, and is marked resolved.

- **ADRs** = decisions already made (`docs/adr/`).
- **Open decisions (OD-1..OD-12)** = the blocking subset tracked in `MASTER_SPEC.md §10`.
- **This file** = the full, broad set — blocking and non-blocking, concrete and exploratory.

Each question has a stable ID (`Q-<AREA>-<n>`). Questions cross-referenced to an OD or ADR are
noted. Nothing here is decided; these are prompts for discussion.

> **Most blocking right now:** Q-PROD-1 (MVP scope), Q-RUNREQ-1 (run request schema),
> Q-REG-1 (component registry trust model), Q-ACCT-1 (portfolio/accounting ownership),
> Q-REPO-1 (shared contracts topology). See the highlights at the end.

---

## A. Scope & separation of concerns

- **Q-SCOPE-1** Does the suite hold portfolio/account/cash state during a run, or is even that an injected, caller-owned ledger?
- **Q-SCOPE-2** Should the suite expose a streaming/online mode (feed one event at a time) so backtest and live share a single code path, in addition to batch historical replay?
- **Q-SCOPE-3** Is forward/paper testing (running on a live feed, no historical data) in scope for the suite, or strictly platform-only?
- **Q-SCOPE-4** Does the suite ever emit orders to an external sink, or is it strictly simulate-and-return?
- **Q-SCOPE-5** Should there be a "replay live divergence" debug mode (feed recorded live data to explain backtest-vs-live differences)?
- **Q-SCOPE-6** Is portfolio analytics (beyond a single strategy's metrics) in scope, or platform-only?

## B. Data contracts & ingestion

- **Q-DATA-1** What is the canonical on-the-wire input format — Arrow IPC, Parquet, JSON, or an injected `DataReader` trait (caller decides storage)?
- **Q-DATA-2** How are data **gaps / missing bars** represented and handled — forward-fill, explicit gap marker, error, or strategy-configurable?
- **Q-DATA-3** Are events assumed pre-sorted by `ts_event`, or does the suite buffer-and-sort? What's the out-of-order policy?
- **Q-DATA-4** How are coarse sources (daily bars) and fine sources (ticks) normalized onto one nanosecond clock in a mixed run?
- **Q-DATA-5** How is data **revision / restatement** handled (point-in-time fundamentals, corrected prints)?
- **Q-DATA-6** Can a single run's dataset exceed memory — is streaming / memory-mapping from disk supported, or in-memory only?
- **Q-DATA-7** Is there a standalone data-validation/lint pass the caller can run *before* a backtest?
- **Q-DATA-8** How are multiple venues for the same asset reconciled (consolidated tape vs. per-venue instruments)?
- **Q-DATA-9** Who owns **entity mapping** (this news/signal/instrument refers to AAPL) — always the caller?
- **Q-DATA-10** What is the contract for "as-of" vs. "latest" reads, and can the suite detect an as-of violation?

## C. Instruments & capabilities

- **Q-INST-1** Can an instrument's capabilities or static fields change **mid-run** (token migration, tick-size change, fee-tier change)?
- **Q-INST-2** How are identity changes handled — ticker/CUSIP changes, a merger creating a new instrument, a token contract migration?
- **Q-INST-3** What is the final canonical `InstrumentId` grammar, and how are collisions avoided across venues/chains?
- **Q-INST-4** Are synthetic/composite instruments (spreads, custom baskets, pairs) first-class instruments or strategy-level constructs?
- **Q-INST-5** Is the capability-flag set a fixed enum, or extensible by the caller? (Affects validation and the registry.)
- **Q-INST-6** How are invalid capability combinations validated (e.g. an instrument claiming both `CLOB` and `AMM`)?

## D. Engines

- **Q-ENG-1** When two engines attach to one instrument (ETF = A + C), what is the evaluation order and how is shared state serialized?
- **Q-ENG-2** Order Book (A): what matching fidelity — top-of-book, L2, or full L3 queue position — and is it configurable per run?
- **Q-ENG-3** A: how is queue priority approximated when only L1/L2 data is available?
- **Q-ENG-4** AMM (B): how much v3 tick-crossing is simulated vs. approximated, and where is the fidelity dial exposed?
- **Q-ENG-5** B: is MEV/sandwich a configurable slippage tax, a data-driven model, or out of scope?
- **Q-ENG-6** Derivatives (E): what is the baseline pricing model (Black-Scholes?), and how do callers swap in alternatives (Heston, local vol)?
- **Q-ENG-7** E: how is American early-exercise optimality decided cheaply per event without solving a full PDE each time?
- **Q-ENG-8** Cash Flow (D): which day-count and curve-interpolation conventions are built in vs. configurable?
- **Q-ENG-9** Marketplace (G): is an NFT "sale" simulated only at observed historical sale prices, or via a demand/acceptance model?
- **Q-ENG-10** Event (H): how is resolution-timing uncertainty represented and simulated?
- **Q-ENG-11** Across engines: how are partial fills, cancels, and order amendments uniformly represented?
- **Q-ENG-12** Across engines: is there one unified fee model (maker/taker, gas, royalties, commission, financing) or per-engine fee logic?
- **Q-ENG-13** How is slippage calibrated when book depth is absent (size-vs-volume heuristics), and is the curve caller-configurable?
- **Q-ENG-14** Is short selling / borrow modeled uniformly or per-asset, and where does borrow availability come from?
- **Q-ENG-15** Is margin/leverage/liquidation logic shared between A (perps) and E (options), or duplicated?
- **Q-ENG-16** How do engines model **latency** (order submission → fill delay), and is it configurable for realism?
- **Q-ENG-17** Intrabar fills: with only OHLC bars, are stop/limit triggers filled optimistically, pessimistically, or via a configurable assumption?

## E. Strategy model

- **Q-STRAT-1** Where exactly is the expression-vs-component boundary — how much logic may a `when` expression hold before it must become a registered component? *(OD-9)*
- **Q-STRAT-2** What is the full grammar of the expression language (operators, functions, types, null handling)?
- **Q-STRAT-3** How are stateful indicators warmed up — is pre-start look-back data supplied, and how much?
- **Q-STRAT-4** ~~How does a strategy express **pairs / spread / relative-value** logic across two instruments declaratively?~~ **RESOLVED:** cross-instrument references `data:<instrument>.field` / `signal:<instrument>.<id>` / `feature:<id>@<instrument>` ([strategy.md](spec/contracts/strategy.md) §4).
- **Q-STRAT-5** How are **multi-leg orders** (option spreads, pairs trades) expressed and executed atomically?
- **Q-STRAT-6** How does a strategy **react to fills, partial fills, and rejections** declaratively (no imperative callback)?
- **Q-STRAT-7** ~~Can strategies be composed/nested (a strategy of strategies)?~~ **RESOLVED:** not nested — composed in a **Plan** by data-flow (screen→entry→exit), each strategy stays flat ([plan.md](spec/contracts/plan.md), OD-6).
- **Q-STRAT-8** ~~How are strategy-level vs. portfolio-level constraints separated when multiple strategies share capital?~~ **RESOLVED:** Plan `account_mode` (shared/isolated) + `conflict_policy`; portfolio-level overlay stays the caller's analytics layer ([plan.md](spec/contracts/plan.md)).
- **Q-STRAT-9** How is rebalancing cadence expressed (every event vs. scheduled vs. signal-triggered)?
- **Q-STRAT-10** How does a strategy declare the capabilities it requires so instruments can be pre-validated against it?
- **Q-STRAT-11** ~~Can a strategy define a derived/intermediate universe (e.g. "top-decile by a feature")?~~ **RESOLVED:** `universe.type: "scanner"` screens a cohort by point-in-time market-data + signal filters ([strategy.md](spec/contracts/strategy.md) §6.3).
- **Q-STRAT-12** How are conflicting insights on the same instrument resolved (two rules firing opposite directions)?

## F. AI models & training

- **Q-MODEL-1** What output types may a model emit — scalar, vector, probability distribution, label, embedding — and how are they typed for downstream binding?
- **Q-MODEL-2** Can models **chain** (one model's output is another's input), and how is ordering/cycles validated?
- **Q-MODEL-3** Is model **inference latency** simulated in backtest (does scoring "take time" relative to the clock)?
- **Q-MODEL-4** How are **ensembles** expressed declaratively?
- **Q-MODEL-5** How is a missing/misaligned feature handled at inference (error vs. impute vs. fallback)?
- **Q-MODEL-6** How does the platform guarantee `model_id@version` resolves to identical weights months later? *(OD-10)*
- **Q-TRAIN-1** Validation gating: may a freshly trained artifact be rejected (worse than incumbent) and the previous `current` retained? Where is the policy declared? *(OD-12)*
- **Q-TRAIN-2** Is the refit cache per-run only, or persisted across runs (which implies the suite touching storage — tension with ADR-0005)?
- **Q-TRAIN-3** Is online/incremental update (vs. full refit) ever represented, or always a full `train` call?
- **Q-TRAIN-4** How is training **cost** surfaced and budgeted in the run queue and benchmarks?

## G. Signals / alternative data

- **Q-SIG-1** Final `signals.md` schema: what does the `Signal` payload carry, and what capability flag gates it?
- **Q-SIG-2** How is point-in-time integrity of a signal **attested** (caller responsibility) and can the suite detect obvious violations?
- **Q-SIG-3** ~~Should raw documents (text) ever enter the suite, or only pre-computed numeric/categorical features?~~ **RESOLVED:** the core never parses raw media/text; it carries point-in-time **references** (`MediaReference`/`DocumentSignal` with a `uri`) that the injected `Model` port resolves and loads for multimodal inference ([signals.md](spec/contracts/signals.md) §4–§5).
- **Q-SIG-4** How are frequency mismatches handled (a daily sentiment signal vs. minute bars)?
- **Q-SIG-5** How are signals keyed to instruments (entity mapping keys supplied by the caller)?

## H. Portfolio, accounting, currency

- **Q-ACCT-1** ~~Does the suite own the portfolio/cash ledger during a run, or is it injected?~~ **RESOLVED:** no — per-trade model + injected `Account` port ([ADR-0010](adr/0010-suite-does-not-own-portfolio.md)).
- **Q-ACCT-2** Single base currency per run, or multi-currency with FX conversion — and where does the FX rate stream come from?
- **Q-ACCT-3** How is **settlement timing** modeled (T+2 equities, T+0 crypto, coupon/dividend pay dates)?
- **Q-ACCT-4** How is buying power / margin computed across **mixed asset classes** in one portfolio?
- **Q-ACCT-5** How are dividends, coupons, and funding credited — accrual vs. cash, and on which date?
- **Q-ACCT-6** How is mark-to-market computed for **sparse/illiquid** holdings (NFTs, illiquid bonds)?
- **Q-ACCT-7** Is tax modeling in scope at all?
- **Q-ACCT-8** How is financing/borrow cost modeled at the portfolio level vs. per position?

## I. Risk

- **Q-RISK-1** Is risk purely strategy-declared, or is there a separate portfolio-level risk overlay? *(OD-6)*
- **Q-RISK-2** How are stops simulated without tick data — optimistic, pessimistic, or configurable intrabar assumption? *(see Q-ENG-17)*
- **Q-RISK-3** What is the precedence ordering among constraints, stops, and the kill-switch?
- **Q-RISK-4** Are VaR / scenario / stress tests in scope, or platform-level?
- **Q-RISK-5** How are cross-instrument exposure/correlation limits expressed?
- **Q-RISK-6** Does a kill-switch halt only the strategy, or can it halt the whole run/portfolio?

## J. Metrics & results

- **Q-METRIC-1** Final `metrics.md`: the exact universal metric set and each per-capability extension.
- **Q-METRIC-2** What is the result schema — equity curve, trade log, position history, model lineage, metrics — and its format?
- **Q-METRIC-3** How is a **benchmark** supplied for relative metrics (alpha/beta), and is it required?
- **Q-METRIC-4** How are returns annualized consistently for 24/7 vs. session markets?
- **Q-METRIC-5** How many return definitions are reported (gross / net-of-fees / after-funding / after-borrow)?
- **Q-METRIC-6** How is uncertainty/confidence disclosed for low-fidelity assets (NFTs, illiquid)?
- **Q-METRIC-7** Are results streamed incrementally for long runs, or only returned at completion?
- **Q-METRIC-8** What level of trade-level attribution is provided (per-signal, per-leg, per-model)?

## K. Run queue, performance, parallelism

- **Q-QUEUE-1** How much orchestration lives in the suite vs. the platform? *(OD-2)*
- **Q-QUEUE-2** Is the **parameter-sweep search** (grid/random/Bayesian) in the suite or the platform?
- **Q-QUEUE-3** How is the immutable dataset shared across parallel runs without copying (zero-copy/Arc)?
- **Q-QUEUE-4** Is determinism guaranteed **bit-identical regardless of thread count**?
- **Q-QUEUE-5** Are run cancellation, timeouts, and per-run memory caps supported?
- **Q-QUEUE-6** Is progress reporting / partial results for long runs supported?
- **Q-QUEUE-7** Is distributed multi-machine execution in scope, or single-node only?

## L. Run request / invocation

- **Q-RUNREQ-1** ~~Full **Run Request** schema~~ **RESOLVED:** defined in [run-request.md](spec/run-request.md) (OD-8).
- **Q-RUNREQ-2** How are injected components (the `Trainer`, `Model`s, custom registry components) passed in?
- **Q-RUNREQ-3** How are data sources bound — file paths, reader handles, in-memory Arrow tables?
- **Q-RUNREQ-4** How are parameter **values** vs. a parameter **sweep** distinguished in one request?

## M. Reproducibility & determinism

- **Q-REPRO-1** Is floating-point determinism guaranteed across architectures (x86 vs ARM), or do we use fixed-point/decimal money types everywhere?
- **Q-REPRO-2** What is the deterministic tie-break ordering for events sharing a `ts_event` across instruments?
- **Q-REPRO-3** How are RNG streams scoped (per-run, per-strategy, per-model) to stay reproducible under parallelism?
- **Q-REPRO-4** Is there a golden-run regression harness (same inputs → byte-identical results) in CI?

## N. Time, calendars, timezones

- **Q-TIME-1** How are exchange calendars supplied (caller data) and represented (holidays, half-days)?
- **Q-TIME-2** How is "daily" defined for 24/7 markets (UTC midnight, exchange-local, configurable)?
- **Q-TIME-3** DST, leap seconds, and timezone normalization policy?
- **Q-TIME-4** How are clocks aligned across venues in a multi-asset, multi-venue run?

## O. Lifecycle, corporate & token events

- **Q-LIFE-1** What is the ordering rule when a corporate/token event and a price event share a timestamp?
- **Q-LIFE-2** How are mergers/spin-offs that create or destroy instruments handled mid-run (position migration)?
- **Q-LIFE-3** What price source is used for forced closes on delisting?
- **Q-LIFE-4** Are airdrops/forks credited automatically by the engine, or surfaced for strategy decision?

## P. Backtest / live parity

- **Q-PARITY-1** How is parity **verified** — a conformance harness running the same strategy in backtest and against recorded live data?
- **Q-PARITY-2** What is *allowed* to differ (latency, fills, partials) and how is the tolerance bounded?
- **Q-PARITY-3** Is the order/execution semantics spec precise enough for an independent live engine to match it exactly?

## Q. Component registry & extensibility

- **Q-REG-1** ~~Trust model: Rust / PyO3 / WASM?~~ **RESOLVED:** tiered — built-in Rust / trusted native / **WASM sandbox** for untrusted/AI; Python excluded from the hot loop ([ADR-0011](adr/0011-component-registry-trust-model.md), [component-registry.md](spec/component-registry.md)).
- **Q-REG-2** How are components versioned and pinned for reproducibility?
- **Q-REG-3** How are components sandboxed to preserve determinism (no I/O, no wall-clock, no unseeded RNG)?
- **Q-REG-4** Are built-in indicators specified by **canonical formula** (to avoid library drift), and where?
- **Q-REG-5** Is there a permission/capability model for what a component may access?

## R. Validation & error handling

- **Q-ERR-1** What is the error taxonomy (manifest violation, capability error, schema error, runtime error)?
- **Q-ERR-2** Is validation fail-fast or collect-all-errors before rejecting?
- **Q-ERR-3** How are non-fatal warnings (degraded fidelity, missing optional data) surfaced in results?
- **Q-ERR-4** On a mid-run failure, is the whole run aborted or can a single instrument/strategy be isolated?

## S. Versioning & schema evolution

- **Q-VER-1** What is the `schema_version` migration policy for strategy JSON as the format grows?
- **Q-VER-2** How are the data/instrument contracts versioned, and what's the backward-compatibility guarantee?
- **Q-VER-3** What is the deprecation policy for contract fields and capability flags?

## T. Testing & conformance

- **Q-TEST-1** Are reference datasets synthetic-only, and how are engine fills validated against known-good references (e.g. real Uniswap swaps, option prices)?
- **Q-TEST-2** Which invariants get property-based tests (no look-ahead, cash conservation, deterministic replay)?
- **Q-TEST-3** Is there a public **conformance suite** a data provider can run to certify their feed satisfies the contracts?

## U. Packaging & distribution

- **Q-PKG-1** Python packaging strategy (maturin wheels, manylinux) and how the Python version tracks the Rust core version?
- **Q-PKG-2** Is a stable C ABI exposed for non-Python hosts?
- **Q-PKG-3** Is a WASM build (browser/edge backtests) a goal?
- **Q-PKG-4** What is the semantic-versioning policy for the library and its contracts?

## V. Repo topology

- **Q-REPO-1** ~~Standalone `*-contracts` package or depend on this repo's `crates/contracts`?~~ **RESOLVED:** standalone, dependency-free shared kernel; built here now, extracted when a 2nd consumer exists; outside systems depend on `contracts`, never the engine ([ADR-0012](adr/0012-standalone-contracts-kernel.md)).
- **Q-REPO-2** Mono-repo vs. multi-repo for the suite / training / platform contracts?

## W. Security & safety

- **Q-SEC-1** How is untrusted strategy JSON guarded against resource exhaustion (huge universes, pathological expressions)?
- **Q-SEC-2** How are untrusted custom components contained (if user-supplied)?
- **Q-SEC-3** How are malformed/adversarial data inputs prevented from panicking the engine?

## X. Observability & debugging

- **Q-OBS-1** Can a run emit an event-by-event trace for debugging, and at what cost?
- **Q-OBS-2** Is there a "why did this trade happen" explanation (which rule/insight/model output triggered it)?
- **Q-OBS-3** What is the structured, deterministic logging contract?
- **Q-OBS-4** Can a run be replayed to a specific event for inspection (time-travel)?

## Y. Statistical methodology

- **Q-STAT-1** Does the suite provide overfitting safeguards (walk-forward splits, out-of-sample holdout, deflated Sharpe)?
- **Q-STAT-2** Is multiple-testing correction offered for large parameter sweeps?
- **Q-STAT-3** Are Monte Carlo / bootstrap resampling of results built in?
- **Q-STAT-4** Is transaction-cost sensitivity analysis a first-class feature?
- **Q-STAT-5** Beyond mechanical look-ahead enforcement, does the suite detect subtler leakage (e.g. model temporal contamination)?

## Z. Product & roadmap

- **Q-PROD-1** ~~MVP engine scope~~ **RESOLVED:** no MVP — specify & build the **end-state** system ([ADR-0009](adr/0009-end-state-system-no-mvp.md)).
- **Q-PROD-2** Which asset class is the first **end-to-end vertical slice** (data → engine → strategy → metrics)?
- **Q-PROD-3** What is the first real strategy used to validate the system against a known result?
- **Q-PROD-4** Who are the first users (you + the platform), and what do they need first?

## AA. Market-data depth, derivation & sufficiency — RESOLVED 2026-06

This batch was resolved in a design pass and written into the contract/engine specs. 16 new
payload types and 11 capability flags were added (see `contracts/market-data.md`,
`contracts/instrument.md`); the decisions below govern how they are used.

- **AA-1 Reference data vs. event streams.** ✅ Bindings carry `binding_type: event_stream | reference`. Event streams replay through the clock; reference data is loaded once and queried by timestamp. → `run-request.md` §4b.
- **AA-2 `HasFeeScheduleUpdates` flag.** ✅ Added for consistency; gates `FeeScheduleUpdate`. → `instrument.md`.
- **AA-3 ADL / insurance-fund behavior.** ✅ `AutoDeleveragingEvent` can force-close a proportional share of the strategy's own qualifying position at the bankrupt trader's bankruptcy price; `InsuranceFundEvent` is context. → `engine-a-order-book.md` §6, `market-data.md` §2.22.
- **AA-4 Dynamic session/status overrides static calendar.** ✅ Engine keeps a current state starting from the static calendar, overridden by `TradingSession`/`TradingStatus` events from their `ts_event`. Most-realistic; static is fallback. → `engine-a-order-book.md` §6.
- **AA-5 Caller-provided data always wins.** ✅ Directly bound data is authoritative; derivation is fallback-only; everything derived is flagged. → `run-request.md` §4, invariant 6.
- **AA-6 Bar boundary alignment.** ✅ Wall-clock aligned, start at `:00`. → `run-request.md` §4a.
- **AA-7 Standard bar construction.** ✅ From trade prints (O=first, H=max, L=min, C=last, V=Σsize); fallback = quote mid. → `run-request.md` §4a, `market-data.md` §2.26.
- **AA-8 Daily-bar boundary.** ✅ UTC-midnight to UTC-midnight. → `run-request.md` §4a. (Closes part of Q-TIME-2 for derived bars.)
- **AA-9 Warmup gating.** ✅ No strategy decisions until minimum lookback bars accumulate *during* the run; derivation runs through warmup. → `run-request.md` §4a.
- **AA-10 Necessity-driven derivation.** ✅ Derive only the `(payload_class, interval)` set the compiled strategy + fill model require (static analysis of the plan); bounds memory. → `run-request.md` §4a.
- **AA-11 Derived-data provenance.** ✅ Anything derived is flagged `derived: true` + `source_class`; emitted via `output.emit: ["derived_data"]` so the caller can persist it. → `run-request.md` §4a/§7.
- **AA-12 Adjusted-series derivation.** ✅ Engine may derive the adjusted series from unadjusted bars + `CorporateAction` under a declared `adjustment_method` (default back-adjust); flagged derived; else signals needing it fail sufficiency. → `run-request.md` §4a.
- **AA-13 Point-in-time for reference lookups.** ✅ Reference entries carry `effective_ts` + `knowable_ts`; lookups enforce `knowable_ts ≤ current_ts`. → `market-data.md` §4.5, `run-request.md` §4b/invariant 4.
- **AA-14 Entity-keyed reference data.** ✅ `reference_bindings` block keyed by `issuer:/universe:/venue:`; instruments declare `issuer_id`. Binds issuer-level data (CreditSpread, CreditRatingEvent) once for many bonds. → `run-request.md` §4b, `instrument.md`.
- **AA-15 `FeeScheduleUpdate` precedence.** ✅ Dynamic overrides static `fee_schedule`; static is fallback. → `market-data.md` §2.21, `engine-a-order-book.md` §6.
- **AA-16 Halt behavior.** ✅ On `Halted`, resting orders are frozen (not cancelled), resume at reopen (reopening auction if `HasAuction`); DAY orders still expire at session close. → `engine-a-order-book.md` §6.
- **AA-17 NFT sell — conservative default.** ✅ Sell fills only on an observed comparable sale; `NftBidEvent` feeds the optional demand model only, never an immediate fill. → `engine-g-marketplace.md` §4, `market-data.md` §2.23.
- **AA-18 SwapEvent pool-state reconstruction.** ✅ Engine B replays real `SwapEvent`s between `PoolState` snapshots (higher fidelity) instead of holding the last snapshot constant. → `engine-b-amm.md` §8a, `market-data.md` §2.19.
- **AA-19 Oracle event ordering.** ✅ Locked → Proposal → (Dispute, no re-open) → FinalSettlement → Resolution; Resolution may arrive without an `OracleEvent` (centralized oracles). → `engine-h-event-resolution.md` §2, `market-data.md` §2.24.
- **AA-20 External liquidation disjoint.** ✅ External `LiquidationEvent` = other participants only (cascade/market-impact); never closes the strategy's own position (always engine-computed from Account + mark). → `engine-a-order-book.md` §6, `market-data.md` §2.22.
- **AA-21 ADL reaches strategy position.** ✅ (See AA-3 — the realism call: ADL does touch the strategy's own qualifying position.)
- **AA-22 UniverseMembership gates the dynamic selector.** ✅ A dynamic universe selector may only pick instruments in-universe (per PIT membership) at that timestamp. → `market-data.md` §2.25.

**Still open from this pass (carried forward):**

- **AA-OPEN-1** `DerivedBar` vs. caller-`Bar` collision at the same `ts_event` is resolved by AA-5 (caller wins), but the exact feature-pipeline *ordering* when both are emitted needs the feature-pipeline spec to state it explicitly.
- **AA-OPEN-2** `TradingSession` (event) vs. exchange-calendar (reference) — override semantics are set (dynamic wins), but the precise merge when a reference calendar *and* a session event disagree on the same interval is not yet edge-case-specified.
- **AA-OPEN-3** Memory ceiling for derived intervals under `intervals: "all_standard"` over a large universe is bounded by necessity-derivation (AA-10) but has no hard cap policy.

## BB. Multi-asset, scanning & strategy composition — RESOLVED 2026-06

Resolved in a design pass and written into the specs.

- **BB-1 Cross-instrument references.** ✅ A strategy evaluated for one instrument can read another's data/signals/features via `data:<instrument>.field`, `signal:<instrument>.<id>`, `feature:<id>@<instrument>`. → `strategy.md` §4. (Trade ETH while forecasting BTC.)
- **BB-2 Watch-vs-trade.** ✅ `instruments` (Run Request) = all data incl. watch-only references; `universe` (Strategy) = the traded subset; the difference is watch-only reference instruments. → `strategy.md` §6.0.
- **BB-3 Scanner universe (engine-agnostic, not "new"-specific).** ✅ `universe.type: "scanner"` screens a **cohort** by point-in-time market-data + signal filters; works for DEX pairs/NFTs/IPOs/prediction markets, any universe. → `strategy.md` §6.3.
- **BB-4 Cohort data source.** ✅ A market-wide source (`cohorts`) **materializes instruments point-in-time** as they appear, via an `instrument_template` + per-asset feed overrides; prevents survivorship bias by construction. → `run-request.md` §4d.
- **BB-5 The Plan (multi-strategy composition).** ✅ A third layer above Strategy holds multiple flat strategies wired by **data-flow** (screen→entry→exit), or concurrent independents. **Not nested.** → `plan.md`. Resolves **OD-6**, **Q-STRAT-7/8**.
- **BB-6 Strategy kickoff = publish/subscribe.** ✅ A `selector` strategy publishes a candidate set; `entry` strategies subscribe via `universe.from`. No imperative calls, no nested ifs. → `plan.md` §4.
- **BB-7 Multiple concurrent strategies.** ✅ A Plan with unwired `standalone` strategies runs them side by side — applies to plain single-asset styles too. → `plan.md` §2–§3.
- **BB-8 Account mode.** ✅ Plan `account_mode`: `shared` (default; net into one injected Account) or `isolated` (per-strategy partition). → `plan.md` §5.
- **BB-9 Conflict policy.** ✅ Plan `conflict_policy`: `net` (default), `priority`, or `reject` for opposing intents on one asset. → `plan.md` §6. (Resolves Q-STRAT-12 at the Plan level.)
- **BB-10 Cohort instrument materialization.** ✅ Per-cohort `instrument_template` of common fields + per-asset overrides from the feed; `price_formation` routes each member to its engine. → `run-request.md` §4d.

**Still open from this pass (carried forward):**

- **Q-PLAN-1** Cross-strategy capital allocation under `shared` (fixed/dynamic/caller-driven split of buying power).
- **Q-PLAN-2** Whether an `exit` strategy can target positions opened by a *specific* entry strategy (position tagging) vs. the netted book.
- **Q-PLAN-3** Screen→entry hand-off latency (configurable delay for realism).

---

## Highlights — the questions actually blocking progress

| Question | Why it blocks | Tracked as | Status |
|---|---|---|---|
| **Q-PROD-1** MVP engine scope | Determines build target | OD-1 | ✅ Resolved — end-state, no MVP (ADR-0009) |
| **Q-RUNREQ-1** Run Request schema | "Pass a strategy in at runtime" was undefined | OD-8 | ✅ Resolved — `run-request.md` |
| **Q-ACCT-1** Portfolio/ledger ownership | Decides if the suite is stateful per run | — | ✅ Resolved — injected `Account` (ADR-0010) |
| **Q-REG-1** Component registry trust model | Gates how strategies are extended | OD-7 | ✅ Resolved — tiered + WASM (ADR-0011) |
| **Q-REPO-1** Shared contracts topology | Shapes how training package & platform consume contracts | OD-11 | ✅ Resolved — standalone kernel (ADR-0012) |
| **Q-PROD-2** First vertical slice | Sequenced into the forthcoming phased plan | — | ⏳ Deferred to planning |

Everything else can be deferred until a strategy or engine actually needs it.
