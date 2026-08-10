# Changelog

**Summary:** Versioned history of agentic-audiolab changes.
Tracks breaking changes, features, and migration notes.
Recent entries appear at the top.

All notable changes to `agentic-audiolab` / `agal`.

## [0.7.0] — Unreleased

### Added
- **AURA skill pack** (`skills/02-frameworks/aura.md`) — PluginLogic, `#[derive(Params)]` with `id=N`, `cargo aura` CLI, format wrappers. Embedded in catalog → `agal skills sync --only frameworks` / `frameworks/aura`.
- **AURA migration skill** (`skills/02-frameworks/aura-migration.md`) — truce→AURA step-by-step: macro→trait, params IDs, editor switch, build, findings reference. Embedded → `frameworks/aura-migration`.
- **Slint skill expanded** (`skills/04-ui/slint.md`) — AuraSlintEditor host-embed, @aura widget catalog, callback wiring, aura-build pipeline, Lx* shared components.
- **`aura.toml` detection** — auto-detected; enables AURA-specific editor migration path.
- **AURA-specific findings:**
  - `aura_missing_plugin_logic` (error) — plugin on AURA but no `impl PluginLogic` detected.
  - `aura_missing_param_ids` (warn) — `#[param(...)]` fields without explicit `id = N`.
  - `aura_truce_migration` (info) — truce imports/macros detected in AURA plugin.
- **`cargo aura` tool hint** — info finding with build/install fix when AURA workspace detected.
- **`has_explicit_id` on `ParamField`** — ast extraction now captures `id = N` from `#[param(...)]`.

### Changed
- **AURA-first defaults** — `default_migrations()` context-aware: `aura.toml` → `truce-slint → aura-editor`, else legacy.
- **Editor target** — defaults to `aura-editor` in AURA workspaces.
- **`append_hints` now wired** — tool hints injected into findings pipeline.

### Removed
- **truce-specific findings** — `logic_macro_mismatch` and `params_without_plugin_macro` removed; replaced by AURA equivalents.

## [0.6.2] — 2026-08-07

### Fixed
- **`lv2-sys` / `lv2-raw` → framework `lv2`** — format wrappers that bind LV2 via sys crates (e.g. AURA `aura-lv2`) now appear in `used_frameworks` and per-node `frameworks`.
- **`agal skills sync` refreshes `agal.agent.md`** — skills count no longer stale after sync (was only rewritten on full `agal .`).
- **`crate_no_dependents` skips nested `/examples/`** — package demos under `crates/*/examples/` no longer info-noise.
- **Notes for `member` nodes** — `examples/` and `tools/` get L1 notes + index section (framework workspaces).

## [0.6.1] — 2026-08-07

### Added
- **`[view]` config section** in `agal.toml`: `default = "overview" | "all" | "plugin" | "crate"` overrides auto-detected default view.
- **`badge-member` CSS** for non-standard node kinds in detail drawer.

### Changed
- **Framework-repo support**: HTML view auto-detects repo type — defaults to "all nodes" when no plugins present (e.g. AURA). Overview falls back to full graph for pure-crate workspaces.
- **View filter**: "member" nodes (neither `plugins/` nor `crates/`) now included in "crates" filter and shown as diamonds in cytoscape.
- **UI text de-plugin-ized**: "plugins + hubs" → "hubs", search placeholder updated, overview metrics adapt to node types.

## [0.6.0] — 2026-08-07

### Changed
- **Dependency bumps**: `notify` 6→8, `syn` 2→3, `toml` 0.8→1 (breaking API changes, code fixed).

## [0.5.4] — 2026-08-06

### Fixed
- **Default output dir is `agal/`** — leftover user-facing strings still said `audiolabs` (agent map title). Install this build so the binary matches the 0.5.x rename.
- **`agal skills sync`** honors `agal.toml` `output_dir` when `-o` is omitted (same resolution as generate).

### Notes
- Legacy folder name `audiolabs/` still works if set explicitly in `output_dir`. New workspaces should use default `agal/` (omit the key or set `output_dir = "agal"`).
- Config filename fallbacks (`audiolabs.toml`, `audio-graph.toml`) remain for migration.

