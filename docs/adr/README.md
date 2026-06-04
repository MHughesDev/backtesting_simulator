# Architecture Decision Records (ADRs)

An ADR captures **one architecturally significant decision**: the context, the choice, and
the consequences. ADRs are the durable "why" behind the system — they are append-only
history, not living docs. When a decision changes, write a *new* ADR that supersedes the old
one rather than editing it.

## Format

We use a lightweight [MADR](https://adr.github.io/madr/)-style format. Start from
[`0000-template.md`](0000-template.md).

## Numbering

`NNNN-short-title.md`, zero-padded, monotonically increasing. Never reuse a number.

## Status lifecycle

```
Proposed → Accepted → (later) Superseded by ADR-XXXX
                    ↘ Rejected
                    ↘ Deprecated
```

## Relationship to other docs

- **Research** (`docs/research/`) provides the evidence an ADR cites.
- **Spec** (`docs/spec/`) describes the resulting system; ADRs explain *why* it's that way.
- An ADR should link to the research conclusion and/or spec section it informs.

## Index

| ADR | Title | Status |
|---|---|---|
| [0001](0001-runtime-rust-python-hybrid.md) | Runtime: Rust core + Python (hybrid) | Accepted |
| [0002](0002-minimal-external-dependencies.md) | Minimal external dependencies (own the contracts) | Accepted |
| [0003](0003-capability-based-instrument-model.md) | Capability-based instrument model (asset ≠ engine) | Accepted |
| [0004](0004-strategy-json-pipeline.md) | Strategy = a single JSON declarative pipeline | Accepted |
| [0005](0005-strategy-not-stored-suite-is-a-library.md) | Suite processes but never stores strategies (library boundary) | Accepted |
| [0006](0006-model-inference-and-training.md) | AI models — inference by default, opt-in PIT training | Accepted |
| [0007](0007-shared-training-pipeline-port.md) | Training is a shared pipeline invoked through a port | Proposed |
