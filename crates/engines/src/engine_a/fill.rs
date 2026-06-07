use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use contracts::{FeeTier, FidelityLevel, Fill, Order, OrderType, Side, Timestamp};

use super::book::BookView;

/// Default taker fee in basis points (5 bps = 0.05%)
pub const DEFAULT_TAKER_BPS: Decimal = dec!(5);
/// Default maker fee in basis points (0 bps)
pub const DEFAULT_MAKER_BPS: Decimal = dec!(0);
/// Basis points divisor
pub const BPS_DIVISOR: Decimal = dec!(10000);

/// Compute the fee for a fill.
/// If fee_schedule is provided, use the first applicable tier.
/// Otherwise use the provided defaults.
pub fn compute_fee(
    fill_qty: Decimal,
    fill_price: Decimal,
    is_maker: bool,
    fee_schedule: Option<&Vec<FeeTier>>,
    taker_fee_override: Option<Decimal>,
) -> Decimal {
    let bps = if let Some(schedule) = fee_schedule {
        if let Some(tier) = schedule.first() {
            if is_maker {
                tier.maker_bps
            } else {
                tier.taker_bps
            }
        } else {
            if is_maker {
                DEFAULT_MAKER_BPS
            } else {
                DEFAULT_TAKER_BPS
            }
        }
    } else if let Some(override_bps) = taker_fee_override {
        if is_maker {
            DEFAULT_MAKER_BPS
        } else {
            override_bps
        }
    } else {
        if is_maker {
            DEFAULT_MAKER_BPS
        } else {
            DEFAULT_TAKER_BPS
        }
    };
    fill_qty * fill_price * bps / BPS_DIVISOR
}

/// Compute slippage in basis points relative to a reference price.
pub fn compute_slippage_bps(fill_price: Decimal, reference_price: Decimal, side: &Side) -> Decimal {
    if reference_price.is_zero() {
        return Decimal::ZERO;
    }
    let diff = match side {
        Side::Buy => fill_price - reference_price,
        Side::Sell => reference_price - fill_price,
        Side::Unknown => (fill_price - reference_price).abs(),
    };
    diff / reference_price * BPS_DIVISOR
}

/// Bar-level fill: fill at the given price (typically next bar open).
pub fn fill_at_price(
    order: &Order,
    fill_price: Decimal,
    ts_fill: Timestamp,
    fee_schedule: Option<&Vec<FeeTier>>,
    taker_fee_override: Option<Decimal>,
    reference_price: Decimal,
) -> Fill {
    let is_maker = matches!(order.order_type, OrderType::Limit);
    let fees = compute_fee(
        order.quantity,
        fill_price,
        is_maker,
        fee_schedule,
        taker_fee_override,
    );
    let slippage_bps = compute_slippage_bps(fill_price, reference_price, &order.side);

    Fill {
        instrument_id: order.instrument_id.clone(),
        side: order.side.clone(),
        fill_price,
        fill_qty: order.quantity,
        fees,
        slippage_bps,
        ts_fill,
        partial: false,
        fidelity: FidelityLevel::Bar,
    }
}

/// L1 fill: fill at touch up to displayed size.
/// Returns None if the book doesn't have sufficient size.
pub fn fill_l1(
    order: &Order,
    book: &BookView,
    ts_fill: Timestamp,
    fee_schedule: Option<&Vec<FeeTier>>,
    taker_fee_override: Option<Decimal>,
) -> Option<Fill> {
    let (touch_price, available_size) = match order.side {
        Side::Buy => book.best_ask()?,
        Side::Sell => book.best_bid()?,
        Side::Unknown => return None,
    };

    let fill_qty = order.quantity.min(available_size);
    let partial = fill_qty < order.quantity;
    let fees = compute_fee(
        fill_qty,
        touch_price,
        false,
        fee_schedule,
        taker_fee_override,
    );
    let slippage_bps = compute_slippage_bps(touch_price, touch_price, &order.side);

    Some(Fill {
        instrument_id: order.instrument_id.clone(),
        side: order.side.clone(),
        fill_price: touch_price,
        fill_qty,
        fees,
        slippage_bps,
        ts_fill,
        partial,
        fidelity: FidelityLevel::L1,
    })
}

/// L2 fill: walk the book levels to fill the order.
/// Returns None if the book is empty.
pub fn fill_l2(
    order: &Order,
    book: &BookView,
    ts_fill: Timestamp,
    fee_schedule: Option<&Vec<FeeTier>>,
    taker_fee_override: Option<Decimal>,
) -> Option<Fill> {
    let fills = match order.side {
        Side::Buy => book.walk_asks(order.quantity),
        Side::Sell => book.walk_bids(order.quantity),
        Side::Unknown => return None,
    };

    if fills.is_empty() {
        return None;
    }

    // Compute VWAP fill price
    let total_qty: Decimal = fills.iter().map(|(_, q)| q).sum();
    let vwap: Decimal = fills.iter().map(|(p, q)| p * q).sum::<Decimal>() / total_qty;

    // Reference price is best touch
    let ref_price = fills[0].0;
    let partial = total_qty < order.quantity;
    let fees = compute_fee(total_qty, vwap, false, fee_schedule, taker_fee_override);
    let slippage_bps = compute_slippage_bps(vwap, ref_price, &order.side);

    Some(Fill {
        instrument_id: order.instrument_id.clone(),
        side: order.side.clone(),
        fill_price: vwap,
        fill_qty: total_qty,
        fees,
        slippage_bps,
        ts_fill,
        partial,
        fidelity: FidelityLevel::L2,
    })
}

/// Try to fill a limit order against the book.
/// Returns Some(Fill) if immediately executable (marketable limit order).
pub fn try_fill_limit(
    order: &Order,
    book: &BookView,
    ts_fill: Timestamp,
    fee_schedule: Option<&Vec<FeeTier>>,
    taker_fee_override: Option<Decimal>,
) -> Option<Fill> {
    let limit_price = order.limit_price?;

    let (touch_price, _) = match order.side {
        Side::Buy => book.best_ask()?,
        Side::Sell => book.best_bid()?,
        Side::Unknown => return None,
    };

    // Check if marketable
    let marketable = match order.side {
        Side::Buy => touch_price <= limit_price,
        Side::Sell => touch_price >= limit_price,
        Side::Unknown => false,
    };

    if !marketable {
        return None;
    }

    // Fill at touch (taker since crossing the spread)
    let fill_qty = order.quantity;
    let fees = compute_fee(
        fill_qty,
        touch_price,
        false,
        fee_schedule,
        taker_fee_override,
    );
    let slippage_bps = compute_slippage_bps(touch_price, limit_price, &order.side);

    Some(Fill {
        instrument_id: order.instrument_id.clone(),
        side: order.side.clone(),
        fill_price: touch_price,
        fill_qty,
        fees,
        slippage_bps,
        ts_fill,
        partial: false,
        fidelity: if book.is_empty() {
            FidelityLevel::L1
        } else {
            FidelityLevel::L2
        },
    })
}
