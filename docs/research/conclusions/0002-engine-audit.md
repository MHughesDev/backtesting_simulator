# Engine Audit: Real-World Trading Mechanics

**Research ID:** 0002
**Date:** 2026-06-05
**Source:** [sources/0002-asset-trading-mechanics.md](../sources/0002-asset-trading-mechanics.md)

Each engine is audited against how its target asset classes are actually traded and what data a
platform receives. Findings are classified as ✅ (confirmed correct), ⚠️ (gap or clarification
needed), or 🔧 (spec updated this session).

---

## Engine A — Order Book (CLOB)

Routes: equities, ETFs (execution), CEX crypto spot, futures, perpetuals, FX, listed options.

### ✅ Confirmed correct

- CLOB with price-time priority matches all targeted venues (NYSE, NASDAQ, CEX, CME, dYdX).
- Data-fidelity ladder (L3 → L2 → L1 → Bar) exactly matches the range of real-world data availability.
- Market / limit / stop / stop-limit order types confirmed across all CLOB venues.
- `HasSessions` correctly gates orders and expires DAY orders; equity pre/regular/after-hours
  sessions are real and materially affect liquidity and spreads.
- `HasFunding` fires at the scheduled funding timestamp (every 8h); the Binance formula
  (`clamp(premium_index + clamp(interest_rate − premium_index, ±0.05%), ±cap)`) is correctly
  specified.
- `HasMarkPrice` is correct; mark price (not last trade) is the reference for liquidation and
  unrealized P&L on perpetuals.
- `HasExpiry` + `HasRollSchedule` correctly handles futures lifecycle.
- `HasSwapRates` with triple on Wednesdays matches FX broker behavior.
- `HasShortBorrow` with daily accrual matches equity borrow desk practice.
- `HasCorporateActions` / `HasTokenEvents` processed before price matching at that timestamp —
  correct event ordering.
- Inverse perp P&L formula in base currency confirmed.
- Maker/taker fee tiers confirmed (Binance regular: 0.10%/0.10%; VIP 5: 0.02%; equity ECN
  rebate model).

### ⚠️ Gap 1: Adjusted vs. unadjusted price series (equities and futures)

**Finding:** Equities and futures *require* two parallel price series — an adjusted series
(backward-adjusted for corporate actions / roll gaps) used for signal computation, and an
unadjusted series (actual traded prices) used for fills and P&L. Merging them silently
distorts both signals and P&L. This is a universal backtesting correctness requirement; it is
not currently mentioned anywhere in the Engine A spec or contracts.

**Impact:** High. A strategy computing RSI on adjusted prices but filling at unadjusted prices
will silently produce incorrect returns. A strategy using only unadjusted prices will see
artificial gaps at corporate action / roll dates.

**Required action:** See §A of spec updates below.

### ⚠️ Gap 2: Continuous contract construction method

**Finding:** `HasRollSchedule` specifies *when* to roll but not *how* to construct the
continuous series for signal computation. Three common methods exist (Panama Canal /
back-adjusted, proportional, unadjusted stitch) with different accuracy/level-preservation
tradeoffs. The method must be a first-class config parameter; different strategies require
different methods.

**Impact:** Medium. Without this, signal computation on futures strategies is undefined in the
spec.

**Required action:** See §A of spec updates below.

### ⚠️ Gap 3: Session-dependent slippage (FX)

**Finding:** FX spreads widen 3–10× outside the London–NY overlap session. The slippage
heuristic in §5.4 uses a single calibrated curve; it does not adjust for session. Off-hours
FX strategies will be systematically over-optimistic.

**Impact:** Medium for off-hours FX strategies; low for equities and crypto.

**Required action:** Note in Engine A §5.4 that the slippage model should accept a
session-aware spread multiplier for FX instruments. Added as an open item.

### ⚠️ Gap 4: Survivorship bias disclosure

**Finding:** If the run universe contains only currently-listed securities (a common default),
results are survivorship-biased. The output contract should carry a flag that a consuming
platform can surface to the analyst.

