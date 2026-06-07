use rand::{rngs::StdRng, SeedableRng};

/// A scoped, seeded RNG for deterministic simulation. Each run gets its own
/// ScopedRng seeded from the RunRequest seed. (ADR-0001: deterministic)
pub struct ScopedRng(StdRng);

impl ScopedRng {
    /// Create a new ScopedRng seeded from `seed`.
    pub fn new(seed: u64) -> Self {
        Self(StdRng::seed_from_u64(seed))
    }

    /// Get a mutable reference to the underlying RNG.
    pub fn rng(&mut self) -> &mut StdRng {
        &mut self.0
    }
}

// SAFETY: StdRng is Send but not Sync. We gate mutable access through &mut self,
// so sharing via &ScopedRng is actually unused — this is safe.
unsafe impl Send for ScopedRng {}
unsafe impl Sync for ScopedRng {}
