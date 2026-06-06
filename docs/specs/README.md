# specs/

Feature, component, data, integration, and system-overview specification files for the backtesting simulator. Specs define what something should do with enough precision for a developer (human or AI) to implement it, and for a tester to verify it.

> To add a new spec, follow the skill: `skills/create-spec.md` and procedure: `procedures/add-spec.md`.

---

## Naming Convention

Spec files are named with a type code + sequential number:

```
<TYPE>-<NNN>-<kebab-name>.md
```

| Type | Code | Used for |
|------|------|----------|
| Feature | `FEAT` | User-visible behavior end-to-end |
| Component | `COMP` | Internal service or module (engines, runner, registry) |
| Data | `DATA` | Schema, model, or data contract (assets, contracts, taxonomy) |
| Integration | `INTG` | Behavior with an external system (ports) |
| System-overview | `SYS` | Unified map linking many specs |

Numbering is per type. The **Spec ID** is `<TYPE>-<NNN>`; cite any line as `<SPEC-ID> §N.M.X`.

---

## Index

| File | Spec ID | Type | Status | Related ADR(s) |
|------|---------|------|--------|----------------|
| [SYS-001-backtesting-simulator-overview.md](./SYS-001-backtesting-simulator-overview.md) | SYS-001 | System-overview | Approved | ADR-0001, ADR-0002, ADR-0003, ADR-0004, ADR-0005, ADR-0009, ADR-0010, ADR-0011, ADR-0012 |
| [DATA-001-data-taxonomy.md](./DATA-001-data-taxonomy.md) | DATA-001 | Data | Approved | — |
| [DATA-002-run-request.md](./DATA-002-run-request.md) | DATA-002 | Data | Approved | ADR-0005, ADR-0010 |
| [DATA-003-instrument-contract.md](./DATA-003-instrument-contract.md) | DATA-003 | Data | Approved | ADR-0003 |
| [DATA-004-market-data-contract.md](./DATA-004-market-data-contract.md) | DATA-004 | Data | Approved | — |
| [DATA-005-signals-contract.md](./DATA-005-signals-contract.md) | DATA-005 | Data | Approved | — |
| [DATA-006-strategy-contract.md](./DATA-006-strategy-contract.md) | DATA-006 | Data | Approved | ADR-0004, ADR-0005, ADR-0006, ADR-0011 |
| [DATA-007-plan-contract.md](./DATA-007-plan-contract.md) | DATA-007 | Data | Approved | ADR-0010 |
| [DATA-008-result-metrics-contract.md](./DATA-008-result-metrics-contract.md) | DATA-008 | Data | Draft | ADR-0010 |
| [DATA-009-equities-asset-spec.md](./DATA-009-equities-asset-spec.md) | DATA-009 | Data | Approved | — |
| [DATA-010-etfs-and-funds-asset-spec.md](./DATA-010-etfs-and-funds-asset-spec.md) | DATA-010 | Data | Approved | — |
| [DATA-011-crypto-spot-cex-asset-spec.md](./DATA-011-crypto-spot-cex-asset-spec.md) | DATA-011 | Data | Approved | — |
| [DATA-012-dex-amm-asset-spec.md](./DATA-012-dex-amm-asset-spec.md) | DATA-012 | Data | Approved | — |
| [DATA-013-futures-asset-spec.md](./DATA-013-futures-asset-spec.md) | DATA-013 | Data | Approved | — |
| [DATA-014-perpetuals-asset-spec.md](./DATA-014-perpetuals-asset-spec.md) | DATA-014 | Data | Approved | — |
| [DATA-015-options-asset-spec.md](./DATA-015-options-asset-spec.md) | DATA-015 | Data | Approved | ADR-0002 |
| [DATA-016-bonds-fixed-income-asset-spec.md](./DATA-016-bonds-fixed-income-asset-spec.md) | DATA-016 | Data | Approved | — |
| [DATA-017-fx-asset-spec.md](./DATA-017-fx-asset-spec.md) | DATA-017 | Data | Approved | — |
| [DATA-018-nfts-asset-spec.md](./DATA-018-nfts-asset-spec.md) | DATA-018 | Data | Approved | — |
| [DATA-019-prediction-markets-asset-spec.md](./DATA-019-prediction-markets-asset-spec.md) | DATA-019 | Data | Approved | — |
| [COMP-001-component-registry.md](./COMP-001-component-registry.md) | COMP-001 | Component | Approved | ADR-0011 |
| [COMP-002-runner-and-run-queue.md](./COMP-002-runner-and-run-queue.md) | COMP-002 | Component | Draft | ADR-0001, ADR-0010 |
| [COMP-003-engine-a-order-book.md](./COMP-003-engine-a-order-book.md) | COMP-003 | Component | Approved | ADR-0010 |
| [COMP-004-engine-b-amm.md](./COMP-004-engine-b-amm.md) | COMP-004 | Component | Approved | ADR-0010 |
| [COMP-005-engine-c-nav.md](./COMP-005-engine-c-nav.md) | COMP-005 | Component | Approved | ADR-0010 |
| [COMP-006-engine-d-cashflow.md](./COMP-006-engine-d-cashflow.md) | COMP-006 | Component | Approved | ADR-0010 |
| [COMP-007-engine-e-derivatives.md](./COMP-007-engine-e-derivatives.md) | COMP-007 | Component | Approved | ADR-0010 |
| [COMP-008-engine-f-synthetic.md](./COMP-008-engine-f-synthetic.md) | COMP-008 | Component | Approved | ADR-0010 |
| [COMP-009-engine-g-marketplace.md](./COMP-009-engine-g-marketplace.md) | COMP-009 | Component | Approved | ADR-0010 |
| [COMP-010-engine-h-event-resolution.md](./COMP-010-engine-h-event-resolution.md) | COMP-010 | Component | Approved | ADR-0010 |
| [INTG-001-account-ledger-port.md](./INTG-001-account-ledger-port.md) | INTG-001 | Integration | Approved | ADR-0010 |
| [INTG-002-ai-model-inference-port.md](./INTG-002-ai-model-inference-port.md) | INTG-002 | Integration | Approved | ADR-0006 |
| [INTG-003-training-port.md](./INTG-003-training-port.md) | INTG-003 | Integration | Approved | ADR-0005, ADR-0006, ADR-0007, ADR-0008 |

---

## Spec Groups

**System overview (1):** SYS-001 — the master map; start here.

**Data specs (19):** DATA-001–DATA-002 (core data model & run request), DATA-003–DATA-008 (input/output contracts), DATA-009–DATA-019 (the eleven asset classes).

**Component specs (10):** COMP-001 (component registry), COMP-002 (runner & run queue), COMP-003–COMP-010 (the eight execution engines A–H).

**Integration specs (3):** INTG-001 (account/ledger port), INTG-002 (AI model inference port), INTG-003 (training port).

> Reference material that is **not** a spec lives in [`../reference/`](../reference/) (e.g. the Engine Deep Dive, which duplicates engine specs for quick lookup).

