pub mod book;
pub mod extensions;
pub mod fill;

use rust_decimal::Decimal;

use contracts::{
    FidelityLevel, Fill, MarketEvent, MarketPayload, Order, OrderResult, OrderType, Side,
    TimeInForce, Timestamp, TradeRecord,
};

use crate::engine::{Engine, EngineContext};
use book::BookView;
use extensions::{BorrowState, DynamicFeeState, FundingState, SessionState};
use fill::{fill_at_price, fill_l1, fill_l2, try_fill_limit};

/// A resting order in the book, waiting to be triggered or filled.
#[derive(Debug, Clone)]
struct RestingOrder {
    order: Order,
    submitted_ts: Timestamp,
    /// The earliest timestamp at which this order can be filled (accounts for latency).
    active_ts: Timestamp,
}

/// Saved bar data for next-bar-open fill logic.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct SavedBar {
    open: Decimal,
    high: Decimal,
    low: Decimal,
    close: Decimal,
    ts: Timestamp,
}

/// Engine A: CLOB / Order Book engine.
///
/// Handles all instruments with `price_formation = Clob`.
///
/// Fill model:
/// - Bar data: market orders fill at NEXT bar's open (buffered until next bar arrives)
/// - L1 data: fill at BBO touch up to displayed size
/// - L2 data: walk book levels for size
/// - Resting limit orders: checked on each bar/trade event
pub struct OrderBookEngine {
    book: BookView,

    /// Current data fidelity level.
    fidelity: FidelityLevel,

    /// Last completed bar (used as reference for fills).
    last_bar: Option<SavedBar>,

    /// Market orders buffered at decision ts, to be filled at next bar open.
    pending_market_orders: Vec<(Order, Timestamp)>,

    /// Resting limit/stop orders.
    resting_orders: Vec<RestingOrder>,

    /// Last trade price (for mark and slippage reference).
    last_trade_price: Option<Decimal>,

    /// Mark price (from MarkUpdate or Funding events).
    mark_price: Option<Decimal>,

    /// Trading status: is the instrument currently halted?
    is_halted: bool,

    /// Simulated order latency in nanoseconds.
    latency_ns: i64,

    // ── Extension states ──────────────────────────────────────────────────────
    session: SessionState,
    funding: FundingState,
    borrow: BorrowState,
    fee_state: DynamicFeeState,

    /// Override taker fee in bps (from RunRequest).
    taker_fee_override: Option<Decimal>,
}

impl OrderBookEngine {
    pub fn new(latency_ns: i64, taker_fee_override: Option<Decimal>) -> Self {
        Self {
            book: BookView::new(),
            fidelity: FidelityLevel::Bar,
            last_bar: None,
            pending_market_orders: Vec::new(),
            resting_orders: Vec::new(),
            last_trade_price: None,
            mark_price: None,
            is_halted: false,
            latency_ns,
            session: SessionState::new(),
            funding: FundingState::new(),
            borrow: BorrowState::new(),
            fee_state: DynamicFeeState::new(),
            taker_fee_override,
        }
    }

    /// Get the effective taker fee bps, considering dynamic updates.
    fn effective_taker_bps(&self, _instrument: &contracts::Instrument) -> Option<Decimal> {
        // Priority: dynamic fee update > run-level override > fee_schedule > default
        if let Some(bps) = self.fee_state.taker_bps {
            return Some(bps);
        }
        if let Some(bps) = self.taker_fee_override {
            return Some(bps);
        }
        None
    }

