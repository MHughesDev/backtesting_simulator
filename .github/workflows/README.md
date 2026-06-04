# .github/workflows/

CI workflows. None defined yet. Planned once code lands:

- **build** — Rust workspace + maturin build of the Python extension.
- **test** — `cargo test` + Python tests; contract-conformance and determinism suites.
- **lint** — `cargo clippy` + `rustfmt`; `ruff`/`mypy` for Python.
- **bench-on-PR** — run `benches/` and flag performance regressions.
