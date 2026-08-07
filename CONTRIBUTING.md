# Contributing

**Summary:** How to build, test, and contribute to agentic-audiolab.
Covers workspace setup, code structure, and submission checklist.
Keep changes scoped and run clippy before opening a PR.

## Build

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Requires Rust **1.85+** (edition 2024). CI runs the same checks (see `.github/workflows/ci.yml`).

## Running

```bash
cargo run -- .
cargo run -- --plugin <name> .
cargo run -- doctor .
cargo run -- --watch .
```

## Project structure

| Path | Purpose |
|------|---------|
| `src/main.rs` | CLI (`agal`) |
| `crates/core/` | library: scan, findings, notes, html, skills |
| `crates/core/tests/fixtures/mini_ws` | integration fixture |
| `examples/agal.toml` | config sample |

## Before submitting

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- Smoke: `cargo run -- /path/to/plugin-workspace`
- Update `CHANGELOG.md` under the current version section

## Docs to keep in sync

| Doc | Role |
|-----|------|
| `README.md` | product identity, scope, CLI, config, findings |
| `crates/core/src/guide.rs` | **Cheatsheet.md** template (regenerated into `agal/`) |
| `skills/00-core/` | DSP constitution (default `agal skills sync`) |
| `skills/06-agents/agent-usage.md` | agent hot path (opt-in: `agal skills sync --only agents`) |
| `examples/agal.toml` | config sample |

After changing the Cheatsheet template, rebuild and run `agal .` on a workspace so
generated `Cheatsheet.md` matches.

## Licensing

All contributions are licensed under MIT.
