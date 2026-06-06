# Spec: COMP-007 — Engine E: Derivatives

**Spec ID:** COMP-007
**Type:** Component (execution engine)
**Status:** Approved
**Date:** 2026-06-06
**Author:** Agent

**`price_formation`:** `CHAIN`
**Routes here:** options and warrants; derivative valuation for futures/perps where a model price
is needed.

Engine E is a **valuation engine**, not just a matcher: it prices contracts from an implied-vol
surface and the underlying, computes greeks, models early exercise/assignment, and settles at
expiry. A volatility input is a **hard requirement** of the data manifest
([assets/options.md](DATA-015-options-asset-spec.md)).

---

## 1. The `Engine` trait

```rust
impl Engine for DerivativesEngine {
    fn on_event(&mut self, ev: &MarketEvent, ctx: &mut EngineContext);  // reprice + greeks + checks
    fn submit_order(&mut self, o: Order, ctx: &mut EngineContext) -> OrderResult;
    fn settle(&mut self, ctx: &mut EngineContext);                       // expiry settlement
    fn supports_order_type(&self, t: OrderType) -> bool;  // market, limit, exercise
}
```

Account/margin state is read from the injected `Account`; the engine owns no portfolio
([ADR-0010](../adr/0010-simulator-does-not-own-portfolio.md)).

---

## 2. Valuation core

Inputs per contract: underlying `S`, strike `K`, time-to-expiry `T`, risk-free rate `r`,
dividend/carry `q`, and volatility `σ` from the IV surface.

### 2.1 Baseline model — Black–Scholes–Merton (European)

```
d1 = [ln(S/K) + (r − q + σ²/2)·T] / (σ·√T)
d2 = d1 − σ·√T
Call = S·e^(−qT)·N(d1) − K·e^(−rT)·N(d2)
Put  = K·e^(−rT)·N(−d2) − S·e^(−qT)·N(−d1)
```

### 2.2 American options

Early exercise can be optimal, so BSM is a lower bound. Baseline: a **binomial/PDE** solver (or
Barone-Adesi-Whaley approximation for speed). The pricing model is pluggable behind a trait —
see **OD-4** (build in Rust vs. optional QuantLib plugin). The engine treats the model as a
deterministic function `(S,K,T,r,q,σ,style) → (price, greeks, exercise_boundary)`.

### 2.3 Greeks (provide-or-derive)

If greeks are supplied in the data, the engine uses them (and may validate against its model);
if absent, it derives them. BSM forms:

```
Δ_call = e^(−qT)·N(d1)          Δ_put = e^(−qT)·(N(d1) − 1)
Γ      = e^(−qT)·φ(d1) / (S·σ·√T)
Vega   = S·e^(−qT)·φ(d1)·√T            (per 1.00 vol; report per 1%)
Θ, ρ   = standard BSM forms (carry-adjusted)
```

---

## 3. IV surface usage

At each event the engine looks up `σ(K, T)` from the latest `IVSurface`
([contracts/market-data.md](DATA-004-market-data-contract.md) §2.4):

- Interpolate across strikes (or moneyness) and expiries — bilinear in `(log-moneyness, T)` by
  default; per-slice SVI is an allowed upgrade.
- `surface_type` may be `AbsoluteStrike` or `DeltaNormalized`; the engine converts as needed.
- Arbitrage-free surfaces are the caller's responsibility; the engine does not re-fit.

The surface as-of each timestamp is mandatory — using a later surface for an earlier date is
look-ahead and is structurally prevented (`ts_event ≤ current_ts`).

---

## 4. Pricing & marking flow (per event)

```
1. read underlying S (Bar/Trade/Quote on underlying_id — must be in the same run)
2. σ ← surface.lookup(K, T)
3. (price, greeks) ← model(S, K, T, r, q, σ, style)        // provide-or-derive
4. mark ← fresh option Quote if present, else model price   // marking source
5. record greeks for attribution; run early-exercise & margin checks (§6, §7)
```

