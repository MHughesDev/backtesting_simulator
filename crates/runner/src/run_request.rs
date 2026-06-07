use rust_decimal::Decimal;

use contracts::{Account, Instrument, MarketEvent, Timestamp};
use strategy::Strategy;

/// A complete specification for a single simulation run.
///
/// The runner validates this, creates engines, and executes the event loop.
/// No state is stored by the simulator itself — account state lives in the
/// injected Account implementation. (ADR-0005, ADR-0010)
pub struct RunRequest {
    /// Unique identifier for this run (for logging/reproducibility).
    pub request_id: String,

    /// All instruments that may appear in the event stream.
    pub instruments: Vec<Instrument>,

    /// The event stream. Will be sorted by (ts_event, instrument_id, seq).
    pub events: Vec<MarketEvent>,

    /// The strategy to run.
    pub strategy: Box<dyn Strategy>,

    /// Account implementation. If None, a SimpleAccount is used.
    pub account: Option<Box<dyn Account>>,

    /// Simulation start timestamp (nanoseconds UTC). Events before this are used for warmup.
    pub time_start: Timestamp,

    /// Simulation end timestamp (nanoseconds UTC).
    pub time_end: Timestamp,

    /// Warmup start timestamp. Events from here to `time_start` warm up indicators.
    /// If None, warmup starts at `time_start`.
    pub warmup_start: Option<Timestamp>,

    /// Random seed for deterministic simulation.
    pub seed: u64,

    /// Order submission latency in nanoseconds.
    pub latency_ns: i64,

    /// Override taker fee in basis points (overrides per-instrument fee schedule).
    pub taker_fee_bps: Option<Decimal>,
}
