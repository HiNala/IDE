[← docs/](./) · [README](../README.md)

# Benchmark summary

Criterion benchmarks live under `crates/*/benches/`. Regenerate this table when cutting a release or after meaningful performance work.

## How to capture a baseline

```bash
cargo bench --workspace -- --save-baseline m08-mvp
```

Compare against a previous baseline (with [critcmp](https://github.com/BurntSushi/critcmp) installed):

```bash
cargo install critcmp --locked
cargo bench --workspace -- --baseline m08-mvp
critcmp main m08-mvp
```

## Current results

| Benchmark crate | Function / group | Median | Notes |
|-----------------|------------------|--------|-------|
| `editor-core` | `rope_benches` (see source) | _run locally_ | Hot-path benches expand in M02+ |
| _others_ | — | — | Add rows as benches are added |

**Last manual run:** _not recorded — run `cargo bench` after M02+ populates rope/layout benches._

---

*CI currently compile-checks benches (`bench.yml`) without storing median times in-repo.*
