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

/// Load non-`fact` atoms from a single note file.
/// `include_types`: if empty, include all non-fact; else only listed types.
pub fn load_atoms_from_file(
    path: &Path,
    source_label: &str,
    include_types: &[&str],
) -> Vec<AtomEntry> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let mut entries = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("[ATOM]") {
            continue;
        }
        let Some(t) = atom_type(trimmed) else {
            continue;
        };
        if t == "fact" {
            continue;
        }
        let include = include_types.is_empty() || include_types.iter().any(|want| *want == t);
        if include {
            entries.push(AtomEntry {
                atom_type: t,
                detail: atom_detail(trimmed),
                source: source_label.to_string(),
            });
        }
    }
    entries
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
            e.file_type().is_file() && e.path().extension().and_then(|s| s.to_str()) == Some("md")
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
    // Template placeholder: "type=decision|lesson|constraint" — multiple types before first space/pipe-space
    // Detect by checking raw rest for "type=…|…" pattern before splitting on field separator " | "
    let type_raw = rest.split(" | ").next().unwrap_or("").trim();
    let type_val_raw = type_raw.strip_prefix("type=").unwrap_or("").trim();
    if type_val_raw.contains('|') {
        return None;
    }
    let val = type_val_raw;
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
            e.file_type().is_file() && e.path().extension().and_then(|s| s.to_str()) == Some("md")
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

/// Query terms for skill matching from a workspace node.
///
/// Full name + hyphen suffixes (`aura-dsp` → `dsp`), not the family prefix
/// (`aura`). Stops `triggers: aura` from attaching the AURA skill to every
/// `aura-*` crate. Frameworks equal to that prefix are skipped too.
pub fn query_terms_for_node(name: &str, frameworks: &[String]) -> Vec<String> {
    let mut terms = vec![name.to_string()];
    let parts: Vec<&str> = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect();
    let family = parts.first().copied().unwrap_or("");
    for part in parts.iter().skip(1) {
        if part.len() >= 3 {
            terms.push((*part).to_string());
        }
    }
    // Family prefix as its own exact term (`aura`), never as a substring of
    // `aura-core`. Lets `triggers: aura` attach the framework skill without
    // firing on hyphenated crate names via contains().
    if family.len() >= 3 && family != name && !terms.iter().any(|t| t == family) {
        terms.push(family.to_string());
    }
    for fw in frameworks {
        if fw != name && !terms.iter().any(|t| t == fw) {
            terms.push(fw.clone());
        }
    }
    terms
}

/// Trigger vs query-term hit.
///
/// Exact match always. Substring only when **neither** side contains `-`,
/// so `aura` does not match `aura-dsp`. Long enough (≥6) to keep
/// `biquad` → `biquadfilter` without `@aura` firing on every crate.
pub fn trigger_hits(trigger: &str, terms: &[String]) -> bool {
    let t = trigger.to_ascii_lowercase();
    for term in terms {
        let e = term.to_ascii_lowercase();
        if e == t {
            return true;
        }
        if e.contains('-') || t.contains('-') {
            continue;
        }
        // Long tokens only (`biquad` ⊂ `biquadfilter`). Short family
        // prefixes (`aura`, `clap`) must be exact so `@aura` / `AuraSlintEditor`
        // do not fire on every AURA crate.
        if t.len() >= 6 && e.len() >= 6 && (e.contains(&t) || t.contains(&e)) {
            return true;
        }
    }
    false
}

