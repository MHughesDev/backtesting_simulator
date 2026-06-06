# Specification Organization Analysis & Remediation Plan

**Date:** 2026-06-06  
**Requirement:** Clear separation of spec files by type (Feature / Component / Data / Integration)  
**Test:** "Something gets its own spec file when you can write observable, testable acceptance criteria for it."

---

## Current State

### Issues Identified

#### 1. **ENGINE_DEEP_DIVE.md** (1994 lines) — DOES NOT FIT THE FOUR-TYPE MODEL

**Type:** None (Reference/Mega-document)

**Problem:** This file attempts to be both a reference guide AND a spec, duplicating content:
- Duplicates engine mechanics from `engines/engine-*.md` (all 8 engines covered)
- Duplicates Run Request details from `run-request.md` (line-by-line reference)
- Contains cross-cutting rules (validation order, run invariants, bar derivation)

**Why this violates the requirement:**
- Not Feature, Component, Data, or Integration
- Not one unit of implementable work
- Cannot be handed to someone to implement without them asking "what do you want me to build?"
- Mixes three conceptually separate concerns in one file

**Example of duplication:**
```
ENGINE_DEEP_DIVE.md §1: "Engine A — Order Book (CLOB)"
  → Same content in engines/engine-a-order-book.md §1

ENGINE_DEEP_DIVE.md §9: "The Run Request — Line by Line"
  → Same content in run-request.md §3–§10
```

---

#### 2. **Missing Integration Specs** — TYPE NOT EXPLICITLY DECLARED

The following are Integration specs (external dependencies the suite calls) but aren't clearly labeled:

| Port | Current location | Issue |
|---|---|---|
| **Model** | `contracts/model.md` | ✅ Exists, but not labeled as Integration |
| **Trainer** | `contracts/training.md` | ✅ Exists, but not labeled as Integration |
| **Account** | `run-request.md` §5 | 🔲 Embedded in Run Request; no standalone Integration spec |

**Acceptance criteria:** "Integration spec exists if the suite calls external code to satisfy a need."  
→ All three ports fit. Account should have a dedicated spec or be expanded in run-request.md.

---

#### 3. **Deferred Specs Lack Acceptance Criteria** — INCOMPLETE FRAMES

| Spec | Status | Problem |
|---|---|---|
| `runner.md` | 🔲 Deferred | Marked as "to be authored"; no acceptance criteria skeleton |
| `contracts/metrics.md` | 🔲 Deferred | Marked as "to be authored"; schema not started |

**The frame is set** (both mentioned in MASTER_SPEC.md §10), but acceptance criteria are absent.

**What's missing:**
- `runner.md`: No acceptance criteria for "parallel execution", "determinism", "latency", "work-stealing"
- `contracts/metrics.md`: No metric schemas (TradeRecord, aggregate metrics), no definitions, no accuracy requirements

---

#### 4. **Type Labels Missing Across ALL Specs** — ORGANIZATIONAL CLARITY

**Current state:** Specs don't explicitly state their type.

**Example:** Is `contracts/strategy.md` a Data spec or a Feature spec?
- It defines a data contract (JSON schema) → Data
- But it also describes a system behavior (composable pipeline) → Could be interpreted as Feature

**Root cause:** No convention for labeling spec types in file headers.

---

#### 5. **Asset Specs Are Data Specs But Not Labeled** — GOOD STRUCTURE, MISSING LABEL

Asset specs (`assets/equities.md`, `assets/options.md`, etc.) are properly structured **Data** specs:
- Define schema (what fields are required, optional, types)
- State acceptance criteria (e.g., "two price series required", "corporate actions must be present")
- Are testable (validate input against contract)

✅ These are correct but should have type labels for consistency.

---

#### 6. **Acceptance Criteria Vary in Explicitness** — QUALITY INCONSISTENCY

| Spec | Acceptance criteria quality |
|---|---|
| `assets/equities.md` | ✅ Explicit ("two price series", "corporate actions REQUIRED") |
| `contracts/strategy.md` | ✅ Explicit ("one format only: JSON") |
| `contracts/model.md` | ✅ Clear ("look-ahead safety", "inference-only") |
| `ENGINE_DEEP_DIVE.md` | ⚠️ Vague (reference details, no testable criteria) |
| `runner.md` | 🔲 Missing (file not started) |
| `contracts/metrics.md` | 🔲 Missing (frame only) |

---

## Remediation Plan

### Phase 1: Reorganize ENGINE_DEEP_DIVE.md (Highest Priority)

**Option A: Archive it** (Recommended)
1. Move `ENGINE_DEEP_DIVE.md` → `REFERENCE_ENGINE_DEEP_DIVE.md` (or `/reference/`)
2. Mark it as a reference guide, not a spec
3. Add a note: "This file duplicates content from `engines/` and `run-request.md` for quick lookup only. For authoritative content, refer to those specs."

