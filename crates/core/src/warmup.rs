/// A gate that opens once the simulation clock has passed the warmup period.
/// Strategies should only act when this gate is open.
#[derive(Debug, Clone)]
pub struct WarmupGate {
    warmup_end_ts: i64,
    open: bool,
}

impl WarmupGate {
    /// Create a new warmup gate that opens at `warmup_end_ts` (nanoseconds UTC).
    pub fn new(warmup_end_ts: i64) -> Self {
        Self {
            warmup_end_ts,
            open: false,
        }
    }

    /// Update gate state based on current simulation time.
    pub fn update(&mut self, current_ts: i64) {
        self.open = current_ts >= self.warmup_end_ts;
    }

    /// Returns true if warmup has completed and strategies may trade.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Returns the timestamp at which the gate will open.
    pub fn warmup_end_ts(&self) -> i64 {
        self.warmup_end_ts
    }
}
