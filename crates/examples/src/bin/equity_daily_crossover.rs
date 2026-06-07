/// EMA crossover example: generates ~500 synthetic daily bars for AAPL,
/// runs an EMA10/EMA30 crossover strategy, and prints the MetricsSummary.
///
/// Data generation: simple random walk with seed 42, starting at $150,
/// daily returns ~ N(0.0001, 0.01^2) (approximated with Box-Muller).
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use contracts::{
    Account, AssetClass, CapabilityFlags, Instrument, InstrumentId, MarketEvent, MarketPayload,
    Order, OrderType, PriceFormation, SettlementType, Side, TimeInForce,
};
use runner::{RunRequest, RunResult, SimpleAccount, SingleRun};
use strategy::{BarData, MarketView, Strategy};

// ── Data generation ──────────────────────────────────────────────────────────

/// Generate synthetic OHLCV daily bars using a seeded random walk.
/// Returns a Vec<MarketEvent> sorted by ts_event.
fn generate_synthetic_bars(
    instrument_id: &str,
    venue_id: &str,
    n_bars: usize,
    start_ts_ns: i64,
    seed: u64,
) -> Vec<MarketEvent> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut events = Vec::with_capacity(n_bars);

    let mut price = dec!(150.0);
    let day_ns: i64 = 86_400_000_000_000; // 1 day in nanoseconds

    for i in 0..n_bars {
        let ts = start_ts_ns + (i as i64) * day_ns;

        // Box-Muller transform for normal random variable
        let u1: f64 = rng.gen::<f64>().max(1e-10);
        let u2: f64 = rng.gen::<f64>();
        let z = ((-2.0 * u1.ln()).sqrt()) * (2.0 * std::f64::consts::PI * u2).cos();

        // Daily return: mean=0.0001, std=0.01
        let daily_return = 0.0001 + 0.01 * z;

        let close =
            price * (Decimal::ONE + Decimal::from_f64(daily_return).unwrap_or(Decimal::ZERO));
        let close = close.max(dec!(1.0)); // floor at $1

        // Generate OHLC around close
        let range_pct = Decimal::from_f64(rng.gen::<f64>() * 0.02 + 0.005).unwrap_or(dec!(0.01));
        let range = close * range_pct;

        let open = price; // open at previous close
        let high =
            close.max(open) + range * Decimal::from_f64(rng.gen::<f64>()).unwrap_or(dec!(0.5));
        let low =
            close.min(open) - range * Decimal::from_f64(rng.gen::<f64>()).unwrap_or(dec!(0.5));
        let volume = Decimal::from_f64(rng.gen::<f64>() * 5_000_000.0 + 1_000_000.0)
            .unwrap_or(dec!(1_000_000));

        // Round to 2 decimal places
        let round2 = |d: Decimal| d.round_dp(2);

        events.push(MarketEvent {
            instrument_id: instrument_id.to_string(),
            entity_id: None,
            venue_id: venue_id.to_string(),
            ts_event: ts,
            ts_available: Some(ts),
            ts_recv: ts,
            seq: i as u64,
            source_id: Some("synthetic_generator".to_string()),
            source_version: Some("1.0".to_string()),
            payload: MarketPayload::Bar {
                open: round2(open),
                high: round2(high),
                low: round2(low.max(dec!(1.0))),
                close: round2(close),
                volume: round2(volume),
                interval_secs: 86400,
                adjusted: false,
            },
        });

        price = close;
    }

    events
}

// ── EMA Crossover Strategy ────────────────────────────────────────────────────

struct EmaStrategy {
    fast_period: usize,
    slow_period: usize,
    units: Decimal,

    fast_ema: Option<Decimal>,
    slow_ema: Option<Decimal>,
    bars_seen: usize,

    // Track position: true = long, false = flat
    is_long: bool,
}