    /// Process a Bar payload.
    fn handle_bar(
        &mut self,
        open: Decimal,
        high: Decimal,
        low: Decimal,
        close: Decimal,
        ts: Timestamp,
        ctx: &mut EngineContext,
    ) {
        let instrument = ctx.instrument;
        let fee_schedule = instrument.fee_schedule.as_ref();
        let taker_override = self.effective_taker_bps(instrument);

        // Fill pending market orders at this bar's open
        let pending = std::mem::take(&mut self.pending_market_orders);
        for (order, decision_ts) in pending {
            // Check latency: order must have been submitted before this bar
            let active_ts = decision_ts + self.latency_ns;
            if active_ts > ts {
                // Still pending — keep it (unusual if latency > bar interval)
                self.pending_market_orders.push((order, decision_ts));
                continue;
            }

            let fill = fill_at_price(
                &order,
                open,
                ts,
                fee_schedule,
                taker_override,
                open, // reference = open price
            );

            let trade_record = TradeRecord {
                ts_decision: decision_ts,
                ts_fill: ts,
                instrument_id: order.instrument_id.clone(),
                side: order.side.clone(),
                order_type: order.order_type.clone(),
                fill_price: fill.fill_price,
                fill_qty: fill.fill_qty,
                fees: fill.fees,
                slippage_bps: fill.slippage_bps,
                partial: fill.partial,
                fidelity: fill.fidelity.clone(),
                rejected: false,
                reject_reason: None,
            };

            if let Some(account) = ctx.account.as_mut() {
                account.report_fill(&fill);
            }
            ctx.fills.push(fill);
            ctx.trade_records.push(trade_record);
        }

        // Check resting limit orders against this bar's high/low
        let mut still_resting = Vec::new();
        let resting = std::mem::take(&mut self.resting_orders);
        for resting_order in resting {
            if resting_order.active_ts > ts {
                still_resting.push(resting_order);
                continue;
            }

            let order = &resting_order.order;
            let filled = match &order.order_type {
                OrderType::Limit => {
                    let limit = match order.limit_price {
                        Some(p) => p,
                        None => {
                            still_resting.push(resting_order);
                            continue;
                        }
                    };
                    // Buy limit: fills if low <= limit price
                    // Sell limit: fills if high >= limit price
                    match order.side {
                        Side::Buy => low <= limit,
                        Side::Sell => high >= limit,
                        Side::Unknown => false,
                    }
                }
                OrderType::Stop | OrderType::StopLimit => {
                    let stop = match order.stop_price {
                        Some(p) => p,
                        None => {
                            still_resting.push(resting_order);
                            continue;
                        }
                    };
                    match order.side {
                        Side::Buy => high >= stop,
                        Side::Sell => low <= stop,
                        Side::Unknown => false,
                    }
                }
                _ => false,
            };

            if filled {
                let fill_price = match &order.order_type {
                    OrderType::Limit => order.limit_price.unwrap_or(open),
                    OrderType::Stop | OrderType::StopLimit => {
                        let stop = order.stop_price.unwrap_or(open);
                        // Stop fills at worst of stop price or open (gap risk)
                        match order.side {
                            Side::Buy => stop.max(open),
                            Side::Sell => stop.min(open),
                            Side::Unknown => stop,
                        }
                    }
                    _ => open,
                };

                let is_maker = matches!(order.order_type, OrderType::Limit);
                let fees = fill::compute_fee(
                    order.quantity,
                    fill_price,
                    is_maker,
                    fee_schedule,
                    taker_override,
                );
                let slippage_bps = fill::compute_slippage_bps(fill_price, fill_price, &order.side);

                let fill = Fill {
                    instrument_id: order.instrument_id.clone(),
                    side: order.side.clone(),
                    fill_price,
                    fill_qty: order.quantity,
                    fees,
                    slippage_bps,
                    ts_fill: ts,
                    partial: false,
                    fidelity: FidelityLevel::Bar,
                };

                let trade_record = TradeRecord {
                    ts_decision: resting_order.submitted_ts,
                    ts_fill: ts,
                    instrument_id: order.instrument_id.clone(),
                    side: order.side.clone(),
                    order_type: order.order_type.clone(),
                    fill_price: fill.fill_price,
                    fill_qty: fill.fill_qty,
                    fees: fill.fees,
                    slippage_bps: fill.slippage_bps,
                    partial: fill.partial,
                    fidelity: fill.fidelity.clone(),
                    rejected: false,
                    reject_reason: None,
                };

                if let Some(account) = ctx.account.as_mut() {
                    account.report_fill(&fill);
                }
                ctx.fills.push(fill);
                ctx.trade_records.push(trade_record);

                // Handle Day orders: cancel at session close (simplified: they always fill if triggered)
                // GTC stays, Day is cancelled at end of day — for now, we just remove it
            } else {
                // Cancel Day orders (they don't persist beyond the bar in bar-only mode)
                match order.time_in_force {
                    TimeInForce::Day | TimeInForce::Ioc | TimeInForce::Fok => {
                        // Cancel — don't re-add
                        let trade_record = TradeRecord {
                            ts_decision: resting_order.submitted_ts,
                            ts_fill: ts,
                            instrument_id: order.instrument_id.clone(),
                            side: order.side.clone(),
                            order_type: order.order_type.clone(),
                            fill_price: Decimal::ZERO,
                            fill_qty: Decimal::ZERO,
                            fees: Decimal::ZERO,
                            slippage_bps: Decimal::ZERO,
                            partial: false,
                            fidelity: FidelityLevel::Bar,
                            rejected: true,
                            reject_reason: Some(format!(
                                "cancelled: {:?} order expired",
                                order.time_in_force
                            )),
                        };
                        ctx.trade_records.push(trade_record);
                    }
                    TimeInForce::Gtc => {
                        still_resting.push(resting_order);
                    }
                }
            }
        }
        self.resting_orders = still_resting;

        // Save bar state
        self.last_bar = Some(SavedBar {
            open,
            high,
            low,
            close,
            ts,
        });
        self.last_trade_price = Some(close);

        // Update fidelity: Bar is baseline; if we have book data, it's at least L1
        if self.fidelity == FidelityLevel::Bar && !self.book.is_empty() {
            self.fidelity = FidelityLevel::L2;
        }

        // Update mark prices in account
        if let Some(account) = ctx.account.as_mut() {
            account.update_mark_price(&instrument.id, close);
        }
    }

