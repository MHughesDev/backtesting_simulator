# Contracts

The five contracts are the system's public interface. The caller must satisfy the input
contracts (1–4); the suite returns the output contract (5).

A "contract" here means a **typed interface with enforced invariants** — not just a data
schema. The suite validates every input against these contracts at run time and rejects
under-specified or malformed inputs with precise error messages.

## The five contracts

| # | Contract | Direction | Spec |
|---|---|---|---|
| 1 | **Instrument** | Caller → Suite | [instrument.md](instrument.md) |
| 2 | **Market Data** | Caller → Suite | [market-data.md](market-data.md) |
| 3 | **Strategy** | Caller supplies as JSON at runtime, Suite compiles & drives | [strategy.md](strategy.md) |
| 4 | **Model** | Caller implements, Suite calls | [model.md](model.md) |
| 5 | **Result / Metrics** | Suite → Caller | [metrics.md](metrics.md) *(TBD)* |

> The suite **stores no strategies** — they are passed in at runtime, compiled, run, and
> discarded. Strategies are expressed in **one format only: JSON** (see
> [ADR-0004](../../adr/0004-strategy-json-pipeline.md),
> [ADR-0005](../../adr/0005-strategy-not-stored-suite-is-a-library.md)).

## Core design rules

1. **The Instrument Contract is the router.** Its `price_formation` field is the only thing
   that selects an engine. Everything else derives from that.
2. **Capabilities gate everything.** A payload variant, an order type, or a metric extension
   is only valid if the instrument declares the corresponding capability flag.
3. **Required is relative.** "Required" data is declared per `(instrument, engine)` by a
   **required-data manifest** on each instrument. There is no globally-required field beyond
   the envelope spine.
4. **Provide-or-derive.** Some values (greeks, continuous futures price, model mark) may be
   supplied by the caller or computed by the engine if absent.
5. **Fail loudly.** A run that does not satisfy its manifest produces a typed error, not
   garbage P&L.