impl EmaStrategy {
    fn new(fast_period: usize, slow_period: usize, units: Decimal) -> Self {
        Self {
            fast_period,
            slow_period,
            units,
            fast_ema: None,
            slow_ema: None,
            bars_seen: 0,
            is_long: false,
        }
    }

    fn ema_alpha(period: usize) -> Decimal {
        // k = 2 / (period + 1)
        Decimal::from(2) / Decimal::from(period + 1)
    }

    fn update_ema(prev: Option<Decimal>, price: Decimal, period: usize) -> Decimal {
        let k = Self::ema_alpha(period);
        match prev {
            Some(prev_ema) => price * k + prev_ema * (Decimal::ONE - k),
            None => price,
        }
    }
}

impl Strategy for EmaStrategy {
    fn on_bar(
        &mut self,
        instrument_id: &InstrumentId,
        bar: &BarData,
        _view: &MarketView,
        _account: &dyn Account,
    ) -> Vec<Order> {
        self.bars_seen += 1;
        let price = bar.close;

        let prev_fast = self.fast_ema;
        let prev_slow = self.slow_ema;

        self.fast_ema = Some(Self::update_ema(prev_fast, price, self.fast_period));
        self.slow_ema = Some(Self::update_ema(prev_slow, price, self.slow_period));

        // Need at least slow_period bars before trading
        if self.bars_seen < self.slow_period {
            return vec![];
        }

        let fast = match self.fast_ema {
            Some(f) => f,
            None => return vec![],
        };
        let slow = match self.slow_ema {
            Some(s) => s,
            None => return vec![],
        };

        let prev_fast = match prev_fast {
            Some(f) => f,
            None => return vec![],
        };
        let prev_slow = match prev_slow {
            Some(s) => s,
            None => return vec![],
        };

        let was_above = prev_fast > prev_slow;
        let is_above = fast > slow;

        let mut orders = vec![];

        if !was_above && is_above && !self.is_long {
            // Golden cross: buy
            orders.push(Order {
                instrument_id: instrument_id.clone(),
                side: Side::Buy,
                order_type: OrderType::Market,
                quantity: self.units,
                limit_price: None,
                stop_price: None,
                time_in_force: TimeInForce::Day,
                max_slippage_bps: None,
                submitted_ts: bar.ts,
            });
            self.is_long = true;
        } else if was_above && !is_above && self.is_long {
            // Death cross: sell
            orders.push(Order {
                instrument_id: instrument_id.clone(),
                side: Side::Sell,
                order_type: OrderType::Market,
                quantity: self.units,
                limit_price: None,
                stop_price: None,
                time_in_force: TimeInForce::Day,
                max_slippage_bps: None,
                submitted_ts: bar.ts,
            });
            self.is_long = false;
        }

        orders
    }

    fn warmup_bars_required(&self) -> usize {
        self.slow_period
    }
}

// ── Main ─────────────────────────────────────────────────────────────────────

