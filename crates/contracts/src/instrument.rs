use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::capability::CapabilityFlags;
use crate::types::{
    Address, AmmVariant, CurrencyCode, DayCountConvention, ExerciseStyle, FeeTier, InstrumentId,
    ItemId, OptionType, OracleType, PerpType, SettlementType, SkuId, TokenInfo, VenueId,
};

/// The single routing field that determines which engine handles an instrument.
/// Never branch on AssetClass — use CapabilityFlags instead (ADR-0003).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PriceFormation {
    /// Engine A — Order Book (stocks, ETFs, CEX crypto, futures, perps, FX)
    Clob,
    /// Engine B — AMM (DEX pools: Uniswap, Raydium, Curve)
    Amm,
    /// Engine C — NAV (mutual funds, ETFs with NAV-based execution)
    Nav,
    /// Engine D — Cash Flow / Dealer (bonds, CDs)
    Dealer,
    /// Engine E — Derivatives (options, warrants)
    Chain,
    /// Engine F — Synthetic / OTC (CFDs, swaps, structured products)
    Otc,
    /// Engine G — Marketplace (NFTs, collectibles, listing-based)
    Marketplace,
    /// Engine H — Event Resolution (prediction markets)
    Oracle,
}

/// Informational classification. Do NOT branch on this — use CapabilityFlags (ADR-0003).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetClass {
    Equity,
    Etf,
    CryptoSpot,
    DexPool,
    Future,
    Perpetual,
    Option,
    Bond,
    Fx,
    ListingAsset,
    Nft,
    PredictionMarket,
    Synthetic,
    Other(String),
}

/// Complete instrument definition. Provides identity, routing, capabilities, and
/// all optional parameters gated by capability flags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instrument {
    // ── identity ──────────────────────────────────────────────────────────────
    /// Canonical identifier: e.g. "AAPL@nasdaq.equity"
    pub id: InstrumentId,
    /// Display symbol: e.g. "AAPL"
    pub symbol: String,
    pub exchange: VenueId,
    /// Quote / P&L currency
    pub currency: CurrencyCode,
    /// Informational only — do not branch on this
    pub asset_class: AssetClass,
    /// Entity key for issuer-level reference data (bonds, credit)
    pub issuer_id: Option<String>,
    /// Broader entity key (company/protocol/collection) for exogenous signals
    pub entity_id: Option<String>,

    // ── routing ───────────────────────────────────────────────────────────────
    /// The ONLY field used to select the engine.
    pub price_formation: PriceFormation,

    // ── capabilities ──────────────────────────────────────────────────────────
    /// Bitmask declaring which features/payloads are valid for this instrument.
    pub capabilities: CapabilityFlags,

    // ── quote parameters ──────────────────────────────────────────────────────
    pub tick_size: Decimal,
    /// Minimum order quantity
    pub lot_size: Decimal,
    /// 1.0 for stocks, 100 for US equity options, 50 for ES futures
    pub contract_multiplier: Decimal,

    // ── settlement ────────────────────────────────────────────────────────────
    pub settlement: SettlementType,

    // ── HasExpiry / HasMaturity ───────────────────────────────────────────────
    pub expiry_date: Option<chrono::NaiveDate>,
    pub maturity_date: Option<chrono::NaiveDate>,

    // ── HasExpiry — options ───────────────────────────────────────────────────
    pub strike: Option<Decimal>,
    pub option_type: Option<OptionType>,
    pub exercise_style: Option<ExerciseStyle>,

    // ── IsLeveraged ───────────────────────────────────────────────────────────
    pub initial_margin_rate: Option<Decimal>,
    pub maintenance_margin_rate: Option<Decimal>,

    // ── HasFunding — perps ────────────────────────────────────────────────────
    pub funding_interval_hours: Option<u8>,
    pub perp_type: Option<PerpType>,

    // ── HasPoolReserves — AMM ─────────────────────────────────────────────────
    pub amm_variant: Option<AmmVariant>,
    pub pool_address: Option<Address>,
    pub token_0: Option<TokenInfo>,
    pub token_1: Option<TokenInfo>,
    pub fee_bps: Option<u16>,

    // ── HasNAV ────────────────────────────────────────────────────────────────
    pub leverage_factor: Option<f64>,
    pub daily_reset: Option<bool>,

    // ── HasCoupon — bonds ─────────────────────────────────────────────────────
    pub coupon_rate: Option<Decimal>,
    pub coupon_frequency: Option<u8>,
    pub day_count: Option<DayCountConvention>,
    pub par_value: Option<Decimal>,
    pub credit_rating: Option<String>,

    // ── IsUnique — NFTs ───────────────────────────────────────────────────────
    pub category_id: Option<String>,
    pub item_id: Option<ItemId>,
    pub chain_id: Option<String>,

    // ── IsFungibleSKU ─────────────────────────────────────────────────────────
    pub sku_id: Option<SkuId>,
    pub condition_tier: Option<String>,

    // ── HasMakerTakerFees ─────────────────────────────────────────────────────
    pub fee_schedule: Option<Vec<FeeTier>>,

    // ── HasSwapRates — FX ─────────────────────────────────────────────────────
    pub base_currency: Option<CurrencyCode>,
    pub pip_size: Option<Decimal>,

    // ── IsBinary — prediction markets ────────────────────────────────────────
    pub question: Option<String>,
    pub resolution_criteria: Option<String>,
    pub oracle_type: Option<OracleType>,
}
