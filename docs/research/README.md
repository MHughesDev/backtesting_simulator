# Research

Evidence behind the system's decisions. This folder is the audit trail: when an ADR or spec
asserts something, the supporting work lives here. The suite owns no data, but it *does* own
the reasoning that shaped its contracts — and that reasoning should be reproducible.

## Folder convention

Research moves left-to-right, raw → distilled → decided:

```
research/
├── sources/      # Raw references. One file per topic. URLs + the specific extracted facts
│                 # each source supports (so claims are traceable to a citation).
├── summaries/    # Distilled synthesis per topic. The "what we found", neutral, weighing
│                 # tradeoffs. May combine several sources.
└── conclusions/  # The decision/answer the research produced. Opinionated. Feeds an ADR
│                 # or a spec, and links to it.
```

## Naming

`NNNN-topic-slug.md`, zero-padded, shared sequence across the three subfolders so a topic's
sources/summary/conclusion share a number where practical (e.g. `0001-*` is the runtime
language work).

## Lifecycle

1. Open a question (usually from a spec gap or an ADR needing backing).
2. Gather **sources** — capture URL, date accessed, and the exact facts used.
3. Write a **summary** — synthesize neutrally; surface tradeoffs and uncertainty.
4. Write a **conclusion** — make the call; link it to the ADR/spec it informs.

## Rules

- **Cite, don't assert.** Every load-bearing factual claim in a summary/conclusion should
  trace to a `sources/` entry.
- **Record uncertainty.** If a claim is an estimate or judgment, label it as such.
- **Date everything.** Web facts rot; note when they were checked.

## Index

| # | Topic | Sources | Summary | Conclusion | Feeds |
|---|---|---|---|---|---|
| 0001 | Runtime / language selection | [sources](sources/0001-runtime-language.md) | [summary](summaries/0001-runtime-language-tradeoffs.md) | [conclusion](conclusions/0001-runtime-selection.md) | [ADR-0001](../adr/0001-runtime-rust-python-hybrid.md) |
