# Execution Engines

An engine is the self-contained simulation of one **price-formation mechanic**. The engine
is selected by the instrument's `price_formation` field — nothing else routes to it.

## Selection rule

> If price formation changes → build a new engine.
> If price formation stays the same → extend the existing engine.

Examples:
- Stocks and CEX crypto both use a CLOB → both use Engine A. Crypto adds fee-tier logic
  and token events as extensions.
- ETFs execute on a CLOB but also use NAV valuation → Engine A for execution, Engine C
  attached via the `HasNAV` capability for valuation.
- AMMs have fundamentally different price formation → Engine B, not an extension of A.

## Engines

| Engine | `price_formation` | Spec | Status |
|---|---|---|---|
| **A** — Order Book | `CLOB` | [engine-a-order-book.md](engine-a-order-book.md) | ✅ Defined |
| **B** — AMM | `AMM` | [engine-b-amm.md](engine-b-amm.md) | ✅ Defined |
| **C** — NAV | `NAV` | [engine-c-nav.md](engine-c-nav.md) | ✅ Defined |
| **D** — Cash Flow | `DEALER` | [engine-d-cashflow.md](engine-d-cashflow.md) | ✅ Defined |
| **E** — Derivatives | `CHAIN` | [engine-e-derivatives.md](engine-e-derivatives.md) | ✅ Defined |
| **F** — Synthetic | `OTC` | [engine-f-synthetic.md](engine-f-synthetic.md) | ✅ Defined |
| **G** — Marketplace | `MARKETPLACE` | [engine-g-marketplace.md](engine-g-marketplace.md) | ✅ Defined |
| **H** — Event Resolution | `ORACLE` | [engine-h-event-resolution.md](engine-h-event-resolution.md) | ✅ Defined |

All eight engines are fully specified.

## What every engine must implement

The `Engine` trait (to be defined in `crates/engines`) requires:

```
trait Engine {
  // Called on each MarketEvent in ts_event order
  fn on_event(&mut self, event: &MarketEvent, ctx: &mut EngineContext);

  // Attempt to fill an order; returns fill or rejection
  fn submit_order(&mut self, order: Order, ctx: &mut EngineContext) -> OrderResult;

  // Called at run end to force-close any open positions
  fn settle(&mut self, ctx: &mut EngineContext);

  // Capability check: is this order type valid for this engine?
  fn supports_order_type(&self, order_type: OrderType) -> bool;
}
```

## Order types per engine

Not all order types are valid on all engines. Attempting to use an unsupported order type
is a contract error (not a runtime failure).

| Order type | A (CLOB) | B (AMM) | C (NAV) | D (Cash Flow) | E (Deriv.) | F (OTC) | G (Mkt) | H (Event) |
|---|---|---|---|---|---|---|---|---|
| Market | ✅ | ✅ (swap) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Limit | ✅ | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ✅ⁱ |
| Stop | ✅ | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ |
| Stop-limit | ✅ | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ |
| Swap (exact-in) | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Swap (exact-out) | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Exercise (option) | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ |
| Listing (NFT) | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ |

- **C (NAV)** market orders are *forward-priced* subscriptions/redemptions — accepted intraday,
  filled at the next struck NAV, not at a price known at submission.
- **D (Cash Flow)** and **F (OTC)** "market" orders are **dealer/bilateral** fills (quoted price ±
  a spread), not order-book fills — there is no resting book on those engines.
- ⁱ **H (Event) limit orders** are ❌ by default but **promote to ✅ when the instrument sets
  `HasClob`** (liquid prediction markets such as Kalshi or major Polymarket markets run a real
  YES/NO CLOB). See [engine-h-event-resolution.md](engine-h-event-resolution.md) §10.

## Engine composition

One instrument may attach multiple engines via capabilities:

| Instrument | Execution engine | Valuation engine |
|---|---|---|
| Standard ETF | A (CLOB fills) | C (NAV tracking) |
| Leveraged ETF | A (CLOB fills) | C (daily reset NAV) |
| Options on futures | A (futures CLOB for underlying) | E (option valuation) |
| Bond ETF | A (CLOB fills) | C (NAV) + D (underlying bond pricing) |

Composition works because engines communicate through a shared `EngineContext` — the
valuation engine updates the mark; the execution engine produces fills; both write to the
same position ledger.
