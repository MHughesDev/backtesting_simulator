# benches/

Performance benchmarks. Speed is a product goal, so it is measured, not assumed.

Tracks, at minimum:
- **Start-to-finish run latency** for representative single backtests.
- **Hot-loop throughput** (events/sec) per engine.
- **Run-queue throughput** — many concurrent backtests (parallel scaling).

Intended to run in CI on PRs to catch regressions.
