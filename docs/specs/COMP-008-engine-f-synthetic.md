# Spec: COMP-008 — Engine F: Synthetic

**Spec ID:** COMP-008
**Type:** Component (execution engine)
**Status:** Approved
**Date:** 2026-06-06
**Author:** Agent

**`price_formation`:** `OTC`
**Routes here:** CFDs, swaps, structured notes, barrier notes, convertibles, autocalls.

Engine F values **custom-payoff** contracts: instruments whose value is defined by a rule over
one or more underlyings, often path-dependent, traded bilaterally (OTC) rather than on an
exchange. The defining idea: the **payoff is a registered component**, never logic embedded in
the strategy JSON.

---

## 1. The `Engine` trait

```rust
impl Engine for SyntheticEngine {
    fn on_event(&mut self, ev: &MarketEvent, ctx: &mut EngineContext);  // eval payoff, accrue financing, check barriers
    fn submit_order(&mut self, o: Order, ctx: &mut EngineContext) -> OrderResult;  // OTC bilateral fill
    fn settle(&mut self, ctx: &mut EngineContext);                       // maturity / redemption
    fn supports_order_type(&self, t: OrderType) -> bool;  // market (OTC) ONLY
}
```

Financing and cash live in the injected `Account`
([ADR-0010](../adr/0010-simulator-does-not-own-portfolio.md)).

---

## 2. The payoff-component model

Each synthetic instrument references a **registered payoff component** (see
[component-registry.md](COMP-001-component-registry.md)) that the engine evaluates each event:

```
payoff_value = payoff_component(underlyings_PIT, contract_params, prior_state) -> (value, new_state)
```

The component is pure, point-in-time, and deterministic — and runs in the appropriate trust tier
(built-in / native / WASM). This keeps arbitrary payoff logic out of the strategy JSON while
allowing unlimited structure. Common payoffs (vanilla CFD, vanilla swap, single-barrier note)
ship as built-ins.

---

## 3. Valuation & marking

`mark = payoff_component(current state)`. For path-dependent products the component carries
state (e.g. whether a barrier has been touched, accumulated coupons). For products needing
option-like valuation (convertibles), the component may call the same pricing primitives Engine
E exposes (composition), but the engine itself stays payoff-agnostic.

---

## 4. Financing / carry

Synthetic exposure is financed:

- **CFDs:** overnight financing on the notional (long pays, short may receive), accrued daily.
- **Swaps:** periodic exchange of legs (e.g. fixed vs. floating); the engine accrues and settles
  each leg on schedule.

Financing is accrued via the `Account` and reported separately from payoff P&L.

---

## 5. Barrier monitoring

For barrier products the engine maintains a knock-in/knock-out state machine:

- **Observation frequency:** continuous (any touch) or discrete (scheduled observation dates),
  per contract.
- On a barrier breach, the payoff component transitions state (knocked-in activates the payoff;
  knocked-out terminates or fixes it). Path dependence is handled in component state, advanced in
  strict `ts_event` order.

---

## 6. Structured products

- **Autocalls:** scheduled observation dates with early-redemption triggers (if the underlying
  is above a level, the note redeems early with a coupon). The engine drives the observation
  schedule and the component evaluates the trigger.
- **Convertibles:** a bond floor plus an embedded conversion option — composes Engine D
  (cash-flow) valuation with an Engine E option value inside the payoff component.

---

## 7. OTC fill model

No order book. Fills are **bilateral**: execute at the marked value plus a configurable
counterparty/bid-ask spread and any financing setup cost. Counterparty terms (financing rate,
spread) are contract metadata.

---

## 8. Output

A `TradeRecord` per fill: `setup` (notional, direction, contract id), `sizing`, `execution`
(entry value, spread, financing terms), `trigger`. Financing accruals, leg settlements, and
barrier/autocall events are emitted as auxiliary records tagged by source.

---

## 9. Determinism & ordering

- Payoff components are deterministic and point-in-time; barrier/observation events apply in
  `ts_event` order.
- No underlying value dated after the decision is visible (look-ahead safety).

---

## 10. Open items / parameters

- The structured-product schema (how autocall/barrier schedules are declared vs. encoded in the
  payoff component).
- Whether a dedicated synthetic/structured **asset spec** is authored alongside this engine
  (currently summarized in [assets/README.md](README.md)).
- Counterparty-risk / credit-valuation-adjustment modeling (likely out of scope).
