# ADR-0011: Component registry trust model — tiered, with a WASM sandbox for untrusted code

- **Status:** Accepted
- **Date:** 2026-06-04
- **Deciders:** Project owner
- **Informed by:** [spec/component-registry.md](../spec/component-registry.md); resolves OD-7 / Q-REG-1

## Context

Strategies are declarative JSON that wire together **components** (indicators, alpha/sizing
functions, universe selectors) by ID. The suite ships built-in components, but custom ones are
needed when built-ins are insufficient. A custom component is **code that runs inside the
deterministic event loop**, so it must be pure, point-in-time, parallel-safe, and deterministic.

The hard question is **who authors components and can they be trusted.** In the end-state
system, components may be written by first-party developers, by the platform's **end users**, or
by **AI**. Untrusted code that can read the clock, do I/O, or peek at future data would silently
break determinism, reproducibility, and look-ahead safety.

## Decision

Adopt a **tiered registry**:

1. **Expression** — a restricted, typed expression grammar inside the JSON for simple inline
   logic. No code; trivially safe.
2. **Built-in** — Rust components compiled into the suite (the standard library). First-party,
   audited; the fast path for the vast majority of strategies.
3. **Trusted plugin** — native Rust (`cdylib`) for first-party extensions where the code is
   controlled and maximum speed is wanted.
4. **Sandboxed (WASM)** — untrusted or AI-authored components run as WebAssembly modules in an
   embedded sandbox (e.g. `wasmtime`). The component contract (purity, no look-ahead,
   determinism) is **enforced by construction**: the sandbox grants no I/O, clock, or network
   access.

**Python is excluded from the per-event hot loop** (boundary cost, GIL contention with the run
queue, and inability to enforce purity). It may be used only for offline authoring that compiles
to a tier above.

Custom components are **version-pinned** for reproducibility, and every referenced component must
be bound before a run starts.

## Alternatives considered

- **Native Rust only** — fastest, but cannot safely run untrusted/AI code (arbitrary native code
  = no sandbox). Rejected as the sole mechanism; kept as the *trusted* tier.
- **Python (PyO3) components** — easiest authoring, but slow in the hot loop, GIL-bound against
  the parallel queue, and cannot enforce determinism/purity. Rejected for the hot loop.
- **Expression-only / no custom code** — maximally safe and simple, but too limited for genuine
  custom alpha. Rejected as the sole mechanism; kept as the simplest tier.
- **WASM for everything** — uniform and safe, but adds a build step and marshaling cost even for
  first-party hot indicators. Rejected as the sole mechanism; used for the *untrusted* tier.

## Consequences

- **Positive:** end users and AI can contribute components **without** compromising the engine's
  guarantees; first-party hot paths stay native-fast; most strategies need no custom code at all;
  reproducibility via version pinning.
- **Negative / accepted tradeoffs:** maintaining multiple execution paths (built-in/native/WASM);
  defining a WASM host interface and data marshaling; a benchmark budget to keep WASM components
  acceptably fast.
- **Follow-ups:** specify the WASM host interface and capabilities (none, by default); the
  marshaling format; whether built-in indicators are pinned to canonical formulas (Q-REG-4).
