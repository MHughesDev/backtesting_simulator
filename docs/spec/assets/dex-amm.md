# Asset Spec: DEX / AMM

**Engine:** B (AMM)
**`price_formation`:** `AMM`
**Capabilities:** `HasPoolReserves | HasGasCost | HasPriceImpact`
For v3-style: add `HasConcentratedLiquidity | HasTickData`

---

## 1. What DEX / AMM is

A **Decentralized Exchange (DEX)** executes swaps without a central order book. Price is
determined algorithmically by the pool's state — not by matching resting orders. This is a
fundamentally different price-formation mechanism, which is why it has its own engine (B)
even though many DEX tokens are also "crypto."

### The critical distinction from CEX

```
CEX (Engine A):  buyer ↔ order book ↔ seller        (CLOB price formation)
DEX (Engine B):  trader ↔ liquidity pool ↔ formula  (AMM price formation)
```

There is **no concept of a limit order in a basic AMM.** Every trade is a market swap against
the pool. You cannot place a resting limit order on Uniswap v2. (v3 concentrated liquidity
approximates it, but execution is still algorithmic, not queue-based.)

### AMM variants

#### Uniswap v2 — Constant Product Market Maker (CPMM)
Invariant: **x · y = k** where x, y are pool reserves of the two tokens.

Price of token B in terms of A = `x / y` (spot).
After swapping `Δx` of token A for token B:
```
New y' = k / (x + Δx · (1 - fee))
Δy = y - y'   (tokens received)
```
Fee (default 0.3%) is taken from the input amount before applying the invariant.

This means:
- Price is always deterministic from pool state — no order book needed.
- Every swap changes the reserves and therefore the price.
- Price impact increases superlinearly with swap size relative to pool size.

#### Uniswap v3 — Concentrated Liquidity AMM (CLAMM)
Extends CPMM. Liquidity providers (LPs) specify a **price range** (expressed as ticks) in
which their capital is active. Outside that range, their capital earns no fees and provides
no liquidity.

Key state: `sqrt_price_x96` (the current pool price as a Q64.96 fixed-point number),
`current_tick`, `liquidity` (active liquidity at the current tick).

When a swap crosses a tick boundary, active liquidity changes — potentially dramatically.
This makes v3 much harder to simulate than v2.

Fee tiers: 0.01%, 0.05%, 0.30%, 1.00% per pool.

#### Curve — StableSwap invariant
For stablecoin pools (USDC/USDT, DAI/USDC). Uses a hybrid invariant between CPMM and
constant-sum to minimize slippage near the 1:1 peg.

#### Raydium (Solana)
Similar concentrated-liquidity design to Uniswap v3 but on Solana. Sub-cent gas fees (in SOL)
vs Ethereum's potentially significant gas costs — changes the economics of small swaps.

---

## 2. What a proper backtest requires

### 2.1 Pool state data (the equivalent of an order book)

The pool state **at the time of each event** is required to correctly compute price impact and
fill amounts. This is fundamentally different from an order book snapshot — the pool state is a
small, deterministic structure.

#### Uniswap v2 pool state

| Field | Type | Notes |
|---|---|---|
| `reserve_0` | u128 | Amount of token 0 in the pool |
| `reserve_1` | u128 | Amount of token 1 in the pool |
| `fee_bps` | u16 | Pool fee in basis points (e.g. 30 = 0.30%) |
| `block_number` | u64 | Block this state was observed |
| `block_timestamp` | i64 (ns UTC) | Block timestamp |

#### Uniswap v3 pool state

| Field | Type | Notes |
|---|---|---|
| `sqrt_price_x96` | u160 | Current price as Q64.96 fixed-point (√P × 2^96) |
| `current_tick` | i32 | Current tick index |
| `liquidity` | u128 | Active liquidity at current tick |
| `fee_bps` | u16 | Pool fee tier |
| `tick_spacing` | u16 | Minimum tick separation for this fee tier |
| `tick_data` | `Vec<TickEntry>` | Per-tick liquidity delta; needed for cross-tick swaps |
| `block_number` | u64 | |
| `block_timestamp` | i64 | |

`tick_data` is required if a simulated swap might cross a tick boundary (i.e. large swaps).
For small swaps relative to pool depth, only the current state is needed.

### 2.2 Swap history data

Individual historical swaps provide ground truth for verifying price impact models.

| Field | Notes |
|---|---|
| `sender` | Wallet address |
| `amount_0_in / amount_1_in` | Tokens sent to pool |
| `amount_0_out / amount_1_out` | Tokens received from pool |
| `sqrt_price_after` | Post-swap price (v3) |
| `gas_used` | Gas consumed (for gas cost modeling) |
| `gas_price_wei` | Gas price at execution time |
| `block_number`, `tx_index` | For ordering within a block |

### 2.3 Gas cost data

