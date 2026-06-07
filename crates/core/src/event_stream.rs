use contracts::{MarketEvent, SimError};

/// An ordered stream of market events. Events are sorted by (ts_event, instrument_id, seq)
/// on construction. Yields events monotonically to prevent look-ahead.
pub struct EventStream {
    events: Vec<MarketEvent>,
    pos: usize,
}

impl EventStream {
    /// Create a new EventStream. Events are sorted by (ts_event, instrument_id, seq).
    pub fn new(mut events: Vec<MarketEvent>) -> Result<Self, SimError> {
        events.sort_by(|a, b| {
            a.ts_event
                .cmp(&b.ts_event)
                .then(a.instrument_id.cmp(&b.instrument_id))
                .then(a.seq.cmp(&b.seq))
        });
        Ok(Self { events, pos: 0 })
    }

    /// Advance and return the next event, or None if exhausted.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<&MarketEvent> {
        let ev = self.events.get(self.pos)?;
        self.pos += 1;
        Some(ev)
    }

    /// Peek at the next event without advancing.
    pub fn peek(&self) -> Option<&MarketEvent> {
        self.events.get(self.pos)
    }

    /// Returns true if there are no more events.
    pub fn is_empty(&self) -> bool {
        self.pos >= self.events.len()
    }

    /// Total number of events in the stream.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Number of events remaining.
    pub fn remaining(&self) -> usize {
        self.events.len().saturating_sub(self.pos)
    }
}
