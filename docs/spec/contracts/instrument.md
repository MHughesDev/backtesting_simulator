# Contract Spec: Instrument

The Instrument Contract defines the **identity, routing, and capability declaration** for
every tradable in the system. It is not market data — it is static (or slowly-changing)
metadata the caller provides once per instrument per run.

---

## The routing field: `price_formation`

This is the single field that selects the engine. Every other classification is informational.

| Value | Engine | Examples |
|---|---|---|
| `CLOB` | A — Order Book | Stocks, ETFs, CEX crypto, futures, perps, FX |
| `AMM` | B — AMM | DEX pools (Uniswap, Raydium, Curve) |
| `NAV` | C — NAV | Mutual funds (execution-only, no CLOB) |
| `DEALER` | D — Cash Flow | Bonds, CDs (yield-derived, dealer market) |
| `CHAIN` | E — Derivatives | Options, warrants |
| `OTC` | F — Synthetic | CFDs, swaps, structured products |
| `MARKETPLACE` | G — Marketplace | NFTs |
| `ORACLE` | H — Event Resolution | Prediction markets |

---

## Capability flags

A bitmask declared by each instrument. Engines and strategies check capabilities before
accessing capability-gated data or order types.

| Flag | Meaning | Used by |
|---|---|---|
| `HasOrderBook` | CLOB data (bids, asks, book depth) is valid | Engine A |
| `HasCorporateActions` | Corporate action events may be emitted | Engine A (equities) |
| `HasShortBorrow` | Borrow rate data is valid; short positions accrue cost | Engine A |
| `HasSessions` | Exchange sessions (pre/regular/after-hours) apply | Engine A |
| `HasMakerTakerFees` | Maker/taker fee schedule applies | Engine A (crypto) |
| `HasTokenEvents` | Fork, airdrop, burn events may be emitted | Engine A (crypto) |
| `HasNAV` | NAV and premium/discount data is valid | Engine C |
| `HasBasket` | Holdings/basket composition data is valid | Engine C |
| `HasDailyReset` | Leveraged/inverse ETF daily reset applies | Engine C |
| `HasCoupon` | Coupon schedule data is valid | Engine D |
| `HasYield` | Yield-based pricing; accrued interest tracked | Engine D |
| `HasMaturity` | Contract has a fixed end date | Engine D, A (futures) |
| `HasCreditRisk` | Credit rating and spread data is valid | Engine D |
| `HasExpiry` | Contract expires on a fixed date (futures, options) | Engine A, E |
| `HasOpenInterest` | Open interest data is valid | Engine A (futures) |
| `HasRollSchedule` | Roll schedule for continuous contract construction | Engine A (futures) |
| `HasFunding` | Funding rate events are emitted | Engine A (perps) |
| `HasMarkPrice` | Mark price (separate from last trade) is valid | Engine A (perps, deriv.) |
| `HasLiquidation` | Position may be liquidated at maintenance margin | Engine A, E |
| `IsLeveraged` | Margin is used; initial/maintenance margin rates apply | Engine A, E |
| `HasGreeks` | Greeks (delta, gamma, theta, vega, rho) are valid | Engine E |
| `HasIVSurface` | Implied volatility surface data is valid | Engine E |
| `HasEarlyExercise` | American-style exercise; early exercise may occur | Engine E |
| `HasSwapRates` | Overnight swap/rollover rates apply | Engine A (FX) |
| `HasSessionLiquidity` | Spread varies by trading session | Engine A (FX) |
| `HasPoolReserves` | Pool reserve data is valid | Engine B |
| `HasConcentratedLiquidity` | Tick-based concentrated liquidity (v3-style) | Engine B |
| `HasGasCost` | Gas costs are a real P&L component | Engine B, G |
| `IsUnique` | Non-fungible; position tracked by token ID, not quantity | Engine G |
| `HasRarity` | Rarity traits and scores are valid | Engine G |
| `HasFloor` | Collection floor price is a valid mark | Engine G |
| `IsIlliquid` | Sparse data; no continuous price; gap interpolation not valid | Engine G |
| `IsBinary` | Resolves to $1 or $0 | Engine H |
| `HasResolution` | Resolution events will be emitted | Engine H |
| `HasOracleRisk` | Oracle dispute possible | Engine H |
| `HasProbabilityPrice` | Price = probability (bounded 0–1) | Engine H |

