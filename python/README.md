# python/ — authoring & integration surface

The Python package (`btsuite`, provisional) that a trading platform and strategy authors use.
Built from the Rust core via PyO3 / maturin. No code yet.

Intended contents:

- **Strategy SDK** — author `Strategy` subclasses in Python; the Rust runtime drives them.
- **High-level API** — load data (Arrow), define instruments, submit runs, collect results.
- **AI-model adapters** — implement the Model Contract over ONNX / TorchScript / remote
  endpoints. The simulator calls models; it never owns weights or trains.

This layer is intentionally thin — per-event hot logic stays in Rust
([ADR-0001](../docs/adr/0001-runtime-rust-python-hybrid.md)).