    /// Process a Quote (L1) payload.
    fn handle_quote(
        &mut self,
        bid: Decimal,
        bid_size: Decimal,
        ask: Decimal,
        ask_size: Decimal,
        ts: Timestamp,
        ctx: &mut EngineContext,
    ) {
        // Update synthetic book with BBO
        use contracts::BookAction;
        self.book
            .apply_delta(&Side::Buy, bid, bid_size, &BookAction::Modify);
        self.book
            .apply_delta(&Side::Sell, ask, ask_size, &BookAction::Modify);

        if self.fidelity == FidelityLevel::Bar {
            self.fidelity = FidelityLevel::L1;
        }

        // Try to fill pending market orders immediately with L1
        let instrument = ctx.instrument;
        let fee_schedule = instrument.fee_schedule.as_ref();
        let taker_override = self.effective_taker_bps(instrument);

        let pending = std::mem::take(&mut self.pending_market_orders);
        let mut remaining = Vec::new();
        for (order, decision_ts) in pending {
            let active_ts = decision_ts + self.latency_ns;
            if active_ts > ts {
                remaining.push((order, decision_ts));
                continue;
            }
            if let Some(fill) = fill_l1(&order, &self.book, ts, fee_schedule, taker_override) {
                let trade_record = TradeRecord {
                    ts_decision: decision_ts,
                    ts_fill: ts,
                    instrument_id: order.instrument_id.clone(),
                    side: order.side.clone(),
                    order_type: order.order_type.clone(),
                    fill_price: fill.fill_price,
                    fill_qty: fill.fill_qty,
                    fees: fill.fees,
                    slippage_bps: fill.slippage_bps,
                    partial: fill.partial,
                    fidelity: fill.fidelity.clone(),
                    rejected: false,
                    reject_reason: None,
                };
                if let Some(account) = ctx.account.as_mut() {
                    account.report_fill(&fill);
                }
                ctx.fills.push(fill);
                ctx.trade_records.push(trade_record);
            } else {
                remaining.push((order, decision_ts));
            }
        }
        self.pending_market_orders = remaining;
    }

    /// Process a Trade payload.
    fn handle_trade(
        &mut self,
        price: Decimal,
        _size: Decimal,
        ts: Timestamp,
        ctx: &mut EngineContext,
    ) {
        self.last_trade_price = Some(price);

        if let Some(account) = ctx.account.as_mut() {
            account.update_mark_price(&ctx.instrument.id, price);
        }

        // Check resting orders
        self.check_resting_on_trade(price, ts, ctx);
    }

