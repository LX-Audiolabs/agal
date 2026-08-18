//! Embedded skill templates + curated sync into workspaces.
//!
//! Canonical skills live in the agal tool repo (`skills/`).
//! Workspaces receive a **copy** only via `agal skills sync` — never on every generate.
//!
//! Default pack: **core** (domain constitution). Everything else is opt-in.
//!
//! `--only` accepts groups (`policy`, `ui`), singles (`ui/slint`), and **presets**
//! (`slint-ui` → `core,ui/slint`). Same presets work via `agal skills sync --preset …`.

use std::fs;
use std::path::Path;

pub const CAVEMAN: &str = include_str!("../../../skills/01-policy/caveman.md");
pub const PONYTAIL: &str = include_str!("../../../skills/01-policy/ponytail.md");
pub const VERSIONING: &str = include_str!("../../../skills/01-policy/versioning.md");
pub const AGENT_USAGE: &str = include_str!("../../../skills/06-agents/agent-usage.md");

pub const DSP_REALTIME: &str = include_str!("../../../skills/00-core/dsp-realtime.md");
pub const DSP_CORRECTNESS: &str = include_str!("../../../skills/00-core/dsp-correctness.md");
pub const AUDIO_THREAD_BOUNDARY: &str =
    include_str!("../../../skills/00-core/audio-thread-boundary.md");
pub const FILTER_BIQUAD: &str = include_str!("../../../skills/00-core/filter-biquad.md");
pub const FRAMEWORK_PATTERNS: &str =
    include_str!("../../../skills/02-frameworks/framework-patterns.md");
pub const AURA: &str = include_str!("../../../skills/02-frameworks/aura.md");
pub const AURA_MIGRATION: &str = include_str!("../../../skills/02-frameworks/aura-migration.md");

pub const CLAP: &str = include_str!("../../../skills/03-formats/clap.md");
pub const VST3: &str = include_str!("../../../skills/03-formats/vst3.md");
pub const LV2: &str = include_str!("../../../skills/03-formats/lv2.md");

pub const SLINT: &str = include_str!("../../../skills/04-ui/slint.md");
pub const EGUI: &str = include_str!("../../../skills/04-ui/egui.md");
pub const ICED: &str = include_str!("../../../skills/04-ui/iced.md");
pub const VIZIA: &str = include_str!("../../../skills/04-ui/vizia.md");

pub const NIH_PLUG_TO_NICE_PLUG: &str =
    include_str!("../../../skills/05-migrations/nih-plug-to-nice-plug.md");

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SkillGroup {
    Core,
    Policy,
    Frameworks,
    Formats,
    Ui,
    Migrations,
    Agents,
}

impl SkillGroup {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Policy => "policy",
            Self::Frameworks => "frameworks",
            Self::Formats => "formats",
            Self::Ui => "ui",
            Self::Migrations => "migrations",
            Self::Agents => "agents",
        }
    }

    /// Directory prefix in the catalog, e.g. `00-core`, `04-ui`.
    pub fn dir_prefix(self) -> &'static str {
        match self {
            Self::Core => "00-core",
            Self::Policy => "01-policy",
            Self::Frameworks => "02-frameworks",
            Self::Formats => "03-formats",
            Self::Ui => "04-ui",
            Self::Migrations => "05-migrations",
            Self::Agents => "06-agents",
        }
    }

    pub fn parse_one(s: &str) -> Result<SkillGroup, String> {
        match s.to_ascii_lowercase().as_str() {
            "core" => Ok(Self::Core),
            "policy" => Ok(Self::Policy),
            "frameworks" | "fw" => Ok(Self::Frameworks),
            "formats" | "format" => Ok(Self::Formats),
            "ui" => Ok(Self::Ui),
            "migrations" | "migration" => Ok(Self::Migrations),
            "agents" | "agent" => Ok(Self::Agents),
            other => Err(format!(
                "unknown skill group '{}'; use core|policy|frameworks|formats|ui|migrations|agents|all  (or single: ui/slint)",
                other
            )),
        }
    }

    /// Group-only list (legacy). Prefer [`parse_selection`] for `--only`.
    pub fn parse_list(spec: &str) -> Result<Vec<SkillGroup>, String> {
        let sel = parse_selection(spec)?;
        let mut groups = Vec::new();
        for f in &sel.files {
            if !groups.contains(&f.group) {
                groups.push(f.group);
            }
        }
        Ok(groups)
    }
}

/// One embedded skill file.
#[derive(Debug, Clone, Copy)]
pub struct SkillFile {
    pub group: SkillGroup,
    pub rel_path: &'static str,
    pub content: &'static str,
}