---

## Full instrument structure

```
Instrument {
  // ── identity ───────────────────────────────────────────────────
  id:                InstrumentId,   // canonical: "AAPL@nasdaq.equity"
  symbol:            String,         // display symbol: "AAPL"
  exchange:          VenueId,
  currency:          CurrencyCode,   // quote / P&L currency
  asset_class:       AssetClass,     // informational classification

  // ── routing (the only thing that matters for engine selection) ─
  price_formation:   PriceFormation, // CLOB | AMM | NAV | DEALER | CHAIN | OTC | MARKETPLACE | ORACLE

  // ── capabilities ───────────────────────────────────────────────
  capabilities:      CapabilityFlags,   // bitmask of flags above

  // ── quote parameters ───────────────────────────────────────────
  tick_size:         Decimal,
  lot_size:          Decimal,        // minimum order quantity
  contract_multiplier: Decimal,      // 1.0 for stocks, 100 for US equity options, 50 for ES

  // ── settlement ─────────────────────────────────────────────────
  settlement:        SettlementType, // Cash | Physical | OnChain | None

  // ── capability-gated static metadata ───────────────────────────
  // Only fields relevant to the instrument's capabilities are set;
  // the rest are absent/null.

  // HasExpiry / HasMaturity
  expiry_date:       Option<Date>,
  maturity_date:     Option<Date>,   // bonds use maturity_date

  // HasExpiry (options only)
  strike:            Option<Decimal>,
  option_type:       Option<OptionType>,       // Call | Put
  exercise_style:    Option<ExerciseStyle>,    // American | European

  // IsLeveraged
  initial_margin_rate:      Option<Decimal>,
  maintenance_margin_rate:  Option<Decimal>,

  // HasFunding (perps)
  funding_interval_hours:   Option<u8>,
  perp_type:                Option<PerpType>,  // Linear | Inverse

  // HasPoolReserves (AMM)
  amm_variant:       Option<AmmVariant>,       // UniswapV2 | UniswapV3 | Curve | Raydium
  pool_address:      Option<Address>,
  token_0:           Option<TokenInfo>,
  token_1:           Option<TokenInfo>,
  fee_bps:           Option<u16>,

  // HasNAV (ETFs/funds)
  leverage_factor:   Option<f64>,   // 1.0 standard, ±N leveraged
  daily_reset:       Option<bool>,

  // HasCoupon (bonds)
  coupon_rate:       Option<Decimal>,
  coupon_frequency:  Option<u8>,
  day_count:         Option<DayCountConvention>,
  par_value:         Option<Decimal>,
  credit_rating:     Option<String>,

  // IsUnique (NFTs)
  collection_address: Option<Address>,
  token_id:           Option<TokenId>,

  // HasMakerTakerFees (crypto CEX)
  fee_schedule:      Option<Vec<FeeTier>>,

  // HasSwapRates (FX)
  base_currency:     Option<CurrencyCode>,   // for FX pairs; quote_currency is top-level currency
  pip_size:          Option<Decimal>,

  // IsBinary (prediction markets)
  question:          Option<String>,
  resolution_criteria: Option<String>,
  oracle_type:       Option<OracleType>,
}
```

---

## Required-data manifest

The Instrument carries (or the caller declares separately) a `DataManifest` that specifies:

```
DataManifest {
  instrument_id:    InstrumentId,
  engine:           EngineType,          // must match price_formation
  required:         Vec<PayloadClass>,   // runs rejected if absent
  optional:         Vec<PayloadClass>,   // improve fidelity if present
  provide_or_derive: Vec<PayloadClass>,  // caller may provide; engine derives if absent
}
```

Each asset spec in `docs/spec/assets/` defines the canonical data manifest for that asset
class. The manifest is validated at run start; a contract violation produces a typed
`ManifestViolation` error with the specific missing payload class named.

---

## Asset class taxonomy (informational)

`asset_class` is informational metadata used for display, grouping, and reporting. It does
not influence engine behavior. The sole routing field is `price_formation`.

```
AssetClass =
  | Equity
  | ETF
  | CryptoSpot
  | DexPool
  | Future
  | Perpetual
  | Option
  | Bond
  | FX
  | NFT
  | PredictionMarket
  | Synthetic
  | Other(String)
```