    /// Check if any resting orders should fill at the given trade price.
    fn check_resting_on_trade(&mut self, price: Decimal, ts: Timestamp, ctx: &mut EngineContext) {
        let instrument = ctx.instrument;
        let fee_schedule = instrument.fee_schedule.as_ref();
        let taker_override = self.effective_taker_bps(instrument);

        let mut still_resting = Vec::new();
        let resting = std::mem::take(&mut self.resting_orders);

        for resting_order in resting {
            if resting_order.active_ts > ts {
                still_resting.push(resting_order);
                continue;
            }

            let order = &resting_order.order;
            let triggered = match &order.order_type {
                OrderType::Limit => {
                    let limit = match order.limit_price {
                        Some(p) => p,
                        None => {
                            still_resting.push(resting_order);
                            continue;
                        }
                    };
                    match order.side {
                        Side::Buy => price <= limit,
                        Side::Sell => price >= limit,
                        Side::Unknown => false,
                    }
                }
                OrderType::Stop | OrderType::StopLimit => {
                    let stop = match order.stop_price {
                        Some(p) => p,
                        None => {
                            still_resting.push(resting_order);
                            continue;
                        }
                    };
                    match order.side {
                        Side::Buy => price >= stop,
                        Side::Sell => price <= stop,
                        Side::Unknown => false,
                    }
                }
                _ => false,
            };

            if triggered {
                let fill_price = order.limit_price.or(order.stop_price).unwrap_or(price);
                let is_maker = matches!(order.order_type, OrderType::Limit);
                let fees = fill::compute_fee(
                    order.quantity,
                    fill_price,
                    is_maker,
                    fee_schedule,
                    taker_override,
                );
                let slippage_bps = fill::compute_slippage_bps(fill_price, price, &order.side);

                let fill = Fill {
                    instrument_id: order.instrument_id.clone(),
                    side: order.side.clone(),
                    fill_price,
                    fill_qty: order.quantity,
                    fees,
                    slippage_bps,
                    ts_fill: ts,
                    partial: false,
                    fidelity: FidelityLevel::L1,
                };
                let tr = TradeRecord {
                    ts_decision: resting_order.submitted_ts,
                    ts_fill: ts,
                    instrument_id: order.instrument_id.clone(),
                    side: order.side.clone(),
                    order_type: order.order_type.clone(),
                    fill_price: fill.fill_price,
                    fill_qty: fill.fill_qty,
                    fees: fill.fees,
                    slippage_bps: fill.slippage_bps,
                    partial: fill.partial,
                    fidelity: fill.fidelity.clone(),
                    rejected: false,
                    reject_reason: None,
                };
                if let Some(account) = ctx.account.as_mut() {
                    account.report_fill(&fill);
                }
                ctx.fills.push(fill);
                ctx.trade_records.push(tr);
            } else {
                still_resting.push(resting_order);
            }
        }
        self.resting_orders = still_resting;
    }
}

impl Engine for OrderBookEngine {
    fn on_event(&mut self, ev: &MarketEvent, ctx: &mut EngineContext) {
        let ts = ev.ts_event;
        let _caps = ctx.instrument.capabilities;

        // Update extension states
        extensions::dispatch_extensions(
            &ev.payload,
            ctx.instrument,
            &mut self.session,
            &mut self.funding,
            &mut self.borrow,
            &mut self.fee_state,
            ctx.fills,
        );

        // Update book for book events
        self.book.update_from_payload(&ev.payload);

        match &ev.payload {
            MarketPayload::Bar {
                open,
                high,
                low,
                close,
                ..
            } => {
                self.handle_bar(*open, *high, *low, *close, ts, ctx);
            }
            MarketPayload::DerivedBar {
                open,
                high,
                low,
                close,
                ..
            } => {
                self.handle_bar(*open, *high, *low, *close, ts, ctx);
            }
            MarketPayload::Quote {
                bid,
                bid_size,
                ask,
                ask_size,
            } => {
                self.handle_quote(*bid, *bid_size, *ask, *ask_size, ts, ctx);
                if self.fidelity == FidelityLevel::Bar {
                    self.fidelity = FidelityLevel::L1;
                }
            }
            MarketPayload::Trade { price, size, .. } => {
                if self.fidelity == FidelityLevel::Bar {
                    self.fidelity = FidelityLevel::L1;
                }
                self.handle_trade(*price, *size, ts, ctx);
            }
            MarketPayload::BookDelta { .. } | MarketPayload::BookSnapshot { .. } => {
                // book already updated above
                self.fidelity = FidelityLevel::L2;
            }
            MarketPayload::OrderBookOrderEvent { .. } => {
                self.fidelity = FidelityLevel::L3;
            }
            MarketPayload::MarkUpdate { mark_price, .. } => {
                self.mark_price = Some(*mark_price);
                if let Some(account) = ctx.account.as_mut() {
                    account.update_mark_price(&ctx.instrument.id, *mark_price);
                }
            }
            MarketPayload::Funding { mark_price, .. } => {
                self.mark_price = Some(*mark_price);
            }
            MarketPayload::TradingStatus { status, .. } => {
                use contracts::TradingStatusKind;
                self.is_halted = matches!(
                    status,
                    TradingStatusKind::Halted | TradingStatusKind::Suspended
                );
            }
            _ => {} // other payload types are informational for Engine A
        }
    }