fn main() {
    // ── 1. Define instrument ──────────────────────────────────────────────
    let instrument = Instrument {
        id: "AAPL@nasdaq.equity".to_string(),
        symbol: "AAPL".to_string(),
        exchange: "nasdaq".to_string(),
        currency: "USD".to_string(),
        asset_class: AssetClass::Equity,
        issuer_id: None,
        entity_id: Some("apple_inc".to_string()),
        price_formation: PriceFormation::Clob,
        capabilities: CapabilityFlags::HasOrderBook | CapabilityFlags::HasTradingStatus,
        tick_size: dec!(0.01),
        lot_size: dec!(1),
        contract_multiplier: Decimal::ONE,
        settlement: SettlementType::Cash,
        expiry_date: None,
        maturity_date: None,
        strike: None,
        option_type: None,
        exercise_style: None,
        initial_margin_rate: None,
        maintenance_margin_rate: None,
        funding_interval_hours: None,
        perp_type: None,
        amm_variant: None,
        pool_address: None,
        token_0: None,
        token_1: None,
        fee_bps: None,
        leverage_factor: None,
        daily_reset: None,
        coupon_rate: None,
        coupon_frequency: None,
        day_count: None,
        par_value: None,
        credit_rating: None,
        category_id: None,
        item_id: None,
        chain_id: None,
        sku_id: None,
        condition_tier: None,
        fee_schedule: None,
        base_currency: None,
        pip_size: None,
        question: None,
        resolution_criteria: None,
        oracle_type: None,
    };

    // ── 2. Generate synthetic bars ────────────────────────────────────────
    // 2024-01-01 in nanoseconds UTC
    let start_2024: i64 = 1_704_067_200_000_000_000_i64;
    // 60 days warmup + 365 days = 425 bars total
    let warmup_bars = 60;
    let total_bars = 425;
    let day_ns: i64 = 86_400_000_000_000;

    let warmup_start_ts = start_2024 - (warmup_bars as i64) * day_ns;
    let time_start = start_2024;
    let time_end = start_2024 + (365i64) * day_ns;

    let events = generate_synthetic_bars(
        "AAPL@nasdaq.equity",
        "nasdaq",
        total_bars,
        warmup_start_ts,
        42,
    );

    println!("Generated {} synthetic daily bars", events.len());

    // ── 3. Create strategy ────────────────────────────────────────────────
    let strategy = EmaStrategy::new(10, 30, dec!(100));

    // ── 4. Create account ─────────────────────────────────────────────────
    let account = SimpleAccount::new(dec!(100_000));

    // ── 5. Build RunRequest ───────────────────────────────────────────────
    let request = RunRequest {
        request_id: "example_ema_crossover_001".to_string(),
        instruments: vec![instrument],
        events,
        strategy: Box::new(strategy),
        account: Some(Box::new(account)),
        time_start,
        time_end,
        warmup_start: Some(warmup_start_ts),
        seed: 42,
        latency_ns: 0,
        taker_fee_bps: Some(dec!(5)),
    };

    // ── 6. Run simulation ─────────────────────────────────────────────────
    let run = match SingleRun::new(request) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Validation failed: {}", e);
            std::process::exit(1);
        }
    };

    let result: RunResult = run.execute();

    // ── 7. Print results ──────────────────────────────────────────────────
    println!("\n=== EMA Crossover Simulation Results ===");
    println!("Request ID:      {}", result.request_id);
    println!("Total trades:    {}", result.metrics.total_trades);
    println!("Total fills:     {}", result.metrics.total_fills);
    println!("Total P&L:       ${:.2}", result.metrics.total_pnl);
    println!("Total fees:      ${:.2}", result.metrics.total_fees);
    println!("Win rate:        {:.1}%", result.metrics.win_rate * 100.0);
    println!("Avg fill price:  ${:.2}", result.metrics.avg_fill_price);
    println!(
        "Avg slippage:    {:.2} bps",
        result.metrics.avg_slippage_bps
    );
    if let Some(equity) = result.metrics.final_equity {
        println!("Final equity:    ${:.2}", equity);
        let pct_return = (equity - dec!(100_000)) / dec!(100_000) * dec!(100);
        println!("Return:          {:.2}%", pct_return);
    }
    println!("========================================\n");

    // Print individual trades
    if !result.trade_records.is_empty() {
        println!("Trade log ({} entries):", result.trade_records.len());
        for (i, tr) in result.trade_records.iter().enumerate().take(20) {
            if !tr.rejected && tr.fill_qty > Decimal::ZERO {
                println!(
                    "  [{:3}] {:?} {:.0} @ ${:.2} | fees=${:.2} | slippage={:.2}bps",
                    i + 1,
                    tr.side,
                    tr.fill_qty,
                    tr.fill_price,
                    tr.fees,
                    tr.slippage_bps
                );
            }
        }
        if result.trade_records.len() > 20 {
            println!("  ... ({} more)", result.trade_records.len() - 20);
        }
    }
}
