//! Hybrid per-node notes: auto header regenerated; human body preserved.
//!
//! AUTO block includes **graph atoms** — dense `[ATOM]` lines derived from the
//! scan (findings, edges, migration). Agents scan those first (L1 disclosure).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::findings::{Finding, Severity};
use crate::{Audiolabs, Edge, Node};

/// Max auto graph atoms per note (token budget for L1).
const MAX_GRAPH_ATOMS: usize = 12;

const AUTO_START: &str = "<!-- AGAL:AUTO-START -->";
const AUTO_END: &str = "<!-- AGAL:AUTO-END -->";
const HUMAN_MARK: &str = "<!-- AGAL:HUMAN — edit below this line; preserved on regenerate -->";

/// Write `notes/<name>.md` for each plugin, crate, and member node.
/// Members (`examples/`, `tools/`) matter in framework workspaces (smoke, CLI).
pub fn write_notes(output_dir: &Path, graph: &Audiolabs) -> Result<usize, String> {
    let notes_dir = output_dir.join("notes");
    fs::create_dir_all(&notes_dir)
        .map_err(|e| format!("cannot create {}: {}", notes_dir.display(), e))?;

    let findings_by_node = index_findings(&graph.findings);
    let mut written = 0usize;

    for n in &graph.nodes {
        if n.kind != "plugin" && n.kind != "crate" && n.kind != "member" {
            continue;
        }
        let path = notes_dir.join(format!("{}.md", sanitize_filename(&n.name)));
        let human = load_human_body(&path);
        let auto = render_auto_section(n, graph, &findings_by_node);
        let body = compose(&auto, human.as_deref());
        fs::write(&path, body).map_err(|e| format!("cannot write {}: {}", path.display(), e))?;
        written += 1;
    }

    write_index(&notes_dir, graph)?;
    ensure_workspace_note(&notes_dir)?;
    Ok(written)
}