    fn submit_order(&mut self, order: Order, ctx: &mut EngineContext) -> OrderResult {
        let ts = ctx.clock.current_ts();
        let instrument = ctx.instrument;

        // Reject if halted
        if self.is_halted {
            let tr = TradeRecord {
                ts_decision: ts,
                ts_fill: ts,
                instrument_id: order.instrument_id.clone(),
                side: order.side.clone(),
                order_type: order.order_type.clone(),
                fill_price: Decimal::ZERO,
                fill_qty: Decimal::ZERO,
                fees: Decimal::ZERO,
                slippage_bps: Decimal::ZERO,
                partial: false,
                fidelity: FidelityLevel::Bar,
                rejected: true,
                reject_reason: Some("trading halted".to_string()),
            };
            ctx.trade_records.push(tr);
            return OrderResult::Rejected("trading halted".to_string());
        }

        // Reject unsupported order types
        if !self.supports_order_type(&order.order_type) {
            let reason = format!("unsupported order type: {:?}", order.order_type);
            let tr = TradeRecord {
                ts_decision: ts,
                ts_fill: ts,
                instrument_id: order.instrument_id.clone(),
                side: order.side.clone(),
                order_type: order.order_type.clone(),
                fill_price: Decimal::ZERO,
                fill_qty: Decimal::ZERO,
                fees: Decimal::ZERO,
                slippage_bps: Decimal::ZERO,
                partial: false,
                fidelity: FidelityLevel::Bar,
                rejected: true,
                reject_reason: Some(reason.clone()),
            };
            ctx.trade_records.push(tr);
            return OrderResult::Rejected(reason);
        }

        let active_ts = ts + self.latency_ns;
        let fee_schedule = instrument.fee_schedule.as_ref();
        let taker_override = self.effective_taker_bps(instrument);

        match &order.order_type {
            OrderType::Market => {
                // With book data: fill immediately
                if !self.book.is_empty() {
                    let fill_opt =
                        fill_l2(&order, &self.book, active_ts, fee_schedule, taker_override)
                            .or_else(|| {
                                fill_l1(&order, &self.book, active_ts, fee_schedule, taker_override)
                            });

                    if let Some(fill) = fill_opt {
                        let tr = TradeRecord {
                            ts_decision: ts,
                            ts_fill: active_ts,
                            instrument_id: order.instrument_id.clone(),
                            side: order.side.clone(),
                            order_type: order.order_type.clone(),
                            fill_price: fill.fill_price,
                            fill_qty: fill.fill_qty,
                            fees: fill.fees,
                            slippage_bps: fill.slippage_bps,
                            partial: fill.partial,
                            fidelity: fill.fidelity.clone(),
                            rejected: false,
                            reject_reason: None,
                        };
                        if let Some(account) = ctx.account.as_mut() {
                            account.report_fill(&fill);
                        }
                        let is_partial = fill.partial;
                        ctx.trade_records.push(tr);
                        let result = if is_partial {
                            OrderResult::PartialFill(fill.clone())
                        } else {
                            OrderResult::Filled(fill.clone())
                        };
                        ctx.fills.push(fill);
                        return result;
                    }
                }
                // No book data or insufficient: buffer for next bar
                self.pending_market_orders.push((order, ts));
                OrderResult::Resting
            }

            OrderType::Limit => {
                // Try immediately if marketable
                if !self.book.is_empty() {
                    if let Some(fill) =
                        try_fill_limit(&order, &self.book, active_ts, fee_schedule, taker_override)
                    {
                        let tr = TradeRecord {
                            ts_decision: ts,
                            ts_fill: active_ts,
                            instrument_id: order.instrument_id.clone(),
                            side: order.side.clone(),
                            order_type: order.order_type.clone(),
                            fill_price: fill.fill_price,
                            fill_qty: fill.fill_qty,
                            fees: fill.fees,
                            slippage_bps: fill.slippage_bps,
                            partial: fill.partial,
                            fidelity: fill.fidelity.clone(),
                            rejected: false,
                            reject_reason: None,
                        };
                        if let Some(account) = ctx.account.as_mut() {
                            account.report_fill(&fill);
                        }
                        let is_partial = fill.partial;
                        ctx.trade_records.push(tr);
                        let result = if is_partial {
                            OrderResult::PartialFill(fill.clone())
                        } else {
                            OrderResult::Filled(fill.clone())
                        };
                        ctx.fills.push(fill);
                        return result;
                    }
                }

                // Cancel IOC/FOK if can't fill immediately
                match order.time_in_force {
                    TimeInForce::Ioc | TimeInForce::Fok => {
                        let reason = format!(
                            "{:?} order could not be filled immediately",
                            order.time_in_force
                        );
                        let tr = TradeRecord {
                            ts_decision: ts,
                            ts_fill: ts,
                            instrument_id: order.instrument_id.clone(),
                            side: order.side.clone(),
                            order_type: order.order_type.clone(),
                            fill_price: Decimal::ZERO,
                            fill_qty: Decimal::ZERO,
                            fees: Decimal::ZERO,
                            slippage_bps: Decimal::ZERO,
                            partial: false,
                            fidelity: FidelityLevel::Bar,
                            rejected: true,
                            reject_reason: Some(reason.clone()),
                        };
                        ctx.trade_records.push(tr);
                        return OrderResult::Rejected(reason);
                    }
                    _ => {}
                }

                // Rest the order
                self.resting_orders.push(RestingOrder {
                    order,
                    submitted_ts: ts,
                    active_ts,
                });
                OrderResult::Resting
            }

            OrderType::Stop | OrderType::StopLimit => {
                // Stop orders always rest until triggered
                self.resting_orders.push(RestingOrder {
                    order,
                    submitted_ts: ts,
                    active_ts,
                });
                OrderResult::Resting
            }
        }
    }

