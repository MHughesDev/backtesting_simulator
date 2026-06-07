use contracts::SimError;

/// Monotonic nanosecond simulation clock. Enforces the invariant that time
/// only moves forward. (ADR-0001: deterministic, monotonic ts_event clock)
#[derive(Debug, Clone)]
pub struct SimulationClock {
    current_ts: i64,
}

impl SimulationClock {
    /// Create a new clock starting at `start_ts` (nanoseconds UTC).
    pub fn new(start_ts: i64) -> Self {
        Self {
            current_ts: start_ts,
        }
    }

    /// Current simulation time in nanoseconds UTC.
    pub fn current_ts(&self) -> i64 {
        self.current_ts
    }

    /// Advance the clock to `new_ts`. Returns an error if `new_ts < current_ts`
    /// (monotonic invariant violation).
    pub fn advance(&mut self, new_ts: i64) -> Result<(), SimError> {
        if new_ts < self.current_ts {
            return Err(SimError::ValidationError(format!(
                "clock went backwards: {} < {}",
                new_ts, self.current_ts
            )));
        }
        self.current_ts = new_ts;
        Ok(())
    }

    /// Returns true if `ts_event` is safe to observe (not in the future).
    /// Used to enforce the look-ahead constraint.
    pub fn is_look_ahead_safe(&self, ts_event: i64) -> bool {
        ts_event <= self.current_ts
    }
}