impl SkillFile {
    fn stem(&self) -> &str {
        path_stem(self.rel_path)
    }
}

fn path_stem(rel: &str) -> &str {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    name.strip_suffix(".md").unwrap_or(name)
}

/// Resolved `--only` selection (deduped, stable catalog order).
#[derive(Debug, Clone)]
pub struct Selection {
    pub files: Vec<SkillFile>,
    /// Human labels for logging (`core`, `ui/slint`, `preset:slint-ui`, …).
    pub labels: Vec<String>,
}

/// Task-loadout presets → `--only` grammar. Stable short names for humans/agents.
///
/// | Preset | Expands to |
/// |--------|------------|
/// | `dsp-fix` | `core` |
/// | `slint-ui` | `core,ui/slint` |
/// | `clap-ship` | `core,formats/clap` |
/// | `agent-playbook` | `agents` |
/// | `policy-edit` | `policy` |
pub fn resolve_preset(name: &str) -> Result<&'static str, String> {
    match name.trim().to_ascii_lowercase().as_str() {
        "dsp-fix" | "dsp" => Ok("core"),
        "slint-ui" => Ok("core,ui/slint"),
        "clap-ship" => Ok("core,formats/clap"),
        "agent-playbook" | "agents-playbook" => Ok("agents"),
        "policy-edit" => Ok("policy"),
        other => Err(format!(
            "unknown preset '{}'; known: {}",
            other,
            PRESET_NAMES.join(", ")
        )),
    }
}

/// Canonical preset names (for help / list).
pub const PRESET_NAMES: &[&str] = &[
    "dsp-fix",
    "slint-ui",
    "clap-ship",
    "agent-playbook",
    "policy-edit",
];

/// True if `name` is a known preset (case-insensitive).
fn is_preset_token(name: &str) -> bool {
    resolve_preset(name).is_ok()
}

/// Parse `--only` / `--preset` spec: groups, singles, and loadout presets.
///
/// Examples:
/// - `core` (default when empty)
/// - `policy`, `ui`
/// - `ui/slint`, `ui/slint.md`, `04-ui/slint`
/// - `core,ui/slint,formats/clap`
/// - `slint-ui` (preset → core + slint)
/// - `all`
pub fn parse_selection(spec: &str) -> Result<Selection, String> {
    let parts: Vec<&str> = spec
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if parts.is_empty() {
        return Ok(select_groups(&[SkillGroup::Core], vec!["core".into()]));
    }

    if parts.iter().any(|p| p.eq_ignore_ascii_case("all")) {
        if parts.len() != 1 {
            return Err("'all' cannot be mixed with other selectors".into());
        }
        return Ok(Selection {
            files: catalog(),
            labels: vec!["all".into()],
        });
    }

    // Expand loadout presets first (e.g. slint-ui → core + ui/slint tokens).
    let mut tokens: Vec<(String, Option<String>)> = Vec::new();
    for part in parts {
        if is_preset_token(part) {
            let expanded = resolve_preset(part)?;
            let preset_label = format!("preset:{}", part.trim().to_ascii_lowercase());
            for sub in expanded.split(',') {
                let sub = sub.trim();
                if !sub.is_empty() {
                    tokens.push((sub.to_string(), Some(preset_label.clone())));
                }
            }
        } else {
            tokens.push((part.to_string(), None));
        }
    }

    let mut want_paths = std::collections::BTreeSet::new();
    let mut labels = Vec::new();
    let mut seen_preset_labels = std::collections::BTreeSet::new();

    for (part, preset_label) in tokens {
        if let Some(pl) = &preset_label {
            if seen_preset_labels.insert(pl.clone()) {
                labels.push(pl.clone());
            }
        }
        let part_norm = part.replace('\\', "/");
        if part_norm.contains('/') {
            let file = resolve_path_selector(&part_norm)?;
            want_paths.insert(file.rel_path);
            if preset_label.is_none() {
                labels.push(format_selector_label(file));
            }
        } else if let Ok(group) = SkillGroup::parse_one(&part_norm) {
            for s in catalog() {
                if s.group == group {
                    want_paths.insert(s.rel_path);
                }
            }
            if preset_label.is_none() {
                labels.push(group.as_str().to_string());
            }
        } else {
            // bare stem: only if unique in catalog
            let stem = part_norm
                .strip_suffix(".md")
                .unwrap_or(part_norm.as_str())
                .to_ascii_lowercase();
            let matches: Vec<SkillFile> = catalog()
                .into_iter()
                .filter(|s| s.stem().eq_ignore_ascii_case(&stem))
                .collect();
            match matches.as_slice() {
                [] => {
                    return Err(format!(
                        "unknown selector '{}'; use a group (ui), group/skill (ui/slint), \
                         path (04-ui/slint), or preset ({})",
                        part,
                        PRESET_NAMES.join("|")
                    ));
                }
                [one] => {
                    want_paths.insert(one.rel_path);
                    if preset_label.is_none() {
                        labels.push(format_selector_label(*one));
                    }
                }
                many => {
                    let opts: Vec<String> = many
                        .iter()
                        .map(|s| format!("{}/{}", s.group.as_str(), s.stem()))
                        .collect();
                    return Err(format!(
                        "ambiguous skill '{}'; disambiguate with group/name: {}",
                        part,
                        opts.join(", ")
                    ));
                }
            }
        }
    }

    let files: Vec<SkillFile> = catalog()
        .into_iter()
        .filter(|s| want_paths.contains(s.rel_path))
        .collect();

    if files.is_empty() {
        return Err("no skills matched selection".into());
    }

    Ok(Selection { files, labels })
}