**Option B: Consolidate** (if content is unique)
1. Extract unique content (cross-cutting rules, validation order, bar derivation)
2. Distribute to appropriate specs:
   - Engine mechanics → engines/engine-*.md (already there)
   - Run Request details → run-request.md (already there)
   - Cross-cutting rules → new `spec/run-execution-invariants.md` (Data spec)
   - Bar derivation → run-request.md §4a or new `spec/bar-derivation.md` (Data spec)
3. Delete or archive ENGINE_DEEP_DIVE.md

**Recommendation:** Archive it. The content already lives in the right places.

---

### Phase 2: Add Type Labels to All Specs

**Standard format (add to every spec file header):**

```markdown
# Spec: [Name]

**Type:** [Feature | Component | Data | Integration]  
**Status:** [✅ Defined | 🔲 Deferred | ⏳ In Progress]  
[One-sentence purpose]

---
```

**Example for `contracts/strategy.md`:**
```markdown
# Contract Spec: Strategy

**Type:** Data (JSON schema contract)  
**Status:** ✅ Defined  
Defines the declarative pipeline for trading strategies: universe → features → alpha → sizing → risk → execution.

---
```

**Example for `contracts/model.md`:**
```markdown
# Contract Spec: AI Endpoint (Model Port)

**Type:** Integration (external AI/ML inference dependency)  
**Status:** ✅ Defined  
Defines how the suite calls external AI endpoints for inference without owning or training models.

---
```

---

### Phase 3: Create/Expand Missing Integration Specs

#### 3a. Account Port Integration Spec

**Current:** Embedded in `run-request.md` §5  
**Action:** Create `contracts/account-integration.md`

**Minimal structure:**
```markdown
# Integration Spec: Account (Ledger Port)

**Type:** Integration  
**Status:** ✅ Defined  

The suite does not own the portfolio ledger. The Account port is the interface the suite
calls to query equity, positions, and collateral, and to report fills.

## Acceptance Criteria
- [ ] Account implementation can answer: "what is current equity?"
- [ ] Account implementation can answer: "what positions do I hold?"
- [ ] Account implementation can answer: "what is my buying power?"
- [ ] Suite can report a fill to Account and Account updates its state atomically
- [ ] Account interface is synchronous (no I/O blocking in the hot loop)
```

---

### Phase 4: Complete Deferred Specs with Acceptance Criteria

#### 4a. runner.md — Component Spec

**Current:** 
```
# Spec: Runner & Run Queue
Status: 🔲 Deferred
```

**Updated to:**
```markdown
# Spec: Runner & Run Queue

**Type:** Component (internal run-queue scheduler)  
**Status:** 🔲 Deferred — to be authored in implementation phase  

Schedules and executes single or multiple backtests concurrently. Owns parallelism,
work-stealing, determinism under concurrency, and output streaming.

## Acceptance Criteria

When implemented, the runner must satisfy:

1. **Parallelism & Concurrency**
   - [ ] Execute N concurrent backtests with Rayon work-stealing (zero-copy data sharing via Arc)
   - [ ] P50 latency for single run ≤ [target ms]; P95 ≤ [target ms]
   - [ ] Throughput: [N runs/sec] for typical strategy

2. **Determinism Under Concurrency**
   - [ ] Bit-identical results regardless of thread count (same RNG seeding)
   - [ ] No data races (Rust guarantees via Send + Sync)
   - [ ] Deterministic tie-break for simultaneous events (documented ordering)

3. **Run Queue Management**
   - [ ] Accept N runs (array of Run Requests) in a single call
   - [ ] Report per-run status (queued, running, complete, error)
   - [ ] Support cancellation of a single run without affecting others
   - [ ] Support per-run memory limits and timeouts

4. **Output Streaming**
   - [ ] Stream results incrementally (don't buffer entire dataset)
   - [ ] Support batching or one-at-a-time result delivery

5. **Error Handling**
   - [ ] One run's failure does not cascade to others
   - [ ] Precise error reporting per run (not generic)
```

#### 4b. contracts/metrics.md — Data Spec

**Current:**
```
# Contract Spec: Result / Metrics
Status: 🔲 Deferred
```

**Updated to:**
```markdown
# Contract Spec: Result / Metrics

**Type:** Data (output schema contract)  
**Status:** 🔲 Deferred — to be authored in metrics/results implementation phase  

Defines the shape of results returned by the suite: the TradeRecord stream and aggregate
metrics computed from it. Metrics are both universal (returns, Sharpe, drawdown) and
capability-gated (greeks, funding P&L, gas costs, Brier score).

## Acceptance Criteria

When implemented, must define:

1. **TradeRecord Schema**
   - [ ] Per-trade structure: timestamp, instrument, side, qty, price, fees, slippage, order info
   - [ ] Capability-gated fields (greeks, funding, gas, etc.)
   - [ ] Testable: a trade log can be replayed to verify P&L

2. **Universal Metrics (all strategies)**
   - [ ] Returns (gross, net-of-fees, after-funding)
   - [ ] Sharpe ratio (annualized, with or without benchmark)
   - [ ] Max drawdown, recovery time
   - [ ] Sortino ratio, Calmar ratio
   - [ ] Win rate, profit factor, avg win/loss
   - [ ] Definitions precise enough to recompute from TradeRecord

3. **Capability-Gated Metric Extensions**
   - [ ] Greeks attribution (options/derivatives)
   - [ ] Funding P&L (perps/perpetuals)
   - [ ] Gas costs / MEV (DEX/AMM)
   - [ ] Brier score (prediction markets)
   - [ ] etc., per engine capability

4. **Precision & Validation**
   - [ ] Accuracy targets for each metric (e.g., Sharpe ±0.01)
   - [ ] Handling of edge cases (zero returns, single trade, no trading days)
   - [ ] Comparison to reference implementations (reference backtester or academic sources)
```

