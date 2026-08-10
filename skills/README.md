# Skills (canonical, in the agal tool)

**Summary:** Domain + policy skill packs for AI-assisted Rust audio work.  
Live **here** in the tool repo (**agentic-audiolab**). Workspaces get a **curated
copy** via CLI — not on every generate.

## Why not auto-copy?

Dumping every skill into the project on each `agal .` recreates the MCP trap:
huge context, weak priority. Hot path = **AGAL.md** + map + one note. Skills =
constitution, loaded on purpose.

## Sync into a plugin workspace

```bash
# default: core only (DSP constitution)
agal skills sync

# whole groups
agal skills sync --only policy
agal skills sync --only policy,agents
agal skills sync --only formats,ui

# single skills (group/name or numbered path)
agal skills sync --only ui/slint
agal skills sync --only formats/clap,ui/slint
agal skills sync --only 04-ui/slint

# mix + full pack
agal skills sync --only core,ui/slint
agal skills sync --only all --force

agal skills list
```

Writes to `<workspace>/agal/skills/` and refreshes **`agal/AGAL.md`**
(skills index). Without `--force`, existing files are skipped (local edits kept).

## Selectors

| Form | Example | Result |
|------|---------|--------|
| **group** | `policy`, `ui` | whole pack |
| **group/skill** | `ui/slint` | one file → `04-ui/slint.md` |
| **numbered path** | `04-ui/slint` | same, catalog path style |
| **stem** (if unique) | `slint` | same as `ui/slint` |
| **mix** | `core,ui/slint` | union, deduped |
| **all** | `all` | everything (alone) |

## Groups (numbered = priority)

| Group | Path | Sync |
|-------|------|------|
| **core** (default) | `00-core/` | DSP realtime, correctness, audio thread boundary, biquad |
| **policy** | `01-policy/` | caveman, ponytail (style — opt-in) |
| **frameworks** | `02-frameworks/` | framework patterns, AURA, truce→AURA migration |
| **formats** | `03-formats/` | CLAP, VST3, LV2 |
| **ui** | `04-ui/` | Slint, egui, iced, vizia |
| **migrations** | `05-migrations/` | e.g. nih-plug → nice-plug |
| **agents** | `06-agents/` | agent-usage playbook |
| **all** | everything above | rare — context bloat |

## Agent read order

**Disclosure:** L3 `AGAL.md` → L2 map (+ delta) → L1 one note → L0 slice/json.  
Next layer only if current is not enough.

1. **L3** `agal/AGAL.md` (skills + budget / loadouts / stack / disclosure)
2. **L2** `agal.agent.md` (map + health)
3. **L2** `agal.delta.md` if present
4. **L1** `agal/notes/<focus>.md` (**one** note; scan `[ATOM]` first)
5. **loadout** — core skills if synced (`00-core/`) — **≤1** skill file default
6. Other packs when the task needs them (`ui/slint`, formats, …)
7. **L0** JSON / slice escalate

**Budget:** 1 note · ≤1 skill · errors before warns. Full tables live in `AGAL.md` after generate.

## Workspace root

Product rules live in root **`AGENTS.md`** (user-owned).  
Point at `agal/AGAL.md`. agal never overwrites root AGENTS.md.

## Registry

Framework index: [registry/frameworks.toml](../registry/frameworks.toml)
