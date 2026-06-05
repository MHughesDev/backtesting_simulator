# Specification Index & Readiness

The complete specification of the **end-state** backtesting suite (no MVP — see
[ADR-0009](../adr/0009-end-state-system-no-mvp.md)). Start at
[MASTER_SPEC.md](MASTER_SPEC.md); this page is the map and the **implementation-planning
readiness check**.

---

## Document map

### Core
| Spec | Status | Purpose |
|---|---|---|
| [MASTER_SPEC.md](MASTER_SPEC.md) | ✅ Defined | North-star: principles, system map, contracts, engines, boundaries |
| [run-request.md](run-request.md) | ✅ Defined | Per-invocation document; per-trade model; injected ports |
| [component-registry.md](component-registry.md) | ✅ Defined | What a component is; trust tiers (built-in / native / WASM) |

### Contracts (`contracts/`)
| Spec | Status | Purpose |
|---|---|---|
| [contracts/instrument.md](contracts/instrument.md) | ✅ Defined | Identity, `price_formation` router, capability flags |
| [contracts/market-data.md](contracts/market-data.md) | ✅ Defined | Event envelope + all payload variants |
| [contracts/strategy.md](contracts/strategy.md) | ✅ Defined | JSON declarative pipeline; sizing; AI inference block |
| [contracts/model.md](contracts/model.md) | ✅ Defined | `Model` inference port; fallback; frequency |
| [contracts/training.md](contracts/training.md) | ✅ Defined | `Trainer` port; pause-train-resume; visibility; retention |
| [contracts/metrics.md](contracts/metrics.md) | 🔲 Deferred | Result/metrics analytics over TradeRecords + injected `Account` |
| [contracts/signals.md](contracts/signals.md) | 🔲 Deferred | Exogenous / alternative-data signal streams |

### Assets (`assets/`) — all ✅ Defined
[equities](assets/equities.md) · [etfs](assets/etfs.md) ·
[crypto-spot-cex](assets/crypto-spot-cex.md) · [dex-amm](assets/dex-amm.md) ·
[futures](assets/futures.md) · [perpetuals](assets/perpetuals.md) ·
[options](assets/options.md) · [bonds](assets/bonds.md) · [fx](assets/fx.md) ·
[nfts](assets/nfts.md) · [prediction-markets](assets/prediction-markets.md)

### Engines (`engines/`)
| Spec | Status |
|---|---|
| [engines/README.md](engines/README.md) (selection rule, order-type matrix, composition) | ✅ Defined |
| [engine-a (Order Book)](engines/engine-a-order-book.md) — full mechanics | ✅ Defined |
| [engine-b (AMM)](engines/engine-b-amm.md) — full mechanics | ✅ Defined |
| [engine-e (Derivatives)](engines/engine-e-derivatives.md) — full mechanics | ✅ Defined |
| engine-c, -d, -f, -g, -h — per-engine internals | 🔲 Deferred (per-phase) |

### Runner
| Spec | Status |
|---|---|
| runner.md (run queue, parallelism, sweep search) | 🔲 Deferred |

---

## What "Defined" vs "Deferred" means

- **✅ Defined** — the architecture and contracts are specified well enough to plan and build
  against. These are stable.
- **🔲 Deferred** — intentionally left for **phase planning**, where writing the spec becomes the
  first atomic task of that phase. Deferring these does **not** reduce scope (the end-state still
  includes them); it sequences the depth work. The *architecture* that constrains them is already
  fixed (e.g. every engine obeys the `Engine` trait and order-type matrix in
  [engines/README.md](engines/README.md); metrics consume the `TradeRecord` stream defined in
  [run-request.md](run-request.md)).

---

## Decisions backing the spec (ADRs)

All architecturally significant choices are recorded in [`../adr/`](../adr/):

0001 runtime (Rust+Python) · 0002 minimal deps · 0003 capability model ·
0004 strategy JSON pipeline · 0005 suite stores no strategies (library) ·
0006 model inference + opt-in training · 0007 training via injected port ·
0008 training scope/method/visibility/retention · 0009 end-state, no MVP ·
0010 suite owns no portfolio (injected `Account`) · 0011 component trust tiers (WASM) ·
0012 standalone contracts kernel.

Open items live in [../OPEN_QUESTIONS.md](../OPEN_QUESTIONS.md).

---

## Readiness for implementation planning

The **architectural spine is complete and internally consistent.** Every load-bearing decision
is made and recorded:

- ✅ Runtime, dependency posture, and the contracts-as-shared-kernel topology.
- ✅ The universal data model (envelope + capability-gated payloads) across all 11 asset classes.
- ✅ Engine selection (`price_formation`) and the `Engine` trait / order-type matrix for all 8.
- ✅ The strategy model (one JSON pipeline), sizing, risk, execution, AI inference, and opt-in
  training (incl. the injected `Trainer` and pause-train-resume).
- ✅ The per-trade execution model and the injected `Account` port (no assumed portfolio).
- ✅ The Run Request (how a run is invoked) and the component registry + trust tiers.
- ✅ Reproducibility/determinism and look-ahead invariants stated system-wide.

The three **load-bearing engines (A Order Book, B AMM, E Derivatives) are now fully specified**
— matching/fill models, pool math (v2/v3/Curve), and option pricing/greeks/exercise.

**Remaining work is depth, not architecture** — the five other engine internals (C, D, F, G, H),
metrics formulas, signals, and the runner — all of which have a fixed architectural frame and
become tasks inside their phases.

**Conclusion: ready to begin implementation planning.** Next: a long-term plan decomposing the
end-state into high-level phases, then per-phase files of discrete, atomic tasks.