**Impact:** Low (engine concern) / High (strategy conclusions). Not a simulation error but a
disclosure gap.

**Required action:** See §A of spec updates below.

---

## Engine B — AMM

Routes: DEX pools (Uniswap v2/v3, Curve, Raydium, other pool-based venues).

### ✅ Confirmed correct

- No order book; every trade is a market swap against the pool invariant — matches all AMM
  venues exactly.
- v2 CPMM formula (`Δout = R_out·Δin_eff / (R_in + Δin_eff)`) is standard and correct.
- v3 CLAMM tick-crossing loop matches the Uniswap v3 whitepaper.
- Curve StableSwap invariant structure (Newton-solve for `D`, then solve output balance) is
  correct.
- Slippage tolerance = `max_slippage_bps` models the on-chain `minAmountOut` guard exactly;
  exceed → revert with no state change.
- Gas as a first-class P&L line confirmed; Ethereum gas can exceed trade value on small swaps.
  Solana gas (~$0.00025) is negligible — the instrument's chain field resolves this.
- MEV/sandwich as an optional configurable model is the right design; it cannot be recovered
  from historical pool state alone.
- Working-copy pattern (apply hypothetical impact; reset on next observed `PoolState`) is
  correct given that the real chain never included our trade.
- `swap_exact_in` / `swap_exact_out` as the only order types confirmed (no limit/stop on AMMs).

### ⚠️ Gap 5: EVM revert gas (open item confirmation)

**Finding:** On EVM chains, a reverted transaction (e.g., slippage exceeded) still consumes
the gas used up to the revert point. This is in the open items section but should be first-class
for strategies with non-trivial revert rates (e.g., aggressive slippage tolerances).

**Impact:** Low in most backtests; material for gas-optimized DeFi strategies.

