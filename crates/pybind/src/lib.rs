// Python bindings skeleton for the trading simulator.
//
// This crate will use PyO3/maturin to expose the simulator to Python.
// See Plan 0006 for the full specification.
//
// Currently this is a skeleton — no actual Python bindings are implemented.
// The crate exists to validate the dependency graph and reserve the namespace.

// Future: #[pyo3::pymodule] and PyO3 bindings go here.
// For now, re-export the key types so this crate at least compiles.
pub use contracts;
pub use runner::{RunRequest, RunResult, SimpleAccount, SingleRun};
