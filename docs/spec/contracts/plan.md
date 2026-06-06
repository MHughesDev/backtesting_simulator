# Contract Spec: Plan (multi-strategy composition)

A **Plan** is the layer above a Strategy. A Strategy is one flat pipeline
(universe → features → models → alpha → sizing → risk → execution). A Plan holds **multiple
strategies and the wiring between them**. The Run Request binds *a Plan*; a single Strategy is the
degenerate one-node Plan, so simple runs are unaffected.

```
Run Request  →  Plan  →  Strategies (each a clean single pipeline)
```

This resolves **OD-6** (multi-strategy portfolios) and **Q-STRAT-7/8** (composition, strategy- vs.
portfolio-level constraints).

---

## 1. Why a Plan, and not nested strategies

Two tempting but wrong shapes:

- **Nested strategies** (a strategy contains a strategy contains a strategy) — turns the JSON into
  a recursive program with control flow. This violates the declarative-only invariant
  ([strategy.md](strategy.md) §14.1): the format would become a programming language.
- **One giant strategy** — stuffing a screen + multiple entry logics + exit logic into one pipeline
  is unreadable and not reusable.

The Plan is the third option: **each strategy stays flat and single-purpose; the Plan composes
them by data-flow.** A strategy *emits* named outputs (a candidate set, a signal, an activation);
another strategy *subscribes* to them as inputs. That is the declarative equivalent of "kick off"
— publish/subscribe, never an imperative call.

---

## 2. Roles

A strategy in a Plan declares a `role`. Roles are about *what a strategy's output means*, not
special engines:

| Role | Output | Typical input |
|---|---|---|
| `selector` (screen) | a **candidate set** (instruments + the ts each became a candidate) | a `scanner`/`dynamic` universe |
| `entry` | **trades** that open/scale positions | a candidate set (`from:` a selector), or a static/dynamic universe |
| `exit` | **trades** that reduce/close positions | `from: "portfolio"` (current positions) |
| `standalone` | trades; no wiring | its own universe |

A Plan with only `standalone` strategies is simply "run N strategies at once." A screen→entry→exit
Plan is the composed case.

---

## 3. Schema

```jsonc
"plan": {
  "plan_id": "meme-scan-and-time-entry",

  "strategies": [
    { "id": "screen",   "role": "selector",
      "strategy": { /* universe.type=scanner + filters; emits a candidate set, places no trades */ } },

    { "id": "pullback", "role": "entry",
      "universe": { "from": "screen" },          // candidate set from the selector
      "strategy": { /* waits for a pullback before buying an admitted candidate */ } },

    { "id": "breakout", "role": "entry",
      "universe": { "from": "screen" },          // same candidates, a different entry style
      "strategy": { /* buys on momentum confirmation instead */ } },

    { "id": "exit_mgr", "role": "exit",
      "universe": { "from": "portfolio" },
      "strategy": { /* trailing stop / target / time-stop management */ } }
  ],

  "account_mode": "shared",        // shared | isolated   (§5)
  "conflict_policy": "net"         // net | priority | reject   (§6)
}
```

- **Fan-out:** a candidate set can feed **any number** of entry strategies (`pullback` and
  `breakout` both subscribe to `screen`). Each entry strategy runs its own pipeline per admitted
  asset — this is "multiple per-asset entry strategies."
- **Concurrent independents:** strategies with no `from:` wiring simply run side by side. This is
  how you "run two strategies at once," and it applies to plain single-asset styles too, not just
  scanning.
- **No nesting:** `strategy` here is always a flat Strategy ([strategy.md](strategy.md)); a Plan
  never contains a Plan.

---

## 4. Data-flow wiring (the "kickoff")

A strategy never calls another. Instead:

- A `selector` strategy publishes a **candidate set**: `{ instrument_id, became_candidate_ts,
  rank?, attributes? }` per admitted asset.
