use std::collections::HashMap;

use rust_decimal_macros::dec;

use contracts::{Fill, Instrument, MarketPayload, PriceFormation, SimError, TradeRecord};
use core_lib::{EventStream, ScopedRng, SimulationClock, WarmupGate};
use engines::{Engine, EngineContext, OrderBookEngine};
use metrics::{compute_metrics, MetricsSummary};
use strategy::MarketView;

use crate::account::SimpleAccount;
use crate::run_request::RunRequest;
use crate::validator::ContractValidator;

/// The result of a completed simulation run.
pub struct RunResult {
    pub request_id: String,
    pub trade_records: Vec<TradeRecord>,
    pub fills: Vec<Fill>,
    pub metrics: MetricsSummary,
}

/// Single-run orchestrator. Validates the request, creates engines, runs the
/// event loop, and returns a RunResult.
pub struct SingleRun {
    request: RunRequest,
}

impl SingleRun {
    /// Create a new SingleRun, validating the request.
    /// Returns SimError if validation fails (SC-3: loud failure on under-specification).
    pub fn new(request: RunRequest) -> Result<Self, SimError> {
        ContractValidator::validate(&request)?;
        Ok(Self { request })
    }

    /// Execute the simulation run.
    pub fn execute(self) -> RunResult {
        let request = self.request;
        let request_id = request.request_id.clone();

        // Build event stream (sorted)
        let event_stream = EventStream::new(request.events).expect("event stream sort failed");

        // Build instrument map
        let instrument_map: HashMap<String, Instrument> = request
            .instruments
            .into_iter()
            .map(|i| (i.id.clone(), i))
            .collect();

        // Create engines (one per instrument)
        let mut engines: HashMap<String, Box<dyn Engine>> = HashMap::new();
        for (id, instrument) in &instrument_map {
            let engine: Box<dyn Engine> = match instrument.price_formation {
                PriceFormation::Clob => Box::new(OrderBookEngine::new(
                    request.latency_ns,
                    request.taker_fee_bps,
                )),
                // Future engines: return empty stubs
                _ => continue,
            };
            engines.insert(id.clone(), engine);
        }

        // Create simulation components
        let clock_start = request.warmup_start.unwrap_or(request.time_start);
        let mut clock = SimulationClock::new(clock_start);
        let mut warmup_gate = WarmupGate::new(request.time_start);
        let mut rng = ScopedRng::new(request.seed);

        // Create account (use provided or create SimpleAccount)
        let mut account: Box<dyn contracts::Account> = request
            .account
            .unwrap_or_else(|| Box::new(SimpleAccount::new(dec!(100_000))));

        // Output buffers
        let mut all_fills: Vec<Fill> = Vec::new();
        let mut all_trade_records: Vec<TradeRecord> = Vec::new();

        // Market view for strategy
        let mut market_view = MarketView::new(clock_start);

        // Strategy
        let mut strategy = request.strategy;

        // Event loop
        let mut events = event_stream;
        while let Some(ev) = events.next() {
            // Skip events outside the run window
            if ev.ts_event > request.time_end {
                break;
            }

            // Advance clock (monotonic)
            let _ = clock.advance(ev.ts_event);

            // Update warmup gate
            warmup_gate.update(clock.current_ts());

            let instrument_id = ev.instrument_id.clone();
            let ts = ev.ts_event;

            // Update market view
            market_view.update(&instrument_id, ts, &ev.payload);

            // Get engine for this instrument
            let Some(engine) = engines.get_mut(&instrument_id) else {
                continue;
            };

            let Some(instrument) = instrument_map.get(&instrument_id) else {
                continue;
            };

            // Per-event fills buffer (for EngineContext)
            let mut event_fills: Vec<Fill> = Vec::new();
            let mut event_trade_records: Vec<TradeRecord> = Vec::new();

            // Call engine.on_event
            // We need to pass account as Option<&mut dyn Account>
            // To avoid borrow conflicts, we'll use unsafe tricks OR restructure.
            // Safe approach: pass account separately
            {
                let mut ctx = EngineContext {
                    clock: &clock,
                    instrument,
                    account: Some(account.as_mut()),
                    fills: &mut event_fills,
                    trade_records: &mut event_trade_records,
                    rng: &mut rng,
                };
                engine.on_event(ev, &mut ctx);
            }

            all_fills.append(&mut event_fills);
            all_trade_records.append(&mut event_trade_records);

            // If warmup gate is open and this is a Bar event, call strategy
            if warmup_gate.is_open() {
                if let MarketPayload::Bar {
                    open,
                    high,
                    low,
                    close,
                    volume,
                    interval_secs,
                    adjusted,
                } = &ev.payload
                {
                    let bar = strategy::BarData {
                        open: *open,
                        high: *high,
                        low: *low,
                        close: *close,
                        volume: *volume,
                        interval_secs: *interval_secs,
                        adjusted: *adjusted,
                        ts,
                    };

                    let orders =
                        strategy.on_bar(&instrument_id, &bar, &market_view, account.as_ref());

                    // Submit orders to engine
                    for order in orders {
                        let order_instrument_id = order.instrument_id.clone();
                        let Some(order_engine) = engines.get_mut(&order_instrument_id) else {
                            continue;
                        };
                        let Some(order_instrument) = instrument_map.get(&order_instrument_id)
                        else {
                            continue;
                        };

                        let mut order_fills: Vec<Fill> = Vec::new();
                        let mut order_trade_records: Vec<TradeRecord> = Vec::new();

                        {
                            let mut ctx = EngineContext {
                                clock: &clock,
                                instrument: order_instrument,
                                account: Some(account.as_mut()),
                                fills: &mut order_fills,
                                trade_records: &mut order_trade_records,
                                rng: &mut rng,
                            };
                            order_engine.submit_order(order, &mut ctx);
                        }

                        all_fills.append(&mut order_fills);
                        all_trade_records.append(&mut order_trade_records);
                    }
                }
            }
        }

        // Settle all engines
        let final_ts = request.time_end;
        let _ = clock.advance(final_ts);

        for (instrument_id, engine) in engines.iter_mut() {
            let Some(instrument) = instrument_map.get(instrument_id) else {
                continue;
            };

            let mut settle_fills: Vec<Fill> = Vec::new();
            let mut settle_trade_records: Vec<TradeRecord> = Vec::new();

            {
                let mut ctx = EngineContext {
                    clock: &clock,
                    instrument,
                    account: Some(account.as_mut()),
                    fills: &mut settle_fills,
                    trade_records: &mut settle_trade_records,
                    rng: &mut rng,
                };
                engine.settle(&mut ctx);
            }

            all_fills.append(&mut settle_fills);
            all_trade_records.append(&mut settle_trade_records);
        }

        // Compute metrics
        let metrics = compute_metrics(&all_trade_records, &all_fills, Some(account.as_ref()));

        RunResult {
            request_id,
            trade_records: all_trade_records,
            fills: all_fills,
            metrics,
        }
    }
}
