use contracts::{Account, InstrumentId, Order};

use crate::market_view::{BarData, MarketView};

/// The Strategy trait. Implementations receive bar events and return orders.
///
/// Strategies are called only after the warmup gate has opened (i.e., after
/// `warmup_start` + `warmup_bars_required()` bars have been observed).
pub trait Strategy: Send {
    /// Called on each Bar event after warmup is complete.
    ///
    /// - `instrument_id`: the instrument that produced the bar
    /// - `bar`: the current bar data
    /// - `view`: the full market view (all instruments' latest data)
    /// - `account`: read-only account state for position sizing
    ///
    /// Returns a list of orders to submit. Orders are routed to the appropriate engine.
    fn on_bar(
        &mut self,
        instrument_id: &InstrumentId,
        bar: &BarData,
        view: &MarketView,
        account: &dyn Account,
    ) -> Vec<Order>;

    /// Number of bars needed before the strategy is ready to trade.
    /// Defaults to 0 (no warmup required beyond the run-level warmup).
    fn warmup_bars_required(&self) -> usize {
        0
    }
}