fn format_selector_label(f: SkillFile) -> String {
    format!("{}/{}", f.group.as_str(), f.stem())
}

fn select_groups(groups: &[SkillGroup], labels: Vec<String>) -> Selection {
    let want: std::collections::BTreeSet<_> = groups.iter().copied().collect();
    let files = catalog()
        .into_iter()
        .filter(|s| want.contains(&s.group))
        .collect();
    Selection { files, labels }
}

/// Resolve `ui/slint`, `ui/slint.md`, `04-ui/slint`, `04-ui/slint.md`.
fn resolve_path_selector(raw: &str) -> Result<SkillFile, String> {
    let raw = raw.trim().trim_start_matches("./");
    let lower = raw.to_ascii_lowercase();
    let (left, right) = lower.split_once('/').ok_or_else(|| {
        format!(
            "invalid skill path '{}'; expected group/skill e.g. ui/slint",
            raw
        )
    })?;
    if right.contains('/') {
        return Err(format!(
            "invalid skill path '{}'; use group/skill (one slash) e.g. ui/slint",
            raw
        ));
    }
    let stem = right.strip_suffix(".md").unwrap_or(right);
    if stem.is_empty() {
        return Err(format!("invalid skill path '{}'; missing skill name", raw));
    }

    // Prefer group name (ui/slint)
    if let Ok(group) = SkillGroup::parse_one(left) {
        let matches: Vec<SkillFile> = catalog()
            .into_iter()
            .filter(|s| s.group == group && s.stem().eq_ignore_ascii_case(stem))
            .collect();
        return match matches.as_slice() {
            [one] => Ok(*one),
            [] => {
                let available: Vec<String> = catalog()
                    .into_iter()
                    .filter(|s| s.group == group)
                    .map(|s| s.stem().to_string())
                    .collect();
                Err(format!(
                    "no skill '{stem}' in group '{}'; available: {}",
                    group.as_str(),
                    available.join(", ")
                ))
            }
            _ => Err(format!("internal: multiple matches for {}/{}", left, stem)),
        };
    }

    // Numbered dir (04-ui/slint) or full rel_path
    let with_md = if lower.ends_with(".md") {
        lower.clone()
    } else {
        format!("{lower}.md")
    };
    let matches: Vec<SkillFile> = catalog()
        .into_iter()
        .filter(|s| {
            let p = s.rel_path.to_ascii_lowercase();
            p == with_md
                || (p.starts_with(&format!("{left}/")) && s.stem().eq_ignore_ascii_case(stem))
        })
        .collect();
    match matches.as_slice() {
        [one] => Ok(*one),
        [] => Err(format!(
            "unknown skill path '{}'; try ui/slint or run `agal skills list`",
            raw
        )),
        many => {
            let opts: Vec<&str> = many.iter().map(|s| s.rel_path).collect();
            Err(format!(
                "ambiguous path '{}'; matches: {}",
                raw,
                opts.join(", ")
            ))
        }
    }
}

