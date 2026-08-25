//! Aggregate [ATOM] entries from notes by type (failure, lesson, decision, …).

use std::fmt::Write as _;
use std::path::Path;
use walkdir::WalkDir;

struct Atom {
    atom_type: String,
    detail: String,
    source: String,
}

fn atom_type(line: &str) -> Option<String> {
    let rest = line.trim().strip_prefix("[ATOM]")?.trim();
    let type_part = rest.split('|').next()?.trim();
    let val = type_part.strip_prefix("type=")?.trim();
    Some(val.to_string())
}

fn atom_detail(line: &str) -> String {
    for part in line.split('|') {
        if let Some(val) = part.trim().strip_prefix("detail=") {
            return val.trim().to_string();
        }
    }
    String::new()
}

/// Scan `<workspace>/<output_dir>/notes/` for `[ATOM]` lines matching `types`.
///
/// Pass `types = &[]` to include all types except `fact`.
/// Returns a markdown report grouped by type.
pub fn aggregate(workspace: &Path, output_dir: &str, types: &[&str]) -> String {
    let notes_dir = workspace.join(output_dir).join("notes");

    if !notes_dir.exists() {
        return format!(
            "# agal findings\n\nno notes directory at `{}/notes/`\n",
            output_dir
        );
    }

    let mut hits: Vec<Atom> = Vec::new();

    for entry in WalkDir::new(&notes_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path().extension().and_then(|s| s.to_str()) == Some("md")
        })
    {
        let path = entry.path();
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let rel = path
            .strip_prefix(workspace)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.starts_with("[ATOM]") {
                continue;
            }
            let Some(t) = atom_type(trimmed) else {
                continue;
            };
            let include = if types.is_empty() {
                t != "fact"
            } else {
                types.iter().any(|want| *want == t)
            };
            if include {
                hits.push(Atom {
                    atom_type: t,
                    detail: atom_detail(trimmed),
                    source: rel.clone(),
                });
            }
        }
    }

    if hits.is_empty() {
        let filter = if types.is_empty() {
            "non-fact".to_string()
        } else {
            types.join("|")
        };
        return format!(
            "# agal findings\n\nno `{}` atoms found in `{}/notes/`\n",
            filter, output_dir
        );
    }

    // Display order: failure first, then requested order, remainder sorted.
    let mut order: Vec<String> = if types.is_empty() {
        let mut seen: Vec<String> = Vec::new();
        for h in &hits {
            if !seen.contains(&h.atom_type) {
                seen.push(h.atom_type.clone());
            }
        }
        seen.sort();
        seen
    } else {
        types.iter().map(|s| s.to_string()).collect()
    };
    if let Some(pos) = order.iter().position(|t| t == "failure") {
        order.remove(pos);
        order.insert(0, "failure".to_string());
    }

    let mut out = String::new();
    let _ = writeln!(
        out,
        "# agal findings\n{} atom(s) in `{}/notes/`\n",
        hits.len(),
        output_dir
    );

    for t in &order {
        let group: Vec<&Atom> = hits.iter().filter(|h| &h.atom_type == t).collect();
        if group.is_empty() {
            continue;
        }
        let _ = writeln!(out, "## {} ({})\n", t, group.len());
        for f in &group {
            if f.detail.is_empty() {
                let _ = writeln!(out, "- _{}_", f.source);
            } else {
                let _ = writeln!(out, "- {} _({})_", f.detail, f.source);
            }
        }
        let _ = writeln!(out);
    }

    out
}
