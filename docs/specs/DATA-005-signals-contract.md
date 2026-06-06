# Spec: DATA-005 — Signals Contract (the Exogenous-Signal Plane)

**Spec ID:** DATA-005
**Type:** Data (schema contract)
**Status:** Approved
**Date:** 2026-06-06
**Author:** Agent

intentionally out of scope — that lives in caller-owned adapters.

This contract defines how **everything outside the market** enters the suite: news, sentiment,
social attention, fundamentals, macro releases, on-chain analytics, scheduled events, and raw
media (posts, images, videos). It is the second of the suite's three data planes (see
[DATA_TAXONOMY.md](DATA-001-data-taxonomy.md)).

The defining rules:

1. **Exogenous data is never a fill source.** It can change what a strategy *decides*, never the
   price the market gives it. Fills come only from the Market-Data Plane.
2. **Look-ahead is enforced by `ts_available`, not `ts_event`.** External information has a
   publication lag; the strategy may only see it once it was knowable.
3. **The suite owns no data and no models.** Signals are pre-computed by the caller; raw media is
   referenced and resolved by the caller's injected `Model` port. The core only aligns, windows,
   and routes — point-in-time.
4. **Generic payloads, never vendor schemas.** The suite recognizes a handful of normalized
   payloads; adapters translate Reddit/X/news/satellite/on-chain feeds into them upstream.


---

## 1. The two timestamps (the heart of this contract)

Every exogenous payload rides the shared `EventEnvelope` (see
[DATA_TAXONOMY.md](DATA-001-data-taxonomy.md) §3) and must carry:

- `ts_event` — when the real-world thing happened (the merger closed, the post was written).
- `ts_available` — when a strategy could **first have known** it (the announcement crossed the
  wire, the post was indexed, the filing was published).

**Enforcement:** a strategy or model may observe an exogenous record only when
`ts_available ≤ current_ts`. This is the only safe rule for external data. Two canonical traps it
closes:

- A merger **effective** on date Y but **announced** on date X (X < Y) — actionable at X, not Y.
- A quarterly filing **dated** to quarter-end but **published** weeks later — visible at publication.

Where a source supplies only one timestamp, `ts_available` defaults to `ts_event` — but the caller
is then asserting there was no lag, and that assertion is the caller's contractual responsibility.
The suite mechanically enforces *no look-ahead given the timestamps*; it cannot detect a feed that
lied about when news broke.

---

## 2. The generic payloads

All exogenous data normalizes onto these. There is no per-vendor or per-category payload type —
the [taxonomy](DATA-001-data-taxonomy.md) categories 21–32 are *what kinds of real-world data map onto
these*, not separate schemas.

### 2.1 `SignalEvent` — the workhorse

```
SignalEvent {
  signal_id:      String,            // canonical name, e.g. "social_attention", "cpi_surprise"
  entity_id:      Option<EntityId>,  // company / protocol / collection / asset the signal is about
  instrument_id:  Option<InstrumentId>, // optional direct link to a tradable
  value:          SignalValue,       // Scalar | Category | Vector | Embedding | Bool
  value_type:     SignalValueType,
  confidence:     Option<Decimal>,   // source/model confidence
  // ts_event, ts_available, source_id, source_version ride the envelope
  metadata:       Option<Map<String, Json>>,
}

SignalValue = Scalar(Decimal) | Category(String) | Vector(Vec<Decimal>) | Embedding(Vec<f32>) | Bool(bool)
```

Any pre-computed numeric/categorical/vector feature — sentiment score, mention count, search
interest, on-chain TVL, analyst revision, macro surprise — is a `SignalEvent`.

### 2.2 `ExternalEvent` — discrete real-world events

```
ExternalEvent {
  event_id:    String,
  event_type:  String,        // "merger", "rating_downgrade", "fda_approval", "exploit", ...
  entity_id:   Option<EntityId>,
  instrument_id: Option<InstrumentId>,
  magnitude:   Option<Decimal>,  // optional normalized severity/size
  attributes:  Option<Map<String, Json>>,
}
```

A merger, a court ruling, a token unlock, a hack, a sanction — anything that *happens* at a point
in time rather than being a continuous metric. Distinct from a `CorporateAction` (Market-Data
Plane), which mechanically adjusts prices/positions; an `ExternalEvent` only informs a decision.

### 2.3 `ScheduledEvent` — known-future dates (the date is the signal)

```
ScheduledEvent {
  event_id:        String,
  event_type:      String,         // "earnings", "fomc", "token_unlock", "product_launch"
  entity_id:       Option<EntityId>,
  scheduled_ts:    i64,            // when the event is expected to occur
  // ts_available = when the schedule itself became known (often long before scheduled_ts)
  confirmed:       Option<bool>,
}
```

The *outcome* is unknown, but the **scheduled date itself is actionable information** the moment
the calendar is published (`ts_available`). "Earnings in 3 days" is a tradeable fact before the
earnings number exists.

