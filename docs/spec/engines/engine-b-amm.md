# Engine B: AMM

**Status:** ✅ Defined.
**`price_formation`:** `AMM`
**Routes here:** DEX pools — Uniswap v2 (CPMM), Uniswap v3 / Raydium (concentrated liquidity),
Curve (StableSwap), and other liquidity-pool venues.

Engine B prices trades **algorithmically from pool state**, not from an order book. There is no
queue and **no limit order** — every trade is a market swap against the pool invariant.

---

## 1. The `Engine` trait

```rust
impl Engine for AmmEngine {
    fn on_event(&mut self, ev: &MarketEvent, ctx: &mut EngineContext);  // ingest PoolState/gas
    fn submit_order(&mut self, o: Order, ctx: &mut EngineContext) -> OrderResult;
    fn settle(&mut self, ctx: &mut EngineContext);
    fn supports_order_type(&self, t: OrderType) -> bool;  // swap_exact_in, swap_exact_out ONLY
}
```

`supports_order_type` returns `false` for `market`/`limit`/`stop`/etc. — submitting any of them
is a compile-time contract error (see [engines/README.md](README.md) order-type matrix). Gas is
a first-class P&L line; account state is read from the injected `Account`
([ADR-0010](../../adr/0010-suite-does-not-own-portfolio.md)).

---

## 2. Pool-state model

State comes from `PoolState` payloads ([contracts/market-data.md](../contracts/market-data.md)
§2.5). The `amm_variant` on the instrument selects the math.

```
PoolWorkingState {
  variant:   UniswapV2 | UniswapV3 | Curve | …,
  // v2:
  reserve_0, reserve_1: u128,
  // v3:
  sqrt_price_x96: u160, current_tick: i32, liquidity: u128, ticks: Vec<TickEntry>,
  // curve:
  balances: Vec<u128>, amp: u64,
  fee_bps: u16,
}
```

The engine prices against the **latest observed** `PoolState`. Our hypothetical swap's impact is
applied to a **working copy** that persists for subsequent swaps until the **next observed
`PoolState` resets to reality** (the real chain never included our trade). This assumption is
recorded; it matters only for large or rapidly-sequenced swaps.

---

## 3. Pricing math

### 3.1 Uniswap v2 — Constant Product (x·y = k)

Fee `f` (e.g. 0.003) is taken from the input. For an exact-in swap of `Δin` (token_in reserve
`R_in`, token_out reserve `R_out`):

```
Δin_eff = Δin · (1 − f)
Δout    = (R_out · Δin_eff) / (R_in + Δin_eff)
```

Exact-out (want `Δout`, solve input):

```
Δin = R_in · Δout / ((R_out − Δout) · (1 − f))      (+1 wei rounding up)
```

Spot price before = `R_out / R_in`; **price impact** = `1 − (Δout/Δin)/(R_out/R_in)`.
Post-swap reserves: `R_in += Δin`, `R_out −= Δout`.

### 3.2 Uniswap v3 / Raydium — Concentrated Liquidity

Price is tracked as `√P` (Q64.96). Within a single tick range, active liquidity `L` is constant:

```
amount0 = L · (1/√P_a − 1/√P_b)
amount1 = L · (√P_b − √P_a)
```

For an exact-in swap of token0 (zeroForOne, price falls), within the current tick:

```
√P_next = (L · √P) / (L + Δin_eff · √P)          Δin_eff = Δin·(1−f)
amount1_out = L · (√P − √P_next)
```

**Tick crossing:** if `√P_next` would pass the next initialized tick boundary, the engine swaps
up to the boundary, **crosses the tick** (updating `L` by that tick's `liquidityNet`), and
continues with the remaining input. Iterate until the input is consumed or liquidity is
exhausted. This requires `ticks` data; see fidelity (§4).

### 3.3 Curve — StableSwap

For pools near a peg, the invariant (n coins, amplification `A`) is:

```
A·nⁿ·Σxᵢ + D = A·D·nⁿ + Dⁿ⁺¹ / (nⁿ·Πxᵢ)
```

Solve `D` by Newton's method, then solve for the new output balance `y` given the input. More
gas-efficient near peg, much lower slippage than CPMM. (Full derivation deferred to
implementation; the invariant + Newton solve is the contract.)

---

## 4. Fidelity dial

| Fidelity | Data | Behavior |
|---|---|---|
| **Exact** | v3 `ticks` (per-tick liquidity) | Full tick-crossing simulation (§3.2) |
| **Approximate** | current `sqrt_price`/`liquidity` only | Assume constant active liquidity (no tick crossing) — valid for swaps small vs. depth; flagged in results |

For v2/Curve the single pool state is sufficient; fidelity concerns are specific to v3.

---

## 5. Execution

```
simulate_swap(working_state, direction, amount_in, max_slippage_bps) -> SwapResult {
    1. compute amount_out + effective price via §3
    2. realized_slippage_bps = impact vs. spot
    3. if realized_slippage_bps > max_slippage_bps → REVERT (no fill, no state change)
    4. else apply reserve/price update to working copy; deduct gas; report fill
}
```

- **Slippage tolerance** (`max_slippage_bps`) models the on-chain `minAmountOut` guard: exceed
  it and the swap reverts (records a rejection, not a bad fill).
- **Gas:** per-swap gas cost from a `GasEvent` stream or run config, in the chain's native unit,
  converted and deducted via `Account`. On EVM this can make small swaps uneconomical; on Solana
  it is negligible. Gas always appears as an explicit P&L line.
- **MEV (optional):** a configurable sandwich tax adds slippage to swaps above a visibility
  threshold; or a data-driven model if MEV data is supplied. Off by default.

---

## 6. LP strategies (extension)

For liquidity-provision strategies the engine also supports mint/burn of LP positions, fee
accrual on swaps through the active range, and **impermanent-loss** accounting vs. holding the
tokens. Marked as an extension; the swap path above is the core.

---

## 7. Output

Each swap produces a `TradeRecord`: `setup` (direction, amount, slippage tol), `sizing` (from
the strategy), `execution` (amount_out, effective price, **price impact**, **gas cost**, revert
reason if any, fidelity used), and `trigger`. Price impact and gas are always populated — they
are the dominant real costs on AMMs.

---

## 8. Determinism & ordering

- Swaps priced against the latest observed `PoolState`; working-copy mutations are deterministic.
- Within a block/timestamp, swaps process in `(instrument_id, seq)` order.
- No swap sees a `PoolState` dated after its own decision (look-ahead safety).

---

## 9. Open items / parameters

- Curve StableSwap Newton-solve tolerances and multi-coin generalization.
- v3 working-copy persistence policy across closely-spaced observed states.
- Default MEV/sandwich tax model and visibility threshold.
- Whether to model failed-transaction gas (reverts still cost gas on-chain).
