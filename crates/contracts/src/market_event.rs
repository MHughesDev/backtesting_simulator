use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::types::{EntityId, InstrumentId, Side, Timestamp, VenueId};

/// The required envelope for every market data event.
/// `ts_event` is the authoritative time source for look-ahead enforcement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketEvent {
    pub instrument_id: InstrumentId,
    /// Company/issuer/protocol/collection (mainly for exogenous + reference linkage)
    pub entity_id: Option<EntityId>,
    pub venue_id: VenueId,
    /// Nanoseconds UTC — when the thing happened (authoritative ordering field)
    pub ts_event: Timestamp,
    /// Nanoseconds UTC — when a strategy could first know it; defaults to ts_event
    pub ts_available: Option<Timestamp>,
    /// Wall-clock time received (for latency modeling; may equal ts_event)
    pub ts_recv: Timestamp,
    /// Per-instrument monotonically increasing sequence number
    pub seq: u64,
    /// Producing source/processor (reproducibility)
    pub source_id: Option<String>,
    /// Parser/model/processor version
    pub source_version: Option<String>,
    pub payload: MarketPayload,
}

/// Action type for order book deltas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BookAction {
    Add,
    Modify,
    Delete,
    Clear,
}

/// Auction type for auction events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuctionType {
    Opening,
    Closing,
    Halt,
    Ipo,
    Other(String),
}

/// Corporate action type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CorporateActionType {
    Dividend {
        amount: Decimal,
        ex_date: i64,
    },
    Split {
        ratio: Decimal,
        ex_date: i64,
    },
    SpinOff {
        new_instrument_id: String,
        ratio: Decimal,
    },
    Merger {
        acquirer_id: String,
        price: Option<Decimal>,
    },
    Delisting {
        effective_ts: i64,
    },
    Other(String),
}

/// Token event type for crypto assets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenEventType {
    Fork {
        new_token_symbol: String,
        ratio: Decimal,
    },
    Airdrop {
        amount: Decimal,
    },
    Burn {
        amount: Decimal,
    },
    Mint {
        amount: Decimal,
    },
    Other(String),
}

/// Trading session type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradingSessionType {
    PreMarket,
    Regular,
    AfterHours,
    Overnight,
    Other(String),
}

/// Trading status kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradingStatusKind {
    Open,
    Halted,
    Suspended,
    Closed,
    PreOpen,
    Auction,
    Other(String),
}

/// IV surface point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IVSurfacePoint {
    pub strike: Decimal,
    pub expiry_ts: Timestamp,
    pub iv: Decimal,
}

/// Pool reserve data for AMM pools (Engine B).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolReserveData {
    pub reserve_0: Decimal,
    pub reserve_1: Decimal,
    pub sqrt_price_x96: Option<Decimal>,
    pub current_tick: Option<i32>,
    pub liquidity: Option<Decimal>,
}

