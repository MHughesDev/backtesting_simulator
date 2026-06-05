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
- **Q-STRAT-4** How does a strategy express **pairs / spread / relative-value** logic across two instruments declaratively?
- **Q-STRAT-5** How are **multi-leg orders** (option spreads, pairs trades) expressed and executed atomically?
- **Q-STRAT-6** How does a strategy **react to fills, partial fills, and rejections** declaratively (no imperative callback)?
- **Q-STRAT-7** Can strategies be composed/nested (a strategy of strategies)? *(related: OD-6)*
- **Q-STRAT-8** How are strategy-level vs. portfolio-level constraints separated when multiple strategies share capital? *(OD-6)*
- **Q-STRAT-9** How is rebalancing cadence expressed (every event vs. scheduled vs. signal-triggered)?
- **Q-STRAT-10** How does a strategy declare the capabilities it requires so instruments can be pre-validated against it?
- **Q-STRAT-11** Can a strategy define a derived/intermediate universe (e.g. "top-decile by a feature")?
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
- **Q-SIG-3** Should raw documents (text) ever enter the suite, or only pre-computed numeric/categorical features?
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
