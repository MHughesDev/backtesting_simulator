# Contract Spec: Signals (exogenous / alternative data)

**Status:** 🔲 Deferred — to be authored during its implementation phase. The architectural
frame below is fixed; this file will be expanded into the full spec.

## Purpose

Defines how **exogenous, point-in-time signal timeseries** (news sentiment, social, search
trends, on-chain metrics, any pre-computed feature) enter the suite. The suite treats a signal
as just another timestamped value — it never knows or cares that a number came from Reddit. All
semantic processing (scraping, NLP, scoring, entity mapping) is **upstream**, in the caller.

## What this spec will define

- The `Signal { name, value, … }` payload variant and the `HasExogenousSignals` capability.
- The two ingestion modes: **pre-computed feature streams** (default) and **in-loop model
  inference** via the `Model` port.
- **Point-in-time integrity** as the caller's contractual responsibility (honest `ts_event`),
  and what the suite can mechanically enforce (no look-ahead) vs. cannot (timestamp honesty).
- How strategies bind signals (`signal:<name>` references — see
  [strategy.md](strategy.md) §4).
- Entity mapping / alignment and frequency-mismatch handling.

## Fixed constraints (already decided)

- Look-ahead safety enforced via `ts_event ≤ current_ts` (same as all data).
- The suite owns no data and no models — signals are produced by the caller.
- Raw documents (text) are out of scope for the core; only numeric/categorical features enter.

## Inputs

- The semantic-data design discussion; [contracts/market-data.md](market-data.md),
  [contracts/model.md](model.md), [strategy.md](strategy.md).
