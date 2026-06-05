# Engine F: Synthetic

**Status:** 🔲 Deferred — authored during its implementation phase. Architectural frame is fixed.

**`price_formation`:** `OTC` · **Routes here:** CFDs, swaps, structured & barrier notes,
convertibles, autocalls.

## What this spec will define
- Custom payoff evaluation; financing/carry costs; barrier monitoring; counterparty terms.
- How a payoff is expressed (a registered component vs. a structured-product schema).

## Fixed constraints (already decided)
- Implements the `Engine` trait ([engines/README.md](README.md)).
- Account/financing state via injected `Account` ([ADR-0010](../../adr/0010-suite-does-not-own-portfolio.md)).
- Deterministic, point-in-time. Payoff logic is a component, not embedded in strategy JSON.

## Source asset spec
Covered under the synthetic/structured class in [assets/README.md](../assets/README.md)
(dedicated asset spec to be authored with this engine).