/// All payload variants. Engine A handles: Bar, Mark, Quote, Trade, BookDelta,
/// BookSnapshot, OrderBookOrderEvent, Funding, MarkUpdate, TradingSession,
/// TradingStatus, AuctionImbalance, AuctionResult, CorporateAction, TokenEvent,
/// BorrowRate, FeeScheduleUpdate, LiquidationEvent, OpenInterest, UniverseMembership,
/// Greeks, IVSurface, ExerciseEvent, InsuranceFundEvent, AutoDeleveragingEvent.
/// Other variants are included for completeness (Engines B-H).
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketPayload {
    // ── 2.1 Universal variants ─────────────────────────────────────────────
    /// Single reference price mark.
    Mark { price: Decimal },

    /// OHLCV bar.
    Bar {
        open: Decimal,
        high: Decimal,
        low: Decimal,
        close: Decimal,
        volume: Decimal,
        interval_secs: u64,
        adjusted: bool,
    },

    /// Best bid and offer (L1).
    Quote {
        bid: Decimal,
        bid_size: Decimal,
        ask: Decimal,
        ask_size: Decimal,
    },

    /// Last trade print.
    Trade {
        price: Decimal,
        size: Decimal,
        aggressor_side: Side,
        trade_id: Option<String>,
    },

    // ── 2.2 Order book (L2 aggregated depth) ─────────────────────────────
    /// Incremental order book update.
    BookDelta {
        side: Side,
        price: Decimal,
        new_size: Decimal,
        action: BookAction,
    },

    /// Full order book snapshot.
    BookSnapshot {
        bids: Vec<(Decimal, Decimal)>, // (price, size)
        asks: Vec<(Decimal, Decimal)>,
        depth: u8,
    },

    // ── 2.3 L3 / MBO per-order events ─────────────────────────────────────
    /// Per-order event (L3/MBO). Requires HasL3OrderBook.
    OrderBookOrderEvent {
        order_id: String,
        action: BookAction,
        side: Side,
        price: Decimal,
        size: Decimal,
        displayed_size: Decimal,
        priority_ts: Timestamp,
        sequence: u64,
    },

    // ── 2.4 Funding (perps) ────────────────────────────────────────────────
    /// Funding rate event. Requires HasFunding.
    Funding {
        rate: Decimal,
        mark_price: Decimal,
        index_price: Decimal,
        next_funding_ts: Timestamp,
    },

    /// Mark price update. Requires HasMarkPrice.
    MarkUpdate {
        mark_price: Decimal,
        index_price: Decimal,
    },

    // ── 2.5 Trading sessions ───────────────────────────────────────────────
    /// Trading session event. Requires HasSessions.
    TradingSession {
        session_type: TradingSessionType,
        open_ts: Timestamp,
        close_ts: Timestamp,
        liquidity_multiplier: Decimal,
    },

    // ── 2.6 Trading status ────────────────────────────────────────────────
    /// Trading status change. Requires HasTradingStatus.
    TradingStatus {
        status: TradingStatusKind,
        reason: Option<String>,
    },

    // ── 2.7 Auctions ──────────────────────────────────────────────────────
    /// Auction imbalance data. Requires HasAuction.
    AuctionImbalance {
        auction_type: AuctionType,
        paired_qty: Decimal,
        imbalance_qty: Decimal,
        imbalance_side: Side,
        indicative_price: Decimal,
        reference_price: Decimal,
    },

    /// Auction result. Requires HasAuction.
    AuctionResult {
        auction_type: AuctionType,
        official_price: Decimal,
        matched_volume: Decimal,
    },

    // ── 2.8 Corporate actions ──────────────────────────────────────────────
    /// Corporate action event. Requires HasCorporateActions.
    CorporateAction { action_type: CorporateActionType },

    // ── 2.9 Token events ──────────────────────────────────────────────────
    /// Token event (fork, airdrop, burn, mint). Requires HasTokenEvents.
    TokenEvent { event_type: TokenEventType },

    // ── 2.10 Short borrow ─────────────────────────────────────────────────
    /// Borrow rate stream. Requires HasBorrowRate.
    BorrowRate {
        borrow_rate_annualized: Decimal,
        short_availability: Decimal,
        hard_to_borrow: bool,
    },

    // ── 2.11 Fee schedule updates ─────────────────────────────────────────
    /// Dynamic fee schedule update. Requires HasFeeScheduleUpdates.
    FeeScheduleUpdate {
        venue_id: String,
        instrument_id: String,
        maker_bps: Decimal,
        taker_bps: Decimal,
        flat_fee: Option<Decimal>,
        tier_rules: Vec<(Decimal, Decimal, Decimal)>, // (min_volume, maker_bps, taker_bps)
        effective_ts: Timestamp,
    },

    // ── 2.12 Liquidations ─────────────────────────────────────────────────
    /// Liquidation event. Requires HasLiquidationStream.
    LiquidationEvent {
        instrument_id: String,
        side: Side,
        qty: Decimal,
        price: Decimal,
        bankruptcy_price: Decimal,
        mark_price: Decimal,
    },

    /// Insurance fund event.
    InsuranceFundEvent {
        venue_id: String,
        balance: Decimal,
        currency: String,
    },

    /// Auto-deleveraging event.
    AutoDeleveragingEvent {
        instrument_id: String,
        affected_side: Side,
        notional: Decimal,
    },

    // ── 2.13 Open interest ────────────────────────────────────────────────
    /// Open interest data. Requires HasOpenInterest.
    OpenInterest {
        open_interest: Decimal,
        volume: Decimal,
        settlement_price: Option<Decimal>,
    },

    // ── 2.14 Universe membership ──────────────────────────────────────────
    /// Universe membership change. Requires HasUniverseMembership.
    UniverseMembership {
        universe_id: String,
        instrument_id: String,
        added_ts: Option<Timestamp>,
        added_knowable_ts: Option<Timestamp>,
        removed_ts: Option<Timestamp>,
        removed_knowable_ts: Option<Timestamp>,
        reason: Option<String>,
    },

    // ── 2.15 Options / Greeks ─────────────────────────────────────────────
    /// Options Greeks. Requires HasGreeks.
    Greeks {
        iv: Decimal,
        delta: Decimal,
        gamma: Decimal,
        theta: Decimal,
        vega: Decimal,
        rho: Decimal,
    },

    /// Implied volatility surface. Requires HasIVSurface.
    IVSurface {
        points: Vec<IVSurfacePoint>,
        model: String,
    },

    /// Option exercise event. Requires HasEarlyExercise.
    ExerciseEvent {
        exercised_qty: Decimal,
        settlement_price: Decimal,
        early: bool,
    },

    // ── 2.16 AMM / DEX — Engine B stubs ─────────────────────────────────
    /// AMM pool state. Requires HasPoolReserves.
    PoolState {
        reserves: PoolReserveData,
        fee_growth_global_0: Option<Decimal>,
        fee_growth_global_1: Option<Decimal>,
    },

    /// AMM swap event. Requires HasSwapEvent.
    SwapEvent {
        amount_0_in: Decimal,
        amount_1_in: Decimal,
        amount_0_out: Decimal,
        amount_1_out: Decimal,
        sender: String,
        recipient: String,
    },

    /// Gas cost event. Requires HasGasCost.
    GasEvent {
        gas_used: u64,
        gas_price_gwei: Decimal,
        base_fee_gwei: Decimal,
    },

    // ── 2.17 NAV / ETF — Engine C stubs ─────────────────────────────────
    /// NAV event. Requires HasNAV.
    Nav {
        nav: Decimal,
        premium_discount: Decimal,
        inav: Option<Decimal>,
    },

    /// ETF holdings snapshot. Requires HasHoldings.
    HoldingsSnapshot {
        holdings: Vec<(String, Decimal, Decimal)>, // (instrument_id, qty, weight)
        cash: Decimal,
        effective_ts: Timestamp,
    },

    /// Creation/redemption basket. Requires HasCreationRedemption.
    CreationRedemptionBasket {
        basket: Vec<(String, Decimal)>, // (instrument_id, qty_per_unit)
        cash_component: Decimal,
        effective_ts: Timestamp,
    },

    // ── 2.18 Bonds / Fixed Income — Engine D stubs ───────────────────────
    /// Coupon event. Requires HasCoupon.
    Coupon {
        coupon_amount: Decimal,
        accrual_start_ts: Timestamp,
        accrual_end_ts: Timestamp,
        payment_ts: Timestamp,
    },

    /// Yield update. Requires HasYield.
    YieldUpdate {
        ytm: Decimal,
        spread_over_treasury: Decimal,
        credit_rating: Option<String>,
    },

    /// Yield curve snapshot. Requires HasYield.
    YieldCurve {
        tenors_years: Vec<f64>,
        rates: Vec<Decimal>,
        curve_id: String,
        effective_ts: Timestamp,
    },

    /// Credit spread event. Requires HasCreditRisk.
    CreditSpread {
        issuer_id: String,
        spread_bps: Decimal,
        cds_rate: Option<Decimal>,
    },

    /// Credit rating change. Requires HasCreditRisk.
    CreditRatingEvent {
        issuer_id: String,
        old_rating: Option<String>,
        new_rating: String,
        outlook: Option<String>,
    },

    // ── 2.19 FX swap rates ───────────────────────────────────────────────
    /// FX overnight swap rates. Requires HasSwapRates.
    SwapRate {
        swap_long: Decimal,
        swap_short: Decimal,
        effective_date: i64,
        triple_swap_day: Option<String>,
    },

    // ── 2.20 Roll schedule ───────────────────────────────────────────────
    /// Contract roll schedule. Requires HasRollSchedule.
    RollSchedule {
        from_contract: String,
        to_contract: String,
        roll_ts: Timestamp,
        roll_method: String,
    },

    // ── 2.21 Marketplace — Engine G stubs ────────────────────────────────
    /// Listing event. Requires IsUnique or IsFungibleSKU.
    ListingEvent {
        listing_id: String,
        item_id: String,
        ask_price: Decimal,
        seller_id: String,
        listed_ts: Timestamp,
    },

    /// Comparable mark event. Requires HasFloor.
    ComparableMarkEvent {
        category_id: String,
        floor_price: Decimal,
        sample_size: u32,
        effective_ts: Timestamp,
    },

    /// Offer event. Requires HasOffer.
    OfferEvent {
        listing_id: String,
        bidder_id: String,
        offer_price: Decimal,
        expiry_ts: Option<Timestamp>,
    },

    /// Auction bid event. Requires HasTimedAuction.
    AuctionBidEvent {
        listing_id: String,
        bidder_id: String,
        bid_price: Decimal,
        bid_ts: Timestamp,
    },

    /// Auction close event. Requires HasTimedAuction.
    AuctionCloseEvent {
        listing_id: String,
        winner_id: Option<String>,
        final_price: Option<Decimal>,
        close_ts: Timestamp,
    },

    // ── 2.22 Prediction markets — Engine H stubs ─────────────────────────
    /// Resolution event. Requires HasResolution.
    Resolution {
        market_id: String,
        outcome: String,
        oracle_id: String,
        resolution_ts: Timestamp,
        dispute_occurred: bool,
    },

    /// Market lifecycle event. Requires HasResolution.
    MarketLifecycleEvent {
        market_id: String,
        status: String,
        effective_ts: Timestamp,
    },

    /// Oracle event. Requires HasOracleRisk.
    OracleEvent {
        oracle_id: String,
        reported_value: Decimal,
        confidence: Option<Decimal>,
        ts: Timestamp,
    },

    // ── 2.23 Derived / synthetic ─────────────────────────────────────────
    /// Derived OHLCV bar (resampled from higher frequency data).
    DerivedBar {
        open: Decimal,
        high: Decimal,
        low: Decimal,
        close: Decimal,
        volume: Decimal,
        interval_secs: u64,
        adjusted: bool,
        source_class: String,
        tick_count: u64,
    },
}
