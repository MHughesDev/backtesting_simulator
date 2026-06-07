/// Capability-gated extensions for Engine A.
/// These are called from the main engine only when the instrument has the
/// corresponding capability flag set.
use rust_decimal::Decimal;

use contracts::{CapabilityFlags, Fill, Instrument, MarketPayload, Timestamp};

/// Session state (HasSessions)
#[derive(Debug, Clone, Default)]
pub struct SessionState {
    pub session_type: Option<String>,
    pub session_open_ts: Option<Timestamp>,
    pub session_close_ts: Option<Timestamp>,
    pub liquidity_multiplier: Decimal,
    pub is_regular_hours: bool,
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            liquidity_multiplier: Decimal::ONE,
            ..Default::default()
        }
    }

    pub fn update_from_payload(&mut self, payload: &MarketPayload) {
        if let MarketPayload::TradingSession {
            session_type,
            open_ts,
            close_ts,
            liquidity_multiplier,
        } = payload
        {
            self.session_type = Some(format!("{:?}", session_type));
            self.session_open_ts = Some(*open_ts);
            self.session_close_ts = Some(*close_ts);
            self.liquidity_multiplier = *liquidity_multiplier;
            // Regular hours if not pre/after market (simplified)
            self.is_regular_hours = true;
        }
    }
}

/// Funding state (HasFunding — perps)
#[derive(Debug, Clone, Default)]
pub struct FundingState {
    pub current_rate: Option<Decimal>,
    pub mark_price: Option<Decimal>,
    pub index_price: Option<Decimal>,
    pub next_funding_ts: Option<Timestamp>,
    pub accrued_funding: Decimal,
}

impl FundingState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_from_payload(&mut self, payload: &MarketPayload) {
        match payload {
            MarketPayload::Funding {
                rate,
                mark_price,
                index_price,
                next_funding_ts,
            } => {
                self.current_rate = Some(*rate);
                self.mark_price = Some(*mark_price);
                self.index_price = Some(*index_price);
                self.next_funding_ts = Some(*next_funding_ts);
            }
            MarketPayload::MarkUpdate {
                mark_price,
                index_price,
            } => {
                self.mark_price = Some(*mark_price);
                self.index_price = Some(*index_price);
            }
            _ => {}
        }
    }

    /// Apply funding to a position. Returns the funding payment (positive = paid, negative = received).
    pub fn apply_funding(&self, position_qty: Decimal, contract_multiplier: Decimal) -> Decimal {
        let rate = match self.current_rate {
            Some(r) => r,
            None => return Decimal::ZERO,
        };
        let mark = match self.mark_price {
            Some(m) => m,
            None => return Decimal::ZERO,
        };
        position_qty * mark * contract_multiplier * rate
    }
}

/// Corporate action state (HasCorporateActions)
#[derive(Debug, Clone, Default)]
pub struct CorporateActionState {
    pub pending_adjustment: Option<Decimal>, // price adjustment factor
}

impl CorporateActionState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply_corporate_action(
        &mut self,
        payload: &MarketPayload,
        _instrument: &Instrument,
    ) -> Option<Decimal> {
        if let MarketPayload::CorporateAction { action_type } = payload {
            match action_type {
                contracts::CorporateActionType::Split { ratio, .. } => {
                    return Some(*ratio);
                }
                contracts::CorporateActionType::Dividend { amount, .. } => {
                    return Some(*amount);
                }
                _ => {}
            }
        }
        None
    }
}

/// Short borrow state (HasShortBorrow)
#[derive(Debug, Clone, Default)]
pub struct BorrowState {
    pub borrow_rate_annualized: Decimal,
    pub hard_to_borrow: bool,
    pub accrued_cost: Decimal,
}

impl BorrowState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_from_payload(&mut self, payload: &MarketPayload) {
        if let MarketPayload::BorrowRate {
            borrow_rate_annualized,
            hard_to_borrow,
            ..
        } = payload
        {
            self.borrow_rate_annualized = *borrow_rate_annualized;
            self.hard_to_borrow = *hard_to_borrow;
        }
    }

    /// Accrue daily borrow cost for a short position.
    /// days = 1.0/252.0 for daily accrual
    pub fn accrue_borrow(&mut self, short_qty: Decimal, mark_price: Decimal, days: Decimal) {
        if short_qty.is_sign_positive() {
            return; // not short
        }
        let daily_cost = short_qty.abs() * mark_price * self.borrow_rate_annualized * days;
        self.accrued_cost += daily_cost;
    }
}

/// Fee schedule state (HasFeeScheduleUpdates)
#[derive(Debug, Clone, Default)]
pub struct DynamicFeeState {
    pub maker_bps: Option<Decimal>,
    pub taker_bps: Option<Decimal>,
}

impl DynamicFeeState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_from_payload(&mut self, payload: &MarketPayload) {
        if let MarketPayload::FeeScheduleUpdate {
            maker_bps,
            taker_bps,
            ..
        } = payload
        {
            self.maker_bps = Some(*maker_bps);
            self.taker_bps = Some(*taker_bps);
        }
    }
}

/// Apply all capability-gated extensions from a payload.
#[allow(dead_code)]
pub fn dispatch_extensions(
    payload: &MarketPayload,
    instrument: &Instrument,
    session: &mut SessionState,
    funding: &mut FundingState,
    borrow: &mut BorrowState,
    fee_state: &mut DynamicFeeState,
    _fills: &mut Vec<Fill>,
) {
    let caps = instrument.capabilities;

    if caps.contains(CapabilityFlags::HasSessions) {
        session.update_from_payload(payload);
    }
    if caps.contains(CapabilityFlags::HasFunding) || caps.contains(CapabilityFlags::HasMarkPrice) {
        funding.update_from_payload(payload);
    }
    if caps.contains(CapabilityFlags::HasBorrowRate)
        || caps.contains(CapabilityFlags::HasShortBorrow)
    {
        borrow.update_from_payload(payload);
    }
    if caps.contains(CapabilityFlags::HasFeeScheduleUpdates) {
        fee_state.update_from_payload(payload);
    }
}