---

### Phase 5: Document Type Convention

**Create a new file:** `spec/SPEC_TYPES.md` (or add to MASTER_SPEC.md)

```markdown
## Spec Types & Organization

Every spec file in this specification is one of four types:

### Type: **Data** (Schema / Model / Contract)
Defines a data structure, schema, or contract that flows through the system.

**When to create:** When you need to specify the shape of information (JSON, Arrow, events).  
**Acceptance criteria:** Schema defined; can be validated; has examples.

**Examples:**
- `contracts/strategy.md` — Strategy JSON schema
- `contracts/market-data.md` — Market data event schema
- `assets/equities.md` — Equity asset requirements
- `run-request.md` — Run Request structure

### Type: **Component** (Internal Service / Module)
An internal piece of the suite with a defined interface (Engine, Runner, Registry, etc.).

**When to create:** When you're specifying how an internal system works.  
**Acceptance criteria:** Interface defined; behavior specified; testable.

**Examples:**
- `engines/engine-a-order-book.md` — Order Book matching engine
- `runner.md` — Run queue scheduler
- `component-registry.md` — Registry trust model

### Type: **Integration** (External Dependency / Port)
How the suite interacts with external systems or code the caller provides.

**When to create:** When the suite calls code outside its process/control.  
**Acceptance criteria:** Interface defined; expectations clear; error handling specified.

**Examples:**
- `contracts/model.md` — AI endpoint inference port
- `contracts/training.md` — Trainer port for model training
- `contracts/account-integration.md` — Account ledger port

### Type: **Feature** (User-Visible Behavior)
An end-to-end user-visible capability with observable acceptance criteria.

**When to create:** When specifying what end users can do.  
**Acceptance criteria:** Observable, testable, measurable.

**Examples:**
- User can upload a strategy JSON and run a backtest
- User can sweep parameters and compare results
- Results are reproducible across machines

---

## Spec Metadata Convention

Every spec file header should include:

```markdown
# [Spec Name]

**Type:** [Feature | Component | Data | Integration]  
**Status:** [✅ Defined | 🔲 Deferred | ⏳ In Progress]  
**Links:** [Related specs/ADRs]

[One-paragraph purpose/overview]
```

Example:
```markdown
# Contract Spec: Strategy

**Type:** Data  
**Status:** ✅ Defined  
**Links:** [ADR-0004](../adr/0004-strategy-json-pipeline.md), [Plan](plan.md)

A strategy is the full, declarative path from data to a trade...
```
```

---

## Summary Table

| Issue | Priority | Action | Phase |
|---|---|---|---|
| ENGINE_DEEP_DIVE.md duplicates content | 🔴 High | Archive or consolidate | 1 |
| Missing type labels on specs | 🟡 Medium | Add type + status headers to all specs | 2 |
| Account port not a standalone spec | 🟡 Medium | Create `contracts/account-integration.md` | 3 |
| runner.md lacks acceptance criteria | 🟡 Medium | Add criteria skeleton | 4 |
| contracts/metrics.md lacks acceptance criteria | 🟡 Medium | Add criteria skeleton | 4 |
| No spec-type documentation | 🟢 Low | Create SPEC_TYPES.md or MASTER_SPEC §10a | 5 |

---

## Implementation Checklist

- [ ] Phase 1: Archive ENGINE_DEEP_DIVE.md or consolidate
- [ ] Phase 2: Add type + status labels to all specs (batch edit)
- [ ] Phase 3: Create contracts/account-integration.md
- [ ] Phase 4a: Expand runner.md with acceptance criteria
- [ ] Phase 4b: Expand contracts/metrics.md with acceptance criteria
- [ ] Phase 5: Create SPEC_TYPES.md or document type convention
- [ ] Verify: Every spec file has a type label and status
- [ ] Verify: Every spec has testable acceptance criteria
- [ ] Verify: No spec duplicates content from another spec

---

## Notes

- **Backwards compatibility:** These changes are organizational only; they don't change the system itself.
- **Content stability:** Specs marked ✅ Defined are stable; Deferred specs will be authored in their phases.
- **Reference docs:** Non-spec reference materials (e.g., archived ENGINE_DEEP_DIVE) should be moved to a `docs/reference/` folder or marked clearly as not authoritative.
