# Sources — Runtime / language selection

Topic 0001. References for the Rust-core-vs-alternatives evaluation.
Facts checked: **2026-06-04**.

---

## S1 — NautilusTrader (existence proof: Rust core + Python bindings)
- **URLs:**
  - https://nautilustrader.io/docs/latest/concepts/architecture/
  - https://github.com/nautechsystems/nautilus_trader
- **Supports:**
  - A production algorithmic-trading + event-driven backtesting platform is built as a
    **Rust-native core** with a Python API — almost exactly the architecture proposed here.
  - Bindings have migrated from **Cython → PyO3** since 2024; the Rust core is exposed via a
    C-FFI (`cbindgen`) / PyO3, and "no Rust toolchain is required at install time."
  - Validates the hybrid pattern (systems-language core, Python edge) for this domain.

## S2 — nanobind benchmarks (C++↔Python binding overhead)
- **URLs:**
  - https://nanobind.readthedocs.io/en/latest/benchmark.html
  - https://nanobind.readthedocs.io/en/latest/why.html
  - https://github.com/wjakob/nanobind
- **Supports:**
  - nanobind reports ~**3×** lower runtime overhead for simple functions and ~**10×** for
    passing classes vs. pybind11; per-instance wrapper overhead 56→24 bytes.
  - Relevant to the *C++ + Python* alternative: modern C++ bindings are cheap, but the
    boundary still matters, reinforcing "keep the per-tick loop on the native side."

## S3 — Python free-threading / GIL (PEP 703, PEP 779)
- **URLs:**
  - https://peps.python.org/pep-0703/
  - https://peps.python.org/pep-0779/
  - https://docs.python.org/3/howto/free-threading-python.html
- **Supports:**
  - Free-threaded (no-GIL) CPython is **experimental in 3.13**, progressing to
    **supported-but-optional in 3.14**, with a ~**5–10%** single-thread penalty.
  - Conclusion: the GIL still constrains a pure-Python run queue today; do not bet the
    concurrency story on free-threading yet. A native core sidesteps this entirely.

## S4 — Polars (Rust-native dataframe / Arrow)
- **URL:** https://www.pola.rs/ · https://github.com/pola-rs/polars
- **Supports:**
  - Polars is written in Rust on Apache Arrow — first-class columnar tooling available
    directly in the Rust core and zero-copy-shareable with Python.

## S5 — QuantLib (mature C++ derivatives library)
- **URL:** https://www.quantlib.org/ · https://github.com/lballabio/QuantLib
- **Supports:**
  - Decades-mature C++ library for derivatives valuation (greeks, vol surfaces, curve
    bootstrapping, exotic payoffs) with SWIG-generated Python bindings. Basis for the
    "C++ wins on derivatives math" point and the optional-plugin idea (ADR-0002 Tier C).

## S6 — Apache Arrow (zero-copy data format)
- **URL:** https://arrow.apache.org/
- **Supports:**
  - Language-agnostic columnar memory format with Rust (`arrow-rs`) and Python (`pyarrow`)
    implementations; enables zero-copy hand-off across the Rust↔Python boundary. Basis for
    classifying Arrow as Tier-A infrastructure in ADR-0002.

> Note: S4–S6 are stable, widely-known project facts used as background; S1–S3 are the
> claims that were explicitly web-verified on the date above.
