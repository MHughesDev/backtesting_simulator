use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// Newtype aliases — these are type aliases, not newtypes, for ergonomics
pub type InstrumentId = String;
pub type VenueId = String;
pub type CurrencyCode = String;
pub type EntityId = String;
pub type ListingId = String;
pub type SkuId = String;
pub type ItemId = String;
pub type Address = String;

/// Nanoseconds since Unix epoch (UTC).
pub type Timestamp = i64;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SettlementType {
    Cash,
    Physical,
    OnChain,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptionType {
    Call,
    Put,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExerciseStyle {
    American,
    European,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerpType {
    Linear,
    Inverse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AmmVariant {
    UniswapV2,
    UniswapV3,
    Curve,
    Raydium,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OracleType {
    Centralized,
    Decentralized,
    ChainlinkAggregator,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DayCountConvention {
    ActAct,
    Dc30_360,
    ActDc360,
    ActDc365,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeTier {
    pub min_volume: Decimal,
    pub maker_bps: Decimal,
    pub taker_bps: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub address: Address,
    pub symbol: String,
    pub decimals: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineType {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
}
