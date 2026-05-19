# Leo IDE — Production-Hardening Implementation Plan

This directory contains an ordered set of implementation specs derived from
the principal-engineer audit performed on 2026-05-13. Each numbered file is
one dispatchable unit of work — sized so a single agent can pick it up,
implement it, write tests, and hand it off for review without holding broad
context about the rest of the plan.

The audit identified ~50 issues spanning critical security gaps,
production observability holes, performance regressions, and code-quality
debt. These are grouped here to minimize merge conflicts, surface
dependencies, and let work proceed in parallel where possible.

---

## Operating principles (apply to every group)

These are non-negotiable. Any deviation must be called out explicitly in
the PR description and approved by the reviewer.

1. **Behavior preservation is the prime directive.** The application
   already encodes important product behavior, even where the code is
   messy. Every change must include a *preservation guarantee section*
   listing the user-visible behaviors that must not change. If a refactor
   risks regression, it is split into smaller steps or deferred.

2. **Build green at every commit.** `cargo check` must pass for the Rust
   side, `npx svelte-check --threshold error` must pass for the frontend.
   No commit is merged with warnings introduced by that commit.

3. **Tests gate every change.** No code modification ships without
   covering tests. Group 0 establishes the testing infrastructure;
   subsequent groups are blocked on it.

4. **No new dependencies without justification.** Reuse what's already
   in `package.json` / `Cargo.toml`. New crates or npm packages require
   a one-paragraph justification in the spec covering supply-chain risk,
   bundle-size impact, and what existing alternative was rejected.

5. **No silent error swallowing.** Every `catch { /* ignore */ }` and
   every Rust `.ok()` chain that drops an error must include a comment
   explaining why silence is correct, OR be replaced with structured
   logging.

6. **Use real icons.** Any UI work uses icons from the existing
   `lucide-svelte` or `@iconify/svelte` collections. No emoji, no ad-hoc
   SVG. Reference IDE: VSCode, Xcode, Zed conventions.

7. **Code review is mandatory.** Each group ends with a sub-agent code
   review pass. The review checklist is embedded in each spec.

8. **Rollback is a feature.** Every change includes a rollback note
   describing how to revert without data loss.

---

## Dispatch order

```
Group 0: Foundation                       [BLOCKS ALL]
        (observability + test infra)
                |
                v
Groups 1, 2, 3, 5  (parallel-safe)        [security, robustness, backend perf, knowledge UX]
                |
                v
Group 4: Frontend Performance              [verify against baseline from G0]
                |
                v
Group 6: Code Quality / Refactoring        [behavior-neutral cleanup, AFTER all functional fixes]
                |
                v
Group 7: UX Polish                         [polish pass]
```

### Why this order

- **G0 first**: without production logging and tests, no other change is
  verifiable in the production build. The chat-save bug that prompted
  this plan was invisible because all `console.*` calls are stripped by
  `vite.config.ts`. Fix that first.
- **G1, G2, G3, G5 in parallel**: these touch disjoint surfaces (security
  primitives, file I/O, HTTP client, knowledge module). They can be
  picked up independently.
- **G4 after G0**: the frontend perf fixes need the test/baseline harness
  from G0 to confirm we don't regress correctness while chasing speed.
- **G6 last among functional groups**: refactors compound merge conflicts
  with feature work. Hold them until the functional fixes have landed.
- **G7 final polish**: touches surface-level UI; do it after the
  behavior is correct.

---

## Group index

| #  | Group                       | Risk     | Est. effort | Depends on |
|----|-----------------------------|----------|-------------|------------|
| 00 | Foundation                  | Low      | M           | —          |
| 01 | Security Hardening          | Medium   | M           | 00         |
| 02 | Resource & Robustness       | Low      | S           | 00         |
| 03 | Performance — Backend       | Low      | M           | 00         |
| 04 | Performance — Frontend      | Medium   | M           | 00, 03     |
| 05 | Knowledge UX & Data         | Medium   | M           | 00         |
| 06 | Code Quality / Refactoring  | High     | L           | 00, 01–05  |
| 07 | UX Polish                   | Low      | S           | 00         |

Risk = likelihood of regression. High-risk groups must include rollback
checkpoints and behavioral diff testing.

Effort: S = ½–1 day, M = 1–3 days, L = 3+ days.

---

## Cross-cutting deliverables

These artifacts are produced once (in Group 0) and reused everywhere:

- **`scripts/test.sh`** — runs `cargo test` and `vitest run` together,
  used in pre-push hook.
- **`docs/testing.md`** — testing conventions, mock harnesses, fixture
  layout.
- **`src/lib/modules/logging/`** — frontend structured logger.
- **`src-tauri/src/modules/log/`** — Rust log sink Tauri command.
- **`tests/fixtures/`** — shared project-root fixtures used across
  filesystem and git tests.

---

## Per-group spec format

Every group spec follows the same structure so dispatched agents know
exactly what to expect:

1. **Goal** — one paragraph stating the outcome.
2. **Audit references** — the specific findings (C1, H4, etc.) covered.
3. **Preservation guarantees** — user-visible behaviors that must not
   change. This is the contract.
4. **Pre-flight** — verifications to run before changing code, so the
   baseline is captured.
5. **Implementation tasks** — the actual work, broken into steps. Each
   step describes the change, the files it touches, and the rationale.
6. **Test plan** — what to write, what to verify. Includes edge cases.
7. **Code review checklist** — what the review sub-agent must verify.
8. **Rollback** — how to revert without data loss.
9. **Out of scope** — what intentionally is NOT touched in this group,
   to prevent scope creep.

---

## Conventions for sub-agent dispatch

When dispatching a group to a sub-agent:

- The sub-agent reads the entire spec for that group AND this README.
- The sub-agent must NOT modify files outside the group's stated scope.
  If a fix requires touching another module, it stops, documents the
  finding, and surfaces it back instead of expanding scope.
- The sub-agent commits in small, reversible steps. Each commit's
  message starts with `[Gxx]` where xx is the group number.
- After implementation, the sub-agent dispatches the code-review
  sub-agent with the spec's review checklist and the diff.
- The sub-agent reports back: what was done, what was deferred, what
  was discovered as new findings.

---

## Audit finding cross-reference

Each finding from the audit (`Cn`, `Hn`, `Mn`, `Ln`) is owned by exactly
one group. The owning group has the canonical fix; if a finding shows up
in another group's discussion, that other group references it without
implementing it.

| Group | Owns                                                      |
|-------|-----------------------------------------------------------|
| 00    | C1, M8, (test infra)                                      |
| 01    | C2, C3, C5, C6, M9, M10                                   |
| 02    | C8, C9, M14, M17                                          |
| 03    | C7, H2, H8, M13                                           |
| 04    | H1, H3, H4, H5, H6, H7, H9, H10, H11, H12, L4             |
| 05    | C4, M2, M3, M6, M7, M16                                   |
| 06    | M1, M4, M5, M15, L1, L5, L9                               |
| 07    | L2, L3, L6, L7, L8, M11, M12                              |

If you can't find a finding in this table, it's either deferred (call it
out) or was deduplicated during planning (check the relevant spec).
