# Engine H: Event Resolution

**Status:** 🔲 Deferred — authored during its implementation phase. Architectural frame is fixed.

**`price_formation`:** `ORACLE` · **Routes here:** prediction markets, binary event contracts.

## What this spec will define
- Binary payoff at resolution ($1 / $0); market lifecycle (Active → Locked → Resolved → Settled).
- Resolution timing uncertainty; oracle-risk / dispute handling; probability-bounded pricing [0,1].
- Brier-score and calibration metrics as the primary evaluation.

## Fixed constraints (already decided)
- Implements the `Engine` trait; rejects fills after `Locked`
  ([engines/README.md](README.md)).
- Price is a probability; resolution closes positions (not a calendar expiry).
- Account state via injected `Account` ([ADR-0010](../../adr/0010-suite-does-not-own-portfolio.md)).
- Deterministic, point-in-time.

## Source asset spec
[prediction-markets](../assets/prediction-markets.md)