/// Full catalog (tool-embedded). Paths are numbered by priority.
pub fn catalog() -> Vec<SkillFile> {
    vec![
        SkillFile {
            group: SkillGroup::Core,
            rel_path: "00-core/dsp-realtime.md",
            content: DSP_REALTIME,
        },
        SkillFile {
            group: SkillGroup::Core,
            rel_path: "00-core/dsp-correctness.md",
            content: DSP_CORRECTNESS,
        },
        SkillFile {
            group: SkillGroup::Core,
            rel_path: "00-core/audio-thread-boundary.md",
            content: AUDIO_THREAD_BOUNDARY,
        },
        SkillFile {
            group: SkillGroup::Core,
            rel_path: "00-core/filter-biquad.md",
            content: FILTER_BIQUAD,
        },
        SkillFile {
            group: SkillGroup::Policy,
            rel_path: "01-policy/caveman.md",
            content: CAVEMAN,
        },
        SkillFile {
            group: SkillGroup::Policy,
            rel_path: "01-policy/ponytail.md",
            content: PONYTAIL,
        },
        SkillFile {
            group: SkillGroup::Policy,
            rel_path: "01-policy/versioning.md",
            content: VERSIONING,
        },
        SkillFile {
            group: SkillGroup::Frameworks,
            rel_path: "02-frameworks/framework-patterns.md",
            content: FRAMEWORK_PATTERNS,
        },
        SkillFile {
            group: SkillGroup::Frameworks,
            rel_path: "02-frameworks/aura.md",
            content: AURA,
        },
        SkillFile {
            group: SkillGroup::Frameworks,
            rel_path: "02-frameworks/aura-migration.md",
            content: AURA_MIGRATION,
        },
        SkillFile {
            group: SkillGroup::Formats,
            rel_path: "03-formats/clap.md",
            content: CLAP,
        },
        SkillFile {
            group: SkillGroup::Formats,
            rel_path: "03-formats/vst3.md",
            content: VST3,
        },
        SkillFile {
            group: SkillGroup::Formats,
            rel_path: "03-formats/lv2.md",
            content: LV2,
        },
        SkillFile {
            group: SkillGroup::Ui,
            rel_path: "04-ui/slint.md",
            content: SLINT,
        },
        SkillFile {
            group: SkillGroup::Ui,
            rel_path: "04-ui/egui.md",
            content: EGUI,
        },
        SkillFile {
            group: SkillGroup::Ui,
            rel_path: "04-ui/iced.md",
            content: ICED,
        },
        SkillFile {
            group: SkillGroup::Ui,
            rel_path: "04-ui/vizia.md",
            content: VIZIA,
        },
        SkillFile {
            group: SkillGroup::Migrations,
            rel_path: "05-migrations/nih-plug-to-nice-plug.md",
            content: NIH_PLUG_TO_NICE_PLUG,
        },
        SkillFile {
            group: SkillGroup::Agents,
            rel_path: "06-agents/agent-usage.md",
            content: AGENT_USAGE,
        },
    ]
}

/// Skills for the given groups (deduped by path).
pub fn select_owned(groups: &[SkillGroup]) -> Vec<SkillFile> {
    select_groups(
        groups,
        groups.iter().map(|g| g.as_str().to_string()).collect(),
    )
    .files
}

/// Print catalog to stdout.
pub fn print_list() {
    println!("agal skills (embedded in tool; sync into workspace with `agal skills sync`)\n");
    println!(
        "selectors: group | group/skill | numbered-path/skill | stem (if unique) | preset | all\n"
    );
    println!("groups: core (default) | policy | frameworks | formats | ui | migrations | agents\n");
    println!("presets (task loadouts):");
    for name in PRESET_NAMES {
        let exp = resolve_preset(name).unwrap_or("?");
        println!("  {name:16} → {exp}");
    }
    println!();
    let mut current = None;
    for s in catalog() {
        let g = s.group.as_str();
        if current != Some(g) {
            println!("[{}]", g);
            current = Some(g);
        }
        println!("  {}   ({} / {})", s.rel_path, g, s.stem());
    }
    println!("\nexamples:");
    println!("  agal skills sync                      # core only");
    println!("  agal skills sync --only policy        # whole group");
    println!("  agal skills sync --only ui/slint      # single skill");
    println!("  agal skills sync --preset slint-ui    # core + slint");
    println!("  agal skills sync --only slint-ui      # same (preset as --only token)");
    println!("  agal skills sync --only core,ui/slint,formats/clap");
    println!("  agal skills sync --only all --force");
}

pub struct SyncOptions {
    pub selection: Selection,
    pub force: bool,
    pub output_dir: String,
}

