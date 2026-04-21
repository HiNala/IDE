[← missions](./) · [Implementation status](../MISSION_IMPLEMENTATION_STATUS.md)

# Sequential mission execution — notes for agents

This file complements [`00_MISSION_INDEX.md`](00_MISSION_INDEX.md). The index defines **intent**; [`../MISSION_IMPLEMENTATION_STATUS.md`](../MISSION_IMPLEMENTATION_STATUS.md) records **what is actually in the tree**.

## What “run every mission until perfect” means in practice

- **M00–M11 (MVP + V2 + packaging):** The codebase is designed to satisfy these layers first. Several items remain **partial** (formal acceptance measurements, installers, signing) — see the implementation status table.
- **M12–M24 (V3):** Each mission is **large** (new UI surfaces, Tree-sitter, diff, git, AI stack, vector index, chat). Treat each as **its own project phase**: read the mission doc end-to-end, run quality gates before/after, merge in vertical slices (data model → tests → GPU wiring → app integration).

## Dependency chain (do not skip)

```
M12 (resize/DPI polish) ─┐
M13 (workspace + buffers) ─┼→ M14 (sidebar / tabs / quick-open)
         │                  │
         └──────────────────┴→ M15+ (highlighting, find, diff, git, AI…)
```

M14 **must not** start until `editor-workspace` + `BufferManager` in `editor-app` are stable; M15+ assume M14 UI shell exists for navigation.

## Quality gates (every merge)

From the mission index: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace`, `cargo build --release`, then **boot** `cargo run --release` on Windows and smoke-test typing, scroll, save, close.

## Honest scope

Completing **M14–M24 to “perfect”** is **months** of engineering, not a single session. Use [`../MISSION_IMPLEMENTATION_STATUS.md`](../MISSION_IMPLEMENTATION_STATUS.md) as the single checklist; update it when a mission moves from “Not started” to “Partial” or “Done”.

*Last updated: 2026-04-20.*
