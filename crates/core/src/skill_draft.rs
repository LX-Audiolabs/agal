use std::path::Path;

use crate::{Node, ast::ProcessIo, atoms};

/// Draft a workspace skill file for `node` at `<output_dir>/skills/<id>.md`.
/// Returns the written path on success.
pub fn draft(
    workspace: &Path,
    output_dir: &str,
    node_name: &str,
    force: bool,
) -> Result<String, String> {
    let graph = crate::scan(workspace, false)?;
    let node = graph
        .nodes
        .iter()
        .find(|n| {
            n.id == node_name
                || n.name == node_name
                || n.id.ends_with(node_name)
                || n.name.eq_ignore_ascii_case(node_name)
        })
        .ok_or_else(|| {
            let names: Vec<&str> = graph.nodes.iter().map(|n| n.id.as_str()).collect();
            format!("node '{}' not found. nodes: {}", node_name, names.join(", "))
        })?;

    // Strip Windows UNC prefix (\\?\) so fs I/O works on extended-length paths.
    let ws_str = workspace.to_string_lossy();
    let ws_stripped = ws_str.trim_start_matches("\\\\?\\");
    let ws = std::path::Path::new(ws_stripped);
    let skills_dir = ws.join(output_dir).join("skills");
    std::fs::create_dir_all(&skills_dir)
        .map_err(|e| format!("cannot create skills dir '{}': {e}", skills_dir.display()))?;

    let out_path = skills_dir.join(format!("{}.md", node.name));
    if out_path.exists() && !force {
        return Err(format!(
            "skill file already exists: {}. Use --force to overwrite.",
            out_path.display()
        ));
    }

    let note_path = ws.join(output_dir).join("notes").join(format!("{}.md", node.name));
    let atoms = atoms::load_atoms_from_file(&note_path, &node.name, &[]);

    let content = render(node, &atoms);
    std::fs::write(&out_path, content)
        .map_err(|e| format!("cannot write '{}': {e}", out_path.display()))?;

    Ok(format!(
        "drafted: {}\n\nFill in the TODO sections, then commit.",
        out_path.display()
    ))
}

/// Return the draft markdown (no I/O) — for MCP preview.
pub fn render_draft(workspace: &Path, node_name: &str) -> Result<String, String> {
    let graph = crate::scan(workspace, false)?;
    let node = graph
        .nodes
        .iter()
        .find(|n| {
            n.id == node_name
                || n.name == node_name
                || n.id.ends_with(node_name)
                || n.name.eq_ignore_ascii_case(node_name)
        })
        .ok_or_else(|| {
            let names: Vec<&str> = graph.nodes.iter().map(|n| n.id.as_str()).collect();
            format!("node '{}' not found. nodes: {}", node_name, names.join(", "))
        })?;

    // Try to load existing atoms even without output_dir context — best effort.
    let atoms: Vec<atoms::AtomEntry> = Vec::new();
    Ok(render(node, &atoms))
}

