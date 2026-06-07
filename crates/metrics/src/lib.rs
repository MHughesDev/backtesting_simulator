use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use contracts::{Account, Fill, Side, TradeRecord};

/// Summary statistics for a completed simulation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummary {
    /// Total number of completed trades (fills).
    pub total_trades: usize,

    /// Total number of fill events (partial fills count separately).
    pub total_fills: usize,

    /// Total realized P&L across all trades.
    pub total_pnl: Decimal,

    /// Total fees paid.
    pub total_fees: Decimal,

    /// Win rate: fraction of profitable trades (0.0 – 1.0).
    pub win_rate: f64,

    /// Average fill price across all fills.
    pub avg_fill_price: Decimal,

    /// Final account equity (if account is available).
    pub final_equity: Option<Decimal>,

    /// Maximum drawdown (0.0 – 1.0, negative values indicate loss).
    pub max_drawdown: f64,

    /// Average slippage in basis points.
    pub avg_slippage_bps: Decimal,
}

impl Default for MetricsSummary {
    fn default() -> Self {
        Self {
            total_trades: 0,
            total_fills: 0,
            total_pnl: Decimal::ZERO,
            total_fees: Decimal::ZERO,
            win_rate: 0.0,
            avg_fill_price: Decimal::ZERO,
            final_equity: None,
            max_drawdown: 0.0,
            avg_slippage_bps: Decimal::ZERO,
        }
    }
}

/// Compute metrics from a list of trade records and an optional account snapshot.
pub fn compute_metrics(
    trades: &[TradeRecord],
    fills: &[Fill],
    account: Option<&dyn Account>,
) -> MetricsSummary {
    let total_trades = trades
        .iter()
        .filter(|t| !t.rejected && t.fill_qty > Decimal::ZERO)
        .count();
    let total_fills = fills.len();

    if total_trades == 0 {
        return MetricsSummary {
            final_equity: account.map(|a| a.equity()),
            ..Default::default()
        };
    }

    let total_fees: Decimal = trades.iter().map(|t| t.fees).sum();

    // Compute realized P&L by pairing buys and sells
    // Simple approach: compute notional flows
    let total_buy_notional: Decimal = trades
        .iter()
        .filter(|t| !t.rejected && t.side == Side::Buy)
        .map(|t| t.fill_price * t.fill_qty)
        .sum();

    let total_sell_notional: Decimal = trades
        .iter()
        .filter(|t| !t.rejected && t.side == Side::Sell)
        .map(|t| t.fill_price * t.fill_qty)
        .sum();

    let total_pnl = total_sell_notional - total_buy_notional - total_fees;

    // Average fill price
    let total_fill_qty: Decimal = trades
        .iter()
        .filter(|t| !t.rejected)
        .map(|t| t.fill_qty)
        .sum();
    let total_fill_notional: Decimal = trades
        .iter()
        .filter(|t| !t.rejected)
        .map(|t| t.fill_price * t.fill_qty)
        .sum();

    let avg_fill_price = if total_fill_qty.is_zero() {
        Decimal::ZERO
    } else {
        total_fill_notional / total_fill_qty
    };

    // Win rate: count profitable round trips
    // Simplified: group by instrument, count sells that are profitable
    let win_rate = compute_win_rate(trades);

    // Average slippage
    let avg_slippage_bps = {
        let n = trades.iter().filter(|t| !t.rejected).count();
        if n == 0 {
            Decimal::ZERO
        } else {
            let sum: Decimal = trades
                .iter()
                .filter(|t| !t.rejected)
                .map(|t| t.slippage_bps)
                .sum();
            sum / Decimal::from(n as u64)
        }
    };

    MetricsSummary {
        total_trades,
        total_fills,
        total_pnl,
        total_fees,
        win_rate,
        avg_fill_price,
        final_equity: account.map(|a| a.equity()),
        max_drawdown: 0.0, // requires equity curve — not computed here
        avg_slippage_bps,
    }
}

/// Compute win rate from trade records using a simple round-trip matching.
fn compute_win_rate(trades: &[TradeRecord]) -> f64 {
    use std::collections::HashMap;

    // Group trades by instrument_id
    let mut by_instrument: HashMap<&str, Vec<&TradeRecord>> = HashMap::new();
    for t in trades
        .iter()
        .filter(|t| !t.rejected && t.fill_qty > Decimal::ZERO)
    {
        by_instrument
            .entry(t.instrument_id.as_str())
            .or_default()
            .push(t);
    }

    let mut total_trips = 0usize;
    let mut winning_trips = 0usize;

    for instrument_trades in by_instrument.values() {
        let mut position: Decimal = Decimal::ZERO;
        let mut cost_basis: Decimal = Decimal::ZERO;
        let mut entry_price: Option<Decimal> = None;

        for trade in instrument_trades.iter() {
            match trade.side {
                Side::Buy => {
                    let new_qty = position + trade.fill_qty;
                    if new_qty > Decimal::ZERO {
                        // Updating or opening long
                        let old_cost = if position > Decimal::ZERO {
                            cost_basis * position
                        } else {
                            Decimal::ZERO
                        };
                        cost_basis = (old_cost + trade.fill_price * trade.fill_qty) / new_qty;
                    }
                    position = new_qty;
                    if entry_price.is_none() {
                        entry_price = Some(trade.fill_price);
                    }
                }
                Side::Sell => {
                    if position > Decimal::ZERO && trade.fill_qty <= position {
                        // Closing long
                        let pnl = (trade.fill_price - cost_basis) * trade.fill_qty;
                        total_trips += 1;
                        if pnl > Decimal::ZERO {
                            winning_trips += 1;
                        }
                    }
                    position -= trade.fill_qty;
                }
                Side::Unknown => {}
            }
        }
    }

    if total_trips == 0 {
        0.0
    } else {
        winning_trips as f64 / total_trips as f64
    }
}