Gas costs are a **real P&L component** on EVM chains, not just a metadata detail.

| Field | Notes |
|---|---|
| `gas_price_wei` | Gwei price at block time |
| `base_fee_wei` | EIP-1559 base fee |
| `estimated_gas_units` | Router-estimated gas for a swap of given size/complexity |

For Solana, fees are in lamports and are typically negligible (~0.0005 SOL per transaction).
For Ethereum, gas can make small swaps uneconomical — a $50 swap might cost $20 in gas.

### 2.4 MEV exposure (optional but important for realistic simulation)

**MEV (Maximal Extractable Value)** refers to value extracted by block producers or
searchers by reordering, inserting, or front-running transactions. For AMM swaps, the
primary form is **sandwich attacks**: a searcher front-runs a detected swap, driving the
price against the victim, then back-runs to profit.

Sandwich attack effect: the victim receives fewer tokens than expected because their
transaction executes at a worse price.

This is difficult to model precisely without MEV-specific data, but the implication is:
- The `max_slippage_bps` parameter (slippage tolerance) sets the worst-case fill.
- Actual realized slippage in live trading is often higher than the model predicts due to MEV.
- The backtest should allow configuring an MEV tax (additional slippage percentage) for
  strategies that would be visible to searchers.

---

## 3. Data contract

### Required manifest (Engine B, v2-style)

```
REQUIRED:
  InstrumentStatic {
    pool_address,
    token_0: { address, symbol, decimals },
    token_1: { address, symbol, decimals },
    amm_variant: Uniswap_v2 | Uniswap_v3 | Curve | Raydium,
    fee_bps,
    chain_id,
    asset_class = DexPool
  }
  PoolState stream (at minimum: one snapshot per block that had a swap)

REQUIRED for v3:
  TickData snapshots (or tick-level delta stream)

OPTIONAL (improves realism):
  SwapHistory stream
  GasPriceHistory stream
```

### Payload variants used

| Payload | Description |
|---|---|
| `PoolState { reserve_0, reserve_1, fee_bps, … }` | v2 state |
| `PoolState { sqrt_price_x96, current_tick, liquidity, … }` | v3 state (same type, different fields populated) |
| `Trade { price, size, … }` | Historical swap (for ground-truth comparison) |
| `GasEvent { base_fee_wei, gas_price_wei }` | Per-block gas |

---

## 4. Engine behavior (Engine B)

The AMM engine **does not maintain an order book.** It maintains pool state and computes
fills deterministically from that state.

```
fill_price, amount_out = amm_engine.simulate_swap(
    pool_state,
    swap_direction,   # token_0_for_1 or token_1_for_0
    amount_in,
    max_slippage_bps
)
```

For v2: closed-form using the CPMM formula.
For v3: iterative tick-crossing simulation.

**Order types on Engine B:** only market swaps (swap_exact_in, swap_exact_out).
Limit orders, stop orders, and IOC orders do not exist on AMMs. The strategy contract
must enforce this — `ctx.submit(LimitOrder)` on an AMM instrument is a contract error.

After each simulated swap, the engine updates pool reserves (v2) or sqrt_price/liquidity (v3).

---

## 5. Performance metrics (extensions)

| Metric | Notes |
|---|---|
| Price impact | Slippage from initial spot price to actual fill price |
| Gas cost P&L | Total gas paid in USD equivalent over the period |
| MEV loss estimate | Estimated value lost to sandwich attacks (if MEV model configured) |
| LP fee income | For LP strategies: fees earned by providing liquidity |
| Impermanent loss | For LP strategies: divergence loss vs. holding the tokens |

---

## 6. Implications for system design

1. **Engine B cannot share the order-book matching logic with Engine A.** Price formation
   is categorically different; they share only the event envelope and the strategy interface.
2. **No limit orders.** The strategy contract capability check for AMM instruments must
   reject all order types except market swaps. This is enforced at contract level.
3. **Pool state cadence ≠ tick cadence.** A pool state snapshot is per-block; individual
   swaps within a block change reserves. If simulating intra-block ordering matters, swap
   history with `tx_index` is needed.
4. **Gas is a first-class P&L line.** Strategies profitable in notional terms can be
   loss-making after gas. The metrics contract must always include gas cost.
5. **v3 is significantly more complex to simulate than v2.** The tick-crossing logic and
   concentrated liquidity math require either full on-chain state (tick data) or a
   simplifying assumption (constant active liquidity for small swaps).

---

## 7. Sources

- arXiv 2410.09983: Backtesting Framework for Concentrated Liquidity Market Makers (Uniswap v3)
- arXiv 2309.13648: MEV costs on Uniswap ("Don't Let MEV Slip")
- AmberData blog: Developing and backtesting LP strategies on Uniswap v2
- arXiv 2103.12732: SoK — DEX with AMM protocols
- 0x.org: Measuring hidden DEX costs
