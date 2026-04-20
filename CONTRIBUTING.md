# Contributing

Thank you for helping build this project. These rules apply to **every** change,
whether from a human or an automated agent.

## Read the docs first

Before writing code, read:

1. [`CONTRIBUTING.md`](./CONTRIBUTING.md) (this file)
2. [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md)
3. [`docs/TECH_STACK.md`](./docs/TECH_STACK.md)
4. [`docs/RUST_CONVENTIONS.md`](./docs/RUST_CONVENTIONS.md)
5. [`reference/README.md`](./reference/README.md) — what the product *is* (frozen PRDs)

## Commits

Use [Conventional Commits](https://www.conventionalcommits.org/):

- `feat(scope): …` — user-visible behavior
- `fix(scope): …` — bug fixes
- `docs(scope): …` — documentation only
- `build(scope): …` — toolchain, workspace, dependencies
- `ci(scope): …` — CI configuration
- `test(scope): …` — tests only
- `chore(scope): …` — mechanical maintenance

Keep commits small and logically scoped. Each commit should compile and pass
tests on its own.

## Quality gates (before every commit)

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo build --release --workspace --locked
```

On Windows, also run `cargo run --release --bin editor-app` periodically and
close the window to verify a clean exit.

## Cross-platform

Primary development is on Windows; CI runs Windows, Linux, and macOS. Any code
touching paths, windowing, clipboard, or process APIs must consider all three
platforms. Use `#[cfg(target_os = "…")]` when behavior must diverge.

## Dependencies

Every new dependency must:

1. Be justified in the commit message and/or `docs/TECH_STACK.md`.
2. Pass `cargo deny check` (licenses, advisories, bans).
3. Prefer minor-pinned versions in `Cargo.toml` (`"1.0"`, not `"*"`).

## Scope

Do not expand MVP/V2 scope in drive-by changes. If something is important but
out of scope, add an entry to [`FOLLOWUPS.md`](./FOLLOWUPS.md) instead of
silently expanding features.

## Optional git hook

```text
git config core.hooksPath .githooks
```

The pre-commit hook runs `cargo fmt --check` and `cargo clippy` with warnings
denied.
