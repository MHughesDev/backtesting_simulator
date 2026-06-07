use std::collections::HashMap;

use rust_decimal::Decimal;

use contracts::{Account, Fill, InstrumentId, Position, Side};

/// A reference implementation of the Account port.
/// Tracks cash, positions, and mark prices. No leverage or margin.
pub struct SimpleAccount {
    /// Starting and current cash balance.
    cash: Decimal,

    /// Open positions: instrument_id → (qty, cost_basis_per_unit)
    positions: HashMap<InstrumentId, (Decimal, Decimal)>,

    /// Mark prices for equity calculation.
    mark_prices: HashMap<InstrumentId, Decimal>,
}

impl SimpleAccount {
    /// Create a new SimpleAccount with the given starting balance.
    pub fn new(starting_balance: Decimal) -> Self {
        Self {
            cash: starting_balance,
            positions: HashMap::new(),
            mark_prices: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn starting_balance(&self) -> Decimal {
        // Not tracked post-init, but useful for reference
        self.cash
    }
}

impl Account for SimpleAccount {
    fn cash(&self) -> Decimal {
        self.cash
    }

    fn equity(&self) -> Decimal {
        let position_value: Decimal = self
            .positions
            .iter()
            .filter(|(_, (qty, _))| !qty.is_zero())
            .map(|(id, (qty, cost_basis))| {
                let mark = self.mark_prices.get(id).copied().unwrap_or(*cost_basis);
                qty * mark
            })
            .sum();
        self.cash + position_value
    }

    fn positions(&self) -> HashMap<InstrumentId, Position> {
        self.positions
            .iter()
            .filter(|(_, (qty, _))| !qty.is_zero())
            .map(|(id, (qty, cost_basis))| {
                let mark_price = self.mark_prices.get(id).copied().unwrap_or(*cost_basis);
                let unrealized_pnl = (*qty) * (mark_price - cost_basis);
                (
                    id.clone(),
                    Position {
                        qty: *qty,
                        cost_basis_per_unit: *cost_basis,
                        mark_price,
                        unrealized_pnl,
                    },
                )
            })
            .collect()
    }

    fn buying_power(&self) -> Decimal {
        self.cash.max(Decimal::ZERO)
    }

    fn collateral_balance(&self) -> Decimal {
        self.cash.max(Decimal::ZERO)
    }

    fn report_fill(&mut self, fill: &Fill) {
        let notional = fill.fill_price * fill.fill_qty;
        let (qty, cost_basis) = self
            .positions
            .entry(fill.instrument_id.clone())
            .or_insert((Decimal::ZERO, Decimal::ZERO));

        match fill.side {
            Side::Buy => {
                // Update average cost basis
                let new_qty = *qty + fill.fill_qty;
                if new_qty.is_zero() {
                    *cost_basis = Decimal::ZERO;
                } else if *qty <= Decimal::ZERO {
                    // Opening new long or covering short
                    *cost_basis = fill.fill_price;
                } else {
                    // Adding to long position — update VWAP cost basis
                    let old_cost = *cost_basis * (*qty).max(Decimal::ZERO);
                    *cost_basis = (old_cost + fill.fill_price * fill.fill_qty) / new_qty;
                }
                *qty = new_qty;
                // Cash decreases
                self.cash -= notional + fill.fees;
            }
            Side::Sell => {
                let new_qty = *qty - fill.fill_qty;
                if new_qty.is_zero() || new_qty.is_sign_negative() {
                    // Position closed or reversed
                    if new_qty.is_sign_negative() {
                        // Short position
                        *cost_basis = fill.fill_price;
                    } else {
                        *cost_basis = Decimal::ZERO;
                    }
                }
                *qty = new_qty;
                // Cash increases
                self.cash += notional - fill.fees;
            }
            Side::Unknown => {
                // Unknown side: just deduct fees
                self.cash -= fill.fees;
            }
        }

        // Update mark price
        self.mark_prices
            .insert(fill.instrument_id.clone(), fill.fill_price);
    }

    fn update_mark_price(&mut self, instrument_id: &InstrumentId, price: Decimal) {
        self.mark_prices.insert(instrument_id.clone(), price);
    }

    fn force_close(
        &mut self,
        instrument_id: &InstrumentId,
        price: Decimal,
        _reason: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some((qty, _cost_basis)) = self.positions.get_mut(instrument_id) {
            let close_qty = *qty;
            if close_qty.is_zero() {
                return Ok(());
            }

            let notional = price * close_qty.abs();
            if close_qty > Decimal::ZERO {
                // Close long
                self.cash += notional;
            } else {
                // Close short
                self.cash -= notional;
            }
            *qty = Decimal::ZERO;
        }
        Ok(())
    }
}