## [0.5.3] — 2026-08-05

### Added
- **Skill loadout presets** — `agal skills sync --preset slint-ui` (also as `--only` tokens): `dsp-fix`, `slint-ui`, `clap-ship`, `agent-playbook`, `policy-edit`; listed in `agal skills list`

## [0.5.2] — 2026-08-05

### Added
- **Equipped (on disk)** in `AGAL.md` — group names + skill file count after generate/skills sync
- **Focus strip** in `AGAL.md` when health ≠ ok — top error/warn findings with path + fix (max 8)
- **Loadout verify column** — clippy / clap-validator commands where applicable
- **`notes/_workspace.md`** — durable workspace memory; seeded once, never overwritten

## [0.5.1] — 2026-08-05

### Added
- **Disclosure layers L3→L0** — progressive read model in `AGAL.md`, agent map hot path, Cheatsheet, agent-usage, README (next layer only if needed)
- **Graph atoms in notes (AUTO)** — dense `[ATOM]` lines from scan (migration, frameworks, key edges, error/warn findings); max 12; human atoms stay below HUMAN; info tool-hints excluded from atoms
- **`AGENTS.local.md`** (gitignored) — private commit identity; public `AGENTS.md` keeps no email; example: `AGENTS.local.md.example`

### Changed
- **Cheatsheet** — L1 = graph atoms first; mix-ups for hand-editing auto atoms; notes atom kinds table

## [0.5.0] — 2026-08-05

### Added
- **Root `AGENTS.md`** (tool repo) — points agents at caveman + ponytail policy skills
- **Agent context budget + task loadouts + stack layers** — regenerated into `AGAL.md` / Cheatsheet; mirrored in agent-usage skill + README
- **Notes `[ATOM]` convention** — optional durable one-liners; empty-note seed includes an Atoms stub
- **`agal doctor` optional symbol tools** — PATH probe for `codegraph`, `codebase-memory-mcp`, `graphify` (report only; no generate findings)
- **Skill frontmatter `triggers` / `verify`** — parsed into `AGAL.md` skill index; core / policy / slint / clap / agent-usage packs filled in

## [0.4.2] — 2026-08-04

### Added
- **`agal.toml` is the canonical config name** — loader order: `agal.toml` → `audiolabs.toml` → legacy `audio-graph.toml`
- **HTML deps tab** — `build` badge on `build_depends_on` / `dev_depends_on` entries
- **`show build edges` persists** — checkbox state stored in `localStorage`

### Fixed
- **Focus view orphaned build-dep nodes** — `focusNode` showed neighbors over build/dev edges without re-showing the edges themselves (e.g. `lx-slint-build` visible but lineless)
- **Hub detection ignored build edges** even with `show build edges` enabled — build-only crates stayed invisible in overview mode

## [0.4.1] — 2026-07-30

### Added
- **Single-skill sync** — `agal skills sync --only ui/slint` (also `04-ui/slint`, unique bare stems, mixes with groups)
- **`audiolabs/AGAL.md`** — agal-owned agent entry (skills index + hot path). Regenerated by `agal .` and `agal skills sync`. Workspace-root `AGENTS.md` stays user-owned.

### Changed
- **`agal skills sync` default is `core` only** (`00-core/`) — domain constitution, not style
- **Skill pack numbering** — `00-core`, `01-policy`, `02-frameworks`, `03-formats`, `04-ui`, `05-migrations`, `06-agents`
- **Policy + agents are opt-in** — `agal skills sync --only policy`, `--only agents`, or `--only all`
- **agent-usage** is its own group (`agents` → `06-agents/`), no longer folded into policy
- **Skills hierarchy** — drop `10-lx/` / `90-project/` escape hatches; packs under `00`–`06` only
- **Migration noise** — completed migrations collapse to one quiet line; plugin lines no longer badge every `[migrated]`
- **Hot path** — start at `AGAL.md`, then structural `audiolabs.agent.md`

## [0.4.0] — 2026-07-29