**Required action:** Promote from open item to a named capability flag `HasRevertGas` (off by
default since most backtests don't need this fidelity). Added to open items with this framing.

### ✅ No other gaps found

The v3 approximate mode (constant-L assumption when tick data absent) and its fidelity flag are
correct and match what is practically available in data.

---

## Engine C — NAV

Routes: mutual funds (execution); ETFs (valuation layer alongside Engine A).

### ✅ Confirmed correct

- Forward pricing (fill at *next* struck NAV, not current) is the defining mutual-fund mechanic
  and is correctly specified.
- Daily reset formula for leveraged/inverse ETFs reproduces volatility decay correctly.
- Dual-role design (execution engine for mutual funds; valuation layer for ETFs) is architecturally
  correct; an ETF routes to Engine A for fills and Engine C for NAV/premium-discount tracking.
- iNAV as intraday signal only, never used as fill price, is correct.
- Premium/discount surface for mean-reversion strategies is correct.
- Expense ratio accrued daily (ER/252) confirmed.

### ✅ No gaps found

Engine C is fully consistent with how mutual funds and ETFs actually operate.

---

## Engine D — Cash Flow

Routes: bonds, treasuries, municipals, corporates, CDs, MBS.

### ✅ Confirmed correct

- Bond PV formula and YTM root-finding are standard and correct.
- Accrued interest formula with day-count conventions (ACT/ACT, 30/360, ACT/360) is correct;
  the day-count convention field is mandatory.
- Dirty price = clean + accrued; fills use dirty price — confirmed correct.
- Dealer fill model (dirty price ± half-spread per liquidity tier) matches OTC bond market reality.
- Duration, modified duration, DV01, convexity — provide-or-derive is correct.
- Coupon schedule as the event stream (credits cash on payment date) is correct.
- Credit rating change → credit spread reprice confirmed.
- MBS prepayment behind a `CPR/PSA` model is the right design.

### ⚠️ Gap 6: Repo financing cost for long bond positions

**Finding:** Leveraged bond strategies (long bonds financed via repo) incur a daily financing
cost analogous to perpetual funding rates. The current spec accrues coupon income and models
short-borrow cost via `HasShortBorrow` (Engine A), but does not address the *repo rate* cost
for a leveraged long bond position. This is a real cost at ~4–5% annualized (varies with Fed
rate), and is the primary carry cost for a leveraged fixed-income portfolio.

**Impact:** Medium for levered bond strategies; zero for unlevered buy-and-hold.

**Required action:** See §D of spec updates below.

### ⚠️ Gap 7: Roll-down return as a named P&L component

**Finding:** As a bond approaches maturity, its yield converges to the curve → price appreciation
if the yield curve is upward-sloping ("roll-down return"). This is a distinct source of return
separate from coupon income and yield-change P&L. Metrics and strategy authors need it labeled.

**Impact:** Low for the engine (it falls out of repricing naturally); medium for metrics
attribution clarity.

**Required action:** Note in Engine D output section that roll-down return is surfaced via the
mark series as the daily mark-to-market change (net of coupon accrual), not as a separate
computed field. Clarification only.

---

## Engine E — Derivatives

Routes: options and warrants; derivative valuation for futures/perps.

### ✅ Confirmed correct

- BSM formula for European options is standard and correct.
- Binomial/BAW for American options is the right choice; early-exercise optimality check per
  event is correct.
- IV surface lookup (bilinear interpolation in log-moneyness × T space) matches OptionMetrics
  industry standard.
- American early-exercise triggers (deep-ITM puts, calls before ex-dividend) confirmed correct.
- Assignment logic (short position assigned when model boundary crossed) confirmed.
- Cash vs. physical settlement at expiry matches exchange rules (VIX/index = cash; equity = physical).
- Contract multiplier (100 shares per US equity option) confirmed; must scale notional and P&L.
- Margin for short options; no margin for long premium — confirmed.
- `surface_type` ∈ {AbsoluteStrike, DeltaNormalized} confirmed as the two industry conventions.

### ⚠️ Gap 8: OHLCV-only path accuracy disclosure

**Finding:** OHLCV-only backtesting for options is widely recognized as producing heavily
distorted results. The spec notes the IV surface as mandatory, but doesn't explicitly warn that
fills derived from OHLCV alone (absent IV surface or option quotes) are low-accuracy. This
should be a named fidelity level in the fill model section.

**Impact:** Low (IV surface is already mandatory in the data manifest); worth documenting for
operator guidance.

**Required action:** See §E of spec updates below.

### ✅ No other significant gaps found

Engine E is the most thoroughly specified engine; its mechanics closely match how options are
actually priced and traded at real venues.

---

## Engine F — Synthetic / OTC

Routes: CFDs, swaps, structured notes, barrier notes, convertibles, autocalls.

### ✅ Confirmed correct

- Payoff-component model (pure, deterministic function registered in component registry) is the
  right architecture for unbounded structured product variety.
- OTC bilateral fill at mark ± counterparty spread matches how these products actually trade.
- Barrier state machine (knock-in/knock-out in ts_event order) is correct.
- CFD overnight financing accrual via Account matches broker practice.
- Swap leg settlement on schedule confirmed.
- Convertible bond as composition of Engine D (bond floor) + Engine E (conversion option) is
  architecturally correct.

### ✅ No gaps found

Engine F is primarily an extensibility framework; the research did not surface concrete mechanics
that are missing. Specific structured product types (e.g., retail structured notes) are handled
via the payoff-component registry, not by extending the engine itself.

---

## Engine G — Marketplace (NFT)

Routes: NFTs, digital collectibles.

### ✅ Confirmed correct

- No order book; only listing/sales event stream — confirmed as the only data that exists.
- Buy fills only against observed listings at-or-before the decision timestamp — correct; cannot
  invent fills.
- Sell fills on later observed comparable sales (conservative default) — correct; this is the
  only honest model without a demand curve.
- Floor price as the mark for unrealized P&L — the right proxy despite its limitations.
- Positions keyed by `{collection_address, token_id}` — correct; NFTs are non-fungible.
- Gas as first-class P&L line — confirmed material for Ethereum; negligible for Solana.
- Marketplace fee + creator royalty as separate P&L lines — confirmed.
- Rarity methodology caller-provided — correct; no universal methodology exists.
- Uncertainty disclosure mandatory — confirmed; sparse liquidity + illiquidity makes results
  inherently high-variance.
- No Bar payloads for `IsUnique` instruments — confirmed; only `Mark` and `NftEvent` are valid.

### ✅ No gaps found

Engine G correctly captures the mechanics of NFT marketplace trading. The conservative fill
model (observed comparable sale) is the right default for a simulation that cannot invent demand.

---

## Engine H — Event Resolution

Routes: prediction markets, binary event contracts.

### ✅ Confirmed correct

- Lifecycle (Active → Locked → Resolved → Settled) matches Polymarket and Kalshi behavior.
- Fills rejected once Locked is correct; real prediction markets stop trading when the event
  has passed and oracle window opens.
- Binary payoff (YES = $1, NO = $0) confirmed.
- Price bounded (0.01, 0.99) until resolution confirmed; no trades at exactly $0 or $1 pre-resolution.
- Oracle dispute window and incorrect-resolution modeling confirmed as a real risk factor.
- Brier score as primary calibration metric confirmed as industry standard for prediction-market
  strategy evaluation.
- Resolution is an event, not a date — confirmed; timing is uncertain and data-driven.

### ⚠️ Gap 9: Liquidity dry-up near resolution

**Finding:** Prediction market volumes drop sharply as the resolution event approaches; the
book becomes one-sided and effective spreads widen substantially. The spec does not model this.
For strategies that hold through the resolution window, this is material.

**Impact:** Low for most strategies (most exits are before lock); medium for strategies holding
through final days.

**Required action:** Note in Engine H §5 (Resolution timing uncertainty) that the order book
becomes materially illiquid near the lock date and that fill model accuracy degrades.

### ⚠️ Gap 10: CLOB extension is confirmed real-world behavior

**Finding:** §10 of the Engine H spec lists "promote limit orders on YES/NO book to first-class"
as an open item. Research confirms that Polymarket runs a full CLOB for major markets (US
elections, Fed meetings), and Kalshi is an SEC-regulated exchange with standard CLOB mechanics.
This is not a fringe extension — for backtesting liquid prediction markets it is necessary.

**Impact:** Medium. Without limit orders, strategies on Kalshi or high-liquidity Polymarket
markets cannot be simulated realistically.

**Required action:** See §H of spec updates below.

---

## Cross-cutting findings

### CC-1: Two price series is an architectural concern, not just Engine A's

The adjusted/unadjusted distinction applies to:
- Equities: splits, dividends
- Futures: roll gaps between contracts

This means the **contracts/market-data contract** should explicitly define both series types,
and the **instrument contract** should indicate which series are available. Engine A consumes
unadjusted for fills; the strategy's feature pipeline consumes adjusted (continuous) for signals.
This is captured in the spec updates below.

### CC-2: Gas as a P&L line is chain-specific

Engine B already handles this. Engine G mentions gas for NFTs. The instrument's `chain` field
(defined in the instrument contract) determines which gas model applies. No change needed; the
design is correct.

### CC-3: Funding rate P&L separation

Engine A emits funding payments as auxiliary records tagged `funding`. Engine D emits coupons
as `coupon`. Engine B emits gas as a P&L line. These separation disciplines are correct and
consistent.

---

## Summary: spec changes applied this session

| Engine | Change |
|---|---|
| **Engine A** | + Two price series (adjusted/unadjusted) in §3 and §9; + continuous series construction method in `HasRollSchedule`; + session-aware slippage open item; + survivorship bias flag in §7 output |
| **Engine B** | + `HasRevertGas` framing in §9 open items |
| **Engine D** | + Repo financing cost in §10 open items; + roll-down return clarification in §8 |
| **Engine E** | + OHLCV-only accuracy warning as named fidelity level in §5 |
| **Engine H** | + Liquidity dry-up note in §5; + CLOB promoted from minor open item to first-class extension note |

Engines C, F, G required no spec changes.