### 2.4 `DocumentSignal` — text-derived

```
DocumentSignal {
  doc_id:       String,
  doc_type:     String,            // "news", "filing", "transcript", "press_release", "social_post"
  entity_id:    Option<EntityId>,
  instrument_id: Option<InstrumentId>,
  features:     Option<Map<String, SignalValue>>, // pre-extracted features (sentiment, topics, …)
  uri:          Option<String>,    // optional PIT pointer to the raw text (resolved by the Model port)
  text_inline:  Option<String>,    // optional short inline text (headlines); large bodies use uri
}
```

The default and preferred form carries **pre-extracted features**. The optional `uri`/`text_inline`
lets a multimodal model read the actual document at inference (§4–§5).

### 2.5 `MediaReference` — images, video, audio (point-in-time pointers)

```
MediaReference {
  media_id:     String,
  modality:     Text | Image | Video | Audio,
  entity_id:    Option<EntityId>,
  instrument_id: Option<InstrumentId>,
  uri:          String,            // PIT pointer to the asset; the suite never loads or parses it
  features:     Option<Map<String, SignalValue>>, // optional pre-extracted features (embeddings, tags)
}
```

**The suite never loads, decodes, or parses the bytes.** A `MediaReference` is a point-in-time
*handle*: it asserts "this asset existed and was knowable at `ts_available`." The injected `Model`
port resolves the `uri` and does the multimodal inference. This is how the suite supports running a
model over the actual posts/images/videos published at time *t* without owning any media.

### 2.6 `EntityMetric` — entity-level time series

```
EntityMetric {
  entity_id:   EntityId,
  metric_id:   String,             // "tvl", "active_addresses", "web_traffic", "headcount"
  value:       Decimal,
}
```

A convenience specialization of `SignalEvent` for entity-keyed scalar time series (on-chain
analytics, web/app alt-data, fundamentals) where there is no single instrument.

---

## 3. Binding signals in the Run Request (multi-source, unlimited, optional)

Exogenous data is bound in a top-level **`signals`** block, **keyed by source**, so different
platforms stay cleanly separated and a caller can attach **as many sources as they want**. An NFT
run might bind one or two sources; a meme-coin run might bind a dozen. All optional.

```jsonc
"signals": {
  "sources": {
    "social_x": {                          // arbitrary caller-chosen source key
      "adapter": "injected:x_adapter",     // caller-owned adapter that normalizes the feed
      "streams": {
        "attention":  { "uri": "...", "payload_class": "SignalEvent",    "signal_id": "social_attention" },
        "posts":      { "uri": "...", "payload_class": "MediaReference", "modality": "Text" }
      }
    },
    "social_reddit": {
      "adapter": "injected:reddit_adapter",
      "streams": {
        "forum_activity": { "uri": "...", "payload_class": "SignalEvent", "signal_id": "forum_activity" }
      }
    },
    "news": {
      "adapter": "injected:news_adapter",
      "streams": {
        "headlines": { "uri": "...", "payload_class": "DocumentSignal", "doc_type": "news" }
      }
    },
    "onchain": {
      "adapter": "injected:onchain_adapter",
      "streams": {
        "exchange_flows": { "uri": "...", "payload_class": "EntityMetric", "metric_id": "exchange_netflow" },
        "whale_txns":     { "uri": "...", "payload_class": "ExternalEvent", "event_type": "whale_transfer" }
      }
    }
  }
}
```

**Binding rules:**

- Each source is independent: its own adapter, its own streams, its own failure isolation. One bad
  source does not poison another.
- Every stream descriptor declares its `payload_class` and the identifying field for that class
  (`signal_id`, `metric_id`, `doc_type`, `event_type`, `modality`) so the suite can route it and
  the strategy can reference it by name without scanning the data.
- All signal bindings are **optional** — the Market-Data Plane is what the engine requires; signals
  are additive. A strategy that references a signal binds the sources that carry it, or fails
  validation (`SignalNotBound`).
- `ts_available` is mandatory for the look-ahead guarantee; a stream missing it defaults
  `ts_available = ts_event` (caller asserts no lag).
- Like reference market data, an entity-keyed signal source resolves to instruments via
  `entity_id` (an instrument declares its `entity_id`/`issuer_id`).

---

## 4. How strategies consume signals

Two ingestion modes, both point-in-time.

### 4.1 As features (pre-computed, the default)

A `SignalEvent`/`EntityMetric` value is referenced exactly like any feature, via the
`signal:<signal_id>` grammar already in the strategy contract
([strategy.md](DATA-006-strategy-contract.md) §4):

```jsonc
"alpha": {
  "insights": [
    { "id": "meme_pump",
      "when": "signal:social_attention > param:attn_floor && feature:rsi14 < 70",
      "direction": "long" }
  ]
}
```

The suite aligns the signal's latest `ts_available ≤ current_ts` value onto the event clock and
exposes it; frequency mismatch (a daily signal vs. minute bars) is handled by last-known-value
carry until the next update, never by interpolation forward.

