# AGENT.md — Agent Operating Instructions

> This file governs how an AI agent should behave inside this repository. Read it before touching anything else. Also read [README.md](./README.md) for project context.

---

## 1. Skill Discovery Protocol (MANDATORY)

**Before processing any user query or executing any task**, search the `skills/` folder for a skill that matches the request.

Steps:
1. List all files in `docs/skills/`.
2. Read the title and `## Purpose` block of any skill that might apply.
3. If a matching skill exists, follow it — do not improvise a different approach.
4. If no skill matches, check `docs/procedures/` for relevant atomic steps to compose.
5. Only freeform if neither skills nor procedures apply. In that case, consider whether to create a new skill or procedure afterward.

This ensures repeatable, consistent outputs and keeps the repo self-improving.

---

## 2. Idea Intake Protocol

When a user describes a new idea or application, do not immediately start producing design artifacts.
First fill in `docs/artifact.md` — the foundational project definition.

Follow `docs/skills/create-artifact.md` to guide the user through it.

The artifact replaces the old success-criteria / failure-modes intake sequence. It covers the same
ground in a more structured, reusable form that directly feeds downstream artifacts.

Do not proceed to research, plans, or specs until the artifact is filled in and confirmed by the user.

---

## 3. Research-First Protocol (MANDATORY)

**Before writing any spec, architecture entry, or design decision, you must research the web first.**
You may not specify, decide, or assume your way into a design. Do the homework, then design.

Research must establish, at minimum:
1. **Prevailing practice** — how is this problem commonly solved? What are the established patterns?
2. **Existing building blocks** — what APIs, services, libraries, or software already exist that
   could be used instead of building from scratch?
3. **Trade-offs** — for every consequential fork, what are the competing options and how do they
   compare? Never present a single option as if it were the only one.

Rules:
- Record findings as a research brief in `docs/research/` (follow `create-research-brief.md`).
- Every trade-off you weigh must be logged in `docs/open-questions.md` — open it when you raise the
  question, resolve it when research settles it.
- **If you cannot access the internet, say so explicitly and stop.** Do not fill the gap with
  assumptions. Surface the blocker to the user rather than guessing at common practice.

This protocol is the reason the repo produces trustworthy design instead of confident speculation.

---

## 4. Folder Responsibilities

| Folder / File | What goes there | Agent action |
|---------------|----------------|--------------|
| `docs/artifact.md` | Foundational project definition — fill in first | Follow `docs/skills/create-artifact.md` |
| `docs/open-questions.md` | Living register of trade-offs and unresolved forks | Log every consequential comparison here |
| `docs/architecture.md` | Current-state system map — emergent, never assumed | Update as ADRs/specs change the system shape |
| `docs/research/` | Findings, comparisons, prior art, technology evaluations | Follow `docs/skills/create-research-brief.md` |
| `docs/plans/` | All plans — working scratchpads and formal versioned roadmaps | Follow `docs/skills/create-plan.md` to create; `docs/skills/execute-plan.md` to execute |
| `docs/specs/` | Component and feature specifications | Follow `docs/skills/create-spec.md` |
| `docs/adr/` | Architecture Decision Records | Follow `docs/skills/create-adr.md` |
| `docs/procedures/` | Atomic task instructions | Only modify when adding/improving a procedure |
| `docs/skills/` | Agent skill definitions | Only modify when adding/improving a skill |

---

## 5. Output Standards

- File names: `kebab-case.md`
- Headings: Title Case for H1, Sentence case for H2+
- Specs are named `<TYPE>-NNN-name.md` (e.g. `FEAT-001-…`) and use hierarchical section numbering (`N.M.X`) — see `add-spec.md`
- Every new artifact must be linked from the relevant folder's `README.md`
- ADRs are numbered: `adr/0001-title.md`, `adr/0002-title.md`, etc.
- Specs reference the ADR(s) that informed them
- Plans reference the research that preceded them

---

## 6. Constraints

- Do not create `.cursor`, `.claude`, `.vscode`, or any editor-specific config files.
- Do not modify `procedures/` without explicitly telling the user what changed and why.
- Do not skip the Skill Discovery Protocol (section 1) for any task.
- Do not produce design artifacts before completing the Idea Intake Protocol (section 2) for new projects.
- Do not write specs, architecture, or decisions before completing the Research-First Protocol (section 3).
- Do not begin executing any milestone in a formal plan without first following `skills/execute-plan.md`.
- Do not execute a plan whose `Derivation Status` is `STALE` — re-derive it first.
- Run `skills/verify-traceability.md` at the end of a design session and before handing a plan to
  execution — do not start implementing against a plan with broken references.
- Do not assume anything about the target application or its architecture. This repo defines documentation
  practice only — it is application- and stack-agnostic by design.
- Keep procedures editor- and agent-agnostic. They must work regardless of the AI tool being used.

---

## 7. Self-Improvement

If you complete a task and realize no skill or procedure covered it:
1. Draft a new procedure in `docs/procedures/` for the atomic steps you followed.
2. Draft a new skill in `docs/skills/` if it warrants a higher-level wrapper.
3. Update the relevant `README.md` index.
4. Mention what you added to the user at the end of the session.

---

## References

- [README.md](./README.md) — project overview and folder structure
- [docs/skills/README.md](./docs/skills/README.md) — full skill index
- [docs/procedures/README.md](./docs/procedures/README.md) — full procedure index
