use bitflags::bitflags;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

bitflags! {
    /// Capability flags for instruments. Each flag gates payload variants,
    /// order types, and engine mechanics. Never branch on AssetClass — use
    /// these flags instead (ADR-0003).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct CapabilityFlags: u64 {
        // Engine A — Order Book / CLOB
        const HasOrderBook          = 1 << 0;
        const HasCorporateActions   = 1 << 1;
        const HasShortBorrow        = 1 << 2;
        const HasSessions           = 1 << 3;
        const HasMakerTakerFees     = 1 << 4;
        const HasTokenEvents        = 1 << 5;

        // Engine C — NAV / ETF
        const HasNAV                = 1 << 6;
        const HasBasket             = 1 << 7;
        const HasDailyReset         = 1 << 8;

        // Engine D — Cash Flow / Bonds
        const HasCoupon             = 1 << 9;
        const HasYield              = 1 << 10;
        const HasMaturity           = 1 << 11;
        const HasCreditRisk         = 1 << 12;

        // Futures/Options shared
        const HasExpiry             = 1 << 13;
        const HasOpenInterest       = 1 << 14;
        const HasRollSchedule       = 1 << 15;

        // Engine A — Perps
        const HasFunding            = 1 << 16;
        const HasMarkPrice          = 1 << 17;
        const HasLiquidation        = 1 << 18;
        const IsLeveraged           = 1 << 19;

        // Engine E — Options/Derivatives
        const HasGreeks             = 1 << 20;
        const HasIVSurface          = 1 << 21;
        const HasEarlyExercise      = 1 << 22;

        // Engine A — FX
        const HasSwapRates          = 1 << 23;
        const HasSessionLiquidity   = 1 << 24;

        // Engine B — AMM / DEX
        const HasPoolReserves       = 1 << 25;
        const HasConcentratedLiquidity = 1 << 26;
        const HasGasCost            = 1 << 27;

        // Engine G — Marketplace / NFTs
        const IsUnique              = 1 << 28;
        const IsFungibleSKU         = 1 << 29;
        const HasRarity             = 1 << 30;
        const HasFloor              = 1 << 31;
        const IsIlliquid            = 1 << 32;
        const HasTimedAuction       = 1 << 33;
        const HasOffer              = 1 << 34;
        const HasRoyalty            = 1 << 35;
        const HasPhysicalFulfillment = 1 << 36;

        // Engine H — Prediction Markets
        const IsBinary              = 1 << 37;
        const HasResolution         = 1 << 38;
        const HasOracleRisk         = 1 << 39;
        const HasProbabilityPrice   = 1 << 40;
        const HasClob               = 1 << 41;

        // Engine A — Advanced order book features
        const HasL3OrderBook        = 1 << 42;
        const HasAuction            = 1 << 43;
        const HasTradingStatus      = 1 << 44;
        const HasUniverseMembership = 1 << 45;

        // Engine C — ETF specific
        const HasHoldings           = 1 << 46;
        const HasCreationRedemption = 1 << 47;

        // Engine B — AMM swap history
        const HasSwapEvent          = 1 << 48;

        // Engine A / E — Liquidation stream
        const HasLiquidationStream  = 1 << 49;
        const HasBorrowRate         = 1 << 50;
        const HasFeeScheduleUpdates = 1 << 51;

        // All engines — Exogenous signal plane
        const HasExogenousSignals   = 1 << 52;
    }
}

impl Serialize for CapabilityFlags {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(self.bits())
    }
}

impl<'de> Deserialize<'de> for CapabilityFlags {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let bits = u64::deserialize(deserializer)?;
        CapabilityFlags::from_bits(bits).ok_or_else(|| {
            serde::de::Error::custom(format!("invalid CapabilityFlags bits: {}", bits))
        })
    }
}
