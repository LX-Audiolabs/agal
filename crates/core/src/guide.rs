//! Workspace folder guide — CLI cheatsheet for humans (Obsidian / VS Code).

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::Audiolabs;

/// Write `Cheatsheet.md` into the agal output root (commands + what lives where).
pub fn write_readme(
    output_dir: &Path,
    graph: &Audiolabs,
    output_dir_name: &str,
) -> Result<(), String> {
    let body = render(graph, output_dir_name);
    let path = output_dir.join("Cheatsheet.md");
    fs::write(&path, body).map_err(|e| format!("cannot write {}: {}", path.display(), e))?;
    Ok(())
}

fn render(graph: &Audiolabs, out: &str) -> String {
    let mut s = String::with_capacity(5120);
    let plugins: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|n| n.kind == "plugin")
        .map(|n| n.name.as_str())
        .collect();
    let crates: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|n| n.kind == "crate")
        .map(|n| n.name.as_str())
        .collect();

    let _ = writeln!(
        s,
        r#"# Cheatsheet — agal

**Summary:** Local orientation layer for humans and AI in a plugin workspace.  
Everything useful lives **in this folder**. Product: **agentic-audiolab**, binary: **`agal`**.

project: **{project}**  
generated: `{generated}`  
tool version: {version}

---

## Quick start (from the workspace root)

```bash
# Rebuild map + note headers + HTML + this Cheatsheet
agal .

# Optional: focus one plugin (1-hop slice JSON)
agal --plugin {example_plugin} .

# PATH check: Clippy + clap-validator (+ optional symbol tools; not run by generate)
agal doctor

# Pull skills once (live in the tool; not auto-copied on every generate)
agal skills sync
```

Requires `agal` on your `PATH`, or install from the tool repo:  
`cargo install --path . --force` in **Agentic Audiolab**.

Config (optional): **`agal.toml`** next to the root `Cargo.toml`  
(older names `agal.toml` and `audio-graph.toml` still load if the new file is missing).

---

## What is in this folder?

| Path | Who | Purpose |
|------|-----|---------|
| **[AGAL.md](./AGAL.md)** | AI first | **agal entry** — skills index + hot path (regenerated) |
| **[Cheatsheet.md](./Cheatsheet.md)** | you | **this file** — CLI + workflow |
| **[agal.html](./agal.html)** | you | graph overview (browser / Obsidian HTML) |
| **[agal.agent.md](./agal.agent.md)** | AI | structural map + **health** (~1–3k tokens) |
| **[agal.delta.md](./agal.delta.md)** | AI / you | structural changes since last generate |
| **[agal.json](./agal.json)** | AI escalate | full edges, params, **all** findings (incl. info) |
| **[notes/](./notes/)** | you + AI | one MD per plugin/crate (auto header + your text) |
| **[notes/_index.md](./notes/_index.md)** | you | index of all notes |
| **skills/** | AI | tool packs after `agal skills sync` — default: core (DSP) |
| `*.slice.json` | AI | after `agal --plugin NAME .`; **refreshed** on later `agal .` |

Workspace-root **`AGENTS.md`**: yours. Point at `{out}/AGAL.md`; agal never overwrites it.

Typical setup: **`{out}/`** is in `.gitignore` (code on GitHub, PM layer local).

---

## CLI commands

Binary: **`agal`**. Always run from the **Cargo workspace root** (root `Cargo.toml`).

### Generate (refresh the map)

| Command | What it does |
|---------|----------------|
| `agal .` | **AGAL.md**, agent map, delta, json, **notes/**, html, Cheatsheet |
| `agal` | same as `agal .` |
| `agal --plugin {example_plugin} .` | same **plus** `{example_plugin}.slice.json` |
| `agal -v .` | verbose JSON (dependency_details, symbols, full catalog) |
| `agal --agent-only .` | skip HTML |
| `agal -o other-dir .` | write somewhere other than `{out}/` |
| `agal --watch .` | regenerate on `.rs` / `Cargo.toml` / `.slint` changes |
| `agal --install-hook .` | post-commit → run `agal` after each commit |

### Skills (not on every generate)

| Command | What it does |
|---------|----------------|
| `agal skills list` | list embedded packs |
| `agal skills sync` | **default: core** → `{out}/skills/00-core/` |
| `agal skills sync --preset slint-ui` | loadout preset → `core` + `ui/slint` |
| `agal skills sync --only policy` | whole group (opt-in) |
| `agal skills sync --only ui/slint` | **single** skill from a group |
| `agal skills sync --only slint-ui` | same as `--preset slint-ui` (preset as token) |
| `agal skills sync --only core,ui/slint` | mix groups + singles |
| `agal skills sync --only all` | everything (rare — context bloat) |
| `agal skills sync --force` | overwrite existing skill files |
| `agal skills sync --output {out}` | different output directory |

Presets: `dsp-fix` · `slint-ui` · `clap-ship` · `agent-playbook` · `policy-edit` (see `agal skills list`).

Without `--force`, existing skill files are **skipped** (local edits kept).

### Query (focused context packs)

| Command | What it does |
|---------|----------------|
| `agal impact <name>` | reverse dependencies — who uses this crate/plugin |
| `agal context --focus <name> --budget 4000` | token-capped Markdown pack (focus + 1-hop + findings) |
| `agal context --focus <name> --budget 4000 --format json` | same pack as JSON |

Use these when you need a quick blast-radius or a focused context bundle without opening the full graph.

### Doctor (external tools — not executed by generate)

| Command | What it does |
|---------|----------------|
| `agal doctor` | PATH: **Clippy** + **clap-validator** + optional symbol tools |
| `agal doctor .` | same with explicit root |

Generate only adds **info** findings (`tool_hint_clippy`, `tool_hint_clap_validator`).  
It does **not** run Clippy or validate built `.clap` files.  
Optional symbol tools (`codegraph`, `codebase-memory-mcp`, …) are **doctor-only** — no generate findings.

---

## Health & findings

| health | Meaning |
|--------|---------|
| **ok** | no error/warn |
| **degraded** | warnings only |
| **blocked** | any **error** — fix before feature work |

- **agent.md**: error + warn only (`path` + `fix` when present); header shows `suppressed: N` when `[[suppress]]` mutes findings
- **json / html**: all severities (info includes tool hints, param surface, …); `findings_suppressed` count
- **suppress** intentional noise in root `agal.toml` (health stays green; count still visible):

```toml
[[suppress]]
code = "large_param_surface"
node = "aurum-slint"    # package name, path, or id; omit = all nodes
reason = "product surface intentional"
```

---

## Workflow

### Human

1. `agal .` after larger code changes (or `--watch`)
2. Open **[agal.html](./agal.html)** — plugins + hubs
3. Edit **[notes/&lt;name&gt;.md](./notes/)** **below** the HUMAN marker only (AUTO block + graph atoms are rewritten)
4. `agal skills sync` once per machine/workspace when needed
5. `agal doctor` before release-style validation

### AI agent (hot path — do not dump the whole folder)

**Disclosure:** open the **next** layer only if the current one is not enough.

| Layer | Artifact |
|-------|----------|
| **L3** | `AGAL.md` — entry, **focus strip**, equipped skills, budget, loadouts |
| **L2** | `agal.agent.md` (+ `delta`) — map + health |
| **L1** | `notes/&lt;focus&gt;.md` — **graph atoms first**, then human body |
| **L0** | slice / `agal.json` — escalate only |
| loadout | ≤1 skill file (match `triggers`) — not a layer |
| durable | `notes/_workspace.md` — workspace memory (never overwritten) |

1. **L3** `AGAL.md`
2. **L2** `agal.agent.md` — structural map + **health**
3. If **blocked**: fix error findings first (`path` + `fix`)
4. **L2** `agal.delta.md` if present
5. **L1** `notes/&lt;focus&gt;.md` (**one** note) — scan **Graph atoms (auto)** before prose
6. **loadout** — skills on demand (`00-core` default, `ui/slint`, …) — **≤1** skill file
7. **L0** escalate: slice / `agal.json`

### Context budget (per turn)

| Cap | Default |
|-----|---------|
| focus notes | **1** |
| skill files | **1** (or one loadout) |
| findings | errors first, then warns; info only on escalate |
| JSON / slice | only when map + note are not enough |

### Task loadouts / presets

| Task | Command | Verify (when applicable) |
|------|---------|--------------------------|
| DSP / realtime | `agal skills sync --preset dsp-fix` | `cargo clippy --workspace --all-targets -- -D warnings` |
| Policy / terse edits | `agal skills sync --preset policy-edit` | — |
| Slint UI | `agal skills sync --preset slint-ui` | — |
| CLAP / formats | `agal skills sync --preset clap-ship` | `clap-validator validate path/to/plugin.clap` (after build) |
| Agent playbook | `agal skills sync --preset agent-playbook` | — |
| Everything | `agal skills sync --only all` (rare) | — |

Presets also work as `--only` tokens (`--only slint-ui`).  
**Equipped** skills (what is already under `skills/`) are listed at the top of **`AGAL.md`** after each generate / skills sync.

### Stack layers

**agal** (structure) · **Clippy** · **clap-validator** · optional symbol tools (codegraph / codebase-memory / graphify — not agal).

### Notes atoms

| Kind | Where | Source |
|------|-------|--------|
| **Graph atoms (auto)** | note AUTO block, fenced `[ATOM]` lines | scan: migration, frameworks, key edges, error/warn (max 12) |
| **Human atoms** | below HUMAN marker | you / agent decisions & lessons |
| **Workspace memory** | `notes/_workspace.md` | durable; created once; **never** overwritten |

```text
[ATOM] type=fact|constraint|decision|lesson | detail=…
```

Info findings (e.g. tool hints) stay in `## findings` — not in graph atoms.

### Skills layout (tool packs only)

| Path | Who writes | Sync |
|------|------------|------|
| `skills/00-core/` | `agal skills sync` (default) | tool pack |
| `skills/01-policy/` … `06-agents/` | `agal skills sync --only …` | opt-in packs |

Live index: **`AGAL.md`**. Without `--force`, existing skill files are skipped (local edits kept).

Example: *“Continue on {example_plugin} — read AGAL.md + notes/{example_plugin}.md.”*

---

## This workspace (auto)

**Plugins:** {plugins}

**Crates:** {crates}

---

## Common mix-ups

| Wrong / unclear | Right |
|-----------------|--------|
| How do I regenerate the map? | **`agal .`** |
| `agal update` | **`agal .`** (no `update` subcommand) |
| Skills missing after generate | expected → **`agal skills sync`** |
| Note text gone after generate | edit only **below** the HUMAN marker |
| Graph atoms missing / stale | run **`agal .`** (AUTO regenerated) |
| Edit graph atoms by hand | don't — they are rewritten; use human atoms instead |
| Commit PM layer | prefer gitignoring `{out}/` |
| agal should run Clippy/validator | **no** — `agal doctor` + info hints only |

---

## Tool repo

**agentic-audiolab** (binary **`agal`**).  
Rebuild: `cargo build --release` or `cargo install --path . --force` in the tool repo.
"#,
        project = graph.project_name,
        generated = graph.generated_at,
        version = graph.version,
        example_plugin = plugins.first().copied().unwrap_or("aether"),
        out = out,
        plugins = if plugins.is_empty() {
            "_none_".into()
        } else {
            plugins
                .iter()
                .map(|p| format!("`{p}`"))
                .collect::<Vec<_>>()
                .join(", ")
        },
        crates = if crates.is_empty() {
            "_none_".into()
        } else {
            crates
                .iter()
                .map(|c| format!("`{c}`"))
                .collect::<Vec<_>>()
                .join(", ")
        },
    );

    s
}
