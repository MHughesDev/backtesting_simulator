# Contract Spec: Model

A **model** is an AI/ML inference dependency that a strategy calls. The suite **does not own,
store, or train** models — it invokes them through this contract. Models are referenced from a
strategy's `models` block (see [strategy.md](strategy.md) §8) and resolved at runtime by the
caller (the trading platform).

This contract makes "the strategy JSON can call an AI function by passing a model ID and the
data needed each inference" precise.

See [ADR-0006](../../adr/0006-model-inference-and-training.md).

---

## 1. Principles

1. **Inference-by-default.** Models arrive pre-trained. The normal path is: load → infer.
2. **Training is explicit and opt-in.** The suite never trains unless a strategy's model node
   sets `training.enabled = true`, and even then it only *orchestrates* a caller-provided,
   preconfigured training method.
3. **The suite owns no weights.** It receives a resolvable handle (`model_id` + `version`),
   not model files. Where weights live is the platform's concern.
4. **Point-in-time and deterministic.** Inference and training see only `ts_event ≤ current_ts`
   data, run under a seed, and are pinned to a version — so a backtest is reproducible.

---

## 2. The interface

The suite drives any model that satisfies this interface (Rust trait shown; the Python SDK and
remote adapters mirror it):

```rust
trait Model {
    /// Identity — pinned for reproducibility.
    fn id(&self) -> ModelId;
    fn version(&self) -> Version;

    /// Pure inference. `inputs` are already PIT-bound by the engine.
    /// `context` carries the optional point-in-time exogenous bundles (news/social/media
    /// references) the strategy requested via `context_inputs` — empty when none were declared.
    /// Must be deterministic given the same inputs + context + seed.
    fn infer(&self, fn_name: &str, inputs: &FeatureFrame, context: &ContextBundle,
             ctx: &InferContext) -> Inference;

    /// Optional. Only called when a strategy opts into training.
    /// `train_data` is guaranteed to contain only ts_event ≤ ctx.current_ts.
    fn fit(&mut self, method: &str, train_data: &FeatureFrame, params: &Params, ctx: &FitContext)
        -> Result<(), ModelError>;
}
```

```rust
struct Inference {
    value:      Value,            // scalar, vector, or label — the primary output
    confidence: Option<f64>,      // optional confidence / probability
    extra:      Map<String, Value>, // any additional named outputs
}

/// The point-in-time exogenous bundle assembled per inference call from the strategy's
/// `context_inputs` (see strategy.md §8.3, signals.md §5). Every item satisfies
/// ts_available ≤ current_ts — the engine cannot hand the model anything not yet knowable.
struct ContextBundle {
    items: Map<String, Vec<ExogenousItem>>,   // keyed by the context_inputs name (e.g. "recent_posts")
}

/// One exogenous item. Numeric/categorical features arrive resolved; raw media/text arrives
/// as a reference (URI + modality) that the MODEL — not the suite — loads and decodes.
enum ExogenousItem {
    Signal   { value: Value, ts_available: i64 },
    Document { features: Map<String, Value>, uri: Option<String>, ts_available: i64 },
    Media    { modality: Modality, uri: String, features: Map<String, Value>, ts_available: i64 },
}
```

---

## 3. How a strategy calls a model

The mapping from the strategy JSON `models` node to this interface:

| Strategy JSON field | Maps to |
|---|---|
| `model_id` + `model_version` | `Model::id()` / `version()` the platform resolves |
| `inference_fn` | `fn_name` argument to `infer` |
| `inputs` (feature bindings) + `input_window` | the PIT `FeatureFrame` passed to `infer` |
| `context_inputs` (PIT exogenous bundles) | the `ContextBundle` passed to `infer` (§9) |
| `frequency` | when the engine calls `infer` |
| `outputs.value` / `outputs.confidence` | names bound from `Inference.value` / `.confidence` |
| `fallback` | what the engine does when `infer` errors or output is stale |
| `determinism.seed` | `InferContext.seed` |
| `training` | gates and configures calls to `fit` |

The engine assembles the `FeatureFrame` from the bound inputs (all PIT-checked), calls
`infer(fn_name, frame, ctx)` at the declared frequency, and publishes the outputs under the
declared names for downstream stages (alpha, sizing) to bind.

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

## 5. Fallback behavior

