use contracts::{
    Account, FidelityLevel, Fill, Instrument, MarketEvent, Order, OrderResult, TradeRecord,
};
use core_lib::{ScopedRng, SimulationClock};

/// The Engine trait. Every price-formation engine implements this. The runner
/// dispatches events and orders here. (ADR-0003: no asset-type branching)
pub trait Engine: Send + Sync {
    /// Process a market event. May update internal book state, trigger fills on
    /// resting orders, update mark prices, etc.
    fn on_event(&mut self, ev: &MarketEvent, ctx: &mut EngineContext);

    /// Submit an order for immediate or resting execution.
    fn submit_order(&mut self, order: Order, ctx: &mut EngineContext) -> OrderResult;

    /// Called at run end. Should settle all open positions.
    fn settle(&mut self, ctx: &mut EngineContext);

    /// Returns true if this engine can handle the given order type.
    fn supports_order_type(&self, t: &contracts::OrderType) -> bool;

    /// Current data fidelity (determines fill modeling quality).
    fn current_fidelity(&self) -> FidelityLevel;
}

/// Shared context passed to every engine call. Contains references to the
/// clock, instrument definition, optional account, and output buffers.
pub struct EngineContext<'a> {
    pub clock: &'a SimulationClock,
    pub instrument: &'a Instrument,
    pub account: Option<&'a mut dyn Account>,
    pub fills: &'a mut Vec<Fill>,
    pub trade_records: &'a mut Vec<TradeRecord>,
    pub rng: &'a mut ScopedRng,
}
