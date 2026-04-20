# Follow-Ups

Deferred work items discovered during missions. One bullet per item.
Every new mission starts by reviewing this file and pulling in anything
that belongs in scope.

Format per entry:

> - **[YYYY-MM-DD · mXX]** Short description.
>   *Why deferred:* …
>   *Earliest mission:* mXX.

---

## Open

- **[2026-04-20 · M01]** GitHub Actions is deprecating Node.js 20 (used by
  `actions/checkout@v4` and similar). Node 20 support is removed September
  2026; starting June 2026 actions are forced to Node 24.
  *Why deferred:* Non-blocking warning only; Node 24 migration requires
  testing all third-party actions we use. Not a performance issue.
  *Earliest mission:* M11 (release engineering) or whenever GitHub prompts
  again, whichever is sooner.

- **[2026-04-20 · M01]** `cosmic-text` version is not yet pinned in
  `docs/TECH_STACK.md`; it must be chosen to be compatible with `glyphon`
  at M04 adoption time.
  *Why deferred:* Cannot pin accurately until we know which `glyphon`
  minor we consume and what its transitive `cosmic-text` bound is.
  *Earliest mission:* M04.

## Resolved

*(none yet)*
