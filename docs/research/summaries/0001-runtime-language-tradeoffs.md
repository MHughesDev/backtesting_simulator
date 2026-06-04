# Summary — Runtime / language tradeoffs

Topic 0001. Neutral synthesis of the runtime options for the backtesting core.
Sources: [0001-runtime-language.md](../sources/0001-runtime-language.md). Checked 2026-06-04.

## Options considered

1. **Rust core** (+ PyO3 bindings)
2. **C++ core** (+ pybind11/nanobind bindings)
3. **Rust + Python hybrid** — Rust hot paths, Python API/authoring
4. **C++ + Python hybrid**
5. **Python + vectorization** (NumPy/Polars/Numba)

## Scoring (weighted to project goals)

Dimensions weighted ×3 (speed: hot-loop/latency/queue; strategy authoring by humans + AI;
AI integration; realistic multi-engine sim; embeddability), ×2 (MVP velocity, data,
derivatives, safety, maintainability), ×1 (build tooling, ops). Each cell scored 0–10; see
the conversation log / conclusion for per-cell justification.

| Option | Weighted score |
|---|---|
| **Rust + Python (hybrid)** | **8.4 / 10** |
| C++ + Python (hybrid) | 7.9 / 10 |
| Rust core (only) | 7.9 / 10 |
| C++ core (only) | 7.1 / 10 |
| Python + vectorization | 6.3 / 10 |

## Key findings

- **Rust and C++ tie on raw speed.** The real differentiators are *memory safety*
  (Rust compiler-enforced vs. C++ manual), *build tooling* (cargo ≫ CMake/vcpkg/conan), and
  *quant-library maturity* (C++ wins via QuantLib — S5).
- **The hybrid pattern is proven for this exact domain.** NautilusTrader runs a Rust-native
  event-driven core with a Python API (S1).
- **Pure Python is structurally wrong for the realism + queue goals.** Vectorization can't
  express order-book matching / liquidation cascades without an event loop, and the GIL
  constrains the run queue; free-threading is not yet dependable (S3).
- **The boundary is cheap but real.** Keep the per-tick loop native; cross to Python only at
  edges (S2 shows even C++ bindings have non-zero overhead).
- **Arrow makes the hybrid boundary zero-copy** (S4, S6).

## Residual uncertainty

- Rust's derivatives ecosystem is immature vs. QuantLib; mitigations are (a) build in Rust,
  or (b) call QuantLib's Python bindings on the API side, or (c) optional plugin later.
- The strategy-authoring form (Python callbacks vs. compiled graph/DSL) is unresolved and
  partly governs how much the language choice matters in the hot loop.
