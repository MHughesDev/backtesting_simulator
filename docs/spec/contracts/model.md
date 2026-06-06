# Contract Spec: AI Endpoint (Model Port)

**Type:** Integration (external AI/ML inference dependency)  
**Status:** ✅ Defined  

An **AI endpoint** is an AI/ML inference dependency that a strategy calls. The suite **does
not own, store, or train** endpoints — it invokes them through this contract. Endpoints are
referenced from a strategy's `ai_endpoints` block (see [strategy.md](strategy.md) §8) and
resolved at runtime by the caller (the trading platform).

An endpoint may be a single pre-trained model, an agent runtime (LangGraph, AutoGen, etc.),
or a multi-step pipeline. All three satisfy the same interface. The `endpoint_type` field
(informational) names the kind; the `scope` field (safety-critical) declares what the endpoint
may access during inference.

This contract makes "the strategy JSON can call an AI function by passing an endpoint ID and
the data needed each inference" precise.

See [ADR-0006](../../adr/0006-model-inference-and-training.md).

---

## 1. Principles

1. **Inference-by-default.** Endpoints arrive ready to use. The normal path is: load → infer.
2. **One interface, all endpoint types.** Whether the endpoint is a single model, an agent
   runtime, or a multi-step pipeline, the suite calls the same `infer()` method. The
   distinction is the `endpoint_type` field (informational) and the `scope` field (safety).
3. **The suite owns no weights and runs no agents.** It receives a resolvable handle
   (`endpoint_id` + `version`), not model files or agent code. Where the implementation lives
   is the platform's concern.
4. **Point-in-time and deterministic.** Inference sees only `ts_event ≤ current_ts` data, runs
   under a seed, and is pinned to a version — so a backtest is reproducible.

---

## 2. The interface

The suite drives any endpoint that satisfies this interface (Rust trait shown; the Python SDK
and remote adapters mirror it):

```rust
trait AIEndpoint {
    /// Identity — pinned for reproducibility.
    fn id(&self) -> EndpointId;
    fn version(&self) -> Version;

    /// Informational: what kind of endpoint this is.
    fn endpoint_type(&self) -> EndpointType;   // Model | AgentRuntime | Pipeline

    /// Safety-critical: what the endpoint may access at call time (see §6).
    fn scope(&self) -> EndpointScope;

    /// Pure inference. `inputs` are already PIT-bound by the engine.
    /// `context` carries the optional point-in-time exogenous bundles (news/social/media
    /// references) the strategy requested via `context_inputs` — empty when none were declared.
    /// Must be deterministic given the same inputs + context + seed.
    fn infer(&self, fn_name: &str, inputs: &FeatureFrame, context: &ContextBundle,
             ctx: &InferContext) -> Inference;
}
```

```rust
struct Inference {
    value:      Value,              // scalar, vector, or label — the primary output
    confidence: Option<f64>,        // optional confidence / probability
    extra:      Map<String, Value>, // any additional named outputs
}

/// The point-in-time exogenous bundle assembled per inference call from the strategy's
/// `context_inputs` (see strategy.md §8.2, signals.md §5). Every item satisfies
/// ts_available ≤ current_ts — the engine cannot hand the endpoint anything not yet knowable.
struct ContextBundle {
    items: Map<String, Vec<ExogenousItem>>,   // keyed by the context_inputs name (e.g. "recent_posts")
}

/// One exogenous item. Numeric/categorical features arrive resolved; raw media/text arrives
/// as a reference (URI + modality) that the ENDPOINT — not the suite — loads and decodes.
enum ExogenousItem {
    Signal   { value: Value, ts_available: i64 },
    Document { features: Map<String, Value>, uri: Option<String>, ts_available: i64 },
    Media    { modality: Modality, uri: String, features: Map<String, Value>, ts_available: i64 },
}
```

---

## 3. How a strategy calls an endpoint

The mapping from the strategy JSON `ai_endpoints` node to this interface:

| Strategy JSON field | Maps to |
|---|---|
| `endpoint_id` + `version` | `AIEndpoint::id()` / `version()` the platform resolves |
| `endpoint_type` | `AIEndpoint::endpoint_type()` (informational) |
| `scope` | `AIEndpoint::scope()` — validated at run start |
| `inference_fn` | `fn_name` argument to `infer` |
| `inputs` (feature bindings) + `input_window` | the PIT `FeatureFrame` passed to `infer` |
| `context_inputs` (PIT exogenous bundles) | the `ContextBundle` passed to `infer` (§10) |
| `frequency` | when the engine calls `infer` |
| `outputs.value` / `outputs.confidence` | names bound from `Inference.value` / `.confidence` |
| `fallback` | what the engine does when `infer` errors or output is stale |
| `determinism.seed` | `InferContext.seed` |

The engine assembles the `FeatureFrame` from the bound inputs (all PIT-checked), calls
`infer(fn_name, frame, bundle, ctx)` at the declared frequency, and publishes the outputs under
the declared names for downstream stages (alpha, sizing) to bind.

---

## 4. Inference frequency

Inference is often the most expensive per-event operation, so frequency is explicit:

| Form | Meaning |
|---|---|
| `{ "on_event": "Bar" }` | Infer on every event of that type |
| `{ "every": "1d", "at": "session_open" }` | Time-scheduled |
| `{ "every_n_events": 5 }` | Every Nth event |
| `{ "cron": "0 0 * * MON" }` | Cron-scheduled |

Between inferences, the last output is held (subject to `fallback.max_staleness`).

---

## 5. Market clock behavior during inference

