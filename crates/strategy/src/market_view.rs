use std::collections::HashMap;

use rust_decimal::Decimal;

use contracts::{InstrumentId, MarketPayload, Timestamp};

/// A snapshot of market data visible to a strategy at a given timestamp.
/// Only contains data that has been observed (no look-ahead).
#[derive(Debug, Clone, Default)]
pub struct MarketView {
    /// Current simulation timestamp (nanoseconds UTC).
    pub ts: Timestamp,

    /// Latest bar for each instrument.
    pub bars: HashMap<InstrumentId, BarData>,

    /// Latest BBO for each instrument.
    pub quotes: HashMap<InstrumentId, QuoteData>,

    /// Latest mark price for each instrument.
    pub marks: HashMap<InstrumentId, Decimal>,

    /// Latest trade for each instrument.
    pub last_trades: HashMap<InstrumentId, TradeData>,
}

/// Bar data snapshot.
#[derive(Debug, Clone)]
pub struct BarData {
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
    pub interval_secs: u64,
    pub adjusted: bool,
    pub ts: Timestamp,
}

/// BBO snapshot.
#[derive(Debug, Clone)]
pub struct QuoteData {
    pub bid: Decimal,
    pub bid_size: Decimal,
    pub ask: Decimal,
    pub ask_size: Decimal,
    pub ts: Timestamp,
}

/// Trade snapshot.
#[derive(Debug, Clone)]
pub struct TradeData {
    pub price: Decimal,
    pub size: Decimal,
    pub ts: Timestamp,
}

impl MarketView {
    pub fn new(ts: Timestamp) -> Self {
        Self {
            ts,
            ..Default::default()
        }
    }

    /// Update the view with a new market event payload.
    pub fn update(&mut self, instrument_id: &InstrumentId, ts: Timestamp, payload: &MarketPayload) {
        self.ts = ts;
        match payload {
            MarketPayload::Bar {
                open,
                high,
                low,
                close,
                volume,
                interval_secs,
                adjusted,
            } => {
                self.bars.insert(
                    instrument_id.clone(),
                    BarData {
                        open: *open,
                        high: *high,
                        low: *low,
                        close: *close,
                        volume: *volume,
                        interval_secs: *interval_secs,
                        adjusted: *adjusted,
                        ts,
                    },
                );
                self.marks.insert(instrument_id.clone(), *close);
            }
            MarketPayload::Quote {
                bid,
                bid_size,
                ask,
                ask_size,
            } => {
                self.quotes.insert(
                    instrument_id.clone(),
                    QuoteData {
                        bid: *bid,
                        bid_size: *bid_size,
                        ask: *ask,
                        ask_size: *ask_size,
                        ts,
                    },
                );
            }
            MarketPayload::Trade { price, size, .. } => {
                self.last_trades.insert(
                    instrument_id.clone(),
                    TradeData {
                        price: *price,
                        size: *size,
                        ts,
                    },
                );
                self.marks.insert(instrument_id.clone(), *price);
            }
            MarketPayload::Mark { price }
            | MarketPayload::MarkUpdate {
                mark_price: price, ..
            } => {
                self.marks.insert(instrument_id.clone(), *price);
            }
            _ => {}
        }
    }
}