### Added
- **Hybrid notes** — `audiolabs/notes/<plugin|crate>.md` with auto header + preserved human body
- **`agal skills sync` / `list`** — skills stay in the tool; curated copy into workspace (default was policy; now core — see Unreleased)
- Policy skills: **caveman**, **ponytail**, updated **agent-usage** read order
- Agent summary lists notes focus targets and skills sync reminder
- **HTML overview mode** (default): plugins + hub crates; build edges off; info findings opt-in
- **`audiolabs/Cheatsheet.md`** — CLI cheatsheet in the workspace folder (Obsidian-friendly)
- **Finding `path` + `fix`** — navigation + short remediation on each finding
- **Health gate** — `ok` / `degraded` / `blocked` in agent.md + CLI
- **Integrity findings** — `workspace_member_missing`, `plugin_not_in_workspace`, `package_not_in_workspace`, `required_dep_missing`
- **Tool hints (info)** — `tool_hint_clippy`, `tool_hint_clap_validator` (PATH-aware `fix`; tools not executed on generate)
- **`agal doctor`** — checklist for Clippy + clap-validator + CLAP plugins
- **Tests** — fixture workspace scan, delta, notes human-body, health/tool_hints unit tests
- **`[[suppress]]` in agal.toml** — mute findings by code (+ optional node)
- **CI** — GitHub Actions: fmt, clippy `-D warnings`, `cargo test --workspace`

### Changed
- Generate **no longer auto-copies** the full skill tree (avoids context bloat)
- Default JSON is slimmer: `dependency_details` only with `-v`
- Version 0.4.0 orientation-layer framing (graph + notes + curated skills)
- **agent.md** lists error+warn only; info findings stay in json/html
- **README naming** — product is **agentic-audiolab** / **`agal`**; honest LX/truce/Slint-first scope
- **Docs polish** — Cheatsheet (health/suppress/doctor/config), agent-usage skill, schema v0.4
- **README** — workspace skills (`10-lx` / `90-project`), authoring contract, selective gitignore,
  slice refresh, suppressed count, generic + dogfood config examples
- **agent.md** — `## skills` index + authoring; `frameworks detected`; `suppressed: N`
- **used_frameworks** — migration endpoints no longer injected as “in use”

## [0.3.2] — 2026-07-28

### Changed
- Generalized README: `nih-plug` → `nice-plug` as primary migration example, de-emphasized `truce`
- Added screenshots to README (`examples/img/`)

## [0.3.1] — 2026-07-27

### Changed
- Renamed crate from `lx-audiolabs` to `rust-audiolabs`
- Updated all repository URLs, internal references, and metadata

## [0.3.0] — 2026-07-26

### Added
- Agent summary (`audiolabs.agent.md`) — compact 1–3k token markdown for AI agents
- Structural findings with severity levels (error/warn/info)
- Graph delta tracking (`audiolabs.delta.md`) — diff vs previous generation
- Stricter domain graph: migration status, IPC signals, param binding analysis
- Polished HTML viewer with search, kind/framework filters, detail drawer

### Changed
- Footer layout: 4-column horizontal grid instead of vertical stack

## [0.2.0] — 2026-07-24

### Added
- Generic framework detection — no LX hardcoding
- Auto-detection of `project_name` and `internal_crates`
- Framework taxonomy via `agal.toml`
- Plugin file trees, focus view, layout comparison matrix
- AST-level import analysis and Cargo dependency resolution
- UI framework detection: `egui`, `Iced`, `Vizia`

### Changed
- Node colors, Slint component display, focused file trees in HTML
- Generalized README for non-truce plugin stacks

## [0.1.1] — 2026-07-22

### Changed
- HTML: removed compare plugin layouts button and overlay
- Smaller nodes, larger padding, cose layout
- Interactive plugin list: focus on click, show neighbors on filter
- Read-only comparison overlay

## [0.1.0] — 2026-07-20

### Added
- Initial release
- Plugin/crate workspace graph generation
- HTML output with Cytoscape.js visualization
- Cargo workspace member discovery
- Basic framework detection
