use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::types::{InstrumentId, Side, Timestamp};

/// Order type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit,
    Stop,
    StopLimit,
}

/// Time in force.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeInForce {
    /// Good till cancelled
    Gtc,
    /// Immediate or cancel
    Ioc,
    /// Fill or kill
    Fok,
    /// Day order (cancelled at session close)
    Day,
}

/// An order submitted by a strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub instrument_id: InstrumentId,
    pub side: Side,
    pub order_type: OrderType,
    pub quantity: Decimal,
    pub limit_price: Option<Decimal>,
    pub stop_price: Option<Decimal>,
    pub time_in_force: TimeInForce,
    /// Maximum acceptable slippage in basis points (Market orders only)
    pub max_slippage_bps: Option<Decimal>,
    pub submitted_ts: Timestamp,
}

/// Data fidelity level — determined by available market data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FidelityLevel {
    /// Bar-level: fill at next bar's open, limited fill modeling
    Bar,
    /// L1: fill at BBO touch, limited size
    L1,
    /// L2: walk the book levels
    L2,
    /// L3: full per-order book data
    L3,
}

/// A fill record for an executed order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fill {
    pub instrument_id: InstrumentId,
    pub side: Side,
    pub fill_price: Decimal,
    pub fill_qty: Decimal,
    pub fees: Decimal,
    pub slippage_bps: Decimal,
    pub ts_fill: Timestamp,
    pub partial: bool,
    pub fidelity: FidelityLevel,
}

/// A position in an instrument.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub qty: Decimal,
    pub cost_basis_per_unit: Decimal,
    pub mark_price: Decimal,
    pub unrealized_pnl: Decimal,
}

impl Position {
    pub fn new(qty: Decimal, cost_basis_per_unit: Decimal) -> Self {
        Self {
            qty,
            cost_basis_per_unit,
            mark_price: cost_basis_per_unit,
            unrealized_pnl: Decimal::ZERO,
        }
    }

    pub fn update_mark(&mut self, mark_price: Decimal) {
        self.mark_price = mark_price;
        self.unrealized_pnl = (mark_price - self.cost_basis_per_unit) * self.qty;
    }
}

/// Complete record of a trade decision + fill outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    pub ts_decision: Timestamp,
    pub ts_fill: Timestamp,
    pub instrument_id: InstrumentId,
    pub side: Side,
    pub order_type: OrderType,
    pub fill_price: Decimal,
    pub fill_qty: Decimal,
    pub fees: Decimal,
    pub slippage_bps: Decimal,
    pub partial: bool,
    pub fidelity: FidelityLevel,
    pub rejected: bool,
    pub reject_reason: Option<String>,
}

/// Result of submitting an order to an engine.
#[derive(Debug, Clone)]
pub enum OrderResult {
    /// Order fully filled immediately.
    Filled(Fill),
    /// Order partially filled; remainder handled per TIF.
    PartialFill(Fill),
    /// Order rejected; reason provided.
    Rejected(String),
    /// Order placed in book, awaiting fill (resting limit/stop order).
    Resting,
}
