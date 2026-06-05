# Engine D: Cash Flow

**Status:** 🔲 Deferred — authored during its implementation phase. Architectural frame is fixed.

**`price_formation`:** `DEALER` · **Routes here:** bonds, treasuries, municipals, CDs, MBS.

## What this spec will define
- Coupon scheduling & accrual; clean/dirty price; yield-derived pricing from a curve + spread.
- Duration/convexity (provide-or-derive); maturity as a forced-close event; credit events.
- MBS prepayment modeling (CPR/PSA).

## Fixed constraints (already decided)
- Implements the `Engine` trait; price is model-derived when no market quote exists
  ([engines/README.md](README.md)).
- Account state via injected `Account` ([ADR-0010](../../adr/0010-suite-does-not-own-portfolio.md)).
- Deterministic, point-in-time.

## Source asset spec
[bonds](../assets/bonds.md)