---

## 5. Fill model

Three fidelity levels, recorded in the `TradeRecord`:

- **Option quotes present** (`fidelity: quotes`): fill against `bid`/`ask` as a taker (limit
  orders rest and fill on cross); partial fills allowed. Spreads on options are wide — this is
  the highest-fidelity path.
- **Surface only** (`fidelity: surface`): fill at `model price ± spread_estimate`, where the
  spread widens with moneyness/illiquidity (configurable). Used when the contract's own quotes
  are stale/absent (common for far-OTM strikes). Accuracy is moderate.
- **OHLCV only** (`fidelity: ohlcv`): using only option OHLCV bars without an IV surface
  produces heavily distorted results — option prices embed implied vol, which changes
  continuously, and a bar close does not reflect a tradeable mid. This path is explicitly flagged
  in the result as **low accuracy**; the data manifest should warn when no IV surface or quotes
  are bound for an option instrument.

- **Contract multiplier** (e.g. 100 for US equity options) scales notional and P&L.

---

## 6. Early exercise & assignment (American)

Each event, for American-style contracts:

- Compute **exercise value** (intrinsic) vs. **continuation value** (model). Flag exercise
  optimal when intrinsic ≥ continuation, plus known triggers: deep-ITM puts, and calls just
  before an ex-dividend date (requires the underlying dividend stream).
- **Long positions:** the strategy may submit an `exercise` order; the engine converts the
  option into the underlying position (physical) or cash (intrinsic, cash-settled).
- **Short positions:** when the optimal-exercise boundary is crossed, the engine **assigns** —
  opens the resulting underlying position, debits/credits cash, and emits an `ExerciseEvent`
  (assignment) to the strategy. Assignment timing follows the model boundary (deterministic).

---

## 7. Expiry settlement

At `expiry_date` (`settle`):

- **OTM** → expires worthless; long loses premium, short keeps it.
- **ITM cash-settled** (index, VIX) → cash = intrinsic × multiplier.
- **ITM physically-settled** (equity options) → deliver/receive the underlying; the short must
  provide it (position opened against the `Account`).

---

## 8. Margin & liquidation

- **Long options:** premium paid up front; no margin.
- **Short options:** require margin; the engine queries collateral & maintenance margin from the
  injected `Account` each event and force-closes (liquidates) if collateral is insufficient
  against the marked value — same mechanic as Engine A's `HasLiquidation`.

---

## 9. Multi-leg & underlying linkage

- Option spreads/pairs are **multiple instruments**; the strategy's execution stage submits each
  leg. Leg **atomicity** (all-or-none) is an execution-layer concern (Q-STRAT-5), not the engine.
- A delta-hedged position needs the **underlying in the same run** (shared clock) so the engine
  can price the option and the strategy can trade the hedge.

---

## 10. Output

Each fill produces a `TradeRecord` with **greeks recorded at fill** (for greek P&L attribution
in metrics), the vol used, the marking source (quote vs. model), and exercise/assignment/expiry
events tagged separately. This feeds the options metric extensions
([assets/options.md](DATA-015-options-asset-spec.md) §5).

---

## 11. Determinism & ordering

- Pricing model and surface interpolation are deterministic; same inputs ⇒ same price/greeks.
- Dividend/ex-date events and assignment apply in `ts_event` order before matching.
- No surface, underlying, or quote dated after the decision is visible (look-ahead safety).

---

## 12. Open items / parameters

- **OD-4:** native Rust pricing models vs. an optional QuantLib plugin behind the pricing trait.
- American-exercise solver choice (binomial vs. PDE vs. BAW) and its accuracy/speed tradeoff.
- Default surface interpolation (bilinear) vs. SVI; extrapolation policy beyond surface bounds.
- Spread-estimate model for surface-only fills.
- Assignment determinism vs. optional probabilistic assignment.
