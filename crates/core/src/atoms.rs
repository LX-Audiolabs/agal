//! ATOM utilities: aggregate [ATOM] entries from notes; match skills by trigger; append atoms.

use std::fmt::Write as _;
use std::io::Write as IoWrite;
use std::path::Path;
use walkdir::WalkDir;

/// Flat atom entry for HTML serialization.
#[derive(serde::Serialize)]
pub struct AtomEntry {
    pub atom_type: String,
    pub detail: String,
    pub source: String,
}

/// Load all non-`fact` atoms from `<workspace>/<output_dir>/notes/` as flat entries.
pub fn load_atoms(workspace: &Path, output_dir: &str) -> Vec<AtomEntry> {
    let notes_dir = workspace.join(output_dir).join("notes");
    if !notes_dir.exists() {
        return Vec::new();
    }
    let mut entries: Vec<AtomEntry> = Vec::new();
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
            let Some(t) = atom_type(trimmed) else { continue };
            if t == "fact" {
                continue;
            }
            entries.push(AtomEntry {
                atom_type: t,
                detail: atom_detail(trimmed),
                source: rel.clone(),
            });
        }
    }
    entries
}

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

/// One matched skill file with its content.
pub struct SkillMatch {
    /// Relative path from workspace root.
    pub rel_path: String,
    /// Full file content (markdown).
    pub content: String,
    /// Matched trigger terms.
    pub matched_triggers: Vec<String>,
}

/// Scan `<workspace>/<output_dir>/skills/` and return skills whose `triggers:`
/// frontmatter field overlaps with any term in `query_terms`.
///
/// `query_terms` = focus node name tokens + framework names + extra keywords.
/// Matching is case-insensitive substring: trigger "biquad" matches term "biquadfilter".
pub fn match_skills(
    workspace: &Path,
    output_dir: &str,
    query_terms: &[&str],
) -> Vec<SkillMatch> {
    let skills_dir = workspace.join(output_dir).join("skills");
    if !skills_dir.exists() || query_terms.is_empty() {
        return Vec::new();
    }

    let lower_terms: Vec<String> = query_terms
        .iter()
        .map(|t| t.to_ascii_lowercase())
        .collect();

    let mut matches: Vec<SkillMatch> = Vec::new();

    for entry in WalkDir::new(&skills_dir)
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

        let triggers = parse_triggers(&content);
        if triggers.is_empty() {
            continue;
        }

        // A skill matches if any trigger is a substring of any query term OR vice versa.
        let matched: Vec<String> = triggers
            .iter()
            .filter(|trigger| {
                let tl = trigger.to_ascii_lowercase();
                lower_terms
                    .iter()
                    .any(|term| term.contains(tl.as_str()) || tl.contains(term.as_str()))
            })
            .cloned()
            .collect();

        if !matched.is_empty() {
            let rel = path
                .strip_prefix(workspace)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            matches.push(SkillMatch {
                rel_path: rel,
                content,
                matched_triggers: matched,
            });
        }
    }

    matches
}

/// Extract comma-separated values from `triggers:` frontmatter field.
fn parse_triggers(content: &str) -> Vec<String> {
    // Frontmatter is between first two `---` lines.
    let mut in_front = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "---" {
            if !in_front {
                in_front = true;
                continue;
            } else {
                break;
            }
        }
        if !in_front {
            continue;
        }
        if let Some(val) = trimmed.strip_prefix("triggers:") {
            return val
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }
    Vec::new()
}

/// Append one `[ATOM]` line to `<workspace>/<output_dir>/notes/_workspace.md`.
///
/// Creates the file (and parent dirs) if absent.
/// Valid types: `lesson`, `failure`, `decision`, `constraint`, `fact`.
pub fn append_atom(
    workspace: &Path,
    output_dir: &str,
    atom_type: &str,
    detail: &str,
) -> Result<(), String> {
    let notes_dir = workspace.join(output_dir).join("notes");
    std::fs::create_dir_all(&notes_dir)
        .map_err(|e| format!("cannot create notes dir: {}", e))?;

    let ws_file = notes_dir.join("_workspace.md");

    // Create with minimal template if absent.
    if !ws_file.exists() {
        std::fs::write(
            &ws_file,
            "# Workspace memory\n\n**Summary:** Durable cross-session notes for agents.\n\n## Atoms\n\n",
        )
        .map_err(|e| format!("cannot create _workspace.md: {}", e))?;
    }

    // Append the atom line.
    let line = format!("[ATOM] type={} | detail={}\n", atom_type, detail);
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&ws_file)
        .map_err(|e| format!("cannot open _workspace.md: {}", e))?;
    file.write_all(line.as_bytes())
        .map_err(|e| format!("cannot write atom: {}", e))?;

    Ok(())
}