Inference can fail: the model is unreachable, returns an error, or its last output is too
stale. The strategy declares the policy; the engine enforces it:

| Policy | Behavior |
|---|---|
| `use_last` | Reuse the last successful output (until `max_staleness`) |
| `use_default` | Substitute `default_value` |
| `skip_trade` | Produce no insight/order this cycle |
| `halt` | Stop the run with a typed error |

A run that silently traded on stale or missing model output would be a correctness bug;
fallback makes the choice explicit and recorded in results.

---

## 6. Training / fitting (opt-in)

> **Training has its own full spec: [training.md](training.md).** It defines the `Trainer`
> port, the backtest pause-train-resume mechanics, the sync-vs-async asymmetry between backtest
> and live, refit caching, and the shared-package architecture. This section is a summary.

When a strategy opts in (`training.enabled`), the engine:

1. Determines refit points from `schedule` (e.g. rolling 1y window, monthly step).
2. At each refit point, assembles `train_data` containing **only** `ts_event ≤ current_ts`.
3. Calls `fit(method, train_data, params, ctx)` — where `method` names a **registered,
   preconfigured** training routine supplied by the caller.
4. Resumes inference with the refit model.

**The suite owns the *orchestration* (when + with what PIT data); the caller owns the
*method* (how).** This preserves "suite owns no models" while enabling walk-forward and
scenario-specific fitting.

**Invariants:**
- Training data is strictly point-in-time — this is what makes walk-forward leak-free.
- Training is deterministic (seeded).
- Refit results are cached by `(method, data_window, params)` so a parameter sweep does not
  retrain identical configurations.
- `freeze_after_first: true` fits once during warmup, then runs inference-only.

---

## 7. Adapters (caller-side, illustrative)

The platform implements `Model` over whatever runtime it uses. The suite ships interface
definitions, not model runtimes:

| Adapter | Use |
|---|---|
| ONNX Runtime (`ort`) | Portable pre-trained inference in-process |
| TorchScript / LibTorch (`tch`) | PyTorch models in-process |
| Python callable (PyO3) | A Python function/model object |
| Remote endpoint | HTTP/gRPC model server (caller manages credentials/latency) |

Remote adapters introduce non-determinism and latency; for reproducible backtests, prefer
in-process pinned-version adapters.

---

## 8. Reproducibility checklist

A model-using backtest is reproducible only if all hold:

- `model_version` is pinned and resolves to identical weights every time.
- `infer` (and `fit`) are deterministic under the supplied seed.
- All inputs are point-in-time.
- If training is enabled, the training method and its data window are deterministic.

The suite enforces PIT and threading determinism; **weight/version stability is the
platform's responsibility** (see open questions in [strategy.md](strategy.md) §16).

---

## 9. Multimodal / context-bundle inference

A model node can ask for **point-in-time exogenous context** beyond its structured `inputs` — the
news, social posts, images, or videos that existed at time *t*. This is how a model scores a meme
coin off the actual posts it launched on, or weighs a stock on the headlines published that hour.
The strategy declares this with `context_inputs` (see [strategy.md](strategy.md) §8.3); the engine
assembles a `ContextBundle` per inference call and passes it to `infer`.

The division of labor is deliberate and preserves "the suite owns no data and no models":

- **The suite** collects, per `context_inputs` entry, every exogenous record with
  `ts_available ≤ current_ts` inside the declared `lookback` window (capped by `max_items`), orders
  it deterministically by `(ts_available, source_id, seq)`, and hands it over as the `ContextBundle`.
  It never decodes media.
- **The model (caller code)** receives the bundle. For `Signal`/`Document` items it gets resolved
  features directly; for `Media` and `Document.uri` items it gets a **reference** (a URI + modality)
  and is responsible for loading and decoding the raw bytes itself. A model may be fully multimodal
  (text + image + video) — that work lives entirely in the caller's adapter.

Two guarantees the engine still enforces around this: **look-ahead safety** (nothing with
`ts_available > current_ts` can appear in a bundle — the exogenous clock, distinct from the
market-data `ts_event` clock, is described in [signals.md](signals.md) §1) and **determinism**
(the same data + seed yields the same bundle, so the inference is reproducible provided the model
itself is seeded). Full mechanics: [signals.md](signals.md) §5.
