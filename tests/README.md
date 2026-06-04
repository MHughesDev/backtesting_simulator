# tests/

Cross-crate integration and **contract-conformance** tests — the checks that span crates or
exercise the Python boundary, complementing each crate's own unit tests.

Of particular importance:
- **Conformance tests** that a caller's data satisfies a required-data manifest, and that the
  suite rejects under-specified runs with precise errors.
- **Determinism / look-ahead** tests: identical inputs produce identical results; no event may
  observe data dated after its `ts_event`.

Uses only synthetic `fixtures/`.
