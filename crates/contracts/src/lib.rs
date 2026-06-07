// contracts — zero intra-workspace dependencies (ADR-0012)
// This crate defines all shared types, traits, and error types.

pub mod capability;
pub mod instrument;
pub mod manifest;
pub mod market_event;
pub mod ports;
pub mod trade;
pub mod types;

// Re-export commonly used items
pub use capability::CapabilityFlags;
pub use instrument::{AssetClass, Instrument, PriceFormation};
pub use manifest::{DataManifest, FidelityWarning, ResolutionConflict, SimError};
pub use market_event::{
    AuctionType, BookAction, CorporateActionType, MarketEvent, MarketPayload, TokenEventType,
    TradingSessionType, TradingStatusKind,
};
pub use ports::{
    Account, ContextBundle, Model, ModelArtifact, ModelDirection, ModelOutput, TrainConfig,
    Trainer, TrainingDataWindow,
};
pub use trade::{
    FidelityLevel, Fill, Order, OrderResult, OrderType, Position, TimeInForce, TradeRecord,
};
pub use types::{
    AmmVariant, CurrencyCode, DayCountConvention, EngineType, EntityId, ExerciseStyle, FeeTier,
    InstrumentId, ItemId, ListingId, OptionType, OracleType, PerpType, SettlementType, Side, SkuId,
    Timestamp, TokenInfo, VenueId,
};
