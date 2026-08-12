use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::delta;
use crate::findings::{Finding, Health, Severity, actionable, count_by_severity, health};
use crate::{Audiolabs, Edge, Node};

/// Max error/warn lines in L3 focus strip.
const MAX_FOCUS_FINDINGS: usize = 8;

/// Compact markdown for AI agents (~1–3k tokens for typical audio workspace).
///
/// `skills_dir` = `<output>/skills` when present — indexes workspace skills + authoring rules.
pub fn render_agent_md(graph: &Audiolabs, skills_dir: Option<&Path>) -> String {
    let mut s = String::with_capacity(4096);
    let counts = count_by_severity(&graph.findings);
    let h = health(&graph.findings);
    let err_n = counts.get("error").copied().unwrap_or(0);
    let warn_n = counts.get("warn").copied().unwrap_or(0);
    let info_n = counts.get("info").copied().unwrap_or(0);
    let suppress_tail = if graph.findings_suppressed > 0 {
        format!(
            " · suppressed: {} (see agal.toml)",
            graph.findings_suppressed
        )
    } else {
        String::new()
    };
    let _ = writeln!(
        s,
        "# agal agent summary\n\n\
         **Summary:** Compact structural map of this audio workspace.  \n\
         Lists members, plugins, crates, frameworks, edges, and findings.  \n\
         Use as first context before opening the full JSON graph.\n\n\
         project: **{}**  \n\
         generated: `{}`  \n\
         version: {}  \n\
         health: **{}**  \n\
         nodes: {} · edges: {} · findings: {} (error={} warn={} info={}){}\n",
        graph.project_name,
        graph.generated_at,
        graph.version,
        h,
        graph.nodes.len(),
        graph.edges.len(),
        graph.findings.len(),
        err_n,
        warn_n,
        info_n,
        suppress_tail,
    );

    // Frameworks detected on nodes (not migration config endpoints).
    let used: BTreeSet<&str> = graph.used_frameworks.iter().map(|x| x.as_str()).collect();
    if !used.is_empty() {
        let _ = writeln!(
            s,
            "## frameworks detected\n{}\n",
            used.into_iter().collect::<Vec<_>>().join(", ")
        );
    }

    // Rules
    if !graph.rules.is_empty() {
        let _ = writeln!(s, "## rules");
        for (k, v) in &graph.rules {
            let _ = writeln!(s, "- **{}**: {}", k, v);
        }
        let _ = writeln!(s);
    }

    // Migration — only when plugins exist and legacy/migrated is non-zero.
    write_migration_section(&mut s, &graph.migration_summary);

    // Plugins (omit section when none — tool/framework workspaces stay quiet)
    let plugins: Vec<&crate::Node> = graph
        .nodes
        .iter()
        .filter(|n| n.kind == "plugin")
        .collect();
    if !plugins.is_empty() {
        let _ = writeln!(s, "## plugins");
        for n in plugins {
            let _ = writeln!(s, "{}", plugin_line(n));
        }
        let _ = writeln!(s);
    }

    // Crates (omit when empty)
    let crates: Vec<&crate::Node> = graph.nodes.iter().filter(|n| n.kind == "crate").collect();
    if !crates.is_empty() {
        let _ = writeln!(s, "## crates");
        for n in crates {
            let _ = writeln!(s, "{}", crate_line(n));
        }
        let _ = writeln!(s);
    }

    // Edges compact by kind
    let _ = writeln!(s, "## edges");
    for kind in edge_kind_order() {
        let group: Vec<&Edge> = graph.edges.iter().filter(|e| e.kind == *kind).collect();
        if group.is_empty() {
            continue;
        }
        let _ = writeln!(s, "### {}", kind);
        for e in group {
            let note = e
                .note
                .as_ref()
                .map(|n| format!(" — {}", n))
                .unwrap_or_default();
            let _ = writeln!(
                s,
                "- `{}` → `{}`{}",
                short_id(&e.from),
                short_id(&e.to),
                note
            );
        }
    }
    let _ = writeln!(s);

    // Findings — agent.md lists error+warn only (info lives in json/html)
    let _ = writeln!(
        s,
        "## findings (error={} warn={} · info={} in json/html)\n\
         health **{}** — if **blocked**, fix errors before feature work.",
        err_n, warn_n, info_n, h,
    );
    let action: Vec<&Finding> = actionable(&graph.findings).collect();
    if action.is_empty() {
        let _ = writeln!(s, "_no error/warn_\n");
    } else {
        for f in action {
            let _ = writeln!(s, "{}", finding_line(f));
        }
        let _ = writeln!(s);
    }

    // Delta (compact)
    if let Some(d) = &graph.delta {
        let _ = writeln!(s, "## delta");
        if d.first_run {
            let _ = writeln!(s, "_first run — no previous graph._\n");
        } else if delta::is_empty(d) {
            let _ = writeln!(s, "_no structural changes since previous graph._\n");
        } else {
            let _ = writeln!(
                s,
                "since `{}`: +{} nodes · -{} nodes · ~{} nodes · +{} edges · -{} edges · +{} findings · -{} resolved\n",
                d.previous_generated_at.as_deref().unwrap_or("?"),
                d.added_nodes.len(),
                d.removed_nodes.len(),
                d.changed_nodes.len(),
                d.added_edges.len(),
                d.removed_edges.len(),
                d.new_findings.len(),
                d.resolved_findings.len(),
            );
            for c in d.changed_nodes.iter().take(8) {
                let _ = writeln!(s, "- **{}**: {}", short_id(&c.id), c.changes.join("; "));
            }
            for f in d.new_findings.iter().take(6) {
                let _ = writeln!(s, "- NEW {}", finding_line(f).trim_start_matches("- "));
            }
            for f in d.resolved_findings.iter().take(4) {
                let _ = writeln!(
                    s,
                    "- RESOLVED [{}] **{}** {}",
                    f.severity,
                    f.code,
                    f.node.as_deref().map(short_id).unwrap_or_default()
                );
            }
            let _ = writeln!(s, "\n_full: `agal.delta.md`_\n");
        }
    }

    // Notes index (focus layer)
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
    let members: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|n| n.kind == "member")
        .map(|n| n.name.as_str())
        .collect();
    let _ = writeln!(
        s,
        "## notes (focus)\n`agal/notes/<name>.md` — auto header + human body\n"
    );
    if !plugins.is_empty() {
        let _ = writeln!(
            s,
            "plugins: {}\n",
            plugins
                .iter()
                .map(|n| format!("`{n}`"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if !crates.is_empty() {
        let _ = writeln!(
            s,
            "crates: {}\n",
            crates
                .iter()
                .map(|n| format!("`{n}`"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if !members.is_empty() {
        let _ = writeln!(
            s,
            "members: {}\n",
            members
                .iter()
                .map(|n| format!("`{n}`"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    // Skills: short pointer — live index + packs live in AGAL.md
    write_skills_section_compact(&mut s, skills_dir);

    // How to use
    let _ = writeln!(
        s,
        "## read order\n\
         Disclosure: **L3** `AGAL.md` → **L2** this file (+ delta) → **L1** one note → **L0** slice/json.  \n\
         Open the next layer only if the current one is not enough.\n\
         \n\
         1. **L3** **`AGAL.md`** (orientation — skills + budget / loadouts / disclosure).  \n\
         2. **L2** **this file** (structural map + health).  \n\
         3. If health is **blocked**, fix error findings first (path + fix fields).  \n\
         4. **L2** **`agal.delta.md`** if present.  \n\
         5. **L1** **`notes/<focus>.md`** (**one** note; scan `[ATOM]` first).  \n\
         6. **loadout** — skills on demand from `AGAL.md` (never dump all; **≤1** skill file).  \n\
         7. **L0** escalate: `agal --plugin NAME .` slice, or `agal.json`.  \n\
         8. HTML is for humans (overview); agents prefer md/json.  \n\
         \n\
         Skills are **not** auto-copied on generate — `agal skills sync` (default: **core** only).  \n\
         Existing `*.slice.json` files are refreshed on every generate; new slices need `--plugin NAME`.  \n\
         Human CLI cheatsheet: **`Cheatsheet.md`** in this folder.\n"
    );

    s
}

/// agal-owned agent entry (`AGAL.md`) — not the user's root AGENTS.md.
///
/// Regenerated by `agal .` and `agal skills sync`. Product rules stay in root AGENTS.md
/// with a one-line pointer here.
pub fn render_agal_md(
    graph: Option<&Audiolabs>,
    skills_dir: Option<&Path>,
    output_dir_name: &str,
) -> String {
    let mut s = String::with_capacity(4096);
    let (project, generated, version, health_s, stats) = match graph {
        Some(g) => {
            let counts = count_by_severity(&g.findings);
            let h = health(&g.findings);
            let err_n = counts.get("error").copied().unwrap_or(0);
            let warn_n = counts.get("warn").copied().unwrap_or(0);
            let info_n = counts.get("info").copied().unwrap_or(0);
            (
                g.project_name.clone(),
                g.generated_at.clone(),
                g.version.clone(),
                h.to_string(),
                format!(
                    "nodes: {} · edges: {} · findings: {} (error={} warn={} info={})",
                    g.nodes.len(),
                    g.edges.len(),
                    g.findings.len(),
                    err_n,
                    warn_n,
                    info_n
                ),
            )
        }
        None => (
            "(unknown — run `agal .`)".into(),
            chrono_stub(),
            env!("CARGO_PKG_VERSION").into(),
            "n/a".into(),
            "map not loaded — run `agal .` for full graph".into(),
        ),
    };

    let _ = writeln!(
        s,
        "# AGAL\n\n\
         **Summary:** agal-owned agent orientation for this workspace.  \n\
         Regenerated by `agal .` and `agal skills sync` — **do not hand-edit**.  \n\
         Put product / team rules in the workspace-root **`AGENTS.md`** and point here.\n\n\
         project: **{project}**  \n\
         generated: `{generated}`  \n\
         tool: agal {version}  \n\
         health: **{health_s}**  \n\
         {stats}  \n\
         output: `{out}/`\n",
        project = project,
        generated = generated,
        version = version,
        health_s = health_s,
        stats = stats,
        out = output_dir_name,
    );

    // P1: top actionable findings at L3 when health is not ok.
    if let Some(g) = graph {
        write_focus_findings_strip(&mut s, g);
    }

    // P1: what is actually on disk under skills/ (not the catalog).
    write_equipped_skills(&mut s, skills_dir);

    let _ = writeln!(
        s,
        "## Root AGENTS.md (yours)\n\n\
         agal **never** overwrites the workspace-root `AGENTS.md`.  \n\
         Minimal hook (keep your own rules above/below):\n\n\
         ```markdown\n\
         ## Orientation (agal)\n\
         Read **`{out}/AGAL.md`** first for map, health, and skills.\n\
         ```\n",
        out = output_dir_name,
    );

    let _ = writeln!(
        s,
        "## Disclosure (read layers)\n\n\
         Progressive disclosure — open the **next** layer only if the current one is not enough:\n\n\
         | Layer | Artifact | Open when |\n\
         |-------|----------|----------|\n\
         | **L3** entry | **this file** (`AGAL.md`) | always first — budget, loadouts, skills index |\n\
         | **L2** map | `agal.agent.md` (+ `delta` if present) | need structure / health / what changed |\n\
         | **L1** focus | `notes/<focus>.md` (scan **`[ATOM]`** first) | work on one plugin/crate |\n\
         | **L0** raw | `*.slice.json` / `agal.json` | map + note still insufficient |\n\
         | durable | `notes/_workspace.md` | cross-cutting decisions (never overwritten) |\n\n\
         Skills are a **side loadout** (≤1 file), not a layer — use `triggers` in the index.  \n\
         When health ≠ ok, read **Focus** strip on this page first.  \n\
         HTML / Cheatsheet = humans. Do **not** skip to L0 by default.\n"
    );

    let _ = writeln!(
        s,
        "## Hot path\n\n\
         1. **L3** — this file (`AGAL.md`)  \n\
         2. **L2** — `agal.agent.md` (structural map + health)  \n\
         3. If **blocked** — fix error findings first (`path` + `fix`)  \n\
         4. **L2** — `agal.delta.md` if present  \n\
         5. **L1** — `notes/<focus>.md` (atoms → open → intent)  \n\
         6. **loadout** — one skill from the pack list below (match `triggers`)  \n\
         7. **L0** escalate — `agal --plugin NAME .` slice, or `agal.json`  \n\
         8. HTML / Cheatsheet are for humans\n"
    );

    let _ = writeln!(
        s,
        "## Context budget (per turn)\n\n\
         Keep context small. Default caps:\n\n\
         | Cap | Default |\n\
         |-----|--------|\n\
         | focus notes | **1** (`notes/<focus>.md`) |\n\
         | skill files loaded | **1** (or one loadout row) |\n\
         | findings to act on | **errors first**, then warns; skip info until escalate |\n\
         | JSON / slice | only after map + note are not enough |\n\n\
         Do **not** dump `skills/`, full `agal.json`, or every note.\n"
    );

    let _ = writeln!(
        s,
        "## Task loadouts\n\n\
         Sync once, then load **only** what the task needs:\n\n\
         | Task | Preset / `--only` | Load in context | Verify (when applicable) |\n\
         |------|-------------------|-----------------|--------------------------|\n\
         | DSP / process / realtime | `--preset dsp-fix` (= `core`) | `00-core/*` as needed | `cargo clippy --workspace --all-targets -- -D warnings` |\n\
         | Policy / terse edits | `--preset policy-edit` (= `policy`) | `caveman` **or** `ponytail` | — |\n\
         | Slint UI | `--preset slint-ui` (= `core,ui/slint`) | `slint` (+ core) | — |\n\
         | CLAP ship / formats | `--preset clap-ship` (= `core,formats/clap`) | `clap` | after build: `clap-validator validate path/to/plugin.clap` |\n\
         | Agent orientation playbook | `--preset agent-playbook` (= `agents`) | `agent-usage` | — |\n\
         | Full pack | `--only all` | rare — context bloat | — |\n"
    );

    let _ = writeln!(
        s,
        "## Stack layers\n\n\
         | Layer | Tool | Role |\n\
         |-------|------|------|\n\
         | **structure** | **agal** | map, health, notes, curated skills |\n\
         | **Rust lint** | Clippy | `agal doctor` + CI — not run by generate |\n\
         | **CLAP binary** | clap-validator | same |\n\
         | **symbols / call graph** | optional (codegraph, codebase-memory, graphify, …) | not agal — use when you need callers/impact |\n"
    );

    let _ = writeln!(
        s,
        "## Notes atoms\n\n\
         **Graph atoms (auto)** — inside each note's AUTO block: migration, frameworks, key edges, \
         error/warn findings as `[ATOM]` lines (regenerated; max 12). Scan these first at L1.\n\n\
         **Human atoms (optional)** — below the HUMAN marker for durable decisions/lessons:\n\n\
         ```text\n\
         [ATOM] type=decision|lesson|constraint | detail=…\n\
         ```\n"
    );

    if let Some(g) = graph {
        write_migration_section(&mut s, &g.migration_summary);
    }

    write_skills_packs_and_index(&mut s, skills_dir);

    let _ = writeln!(
        s,
        "## Artifacts in this folder\n\n\
         | Path | Role |\n\
         |------|------|\n\
         | **`AGAL.md`** | **AI entry** (this file) |\n\
         | `agal.agent.md` | structural map + health |\n\
         | `agal.delta.md` | structural diff |\n\
         | `agal.json` | full edges / params / info findings |\n\
         | `notes/` | auto header + human body + graph atoms |\n\
         | `notes/_workspace.md` | durable workspace memory (**never** overwritten) |\n\
         | `skills/` | synced tool packs (see above) |\n\
         | `Cheatsheet.md` | human CLI guide |\n\
         | `agal.html` | human graph |\n\
         | `*.slice.json` | 1-hop plugin focus |\n"
    );

    let _ = writeln!(
        s,
        "## Commands\n\n\
         ```bash\n\
         agal .                              # refresh map + AGAL.md + notes + html\n\
         agal --plugin aether .              # + one-hop slice\n\
         agal skills sync                    # default: core (00-core)\n\
         agal skills sync --preset slint-ui  # core + slint (loadout)\n\
         agal skills sync --only ui/slint    # single pack file\n\
         agal skills sync --only policy,agents\n\
         agal skills list\n\
         agal doctor                         # Clippy + clap-validator + optional symbol tools\n\
         ```\n"
    );

    s
}

/// Write `{output_dir}/AGAL.md`. `graph` optional (skills-sync may only have JSON on disk).
pub fn write_agal_md(
    output_dir: &Path,
    graph: Option<&Audiolabs>,
    output_dir_name: &str,
) -> Result<(), String> {
    let skills_dir = output_dir.join("skills");
    let skills = if skills_dir.is_dir() {
        Some(skills_dir.as_path())
    } else {
        None
    };
    let body = render_agal_md(graph, skills, output_dir_name);
    let path = output_dir.join("AGAL.md");
    fs::write(&path, body).map_err(|e| format!("cannot write {}: {}", path.display(), e))?;
    Ok(())
}

/// After `skills sync`: refresh AGAL.md + agent.md skills line using `agal.json` if present.
pub fn refresh_agal_after_skills_sync(
    project_root: &Path,
    output_dir_name: &str,
) -> Result<(), String> {
    let output_dir = project_root.join(output_dir_name);
    fs::create_dir_all(&output_dir)
        .map_err(|e| format!("cannot create {}: {}", output_dir.display(), e))?;
    let json_path = output_dir.join("agal.json");
    let graph = crate::delta::load_previous(&json_path).map(|mut g| {
        g.delta = None;
        g
    });
    write_agal_md(&output_dir, graph.as_ref(), output_dir_name)?;
    // Keep agent map skills count in sync (was only updated on full `agal .`).
    if let Some(g) = graph.as_ref() {
        let skills_dir = output_dir.join("skills");
        let skills = if skills_dir.is_dir() {
            Some(skills_dir.as_path())
        } else {
            None
        };
        let agent_md = render_agent_md(g, skills);
        let agent_path = output_dir.join("agal.agent.md");
        fs::write(&agent_path, agent_md)
            .map_err(|e| format!("cannot write {}: {}", agent_path.display(), e))?;
        println!(
            "  agal: {}/AGAL.md + agal.agent.md (skills index refreshed)",
            output_dir_name
        );
    } else {
        println!("  agal: {}/AGAL.md (skills index refreshed)", output_dir_name);
    }
    Ok(())
}

fn chrono_stub() -> String {
    // Avoid pulling chrono into core — generate() always has a real stamp.
    "skills-sync".into()
}

fn write_migration_section(s: &mut String, ms: &crate::MigrationSummary) {
    // Non-plugin workspaces (tools, CLI/UI apps) have no migration theatre.
    if ms.migrations.is_empty() || ms.total_plugins == 0 {
        return;
    }
    if ms.total_legacy > 0 {
        let _ = writeln!(
            s,
            "## migration (open)\nplugins: {} · **legacy: {}** · migrated: {}\n",
            ms.total_plugins, ms.total_legacy, ms.total_migrated
        );
        for (id, d) in &ms.migrations {
            if d.legacy_count > 0 {
                let _ = writeln!(
                    s,
                    "- **{}** `{}`→`{}`: {} legacy ({})",
                    id,
                    d.from,
                    d.to,
                    d.legacy_count,
                    d.legacy_plugins.join(", ")
                );
            }
        }
        let _ = writeln!(s);
        return;
    }
    // Done — one quiet line, no per-plugin migration theatre.
    if ms.total_migrated > 0 {
        let _ = writeln!(
            s,
            "## migration\n_complete_ — {} plugins on target adapters (no open legacy).\n",
            ms.total_migrated
        );
    }
}

/// L3 strip: top error/warn findings when health is degraded or blocked.
fn write_focus_findings_strip(s: &mut String, graph: &Audiolabs) {
    let h = health(&graph.findings);
    if h == Health::Ok {
        return;
    }
    let mut items: Vec<&Finding> = actionable(&graph.findings).collect();
    if items.is_empty() {
        return;
    }
    items.sort_by_key(|f| match f.severity {
        Severity::Error => 0u8,
        Severity::Warn => 1,
        Severity::Info => 2,
    });
    let total = items.len();
    let shown = items.iter().take(MAX_FOCUS_FINDINGS);

    let _ = writeln!(
        s,
        "## Focus (health = **{h}**)\n\n\
         Fix these before feature work. Full list: `agal.agent.md`.\n"
    );
    for f in shown {
        let mut line = format!("- [{}] **{}**: {}", f.severity, f.code, truncate_line(&f.message, 100));
        if let Some(node) = &f.node {
            line.push_str(&format!(" · `{}`", short_node(node)));
        }
        if let Some(path) = &f.path {
            line.push_str(&format!(" · `{}`", path));
        }
        if let Some(fix) = &f.fix {
            line.push_str(&format!(" · fix: {}", truncate_line(fix, 80)));
        }
        let _ = writeln!(s, "{line}");
    }
    if total > MAX_FOCUS_FINDINGS {
        let _ = writeln!(
            s,
            "\n_… {} more error/warn in `agal.agent.md`._\n",
            total - MAX_FOCUS_FINDINGS
        );
    } else {
        let _ = writeln!(s);
    }
}

/// What skill packs are actually present under `skills/` (on disk).
fn write_equipped_skills(s: &mut String, skills_dir: Option<&Path>) {
    let _ = writeln!(s, "## Equipped (on disk)\n");
    let Some(dir) = skills_dir.filter(|d| d.is_dir()) else {
        let _ = writeln!(
            s,
            "_no `skills/` yet_ — run `agal skills sync` (default: **core**).\n"
        );
        return;
    };
    let index = index_skill_files(dir);
    if index.is_empty() {
        let _ = writeln!(
            s,
            "_`skills/` empty_ — run `agal skills sync` (default: **core**).\n"
        );
        return;
    }

    // Group dirs: 00-core, 01-policy, … → short names for humans.
    let mut groups: BTreeSet<String> = BTreeSet::new();
    for e in &index {
        if let Some(g) = e.rel_path.strip_prefix("skills/") {
            let group = g.split('/').next().unwrap_or(g);
            groups.insert(skill_group_short(group).to_string());
        }
    }
    let group_list = groups.into_iter().collect::<Vec<_>>().join(", ");
    let _ = writeln!(
        s,
        "**{n}** skill file(s) · groups: **{groups}**  \n\
         Catalog + full index below. Sync more: `agal skills sync --only …`.\n",
        n = index.len(),
        groups = group_list,
    );
}

fn skill_group_short(dir: &str) -> &str {
    match dir {
        "00-core" => "core",
        "01-policy" => "policy",
        "02-frameworks" => "frameworks",
        "03-formats" => "formats",
        "04-ui" => "ui",
        "05-migrations" => "migrations",
        "06-agents" => "agents",
        "07-aura" => "aura",
        "07-codewig" => "codewig",
        other => other,
    }
}

fn truncate_line(s: &str, max: usize) -> String {
    let mut out: String = s.chars().take(max).collect();
    if s.chars().count() > max {
        out.push('…');
    }
    out
}

fn short_node(id: &str) -> String {
    id.trim_start_matches("plugins/")
        .trim_start_matches("crates/")
        .to_string()
}

fn write_skills_section_compact(s: &mut String, skills_dir: Option<&Path>) {
    let _ = writeln!(s, "## skills\n");
    let _ = writeln!(
        s,
        "Full pack list + index: **`AGAL.md`**. Load on demand only.\n"
    );
    let n = skills_dir
        .filter(|d| d.is_dir())
        .map(index_skill_files)
        .map(|v| v.len())
        .unwrap_or(0);
    if n == 0 {
        let _ = writeln!(
            s,
            "_no skills synced yet — `agal skills sync` (default core)._\n"
        );
    } else {
        let _ = writeln!(s, "_{n} skill file(s) under `skills/` — see AGAL.md._\n");
    }
}

fn write_skills_packs_and_index(s: &mut String, skills_dir: Option<&Path>) {
    let _ = writeln!(
        s,
        "## Skills (tool packs)\n\n\
         Canonical packs live in the **agal tool**. Sync a curated copy into `skills/`:\n\n\
         | Group | Path | Sync |\n\
         |-------|------|------|\n\
         | **core** (default) | `skills/00-core/` | `agal skills sync` |\n\
         | policy | `skills/01-policy/` | `--only policy` |\n\
         | frameworks | `skills/02-frameworks/` | `--only frameworks` |\n\
         | formats | `skills/03-formats/` | `--only formats` |\n\
         | ui | `skills/04-ui/` | `--only ui` or `ui/slint` |\n\
         | migrations | `skills/05-migrations/` | `--only migrations` |\n\
         | agents | `skills/06-agents/` | `--only agents` |\n\n\
         - **Load on demand** — never dump `skills/` into context.\n\
         - Without `--force`, existing files are **skipped** (local edits kept).\n\
         - Prefer adapting pack files in place; do **not** invent root `*_SKILL.md`.\n\
         - Optional frontmatter: `id`, `summary` (≤120 chars), `triggers`, `verify`, `adapted: true`.\n"
    );

    let Some(dir) = skills_dir.filter(|d| d.is_dir()) else {
        let _ = writeln!(
            s,
            "### present\n\n_no `skills/` yet — run `agal skills sync`._\n"
        );
        return;
    };
    let index = index_skill_files(dir);
    if index.is_empty() {
        let _ = writeln!(
            s,
            "### present\n\n_`skills/` empty — run `agal skills sync` (core)._\n"
        );
        return;
    }
    let _ = writeln!(s, "### present (id · summary · path)\n");
    for entry in index {
        let _ = writeln!(
            s,
            "- **{}** — {} · [`{}`](./{})",
            entry.id, entry.summary, entry.rel_path, entry.rel_path
        );
        let mut meta = Vec::new();
        if let Some(t) = &entry.triggers {
            meta.push(format!("triggers: {t}"));
        }
        if let Some(v) = &entry.verify {
            meta.push(format!("verify: {v}"));
        }
        if !meta.is_empty() {
            let _ = writeln!(s, "  · {}", meta.join(" · "));
        }
    }
    let _ = writeln!(s);
}

#[derive(Debug)]
struct SkillIndexEntry {
    id: String,
    summary: String,
    rel_path: String,
    triggers: Option<String>,
    verify: Option<String>,
}

fn index_skill_files(skills_dir: &Path) -> Vec<SkillIndexEntry> {
    let mut out = Vec::new();
    let walker = walkdir::WalkDir::new(skills_dir)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.')
        });
    for entry in walker.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let Some(rel) = path.strip_prefix(skills_dir).ok() else {
            continue;
        };
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        // Skip folder readmes / authoring meta noise if any
        if rel_str.eq_ignore_ascii_case("README.md") {
            continue;
        }
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let fm = parse_skill_frontmatter(&text, &rel_str);
        out.push(SkillIndexEntry {
            id: fm.id,
            summary: fm.summary,
            rel_path: format!("skills/{}", rel_str),
            triggers: fm.triggers,
            verify: fm.verify,
        });
    }
    out.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    out
}

/// Parsed skill frontmatter (YAML-ish between `---` fences).
#[derive(Debug, Clone)]
struct SkillFrontmatter {
    id: String,
    summary: String,
    triggers: Option<String>,
    verify: Option<String>,
}

/// Parse YAML-ish frontmatter; fallback id/summary from path/title.
fn parse_skill_frontmatter(text: &str, rel_path: &str) -> SkillFrontmatter {
    let fallback_id = Path::new(rel_path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| rel_path.to_string());
    let mut id = fallback_id.clone();
    let mut summary = String::new();
    let mut triggers: Option<String> = None;
    let mut verify: Option<String> = None;

    let trimmed = text.trim_start();
    if let Some(rest) = trimmed.strip_prefix("---") {
        if let Some(end) = rest.find("\n---") {
            let fm = &rest[..end];
            for line in fm.lines() {
                let line = line.trim();
                if let Some(v) = line.strip_prefix("id:") {
                    let v = v.trim().trim_matches('"').trim_matches('\'');
                    if !v.is_empty() {
                        id = v.to_string();
                    }
                } else if let Some(v) = line.strip_prefix("summary:") {
                    let v = v.trim().trim_matches('"').trim_matches('\'');
                    if !v.is_empty() {
                        summary = v.to_string();
                    }
                } else if let Some(v) = line.strip_prefix("triggers:") {
                    if let Some(t) = normalize_fm_list(v) {
                        triggers = Some(t);
                    }
                } else if let Some(v) = line.strip_prefix("verify:") {
                    if let Some(t) = normalize_fm_list(v) {
                        verify = Some(t);
                    }
                }
            }
        }
    }
    if summary.is_empty() {
        // First markdown heading or first non-empty line after frontmatter
        for line in text.lines() {
            let t = line.trim();
            if t.starts_with('#') {
                summary = t.trim_start_matches('#').trim().to_string();
                break;
            }
        }
        if summary.is_empty() {
            summary = "(no summary)".into();
        }
    }
    // Keep index token-cheap
    if summary.len() > 120 {
        summary.truncate(117);
        summary.push_str("...");
    }
    SkillFrontmatter {
        id,
        summary,
        triggers,
        verify,
    }
}

/// Normalize `a, b` or `["a", "b"]` / `['a']` into a short comma-separated string.
fn normalize_fm_list(raw: &str) -> Option<String> {
    let mut v = raw.trim().trim_matches('"').trim_matches('\'').to_string();
    if v.starts_with('[') && v.ends_with(']') {
        v = v[1..v.len() - 1].to_string();
    }
    let parts: Vec<String> = v
        .split(',')
        .map(|p| {
            p.trim()
                .trim_matches('"')
                .trim_matches('\'')
                .trim()
                .to_string()
        })
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() {
        return None;
    }
    let mut joined = parts.join(", ");
    if joined.len() > 160 {
        joined.truncate(157);
        joined.push_str("...");
    }
    Some(joined)
}

fn plugin_line(n: &Node) -> String {
    // Only surface migration status when open work remains (legacy / unknown).
    let mig_badge = match n.migration_status.as_deref() {
        Some("legacy") => " [legacy]",
        Some("unknown") => " [mig=?]",
        _ => "",
    };
    let fw = n.frameworks.join("+");
    let mut extras = Vec::new();
    if let Some(ast) = &n.ast_summary {
        if let Some(logic) = ast.plugin_logic_impls.iter().next() {
            extras.push(format!("logic={}", logic));
        }
        if let Some(params) = ast.params_structs.iter().next() {
            let nfields = ast.params_fields.get(params).map(|f| f.len()).unwrap_or(0);
            extras.push(format!("params={}({})", params, nfields));
        }
        if !ast.ipc_signals.is_empty() {
            extras.push(format!(
                "ipc={}",
                ast.ipc_signals
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        if !ast.slint_components.is_empty() {
            extras.push(format!("slint_comp={}", ast.slint_components.len()));
        }
        let roles = ast.role_tags();
        if !roles.is_empty() {
            extras.push(format!(
                "roles={}",
                roles.into_iter().collect::<Vec<_>>().join(",")
            ));
        }
    }
    let deps: Vec<String> = n.internal_deps.iter().map(|d| short_id(d)).collect();
    format!(
        "- **{}** `{}`{} fw=`{}` deps=[{}] {}",
        n.name,
        n.id,
        mig_badge,
        fw,
        deps.join(", "),
        extras.join(" ")
    )
}

fn crate_line(n: &Node) -> String {
    let mut extras = Vec::new();
    if let Some(ast) = &n.ast_summary {
        if ast.api_surface_total > 0 {
            extras.push(format!("api={}/{}", ast.api_surface.len(), ast.api_surface_total));
        }
        if !ast.slint_exports.is_empty() {
            extras.push(format!("slint_export={}", ast.slint_exports.len()));
        }
        if !ast.ipc_signals.is_empty() {
            extras.push(format!(
                "ipc={}",
                ast.ipc_signals
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        if ast.process_method_count > 0 {
            extras.push(format!("process_methods={}", ast.process_method_count));
        }
    }
    let deps: Vec<String> = n.internal_deps.iter().map(|d| short_id(d)).collect();
    format!(
        "- **{}** `{}` deps=[{}] {}",
        n.name,
        n.id,
        deps.join(", "),
        extras.join(" ")
    )
}

fn finding_line(f: &Finding) -> String {
    let icon = match f.severity {
        Severity::Error => "ERR",
        Severity::Warn => "WRN",
        Severity::Info => "INF",
    };
    let node = f
        .node
        .as_ref()
        .map(|n| format!(" `{}`", short_id(n)))
        .unwrap_or_default();
    let mut line = format!("- [{}] **{}**{}: {}", icon, f.code, node, f.message);
    if let Some(path) = &f.path {
        line.push_str(&format!(" · path=`{}`", path));
    }
    if let Some(fix) = &f.fix {
        line.push_str(&format!(" · fix: {}", fix));
    }
    line
}

fn short_id(id: &str) -> String {
    id.trim_start_matches("plugins/")
        .trim_start_matches("crates/")
        .to_string()
}

fn edge_kind_order() -> &'static [&'static str] {
    &[
        "depends_on",
        "build_depends_on",
        "dev_depends_on",
        "uses_ui",
        "ipc_peer",
        "runtime_depends_on",
    ]
}

/// JSON slice: one node + 1-hop edges + related findings.
pub fn plugin_slice(graph: &Audiolabs, plugin_name: &str) -> Option<serde_json::Value> {
    let node = graph.nodes.iter().find(|n| {
        n.name == plugin_name || n.id == plugin_name || n.id.ends_with(&format!("/{}", plugin_name))
    })?;
    let id = &node.id;
    let related_edges: Vec<&Edge> = graph
        .edges
        .iter()
        .filter(|e| e.from == *id || e.to == *id)
        .collect();
    let neighbor_ids: BTreeSet<&str> = related_edges
        .iter()
        .flat_map(|e| [e.from.as_str(), e.to.as_str()])
        .collect();
    let neighbors: Vec<&Node> = graph
        .nodes
        .iter()
        .filter(|n| neighbor_ids.contains(n.id.as_str()) && n.id != *id)
        .collect();
    let findings: Vec<&Finding> = graph
        .findings
        .iter()
        .filter(|f| f.node.as_deref() == Some(id.as_str()))
        .collect();

    Some(serde_json::json!({
        "plugin": node,
        "neighbors": neighbors,
        "edges": related_edges,
        "findings": findings,
    }))
}

#[cfg(test)]
mod tests {
    use super::{parse_skill_frontmatter, render_agal_md};
    use crate::findings::{Finding, Severity};
    use crate::Audiolabs;

    #[test]
    fn frontmatter_id_and_summary() {
        let text = "---\nid: slint-lx\ngroup: ui\nsource: workspace\nsummary: hello world skill\n---\n\n# Title\n";
        let fm = parse_skill_frontmatter(text, "04-ui/slint.md");
        assert_eq!(fm.id, "slint-lx");
        assert_eq!(fm.summary, "hello world skill");
        assert!(fm.triggers.is_none());
        assert!(fm.verify.is_none());
    }

    #[test]
    fn frontmatter_triggers_and_verify() {
        let text = "---\n\
id: dsp-realtime\n\
summary: RT rules\n\
triggers: [process, realtime, \"audio callback\"]\n\
verify: review process() for alloc/lock\n\
---\n\n# DSP\n";
        let fm = parse_skill_frontmatter(text, "00-core/dsp-realtime.md");
        assert_eq!(fm.id, "dsp-realtime");
        assert_eq!(
            fm.triggers.as_deref(),
            Some("process, realtime, audio callback")
        );
        assert_eq!(
            fm.verify.as_deref(),
            Some("review process() for alloc/lock")
        );
    }

    #[test]
    fn frontmatter_fallback_from_path() {
        let fm = parse_skill_frontmatter("# Only heading\n", "04-ui/slint.md");
        assert_eq!(fm.id, "slint");
        assert_eq!(fm.summary, "Only heading");
    }

    #[test]
    fn agal_md_lists_skills_without_legacy_paths() {
        let md = render_agal_md(None, None, "agal");
        assert!(md.contains("# AGAL"));
        assert!(md.contains("00-core"));
        assert!(md.contains("04-ui"));
        assert!(!md.contains("10-lx"));
        assert!(!md.contains("90-project"));
        assert!(md.contains("Context budget"));
        assert!(md.contains("Task loadouts"));
        assert!(md.contains("Stack layers"));
        assert!(md.contains("[ATOM]"));
        assert!(md.contains("Disclosure"));
        assert!(md.contains("**L3**"));
        assert!(md.contains("**L0**"));
        assert!(md.contains("Equipped (on disk)"));
        assert!(md.contains("no `skills/` yet") || md.contains("Equipped"));
        assert!(md.contains("clap-validator validate"));
        assert!(md.contains("notes/_workspace.md"));
    }

    #[test]
    fn agal_md_focus_strip_when_blocked() {
        let g = Audiolabs {
            version: "0.0.0".into(),
            generated_at: "t".into(),
            project_root: ".".into(),
            project_name: "t".into(),
            used_frameworks: vec![],
            frameworks: vec![],
            nodes: vec![],
            edges: vec![],
            findings: vec![
                Finding::new(Severity::Error, "migration_legacy", "still on old editor")
                    .with_path("plugins/x")
                    .with_fix("migrate to new editor"),
                Finding::new(Severity::Warn, "large_param_surface", "many params"),
                Finding::new(Severity::Info, "tool_hint_clippy", "run clippy"),
            ],
            findings_suppressed: 0,
            migration_summary: crate::MigrationSummary {
                total_plugins: 0,
                total_legacy: 0,
                total_migrated: 0,
                migrations: Default::default(),
            },
            rules: Default::default(),
            delta: None,
        };
        let md = render_agal_md(Some(&g), None, "agal");
        assert!(md.contains("## Focus (health = **blocked**)"));
        assert!(md.contains("migration_legacy"));
        assert!(md.contains("large_param_surface"));
        assert!(!md.contains("tool_hint_clippy") || md.find("## Focus").is_some());
        // info must not appear in focus strip lines
        let focus = md
            .split("## Focus")
            .nth(1)
            .and_then(|r| r.split("## ").next())
            .unwrap_or("");
        assert!(!focus.contains("tool_hint_clippy"));
    }

    #[test]
    fn agal_md_equipped_lists_groups() {
        let dir = std::env::temp_dir().join(format!(
            "agal_skills_test_{}",
            std::process::id()
        ));
        let core = dir.join("00-core");
        std::fs::create_dir_all(&core).unwrap();
        std::fs::write(
            core.join("dsp-realtime.md"),
            "---\nid: dsp-realtime\nsummary: rt\n---\n# RT\n",
        )
        .unwrap();
        let md = render_agal_md(None, Some(dir.as_path()), "agal");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(md.contains("Equipped (on disk)"));
        assert!(md.contains("skill file(s)"));
        assert!(
            md.contains("groups: **core**") || md.contains("**core**"),
            "expected core group in equipped: {md}"
        );
    }
}
