# Spec: COMP-001 — Component Registry

**Spec ID:** COMP-001
**Type:** Component (registry & trust model)
**Status:** Approved
**Date:** 2026-06-06
**Author:** Agent

This document defines what a **component** is, how strategies use components, and the **trust
model** for running them safely. See [ADR-0011](../adr/0011-component-registry-trust-model.md).


---

## 1. What is a "component"?

A **component** is a small, named, reusable building block that fills **one slot** in the
strategy pipeline. The strategy JSON contains **no logic of its own** — it *wires together*
components by ID and configures them. The components do the actual computing.

Concrete examples, by pipeline stage:

| Stage | Example components |
|---|---|
| `universe` | `top_n_by_volume`, `index_membership` (a selector) |
| `features` | `RSI`, `MACD`, `STDDEV`, `news_embedder` (an indicator/feature) |
| `alpha` | a factor model that turns features into a signal |
| `sizing` | `volatility_target`, `kelly`, a custom position sizer |
| `risk` | a custom constraint |

> **Analogy.** The strategy JSON is a **recipe**; components are the **appliances** (blender,
> oven, scale). The recipe says "blend for 30s at speed 4" — it references the blender and its
> settings, but it does not contain the blender's motor. Swap in a better blender without
> rewriting the recipe.

A component is, formally, a **pure function with optional state**:
`(inputs, params, prior_state) → (outputs, new_state)`.

> Note: AI **models** are a *separate* concept (the `Model` port — see
> [contracts/model.md](INTG-002-ai-model-inference-port.md)). A model is not a component; but a component may
> read a model's output as one of its inputs.

---

## 2. Built-in vs. custom components

| | **Built-in** | **Custom** |
|---|---|---|
| Who writes it | The simulator (a standard library) | You, your platform's users, or an AI |
| Referenced by | `type` (e.g. `"type": "RSI"`) | `ref` (e.g. `"ref": "my_factor_model"`) |
| Bound in Run Request | No binding needed | Bound under `components` with a `kind` |
| Trust | Audited, first-party | Depends on who wrote it (§4) |

Most strategies should be expressible with **built-ins + the JSON expression language alone**.
Custom components are the exception, not the rule.

---

## 3. The component contract (every component must obey)

Because components run **inside the deterministic event loop**, every component — built-in or
custom — must be:

1. **Pure** — no wall-clock reads, no network/disk I/O, no unseeded randomness.
2. **Point-in-time** — sees only the data it is handed; cannot peek at the future.
3. **Parallel-safe** — no shared mutable global state (the run queue runs many in parallel).
4. **Deterministic** — same inputs ⇒ same outputs, every time.

These are the same invariants the whole simulator guarantees. A component that violates them would
silently corrupt determinism, reproducibility, or look-ahead safety.

---

## 4. The trust problem

- **Built-in components** — we wrote and audited them. Trusted.
- **Custom components** — *who* wrote them?
  - **First-party** (you / your platform team) → trusted; you control the code.
  - **Untrusted** (an end user of your platform, or AI-generated code) → a buggy or malicious
    one could try to read the clock, hit the network, or read future data, breaking the
    guarantees. We cannot simply hope it behaves — we must **enforce** the contract.

Different trust levels need different handling. That is what the tiers in §5 provide.

---

## 5. Trust tiers (the decision)

Three tiers, by how a component is provided and how its safety is ensured. Plus the expression
language for simple inline logic (no code at all).

| Tier | Form | Trust required | How §3 is ensured | Speed |
|---|---|---|---|---|
| **Expression** | A typed expression string in the JSON | None | Restricted grammar — can't do anything but compute over named values | ⭐ Fastest |
| **Built-in** | Rust, compiled into the simulator | Full (first-party) | Code review + tests | ⭐ Fastest |
| **Trusted plugin** | Native Rust (`cdylib`) | Full (you control it) | You vouch for it; same process | ⭐ Fast |
| **Sandboxed** | **WASM** module | **None** | **Enforced by the sandbox** — the code physically cannot do I/O, read the clock, or touch the network unless the host grants it (we don't) | ✅ Fast (near-native) |

**Python is intentionally excluded from the per-event hot loop** (its boundary cost and the GIL
fight the parallel run queue, and it cannot enforce purity). Python may still be used for
*offline/research* component authoring that compiles down to one of the tiers above.

### What is WASM and a "sandbox"? (plain language)

**WASM** (WebAssembly) is a portable, compiled binary format. You write a component in almost
any language (Rust, C, AssemblyScript, Go…) and compile it to a small `.wasm` file the simulator
can load and run.

A **sandbox** is a sealed room for that code. When the simulator runs a `.wasm` component inside an
embedded WASM runtime (e.g. `wasmtime`), the code inside **can only do arithmetic and talk to
the host through functions we explicitly hand it.** By default it **cannot** read files, open
network connections, read the system clock, or spawn threads. Those abilities simply do not
exist inside the sandbox unless we grant them — and for components, we don't.

> **Analogy.** Running a program you downloaded directly on your laptop gives it the run of your
> machine — fine if you trust it, dangerous if you don't. A WASM sandbox is like making it run
> inside a sealed glovebox that only lets it take numbers in and hand numbers back. Even
> hostile code can't break out.

This is why WASM is the right tier for **untrusted or AI-generated** components: the safety
guarantees (purity, no look-ahead, determinism) hold **by construction**, not by trusting the
author.

---

## 6. Referencing (strategy) and binding (run request)

**In the Strategy JSON** — reference by `type` (built-in) or `ref` (custom):

```jsonc
"features": [
  { "id": "rsi14", "type": "RSI", "input": "data:close", "params": { "period": 14 } },
  { "id": "factor", "ref": "my_factor_model", "input": "feature:rsi14" }
]
```

**In the Run Request** — bind each custom `ref` to a concrete implementation and declare its
trust tier (see [run-request.md](DATA-002-run-request.md) §6):

```jsonc
"components": {
  "my_factor_model": { "kind": "wasm",    "uri": "...", "version": "1.4.0" },
  "top_n_by_volume": { "kind": "builtin" },
  "house_sizer":     { "kind": "native",  "uri": "...", "version": "2.0.1" }
}
```

`kind` ∈ `builtin | native | wasm`. The simulator validates that every `ref` in the strategy is
bound before the run starts.

---

## 7. Versioning & reproducibility

Custom components are **pinned by version**, exactly like models, so a backtest is reproducible:
the same strategy + the same component versions + the same data ⇒ identical results. A
component whose version cannot be resolved is a typed error at run start.

---

## 8. Design guidance

1. **Prefer built-ins + expressions.** Reach for a custom component only when no built-in fits.
2. **Untrusted ⇒ WASM.** Anything authored by a platform user or an AI runs in the sandbox.
3. **First-party hot paths ⇒ native or built-in.** Where you control the code and need maximum
   speed.
4. **Keep components pure and small.** A component is a function, not a program.

---

## 9. Open questions

- Exact WASM **host interface**: which capabilities (if any) are ever granted, and the
  data-marshaling format across the sandbox boundary.
- WASM **performance budget** vs. native for hot indicators (benchmark target).
- Whether built-in indicators are also specified by **canonical formula** to prevent drift
  (Q-REG-4).