The simulation clock is the **historical event stream** and advances only when the next market
event arrives — it has no relationship to wall-clock time or inference duration.

When the engine calls `infer()`:

- The clock is paused at `current_ts` (the timestamp of the market event that triggered the
  call).
- `infer()` is **synchronous from the event loop's perspective**: it runs, returns, and the
  engine then processes the next historical event at its actual historical timestamp.
- **No artificial time is injected** based on how long inference takes. The next event's
  `ts_event` is whatever it is in the historical data — the engine does not add a latency
  offset to simulate "the model was slow."

This keeps the clock a faithful replay of history. Order-submission-to-fill latency
(`execution_defaults.latency`) is a separate real market mechanic — it reflects the time between
a strategy's order submission and the exchange processing it, observable in historical data.
That is entirely unrelated to AI inference duration.

Agent runtimes and pipelines with multiple internal steps follow the same rule: however many
steps the endpoint internally executes, the simulation clock does not move until `infer()`
returns and the next historical market event is processed.

---

## 6. Scope

The `scope` field declares what an endpoint may access when `infer()` is called. It is
**validated at run start** and enforced by the suite.

| `scope` | What the endpoint may access | Backtest |
|---|---|---|
| `data_scoped` | Only the `FeatureFrame` and `ContextBundle` passed to `infer()` | ✅ Always valid |
| `archived_tools_only` | Caller-managed historical archives via the signals plane, with `ts_available` enforcement | ✅ Valid when archives are point-in-time correct |
| `live_external` | Live internet, live APIs, external services | ❌ Rejected at validation for all backtest runs |

`live_external` is rejected at validation — not just warned — because an endpoint with live
internet access makes the backtest non-reproducible and non-point-in-time by construction. If
an endpoint declares `live_external`, the run is rejected with a `ScopeViolation` error before
any data is processed.

`archived_tools_only` is for agent runtimes or pipelines that use a tool-calling interface to
access historical data. The caller is responsible for ensuring the archive respects
`ts_available` — the suite enforces look-ahead only on the `FeatureFrame` and `ContextBundle`
it assembles; it cannot enforce it on data a caller-managed tool returns.

---

## 7. Fallback behavior

Inference can fail: the endpoint is unreachable, returns an error, or its last output is too
stale. The strategy declares the policy; the engine enforces it:

| Policy | Behavior |
|---|---|
| `use_last` | Reuse the last successful output (until `max_staleness`) |
| `use_default` | Substitute `default_value` |
| `skip_trade` | Produce no insight/order this cycle |
| `halt` | Stop the run with a typed error |

A run that silently traded on stale or missing endpoint output would be a correctness bug;
fallback makes the choice explicit and recorded in results.

---

## 8. Adapters (caller-side, illustrative)

The platform implements `AIEndpoint` over whatever runtime it uses. The suite ships interface
definitions, not runtimes. Any adapter that satisfies the trait can be wired in:

| Adapter | Use |
|---|---|
| ONNX Runtime (`ort`) | Portable pre-trained inference in-process |
| TorchScript / LibTorch (`tch`) | PyTorch models in-process |
| Python callable (PyO3) | A Python function, model object, or callable pipeline |
| Remote model server | HTTP/gRPC model server (caller manages credentials) |
| Agent runtime (LangGraph, AutoGen, etc.) | Multi-step agent wrapped in the `infer()` interface |
| Custom pipeline | Any caller-defined multi-step computation satisfying the trait |

Remote and agent adapters may introduce non-determinism. For reproducible backtests, prefer
in-process pinned-version adapters and `data_scoped` scope.

---

## 9. Reproducibility checklist

A backtest using AI endpoints is reproducible only if all hold:

- `version` is pinned and resolves to identical weights/code every time.
- `infer()` is deterministic under the supplied seed.
- All inputs are point-in-time.
- For `archived_tools_only` endpoints, the caller-managed archive is itself point-in-time
  correct.

The suite enforces PIT on its own data surfaces (`FeatureFrame`, `ContextBundle`) and threading
determinism; **weight/code/version stability is the platform's responsibility** (see open
questions in [strategy.md](strategy.md) §16).

---

## 10. Multimodal / context-bundle inference

An endpoint node can ask for **point-in-time exogenous context** beyond its structured `inputs`
— the news, social posts, images, or videos that existed at time *t*. This is how an endpoint
scores a listing off its description and photos, or weighs a stock on the headlines published
that hour. The strategy declares this with `context_inputs` (see [strategy.md](strategy.md)
§8.2); the engine assembles a `ContextBundle` per inference call and passes it to `infer()`.

The division of labor is deliberate and preserves "the suite owns no data and no endpoints":

- **The suite** collects, per `context_inputs` entry, every exogenous record with
  `ts_available ≤ current_ts` inside the declared `lookback` window (capped by `max_items`),
  orders it deterministically by `(ts_available, source_id, seq)`, and hands it over as the
  `ContextBundle`. It never decodes media.
- **The endpoint (caller code)** receives the bundle. For `Signal`/`Document` items it gets
  resolved features directly; for `Media` and `Document.uri` items it gets a **reference** (a
  URI + modality) and is responsible for loading and decoding the raw bytes itself.

Two guarantees the engine still enforces: **look-ahead safety** (nothing with
`ts_available > current_ts` can appear in a bundle — described in [signals.md](signals.md) §1)
and **determinism** (the same data + seed yields the same bundle, so inference is reproducible
provided the endpoint itself is seeded). Full mechanics: [signals.md](signals.md) §5.
