use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::trade::{Fill, Position};
use crate::types::{InstrumentId, Timestamp};

/// Account port — injected by the caller. The simulator never stores account state
/// itself; it calls these methods to report fills and query buying power (ADR-0005).
pub trait Account: Send + Sync {
    /// Current cash balance.
    fn cash(&self) -> Decimal;

    /// Total account equity = cash + sum(position market values).
    fn equity(&self) -> Decimal;

    /// All open positions keyed by instrument_id.
    fn positions(&self) -> HashMap<InstrumentId, Position>;

    /// Available buying power (may differ from cash when margin is used).
    fn buying_power(&self) -> Decimal;

    /// Collateral balance (relevant for leveraged instruments).
    fn collateral_balance(&self) -> Decimal;

    /// Called by the engine after a fill is executed. The account implementation
    /// should update cash and positions accordingly.
    fn report_fill(&mut self, fill: &Fill);

    /// Update the mark price for a position (for unrealized P&L calculation).
    fn update_mark_price(&mut self, instrument_id: &InstrumentId, price: Decimal);

    /// Force-close a position at the given price (e.g. margin call, liquidation, run end).
    fn force_close(
        &mut self,
        instrument_id: &InstrumentId,
        price: Decimal,
        reason: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

// ── AI / training integration ports (Plan 0009) ──────────────────────────────

/// Context bundle passed to a model for inference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBundle {
    pub ts: Timestamp,
    pub features: HashMap<String, Decimal>,
}

/// Output from a model inference call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelOutput {
    pub direction: ModelDirection,
    pub confidence: f64,
}

/// Direction signal from a model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelDirection {
    Long,
    Short,
    Flat,
}

/// Model port — injected by the caller for AI-driven strategies.
pub trait Model: Send + Sync {
    fn infer(&self, context: &ContextBundle) -> ModelOutput;
}

/// Training configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainConfig {
    pub method_id: String,
    pub params: HashMap<String, f64>,
}

/// Training data window specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingDataWindow {
    pub start_ts: Timestamp,
    pub end_ts: Timestamp,
}

/// Trained model artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelArtifact {
    pub id: String,
    pub version: u32,
}

/// Trainer port — injected by the caller for model training integration.
pub trait Trainer: Send + Sync {
    fn train(&self, config: &TrainConfig, data_window: &TrainingDataWindow) -> ModelArtifact;
}