/// Scan `<workspace>/<output_dir>/skills/` and return skills whose `triggers:`
/// frontmatter field overlaps with any term in `query_terms`.
pub fn match_skills(workspace: &Path, output_dir: &str, query_terms: &[&str]) -> Vec<SkillMatch> {
    let skills_dir = workspace.join(output_dir).join("skills");
    if !skills_dir.exists() || query_terms.is_empty() {
        return Vec::new();
    }

    let terms: Vec<String> = query_terms.iter().map(|t| (*t).to_string()).collect();
    let mut matches: Vec<SkillMatch> = Vec::new();

    for entry in WalkDir::new(&skills_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file() && e.path().extension().and_then(|s| s.to_str()) == Some("md")
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

        let matched: Vec<String> = triggers
            .iter()
            .filter(|trigger| trigger_hits(trigger, &terms))
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

/// Append one `[ATOM]` line to a note in `<workspace>/<output_dir>/notes/`.
///
/// - `note`: target note stem (e.g. `"aura-dsp"` → `notes/aura-dsp.md`).
///   Pass `None` to target `_workspace.md` (always created if absent).
///   If a named note doesn't exist, falls back to `_workspace.md` and returns a warning.
///
/// Valid types: `lesson`, `failure`, `decision`, `constraint`.
pub fn append_atom(
    workspace: &Path,
    output_dir: &str,
    atom_type: &str,
    detail: &str,
    note: Option<&str>,
) -> Result<String, String> {
    let notes_dir = workspace.join(output_dir).join("notes");
    std::fs::create_dir_all(&notes_dir).map_err(|e| format!("cannot create notes dir: {}", e))?;

    let (target, fallback_msg) = if let Some(name) = note {
        let p = notes_dir.join(format!("{}.md", name));
        if p.exists() {
            (p, None)
        } else {
            // Named note missing — fall back to _workspace.md
            let msg = format!(
                "note `{}.md` not found in `{}/notes/` — wrote to _workspace.md instead. Run `agal .` first to generate crate notes.",
                name, output_dir
            );
            (notes_dir.join("_workspace.md"), Some(msg))
        }
    } else {
        (notes_dir.join("_workspace.md"), None)
    };

    // Create _workspace.md if absent.
    if !target.exists() {
        std::fs::write(
            &target,
            "# Workspace memory\n\n**Summary:** Durable cross-session notes for agents.\n\n## Atoms\n\n",
        )
        .map_err(|e| format!("cannot create _workspace.md: {}", e))?;
    }

    let line = format!("[ATOM] type={} | detail={}\n", atom_type, detail);
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&target)
        .map_err(|e| format!("cannot open {}: {}", target.display(), e))?;
    file.write_all(line.as_bytes())
        .map_err(|e| format!("cannot write atom: {}", e))?;

    Ok(fallback_msg.unwrap_or_default())
}

fn parse_frontmatter_field(content: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}:");
    let mut in_front = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "---" {
            if !in_front {
                in_front = true;
                continue;
            }
            break;
        }
        if !in_front {
            continue;
        }
        if let Some(val) = trimmed.strip_prefix(&prefix) {
            let v = val.trim().trim_matches('"').to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

/// Load one synced skill by stem, `id:` frontmatter, or unique path substring.
pub fn skill_by_query(
    workspace: &Path,
    output_dir: &str,
    query: &str,
) -> Result<String, String> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return Err("skill query is empty".to_string());
    }
    let skills_dir = workspace.join(output_dir).join("skills");
    if !skills_dir.exists() {
        return Err("no agal/skills/ — run `agal skills sync` first".to_string());
    }

    let mut hits: Vec<(String, String)> = Vec::new();
    for entry in WalkDir::new(&skills_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file() && e.path().extension().and_then(|s| s.to_str()) == Some("md")
        })
    {
        let path = entry.path();
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        let rel = path
            .strip_prefix(workspace)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let id = parse_frontmatter_field(&content, "id")
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();
        let rel_l = rel.to_ascii_lowercase();
        if stem == q || id == q || rel_l.ends_with(&format!("/{q}.md")) {
            hits.push((rel, content));
        }
    }

    match hits.len() {
        0 => {
            // Unique substring fallback (e.g. "dsp-realtime").
            let mut soft: Vec<(String, String)> = Vec::new();
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
                let Ok(content) = std::fs::read_to_string(path) else {
                    continue;
                };
                let rel = path
                    .strip_prefix(workspace)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .replace('\\', "/");
                if rel.to_ascii_lowercase().contains(&q) {
                    soft.push((rel, content));
                }
            }
            match soft.len() {
                1 => Ok(format!("# {}\n\n{}", soft[0].0, soft[0].1.trim())),
                0 => Err(format!(
                    "no skill matching '{query}'. Try `id` / stem (e.g. clap, dsp-realtime, aura)."
                )),
                _ => Err(format!(
                    "ambiguous skill '{query}'. Candidates: {}",
                    soft.iter()
                        .map(|(p, _)| p.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            }
        }
        1 => Ok(format!("# {}\n\n{}", hits[0].0, hits[0].1.trim())),
        _ => Err(format!(
            "ambiguous skill '{query}'. Candidates: {}",
            hits.iter()
                .map(|(p, _)| p.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_terms_family_is_exact_not_split() {
        let fw = vec!["aura".to_string(), "clap".to_string()];
        let terms = query_terms_for_node("aura-dsp", &fw);
        assert_eq!(terms, vec!["aura-dsp", "dsp", "aura", "clap"]);
    }

    #[test]
    fn aura_substring_does_not_hit_hyphenated_name() {
        assert!(!trigger_hits("aura", &["aura-core".into()]));
        assert!(trigger_hits("aura", &query_terms_for_node("aura-core", &[])));
        assert!(trigger_hits("aura-core", &["aura-core".into()]));
        assert!(trigger_hits("dsp", &query_terms_for_node("aura-dsp", &[])));
        assert!(!trigger_hits("dsp", &query_terms_for_node("aura-core", &[])));
        assert!(trigger_hits("biquad", &["biquadfilter".into()]));
        assert!(!trigger_hits("aura", &["aura-dsp".into()]));
        assert!(!trigger_hits("@aura", &["aura".into()]));
        assert!(!trigger_hits("AuraSlintEditor", &["aura".into()]));
        assert!(!trigger_hits("truce to aura", &["aura".into()]));
    }
}