/// Copy selected skills into `project_root/<output_dir>/skills/`.
pub fn sync(project_root: &Path, opts: &SyncOptions) -> Result<usize, String> {
    let skills_root = project_root.join(&opts.output_dir).join("skills");
    fs::create_dir_all(&skills_root)
        .map_err(|e| format!("cannot create {}: {}", skills_root.display(), e))?;

    let files = &opts.selection.files;
    if files.is_empty() {
        return Err("no skills matched selection".into());
    }

    let mut written = 0usize;
    let mut skipped = 0usize;
    for s in files {
        let target = skills_root.join(s.rel_path);
        if target.exists() && !opts.force {
            skipped += 1;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {}", parent.display(), e))?;
        }
        fs::write(&target, s.content)
            .map_err(|e| format!("cannot write {}: {}", target.display(), e))?;
        written += 1;
        println!(
            "  wrote {}",
            target
                .strip_prefix(project_root)
                .unwrap_or(&target)
                .display()
        );
    }

    println!(
        "skills sync: {} written, {} skipped (use --force to overwrite) → {}/skills/",
        written, skipped, opts.output_dir
    );
    println!("  selection: {}", opts.selection.labels.join(", "));

    // Keep AGAL.md skill index in sync with what is on disk now.
    if let Err(e) = crate::agent::refresh_agal_after_skills_sync(project_root, &opts.output_dir) {
        eprintln!("warning: could not refresh AGAL.md: {e}");
    }

    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_empty_is_core() {
        let s = parse_selection("").unwrap();
        assert!(s.files.iter().all(|f| f.group == SkillGroup::Core));
        assert_eq!(s.files.len(), 4);
    }

    #[test]
    fn group_policy() {
        let s = parse_selection("policy").unwrap();
        // caveman, ponytail, versioning
        assert_eq!(s.files.len(), 3);
        assert!(s.files.iter().all(|f| f.group == SkillGroup::Policy));
    }

    #[test]
    fn group_frameworks() {
        let s = parse_selection("frameworks").unwrap();
        // framework-patterns, aura, aura-migration
        assert_eq!(s.files.len(), 3);
        assert!(s.files.iter().all(|f| f.group == SkillGroup::Frameworks));
        assert!(
            s.files
                .iter()
                .any(|f| f.rel_path == "02-frameworks/aura.md")
        );
        assert!(
            s.files
                .iter()
                .any(|f| f.rel_path == "02-frameworks/aura-migration.md")
        );
    }

    #[test]
    fn single_frameworks_aura() {
        let s = parse_selection("frameworks/aura").unwrap();
        assert_eq!(s.files.len(), 1);
        assert_eq!(s.files[0].rel_path, "02-frameworks/aura.md");
    }

    #[test]
    fn single_ui_slint() {
        let s = parse_selection("ui/slint").unwrap();
        assert_eq!(s.files.len(), 1);
        assert_eq!(s.files[0].rel_path, "04-ui/slint.md");
    }

    #[test]
    fn single_numbered_path() {
        let s = parse_selection("04-ui/slint").unwrap();
        assert_eq!(s.files.len(), 1);
        assert_eq!(s.files[0].rel_path, "04-ui/slint.md");
    }

    #[test]
    fn mix_group_and_single() {
        let s = parse_selection("core,ui/slint").unwrap();
        assert_eq!(s.files.len(), 5); // 4 core + slint
        assert!(s.files.iter().any(|f| f.rel_path == "04-ui/slint.md"));
    }

    #[test]
    fn bare_unique_stem() {
        let s = parse_selection("slint").unwrap();
        assert_eq!(s.files.len(), 1);
        assert_eq!(s.files[0].rel_path, "04-ui/slint.md");
    }

    #[test]
    fn unknown_skill_in_group() {
        let err = parse_selection("ui/nope").unwrap_err();
        assert!(err.contains("no skill"), "{err}");
        assert!(err.contains("slint"), "{err}");
    }

    #[test]
    fn all_alone() {
        let s = parse_selection("all").unwrap();
        assert_eq!(s.files.len(), catalog().len());
    }

    #[test]
    fn preset_slint_ui() {
        let s = parse_selection("slint-ui").unwrap();
        assert_eq!(s.files.len(), 5); // 4 core + slint
        assert!(s.files.iter().any(|f| f.rel_path == "04-ui/slint.md"));
        assert!(s.files.iter().any(|f| f.group == SkillGroup::Core));
        assert!(s.labels.iter().any(|l| l == "preset:slint-ui"));
    }

    #[test]
    fn preset_clap_ship() {
        let s = parse_selection("clap-ship").unwrap();
        assert!(s.files.iter().any(|f| f.rel_path == "03-formats/clap.md"));
        assert!(
            s.files
                .iter()
                .all(|f| { f.group == SkillGroup::Core || f.rel_path == "03-formats/clap.md" })
        );
        assert_eq!(s.files.len(), 5);
    }

    #[test]
    fn preset_via_resolve() {
        assert_eq!(resolve_preset("dsp-fix").unwrap(), "core");
        assert_eq!(resolve_preset("DSP").unwrap(), "core");
        assert!(resolve_preset("nope").is_err());
    }
}