- An `entry` strategy names `universe: { "from": "<selector_id>" }`. Its pipeline then runs **per
  candidate**, scoped to that instrument (and that instrument's cross-instrument references, §4 of
  strategy.md), starting no earlier than `became_candidate_ts`.
- The same publish/subscribe applies to signals: a strategy may expose a named output another
  strategy reads as a gating input.

Point-in-time safety holds across the wiring: a downstream strategy can act on a candidate only at
or after the candidate became known (`became_candidate_ts`, itself bounded by `ts_available`).

This is the meme-coin example end to end: **one `selector`** (scan the cohort, filter by volume /
market cap / top-10-holder concentration / social score) → **one or more `entry` strategies** that
time the actual buy so you don't buy the top → **an optional `exit` manager**. Multiple flat
strategies, wired by the candidate set — not one blob and not nested ifs.

---

## 5. Capital across strategies (`account_mode`)

The suite owns no portfolio ([ADR-0010](../adr/0010-suite-does-not-own-portfolio.md)); this
governs how the injected `Account` is shared:

| Mode | Meaning |
|---|---|
| `shared` (default) | All strategies net into **one** injected `Account` — a real multi-strategy book; strategies can offset each other; one equity curve. |
| `isolated` | Each strategy gets its **own** `Account` partition — independent P&L, no interaction. |

Portfolio-level netting/risk **beyond** the Account (cross-strategy exposure aggregation, overlay
risk) remains the caller's analytics layer, consistent with the integration boundary in
[MASTER_SPEC.md](../MASTER_SPEC.md) §8.

---

## 6. Conflict resolution (`conflict_policy`)

When two strategies act on the same instrument oppositely (Q-STRAT-12):

| Policy | Behavior |
|---|---|
| `net` (default) | Under `shared` accounting, opposing intents net into a single position delta. |
| `priority` | Strategies are ordered; the higher-priority strategy's intent wins; lower is suppressed for that instrument/event. |
| `reject` | A conflicting pair is flagged and neither trades (fail-loud; surfaced as a warning). |

Under `isolated` accounting strategies never share a position, so conflicts cannot arise.

---

## 7. Determinism & ordering

- Within one `ts_event`, Plan evaluation runs in declared role order: `selector` → `entry` →
  `exit`, then strategies within a role in declared order. This makes candidate sets visible to
  entries in the same step and keeps results deterministic.
- All strategies in a Plan share the run's single clock and seed; cross-strategy wiring never
  exposes future data.

---

## 8. Validation

At run start (extends [run-request.md](../run-request.md) §10):

- Every `universe.from` names a strategy that exists in the Plan and has a compatible role
  (`from: <selector>` must reference a `selector`; `from: "portfolio"` is always valid).
- No wiring cycles (the Plan is a DAG of strategies).
- If `account_mode: "isolated"`, an `Account` partition is resolvable per strategy; if `"shared"`,
  one `Account` is injected. Missing → typed error.
- Each contained Strategy validates independently ([strategy.md](strategy.md)).

---

## 9. Invariants

1. **Strategies stay flat.** A Plan composes strategies; a Strategy never contains a Strategy and a
   Plan never contains a Plan.
2. **Composition is data-flow.** Wiring is publish/subscribe over named outputs, never imperative
   calls or nested control flow.
3. **Point-in-time across wiring.** A downstream strategy sees an upstream output only once it was
   knowable.
4. **Same parity.** A Plan runs identically in backtest and live, exactly like a single Strategy.
5. **No owned portfolio.** Capital lives in the injected `Account`(s); the Plan only declares how
   they are shared.

---

## 10. Open questions

- **Q-PLAN-1** Cross-strategy capital allocation under `shared` (fixed weights, dynamic, or
  caller-driven) — how is buying power split when several strategies compete for it?
- **Q-PLAN-2** Whether an `exit` strategy may act on positions opened by a *specific* entry
  strategy (position tagging) vs. the netted book only.
- **Q-PLAN-3** Hand-off latency: is there a configurable delay between a candidate being published
  and an entry strategy being allowed to act (realism for screen→entry pipelines)?
