---
description: Build a complete skill library under `.claude/skills/` for this repository by discovering project knowledge, authoring validated skills, and performing a full multi-agent review before finalizing.
---

You are a distinguished fellow on this project who is retiring. Your final task: build a complete skill library under `.claude/skills/` so that junior/mid-level engineers and smaller AI models (Sonnet-class) can carry this project forward without you — cheaper sessions must be able to debug, extend, validate, and eventually advance this project at the standard I hold today. Use multi-agent orchestration (workflows) for authoring and review; token cost is not a constraint, correctness is.

## Phase 1 — Discover before you write (no skill authoring yet)

Investigate the repo like an incoming principal engineer: README/manifest/contributordocs, the build system, the test suite and how it's actually run, CI config, docs directories, git history (what changed, what got reverted, what stalled on dead branches), open TODO/FIXME hotspots, issue-shaped artifacts, generated-data or deploy conventions, and any project memory/notes available to you. Then ask me AT MOST five questions, only for what the repo cannot tell you — likely: (1) what is the hardest live problem right now, (2) what unwritten discipline rules exist (things you're not allowed to do that no doc states), (3) who is the audience for this library and what do they NOT know, (4) what past failures cost the most time, (5) what does "beyond state of the art" mean for this project. Fold my answers into everything below.

## Phase 2 — Author the library (parallel agents, one skill per agent)

Instantiate this taxonomy, ADAPTED to what Phase 1 found — merge categories that are thin here, split ones that are deep, add domain categories I haven't imagined. Aim for 10–16 skills:

CORE (every project has these):

- `<project>-change-control` — how changes are classified, gated, reviewed here; the project's non-negotiables with the rationale and the historical incident behind each.

- `<project>-debugging-playbook` — symptom→triage table for this project's failure modes; the traps that cost real time (each with its story); discriminating experiments.

- `<project>-failure-archaeology` — the chronicle: every major investigation, dead end, rejected fix, and revert, as symptom → root cause → evidence → status, so no one re-fights a settled battle. Mine git history and docs hard for this.

- `<project>-architecture-contract` — the system's load-bearing design decisions and WHY; the invariants that must hold; the open known-weak points, stated plainly.

- `<domain>-reference` — the domain-theory knowledge pack a mid-level person lacks (the field's math/protocols/standards as they apply HERE, not a textbook).

- `<project>-config-and-flags` — catalog of every configuration axis: options, defaults, which are production vs experimental, guards; how to add one (checklist); with re-verification commands since flags drift.

- `<project>-build-and-env` — recreate the environment from scratch; known traps.

- `<project>-run-and-operate` — running/deploying the thing: command anatomy, data or artifact conventions, what output lands where.

- `<project>-diagnostics-and-tooling` — how to MEASURE instead of eyeball: the project's diagnostic tools with interpretation guides; ship actual scripts inside the skill's `scripts/` directory where they exist or where you can write them.

- `<project>-validation-and-qa` — what counts as evidence here; acceptance-threshold discipline; the certified/golden inventory; how to add tests.

- `<project>-docs-and-writing` — maintaining the docs of record; templates; house style.

- `<project>-external-positioning` — papers/releases/ecosystem: what's novel vs known, what must be proven before claiming, reproducibility standards.

ADVANCED (the layer that makes juniors dangerous, in the good way):

13. `<project>-<hardest-problem>-campaign` — an EXECUTABLE, decision-gated campaign for the hardest live problem from Phase 1: numbered phases, exact commands, EXPECTED observations/numbers at every gate ("if you see X instead → branch to Y"), the solution menu ranked with theory/derivation obligations for each, known wrong paths explicitly fenced off, and a validation-and-promotion protocol that routes through the project's change control — success must be measurable, never judged by eye.

14. `<project>-proof-and-analysis-toolkit` — the first-principles analysis methods of this domain (whatever "prove it, don't just install it" means here), each as a recipe with a worked example from this repo's history.

15. `<project>-research-frontier` — open problems where this project could advance the state of the art: for each, why current SOTA fails, this project's specific asset, the first three concrete steps IN THIS REPO, and a falsifiable "you have a result when…" milestone.

16. `<project>-research-methodology` — the discipline that turns a hunch into an accepted result here: the evidence bar (one mechanism must explain ALL observations including negatives, and survive assigned adversarial refutation), hypothesis-predicts-numbers-before-running, the idea lifecycle from experiment flag to adopted change or documented retirement, and where good ideas historically came from.

## AUTHORING RULES (bake into every agent's prompt)

- Audience: zero-context mid-level engineer or Sonnet-class model.
- Imperative runbook voice.
- Copy-pasteable commands.
- Every jargon term defined once.
- Tables and checklists.
- Each skill says when NOT to use it and which sibling to use instead.
- Format: `.claude/skills/<name>/SKILL.md`.
- YAML frontmatter with `name` and a trigger-rich `description` (exactly when a model should load it).
- GROUND TRUTH ONLY: verify every command, flag, path, and claim against the repo before stating it.
- Embed knowledge; don't reference private/user-specific paths as load-bearing sources.
- Date-stamp volatile facts.
- End every skill with a "Provenance and maintenance" section containing one-line re-verification commands.
- No oversell: unproven things stay labeled open/candidate.
- Nothing may contradict the project's own manifest/rules.
- No skill may route around its change-control.
- Write ONLY inside `.claude/skills/`.
- The rest of the repository is read-only.
- Never execute mutating git commands.

## Phase 3 — Review and fix (after ALL skills exist)

Three parallel reviewers over the complete set, then one fixer.

### FACTUAL

Re-verify flags, paths, commands and citations against the repository.

Flag anything invented or stale.

Severity:

- Blocking
- Important
- Minor

### DOCTRINE

Check:

- contradictions
- violations of project rules
- overstated claims
- missing change-control gates

### USABILITY

Check:

- trigger quality
- duplication
- one canonical home per fact
- self-containedness
- scanability

The Fixer must resolve every Blocking and Important issue before completion.

Finally provide:

- Skill inventory with one-line descriptions.
- What was verified by spot-check.
- What remains uncertain.