    fn settle(&mut self, ctx: &mut EngineContext) {
        let ts = ctx.clock.current_ts();
        let instrument = ctx.instrument;
        let fee_schedule = instrument.fee_schedule.as_ref();
        let taker_override = self.effective_taker_bps(instrument);

        // Get settle price
        let settle_price = self
            .mark_price
            .or(self.last_trade_price)
            .or_else(|| self.last_bar.as_ref().map(|b| b.close))
            .unwrap_or(Decimal::ZERO);

        if settle_price.is_zero() {
            return;
        }

        // Force-close any remaining positions via the account port
        if let Some(account) = ctx.account.as_mut() {
            let positions = account.positions();
            for (instrument_id, position) in &positions {
                if !position.qty.is_zero() {
                    let _ = account.force_close(instrument_id, settle_price, "run_end");
                }
            }
        }

        // Cancel all resting orders
        for resting_order in self.resting_orders.drain(..) {
            let order = &resting_order.order;
            let tr = TradeRecord {
                ts_decision: resting_order.submitted_ts,
                ts_fill: ts,
                instrument_id: order.instrument_id.clone(),
                side: order.side.clone(),
                order_type: order.order_type.clone(),
                fill_price: Decimal::ZERO,
                fill_qty: Decimal::ZERO,
                fees: Decimal::ZERO,
                slippage_bps: Decimal::ZERO,
                partial: false,
                fidelity: FidelityLevel::Bar,
                rejected: true,
                reject_reason: Some("cancelled: run ended".to_string()),
            };
            ctx.trade_records.push(tr);
        }

        // Fill any pending market orders at settle price
        let pending = std::mem::take(&mut self.pending_market_orders);
        for (order, decision_ts) in pending {
            let fill = fill_at_price(
                &order,
                settle_price,
                ts,
                fee_schedule,
                taker_override,
                settle_price,
            );
            let tr = TradeRecord {
                ts_decision: decision_ts,
                ts_fill: ts,
                instrument_id: order.instrument_id.clone(),
                side: order.side.clone(),
                order_type: order.order_type.clone(),
                fill_price: fill.fill_price,
                fill_qty: fill.fill_qty,
                fees: fill.fees,
                slippage_bps: fill.slippage_bps,
                partial: fill.partial,
                fidelity: fill.fidelity.clone(),
                rejected: false,
                reject_reason: None,
            };
            if let Some(account) = ctx.account.as_mut() {
                account.report_fill(&fill);
            }
            ctx.fills.push(fill);
            ctx.trade_records.push(tr);
        }
    }

    fn supports_order_type(&self, t: &OrderType) -> bool {
        matches!(
            t,
            OrderType::Market | OrderType::Limit | OrderType::Stop | OrderType::StopLimit
        )
    }

    fn current_fidelity(&self) -> FidelityLevel {
        self.fidelity.clone()
    }
}