/// Durable workspace-level memory. Created once; **never** overwritten by generate.
fn ensure_workspace_note(notes_dir: &Path) -> Result<(), String> {
    let path = notes_dir.join("_workspace.md");
    if path.exists() {
        return Ok(());
    }
    let body = r#"# Workspace memory

**Summary:** Durable cross-plugin / cross-crate notes for agents.  
**Never overwritten** by `agal .` (unlike per-node AUTO blocks).  
Keep this file short (~80 lines). Prefer `[ATOM]` one-liners.

## Atoms

```text
[ATOM] type=decision|lesson|constraint | detail=…
```

## Open

- [ ]

## Decisions

_Workspace-wide architecture choices worth remembering._
"#;
    fs::write(&path, body).map_err(|e| format!("cannot write {}: {}", path.display(), e))?;
    Ok(())
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn index_findings(findings: &[Finding]) -> BTreeMap<String, Vec<&Finding>> {
    let mut m: BTreeMap<String, Vec<&Finding>> = BTreeMap::new();
    for f in findings {
        if let Some(node) = &f.node {
            m.entry(node.clone()).or_default().push(f);
        }
    }
    m
}

fn load_human_body(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    if let Some(idx) = content.find(HUMAN_MARK) {
        let after = &content[idx + HUMAN_MARK.len()..];
        return Some(after.trim_start_matches(['\r', '\n']).to_string());
    }
    // Legacy / first human draft without marker: keep everything after AUTO_END if present.
    if let Some(idx) = content.find(AUTO_END) {
        let after = &content[idx + AUTO_END.len()..];
        let trimmed = after
            .trim_start_matches(['\r', '\n'])
            .trim_start_matches(HUMAN_MARK)
            .trim_start_matches(['\r', '\n']);
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}

fn compose(auto: &str, human: Option<&str>) -> String {
    let mut s = String::with_capacity(auto.len() + 512);
    s.push_str(AUTO_START);
    s.push('\n');
    s.push_str(auto);
    if !auto.ends_with('\n') {
        s.push('\n');
    }
    s.push_str(AUTO_END);
    s.push_str("\n\n");
    s.push_str(HUMAN_MARK);
    s.push_str("\n\n");
    match human {
        Some(h) if !h.is_empty() => {
            s.push_str(h);
            if !h.ends_with('\n') {
                s.push('\n');
            }
        }
        _ => {
            s.push_str("## Intent\n\n_Why this crate/plugin exists. Edit freely._\n\n");
            s.push_str("## Open\n\n- [ ] \n\n");
            s.push_str("## Decisions\n\n_Architecture choices worth remembering._\n\n");
            s.push_str("## Atoms (human)\n\n");
            s.push_str(
                "_Graph atoms live **above** in AUTO. Add durable decisions/lessons here:_\n\n",
            );
            s.push_str("```text\n");
            s.push_str("[ATOM] type=decision|lesson|constraint | detail=…\n");
            s.push_str("```\n");
        }
    }
    s
}

fn render_auto_section(
    n: &Node,
    graph: &Audiolabs,
    findings_by_node: &BTreeMap<String, Vec<&Finding>>,
) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# {}\n", n.name);
    let _ = writeln!(
        s,
        "> Auto-generated from workspace scan. Do not edit between AUTO markers.\n"
    );
    let _ = writeln!(s, "| | |");
    let _ = writeln!(s, "|---|---|");
    let _ = writeln!(s, "| kind | `{}` |", n.kind);
    let _ = writeln!(s, "| path | `{}` |", n.id);
    if let Some(v) = &n.version {
        let _ = writeln!(s, "| version | {} |", v);
    }
    if let Some(d) = &n.description {
        let _ = writeln!(s, "| description | {} |", d);
    }
    if !n.frameworks.is_empty() {
        let _ = writeln!(s, "| frameworks | {} |", n.frameworks.join(", "));
    }
    if let Some(m) = &n.migration_status {
        let _ = writeln!(s, "| migration | **{}** |", m);
    }
    let _ = writeln!(s, "| generated | `{}` |", graph.generated_at);
    let _ = writeln!(s);

    // L1 first: dense atoms agents scan before prose sections below.
    s.push_str(&render_graph_atoms(n, graph, findings_by_node));

    if !n.internal_deps.is_empty() {
        let _ = writeln!(s, "## deps (workspace)");
        for d in &n.internal_deps {
            let _ = writeln!(s, "- `{}`", short_id(d));
        }
        let _ = writeln!(s);
    }

    // Dependents: who points at this node (blast radius)
    let dependents: Vec<&Edge> = graph.edges.iter().filter(|e| e.to == n.id).collect();
    if !dependents.is_empty() {
        let _ = writeln!(s, "## dependents (inbound)");
        for e in dependents {
            let _ = writeln!(
                s,
                "- `{}` --{}--> `{}`",
                short_id(&e.from),
                e.kind,
                short_id(&e.to)
            );
        }
        let _ = writeln!(s);
    }

    // Outbound semantic edges
    let outbound: Vec<&Edge> = graph
        .edges
        .iter()
        .filter(|e| e.from == n.id && e.kind != "depends_on" && e.kind != "build_depends_on")
        .collect();
    if !outbound.is_empty() {
        let _ = writeln!(s, "## semantic edges");
        for e in outbound {
            let note = e
                .note
                .as_ref()
                .map(|n| format!(" — {}", n))
                .unwrap_or_default();
            let _ = writeln!(s, "- **{}** → `{}`{}", e.kind, short_id(&e.to), note);
        }
        let _ = writeln!(s);
    }

    if let Some(ast) = &n.ast_summary {
        let _ = writeln!(s, "## structure");
        if !ast.plugin_logic_impls.is_empty() {
            let _ = writeln!(
                s,
                "- logic: {}",
                ast.plugin_logic_impls
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        if !ast.params_structs.is_empty() {
            for ps in &ast.params_structs {
                let nfields = ast.params_fields.get(ps).map(|f| f.len()).unwrap_or(0);
                let _ = writeln!(s, "- params: {} ({} fields)", ps, nfields);
            }
        }
        if !ast.process_hooks.is_empty() {
            let _ = writeln!(
                s,
                "- process: {}",
                ast.process_hooks
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        if !ast.editor_functions.is_empty() {
            let _ = writeln!(s, "- editor: yes");
        }
        if !ast.plugin_formats.is_empty() {
            let _ = writeln!(
                s,
                "- formats: {}",
                ast.plugin_formats
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        if !ast.ipc_signals.is_empty() {
            let _ = writeln!(
                s,
                "- ipc: {}",
                ast.ipc_signals
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        if !ast.slint_components.is_empty() {
            let _ = writeln!(s, "- slint components: {}", ast.slint_components.len());
        }
        if !ast.slint_exports.is_empty() {
            let _ = writeln!(s, "- slint exports: {}", ast.slint_exports.len());
        }
        if ast.process_method_count > 0 && n.kind == "crate" {
            let _ = writeln!(s, "- process methods (DSP): {}", ast.process_method_count);
        }
        if !ast.public_api.is_empty() && n.kind == "crate" {
            let _ = writeln!(
                s,
                "- public_api symbols: {} (see json)",
                ast.public_api.len()
            );
        }
        let roles = ast.role_tags();
        if !roles.is_empty() {
            let _ = writeln!(
                s,
                "- roles: {}",
                roles.into_iter().collect::<Vec<_>>().join(", ")
            );
        }
        let _ = writeln!(s);
    }

    // L1½: ranked signatures for this crate; for plugins, show 1-hop dep surfaces.
    if n.kind == "crate" || n.kind == "member" {
        if let Some(ast) = &n.ast_summary {
            if !ast.api_surface.is_empty() {
                let _ = writeln!(s, "## api surface");
                for sym in &ast.api_surface {
                    let loc = if sym.line == 0 {
                        sym.file.clone()
                    } else {
                        format!("{}:{}", sym.file, sym.line)
                    };
                    let _ = writeln!(s, "- `{}` · `{}`", sym.signature, loc);
                }
                if ast.api_surface_total > ast.api_surface.len() {
                    let _ = writeln!(
                        s,
                        "- … +{} more public symbols",
                        ast.api_surface_total - ast.api_surface.len()
                    );
                }
                let _ = writeln!(s);
            }
        }
    } else if n.kind == "plugin" {
        let dep_surface = render_plugin_dep_surface(n, graph, 2);
        if !dep_surface.is_empty() {
            let _ = writeln!(s, "## api surface (workspace deps)");
            let _ = writeln!(s, "{}", dep_surface);
        }
    }

    if let Some(fs) = findings_by_node.get(&n.id) {
        let _ = writeln!(s, "## findings");
        for f in fs {
            let mut line = format!("- [{}] **{}**: {}", f.severity, f.code, f.message);
            if let Some(path) = &f.path {
                line.push_str(&format!(" · `{}`", path));
            }
            if let Some(fix) = &f.fix {
                line.push_str(&format!(" · fix: {}", fix));
            }
            let _ = writeln!(s, "{}", line);
        }
        let _ = writeln!(s);
    }

    let _ = writeln!(
        s,
        "## agent focus\n\
         **L1:** scan **Graph atoms** above first, then human body below HUMAN.  \n\
         After `agal.agent.md` (L2). Escalate L0: `{}` in json / `agal --plugin {} .`\n",
        n.id, n.name
    );

    s
}

/// Dense one-liners from the graph — regenerated; not human-edited.
fn render_graph_atoms(
    n: &Node,
    graph: &Audiolabs,
    findings_by_node: &BTreeMap<String, Vec<&Finding>>,
) -> String {
    let mut atoms: Vec<(&str, String)> = Vec::new();

    atoms.push((
        "fact",
        format!("kind={} id={}", n.kind, sanitize_atom_detail(&n.id)),
    ));

    if !n.frameworks.is_empty() {
        atoms.push(("fact", format!("frameworks={}", n.frameworks.join("+"))));
    }

    match n.migration_status.as_deref() {
        Some("legacy") => atoms.push((
            "constraint",
            "migration=legacy — migrate before feature work".into(),
        )),
        Some("unknown") => atoms.push(("constraint", "migration=unknown".into())),
        Some("migrated") => atoms.push(("fact", "migration=migrated".into())),
        Some(other) => atoms.push(("fact", format!("migration={other}"))),
        None => {}
    }

    if let Some(ast) = &n.ast_summary {
        if !ast.plugin_formats.is_empty() {
            let formats: Vec<_> = ast.plugin_formats.iter().cloned().collect();
            atoms.push(("fact", format!("formats={}", formats.join("+"))));
        }
        let roles = ast.role_tags();
        if !roles.is_empty() {
            let roles: Vec<_> = roles.into_iter().collect();
            atoms.push(("fact", format!("roles={}", roles.join("+"))));
        }
        if !ast.process_hooks.is_empty() || ast.process_method_count > 0 {
            atoms.push(("fact", "has_process=true".into()));
        }
        if !ast.editor_functions.is_empty() {
            atoms.push(("fact", "has_editor=true".into()));
        }
    }

    // Semantic outbound (UI / IPC / runtime) — highest signal edges.
    for e in graph.edges.iter().filter(|e| e.from == n.id) {
        if matches!(
            e.kind.as_str(),
            "uses_ui" | "ipc_peer" | "runtime_depends_on"
        ) {
            atoms.push(("fact", format!("{}→{}", e.kind, short_id(&e.to))));
        }
    }

    // Workspace deps (cap) — who this node needs.
    for d in n.internal_deps.iter().take(3) {
        atoms.push(("fact", format!("depends_on={}", short_id(d))));
    }

    // Inbound blast radius (cap).
    for e in graph.edges.iter().filter(|e| e.to == n.id).take(3) {
        atoms.push((
            "fact",
            format!("used_by={} via {}", short_id(&e.from), e.kind),
        ));
    }

    // Actionable findings only (error / warn). Info stays in full findings list.
    if let Some(fs) = findings_by_node.get(&n.id) {
        let mut actionable: Vec<&&Finding> = fs
            .iter()
            .filter(|f| matches!(f.severity, Severity::Error | Severity::Warn))
            .collect();
        actionable.sort_by_key(|f| match f.severity {
            Severity::Error => 0u8,
            Severity::Warn => 1,
            Severity::Info => 2,
        });
        for f in actionable {
            let mut detail = format!(
                "[{}] {}: {}",
                f.severity,
                f.code,
                truncate_chars(&f.message, 80)
            );
            if let Some(fix) = &f.fix {
                detail.push_str(" | fix: ");
                detail.push_str(&truncate_chars(fix, 60));
            }
            atoms.push(("constraint", sanitize_atom_detail(&detail)));
        }
    }

    atoms.truncate(MAX_GRAPH_ATOMS);
    if atoms.is_empty() {
        return String::new();
    }

    let mut s = String::new();
    let _ = writeln!(s, "## Graph atoms (auto)\n");
    let _ = writeln!(
        s,
        "_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._\n"
    );
    let _ = writeln!(s, "```text");
    for (ty, detail) in &atoms {
        let _ = writeln!(
            s,
            "[ATOM] type={} | detail={}",
            ty,
            sanitize_atom_detail(detail)
        );
    }
    let _ = writeln!(s, "```\n");
    s
}

fn truncate_chars(s: &str, max: usize) -> String {
    let mut out: String = s.chars().take(max).collect();
    if s.chars().count() > max {
        out.push('…');
    }
    out
}

fn sanitize_atom_detail(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\n' | '\r' | '|' => ' ',
            c => c,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn write_index(notes_dir: &Path, graph: &Audiolabs) -> Result<(), String> {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "# notes index\n\n\
         Auto-generated. Per-node notes for focus work.\n\n\
         **Workspace memory** (durable, never overwritten): [`_workspace.md`](./_workspace.md)\n\n\
         ## plugins\n"
    );
    for n in graph.nodes.iter().filter(|n| n.kind == "plugin") {
        let _ = writeln!(
            s,
            "- [{}]({}.md) — `{}`{}",
            n.name,
            sanitize_filename(&n.name),
            n.id,
            n.migration_status
                .as_ref()
                .map(|m| format!(" · {}", m))
                .unwrap_or_default()
        );
    }
    let _ = writeln!(s, "\n## crates\n");
    for n in graph.nodes.iter().filter(|n| n.kind == "crate") {
        let _ = writeln!(
            s,
            "- [{}]({}.md) — `{}`",
            n.name,
            sanitize_filename(&n.name),
            n.id
        );
    }
    let members: Vec<&Node> = graph.nodes.iter().filter(|n| n.kind == "member").collect();
    if !members.is_empty() {
        let _ = writeln!(s, "\n## members (examples / tools)\n");
        for n in members {
            let _ = writeln!(
                s,
                "- [{}]({}.md) — `{}`",
                n.name,
                sanitize_filename(&n.name),
                n.id
            );
        }
    }
    let path = notes_dir.join("_index.md");
    fs::write(&path, s).map_err(|e| format!("cannot write {}: {}", path.display(), e))?;
    Ok(())
}

fn short_id(id: &str) -> String {
    id.trim_start_matches("plugins/")
        .trim_start_matches("crates/")
        .to_string()
}

/// Top symbols from a plugin's workspace dependencies (focus + 1-hop API strip).
fn render_plugin_dep_surface(n: &Node, graph: &Audiolabs, per_dep: usize) -> String {
    let mut s = String::new();
    let dep_ids: Vec<&str> = n.internal_deps.iter().map(|d| d.as_str()).collect();
    if dep_ids.is_empty() {
        return s;
    }
    for dep_id in dep_ids.iter().take(4) {
        let Some(dep_node) = graph.nodes.iter().find(|x| x.id == *dep_id) else {
            continue;
        };
        let Some(ast) = &dep_node.ast_summary else {
            continue;
        };
        if ast.api_surface.is_empty() {
            continue;
        }
        let _ = writeln!(
            &mut s,
            "- from `{}` (`{}`):",
            dep_node.name,
            short_id(dep_id)
        );
        for sym in ast.api_surface.iter().take(per_dep) {
            let loc = if sym.line == 0 {
                sym.file.clone()
            } else {
                format!("{}:{}", sym.file, sym.line)
            };
            let _ = writeln!(&mut s, "  - `{}` · `{}`", sym.signature, loc);
        }
        if ast.api_surface_total > per_dep {
            let _ = writeln!(&mut s, "  - … +{} more", ast.api_surface_total - per_dep);
        }
    }
    s
}