fn render(node: &Node, atoms: &[atoms::AtomEntry]) -> String {
    let ast = node.ast_summary.as_ref();
    let date = crate::now_rfc3339();
    let date = &date[..10]; // YYYY-MM-DD

    // --- Frontmatter ---
    let plugin_types: Vec<String> = ast
        .map(|a| a.plugin_logic_impls.iter().chain(a.plugin_impls.iter()).cloned().collect())
        .unwrap_or_default();
    let params_types: Vec<String> = ast
        .map(|a| a.params_structs.iter().cloned().collect())
        .unwrap_or_default();
    let formats: Vec<String> = ast
        .map(|a| a.plugin_formats.iter().map(|f| f.to_uppercase()).collect())
        .unwrap_or_default();

    let mut triggers: Vec<String> = vec![node.name.clone()];
    triggers.extend(plugin_types.iter().cloned());
    triggers.extend(params_types.iter().cloned());
    let triggers_str = triggers.join(", ");

    let io = ast.map(|a| &a.process_io).cloned().unwrap_or_default();
    let io_summary = io_one_liner(&io);
    let mut summary_parts: Vec<String> = Vec::new();
    if !formats.is_empty() { summary_parts.push(formats.join("/")); }
    if !io_summary.is_empty() { summary_parts.push(io_summary); }
    let summary = if summary_parts.is_empty() {
        node.name.clone()
    } else {
        format!("{} — {}", node.name, summary_parts.join(", "))
    };

    let mut out = String::new();
    out.push_str("---\n");
    out.push_str("source: auto-draft\n");
    out.push_str("copied_by: agal skill draft\n");
    out.push_str(&format!("date: {date}\n"));
    out.push_str("adapted: false\n");
    out.push_str(&format!("id: {}\n", node.name));
    out.push_str("group: plugins\n");
    out.push_str(&format!("summary: {summary}\n"));
    out.push_str(&format!("triggers: {triggers_str}\n"));
    out.push_str("verify: <!-- TODO: add verification checklist -->\n");
    out.push_str("---\n\n");

    // --- Header ---
    out.push_str(&format!("# {}\n\n", node.name));
    out.push_str("**Summary:** <!-- TODO: describe what this plugin/crate does -->\n\n");
    out.push_str("> Auto-drafted by `agal skill draft`. Fill in the TODO sections.\n\n");

    // --- Plugin structure ---
    if !plugin_types.is_empty() {
        out.push_str("## Plugin Structure\n\n");
        for impl_type in &plugin_types {
            let params = params_types.first().map(|s| s.as_str()).unwrap_or("Params");
            out.push_str("```rust\n");
            out.push_str(&format!("impl PluginLogic for {impl_type} {{\n"));
            out.push_str(&format!("    type Params = {params};\n"));
            out.push_str("    type DspState = DspState; // alloc in init/reset, never in process\n");
            out.push_str("    // ...\n");
            out.push_str("}\n");
            out.push_str("```\n\n");
        }
    }

    // --- Parameters ---
    let params_fields = ast
        .map(|a| &a.params_fields)
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if !params_fields.is_empty() {
        out.push_str("## Parameters\n\n");
        out.push_str("| Field | Type | Display |\n|---|---|---|\n");
        for (struct_name, fields) in params_fields {
            let _ = struct_name;
            for f in fields {
                let display = f.display_name.as_deref().unwrap_or("—");
                out.push_str(&format!("| `{}` | {} | {} |\n", f.name, f.ty, display));
            }
        }
        out.push('\n');
    }

    // --- Process I/O ---
    if !io.is_empty() {
        out.push_str("## Process I/O\n\n");
        out.push_str(crate::process_map::render(&node.name, &io).trim_start_matches(&format!("## Process I/O: {}\n\n", node.name)));
        out.push('\n');
    }

    // --- Formats ---
    if !formats.is_empty() {
        out.push_str("## Formats\n\n");
        out.push_str(&formats.join(", "));
        out.push_str("\n\n");
    }

    // --- Atoms ---
    let notable: Vec<_> = atoms
        .iter()
        .filter(|a| matches!(a.atom_type.as_str(), "lesson" | "failure" | "constraint"))
        .collect();
    if !notable.is_empty() {
        out.push_str("## Known Lessons / Failures\n\n");
        for atom in notable {
            out.push_str(&format!(
                "- **[{}]** {}\n",
                atom.atom_type.to_uppercase(),
                atom.detail
            ));
        }
        out.push('\n');
    }

    // --- TODOs ---
    out.push_str("## TODO\n\n");
    out.push_str("- [ ] Describe the algorithm / DSP approach\n");
    out.push_str("- [ ] Add host compatibility notes\n");
    out.push_str("- [ ] Add common failure modes\n");
    if formats.iter().any(|f| f == "CLAP") {
        out.push_str("- [ ] CLAP-specific quirks (notes dialect, voice stack behavior)\n");
    }
    if io.sidechain_in {
        out.push_str("- [ ] Document sidechain signal path\n");
    }
    if io.transport {
        out.push_str("- [ ] Document transport-sync behavior\n");
    }

    out
}

fn io_one_liner(io: &ProcessIo) -> String {
    let mut parts = Vec::new();
    if io.midi_in || io.midi_out { parts.push("MIDI"); }
    if io.notes_in || io.notes_out { parts.push("CLAP notes"); }
    if io.sidechain_in { parts.push("sidechain"); }
    if io.transport { parts.push("transport"); }
    parts.join(" + ")
}
