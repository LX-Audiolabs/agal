# AGENTS.md — agentic-audiolab (`agal`)

Rules for **this** repo (the `agal` tool), not for dogfood plugin workspaces.

## Policy (always on)

Before coding or long replies, load:

| Skill | Path | Role |
|-------|------|------|
| **caveman** | [`skills/01-policy/caveman.md`](skills/01-policy/caveman.md) | Dense talk: no fluff, exact terms |
| **ponytail** | [`skills/01-policy/ponytail.md`](skills/01-policy/ponytail.md) | Smallest correct change; YAGNI; fewest files |

**Default stance:** ponytail for every edit · caveman for implementation chat.  
Code, commits, PR text: normal prose. Security / irreversible: clear full sentences.

Stop caveman: user says `stop caveman` / `normal mode`.  
Ponytail marks deferrals with `ponytail:` comments.

Do **not** invent parallel style guides here — edit the skill files if policy changes.

## What this product is

AI layer for LX plugin work. **AURA** is the framework (runtime / formats / CLI). **AGAL** orients agents on top. Together: **JUCE-like** plugin work with AI — **Rust + Slint**, not C++.

Orientation layer for AI-assisted Rust audio workspaces:

| | |
|--|--|
| Crate / folder | `agentic-audiolab` |
| Binary | **`agal`** |
| Library | `crates/core` |
| Canonical skills | `skills/` (synced into workspaces via CLI) |
| Config sample | `examples/agal.toml` |

Dogfood shape: LX Audiolabs (`plugins/` + `crates/`, AURA, Slint).  
Does **not** replace Clippy, clap-validator, or graphify.

## Orientation in *this* repo

No generated `agal/` folder for the tool repo itself is required for day-to-day work.

1. **`AGENTS.md`** (this file) — policy + scope
2. **`README.md`** — product identity, CLI, config
3. **`CONTRIBUTING.md`** — build / test / PR checklist
4. **`skills/`** — load **one** pack file when needed; never dump the tree
5. Agent hot path for *plugin* workspaces: `skills/06-agents/agent-usage.md`  
   (disclosure L3→L0 · budget · loadouts · stack · notes atoms — same as generated `AGAL.md`)

## Versioning (SemVer)

Single workspace version: root `Cargo.toml` + `crates/core/Cargo.toml` (keep in sync).
Pre-1.0 semver: `0.MAJOR.MINOR` — MAJOR for breaking, MINOR for features/fixes.
Bump BEFORE commit, not after. Changelog: `CHANGELOG.md`.

## Build / test

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -- .                    # smoke against a plugin workspace path
cargo install --path . --force    # install `agal` on PATH
```

Rust **1.85+** (edition 2024).

## Scope discipline

- Prefer fix / reuse in `crates/core` over new crates.
- Skills: numbered packs only (`00-core` … `06-agents`). No root `*_SKILL.md`.
- agal must **never** overwrite a workspace root `AGENTS.md` (user-owned there).
- Cheatsheet template lives in `crates/core/src/guide.rs` — keep in sync with README after CLI changes.
- Update `CHANGELOG.md` under the current version section when behavior changes.

## Local identity (private)

**Do not put email / tokens / machine paths in this file** — it is versioned.  
Private commit identity & auth live in **`AGENTS.local.md`** (gitignored).  
Agents: if that file exists, prefer it for git author / push auth hints.

## github commits & push

Commits: `user.name=lxndrbe` · `user.email=ardvinnamoon@gmail.com`  
GitHub auth: `github.user=lxndrbe`

## Do not

- Expand policy into this file (link skills)
- Auto-load every skill on every turn
- Speculative architecture mid-task (ponytail)
- Ceremonial agent prose (caveman)