### 4.2 As model inputs (in-loop inference, including multimodal)

A model node in the strategy's `models` stage can request a **point-in-time context bundle** of
exogenous data assembled fresh at each inference call. This is how a model runs over the news,
posts, images, and videos that existed at time *t*. See §5.

---

## 5. Point-in-time model context bundles (the multimodal path)

A model node declares `context_inputs` — point-in-time queries the suite resolves against the
bound signal sources at each inference call, handing the result to the injected `Model` port:

```jsonc
"models": [
  {
    "id": "meme_oracle",
    "model_id": "multimodal-meme-scorer",
    "model_version": "2.1.0",
    "inference_fn": "score",
    "frequency": { "on_event": "Bar" },

    "inputs": {                                  // structured feature inputs (existing)
      "price_ctx": "feature:vol20"
    },

    "context_inputs": {                          // NEW — PIT exogenous bundles assembled per call
      "recent_posts":  { "from": "social_x.posts",        "lookback": "1h", "max_items": 200 },
      "reddit_buzz":   { "from": "social_reddit.forum_activity", "lookback": "6h" },
      "headlines":     { "from": "news.headlines",        "lookback": "24h" },
      "flows":         { "from": "onchain.exchange_flows", "lookback": "24h" }
    },

    "outputs": { "value": "meme_score", "confidence": "meme_conf" },
    "fallback": { "policy": "use_last", "max_staleness": "2h" }
  }
]
```

**Semantics:**

- At each inference call, the suite collects, from each named source/stream, every record with
  `ts_available ≤ current_ts` within the `lookback` window (optionally capped by `max_items`), and
  passes the bundle to the injected `Model` port alongside the structured `inputs`.
- For `MediaReference`/`DocumentSignal` items, the bundle contains the **references** (URIs +
  modality + any pre-extracted features). The **injected model resolves the URIs and loads the raw
  bytes** — the suite never does. The model is caller code and may be fully multimodal.
- **Look-ahead is enforced for the bundle**: nothing with `ts_available > current_ts` can appear.
  The model literally cannot be handed a post or image that was not yet knowable.
- The model's outputs bind downstream exactly like any model output (`model:meme_oracle.value`),
  feeding `alpha` → `sizing` → `risk` → `execution`.
- **Determinism:** the bundle is assembled deterministically (stable ordering by
  `(ts_available, source_id, seq)`); given the same data and seed, the same bundle is produced. The
  injected model must itself be deterministic (seeded) for reproducibility.

This is the mechanism the user asked for: *"pass in all relevant other data — reddit/x posts,
images, videos, news at time t — into an AI model each time the model is called."*

---

## 6. What the suite enforces vs. what the caller owns

| Concern | Owner |
|---|---|
| Honest `ts_event` / `ts_available` on every record | **Caller** (contractual) |
| No look-ahead given the timestamps (`ts_available ≤ current_ts`) | **Suite** (mechanical) |
| Scraping, NLP, scoring, entity mapping, feature extraction | **Caller** (upstream adapters) |
| Loading/decoding raw media bytes at inference | **Caller** (injected `Model` port) |
| Aligning, windowing, routing signals point-in-time | **Suite** |
| Assembling deterministic PIT context bundles | **Suite** |
| Multimodal model inference itself | **Caller** (injected model) |
| Signal storage/versioning | **Caller** (suite stores nothing) |

---

## 7. Capability flag

- `HasExogenousSignals` — the instrument/run participates in the Exogenous-Signal Plane. Gates the
  `signal:*` reference grammar and `context_inputs`. Absent → a strategy referencing a signal fails
  validation.

---

## 8. Invariants

1. **Never a fill source.** No exogenous payload can set, adjust, or override a fill price.
2. **`ts_available` look-ahead.** Exogenous look-ahead uses `ts_available`; market-data look-ahead
   uses `ts_event`. Both are `≤ current_ts`.
3. **References, not blobs.** The core never loads raw media/text; it carries PIT references the
   injected model resolves.
4. **Source isolation.** Sources are independent and individually optional; one source's absence or
   failure never blocks another.
5. **Deterministic bundling.** PIT context bundles are assembled in a stable, reproducible order.
6. **Suite owns nothing.** No signal data, no media, no model weights are stored by the suite.

---

## 9. Open questions

- **Q-SIG-BUNDLE-1** Max bundle size / memory policy when a `lookback` window spans a high-volume
  social source (cap by `max_items`, by bytes, or by both?).
- **Q-SIG-BUNDLE-2** Caching identical bundles across a parameter sweep (same data, same window) to
  avoid re-assembling per run — tension with the stateless-suite principle.
- **Q-SIG-3** Frequency-mismatch policy beyond last-known-value carry (explicit staleness horizon
  per signal?).
- **Q-SIG-5** Entity-mapping conflicts when one `entity_id` maps to several instruments (a signal
  about an issuer feeding many bonds/options).
