use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[allow(dead_code)]
pub struct ProjectConfig {
    /// Override auto-detected project name (from workspace Cargo.toml).
    #[serde(default)]
    pub project_name: Option<String>,
    /// Custom frameworks beyond the default taxonomy.
    #[serde(default)]
    pub frameworks: BTreeMap<String, FrameworkSpec>,
    /// Declared migrations between editor adapters or frameworks.
    #[serde(default)]
    pub migrations: BTreeMap<String, MigrationSpec>,
    /// Manual internal crate names (auto-detection via path resolution handles most cases).
    #[serde(default)]
    pub internal_crates: Vec<String>,
    /// Custom rules shown in the JSON output.
    #[serde(default)]
    pub rules: BTreeMap<String, String>,
    /// Override default output directory (default: "agal/").
    #[serde(default)]
    pub output_dir: Option<String>,
    /// Crate package names treated as shared Slint UI libraries (for uses_ui edges).
    /// Auto-detected when a crate exports Lx* components; this extends the set.
    #[serde(default)]
    pub ui_crates: Vec<String>,
    /// Package names that provide shared-memory / IPC hubs.
    #[serde(default)]
    pub ipc_hubs: Vec<String>,
    /// Findings to silence (intentional exceptions). Matched after analyze.
    #[serde(default)]
    pub suppress: Vec<SuppressRule>,
    /// HTML graph view preferences.
    #[serde(default)]
    pub view: ViewConfig,
}

/// HTML graph view settings.
///
/// ```toml
/// [view]
/// default = "all"  # "overview" | "all" | "plugin" | "crate" (auto: overview if plugins, else all)
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ViewConfig {
    /// Default graph view mode. Auto-detected from repo contents when unset.
    #[serde(default)]
    pub default: Option<String>,
}

/// One intentional mute for a finding code (optionally scoped to a node).
///
/// ```toml
/// [[suppress]]
/// code = "large_param_surface"
/// node = "aurum-slint"          # optional: id, path, or package name; omit / "*" = all
/// reason = "product surface intentional"
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SuppressRule {
    pub code: String,
    #[serde(default)]
    pub node: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FrameworkSpec {
    pub name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MigrationSpec {
    pub from: String,
    pub to: String,
}

/// Built-in editor-adapter migration when the config omits `[migrations.*]`.
/// AURA-first: when an `aura.toml` or `aura-*` crates are present, default to
/// `truce-slint → aura-editor`; otherwise legacy `truce-slint → lx-slint-editor`.
pub fn default_migrations(is_aura: bool) -> BTreeMap<String, MigrationSpec> {
    let mut m = BTreeMap::new();
    m.insert(
        "truce-slint".to_string(),
        MigrationSpec {
            from: "truce-slint".to_string(),
            to: if is_aura {
                "aura-editor".to_string()
            } else {
                "lx-slint-editor".to_string()
            },
        },
    );
    m
}

/// Detect whether a workspace root is an AURA workspace (has `aura.toml`).
pub fn is_aura_workspace(project_root: &Path) -> bool {
    project_root.join("aura.toml").is_file()
}

/// Returns the aura config path if present.
pub fn aura_config_path(project_root: &Path) -> Option<std::path::PathBuf> {
    let candidate = project_root.join("aura.toml");
    if candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}

impl ProjectConfig {
    /// Load the project config (`agal.toml`, falling back to `audiolabs.toml` / `audio-graph.toml`).
    pub fn load(project_root: &Path) -> Self {
        let path = config_path(project_root);
        let mut cfg = if let Some(path) = path {
            match fs::read_to_string(&path) {
                Ok(mut content) => {
                    content.retain(|c| c != '\r');
                    content
                        .parse::<toml::Table>()
                        .map(toml::Value::Table)
                        .ok()
                        .and_then(|v| v.try_into::<ProjectConfig>().ok())
                        .unwrap_or_default()
                }
                Err(_) => Self::default(),
            }
        } else {
            Self::default()
        };
        // Zero-config: still track the known editor-adapter migration.
        if cfg.migrations.is_empty() {
            cfg.migrations = default_migrations(is_aura_workspace(project_root));
        }
        cfg
    }
}

/// Returns the config path next to the root Cargo.toml.
/// Lookup order: `agal.toml` (current) → `audio-graph.toml` (legacy).
pub fn config_path(project_root: &Path) -> Option<std::path::PathBuf> {
    for name in ["agal.toml", "audiolabs.toml", "audio-graph.toml"] {
        let candidate = project_root.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn load_registry_frameworks() -> BTreeMap<String, FrameworkSpec> {
    let mut map = BTreeMap::new();
    let Some(reg) = crate::registry::load() else {
        return map;
    };

    for (id, fw) in reg.frameworks {
        let mut notes = Vec::new();
        if let Some(h) = &fw.homepage {
            notes.push(format!("homepage: {}", h));
        }
        if let Some(d) = &fw.docs {
            notes.push(format!("docs: {}", d));
        }
        if let Some(s) = &fw.skill_source {
            notes.push(format!("skill: {}", s));
        }
        if let Some(m) = &fw.migrates_from {
            notes.push(format!("migrates from: {}", m));
        }
        map.insert(
            id.clone(),
            FrameworkSpec {
                name: fw.name,
                kind: "framework".to_string(),
                notes: if notes.is_empty() {
                    None
                } else {
                    Some(notes.join("; "))
                },
            },
        );
    }

    for (id, ui) in reg.ui {
        let mut notes = Vec::new();
        if let Some(h) = &ui.homepage {
            notes.push(format!("homepage: {}", h));
        }
        if let Some(d) = &ui.docs {
            notes.push(format!("docs: {}", d));
        }
        map.insert(
            id.clone(),
            FrameworkSpec {
                name: ui.name,
                kind: "ui_framework".to_string(),
                notes: if notes.is_empty() {
                    None
                } else {
                    Some(notes.join("; "))
                },
            },
        );
    }

    for (id, fmt) in reg.formats {
        let mut notes = Vec::new();
        if let Some(s) = &fmt.spec {
            notes.push(format!("spec: {}", s));
        }
        if let Some(c) = &fmt.changelog {
            notes.push(format!("changelog: {}", c));
        }
        map.insert(
            id.clone(),
            FrameworkSpec {
                name: id.to_uppercase(),
                kind: "plugin_format".to_string(),
                notes: if notes.is_empty() {
                    None
                } else {
                    Some(notes.join("; "))
                },
            },
        );
    }

    map
}

pub fn default_taxonomy() -> BTreeMap<String, FrameworkSpec> {
    let mut map = load_registry_frameworks();

    for (id, name, kind) in [
        ("aura", "AURA", "framework"),
        ("truce", "truce.audio", "framework"),
        ("nih-plug", "nih-plug", "framework"),
        ("clack", "clack", "framework"),
        ("juce", "JUCE", "framework"),
    ] {
        map.insert(
            id.to_string(),
            FrameworkSpec {
                name: name.to_string(),
                kind: kind.to_string(),
                notes: None,
            },
        );
    }

    for (id, name) in [
        ("slint", "Slint"),
        ("egui", "egui"),
        ("iced", "Iced"),
        ("vizia", "Vizia"),
        ("wry", "WRY"),
        ("baseview", "baseview"),
        ("raw-window-handle", "raw-window-handle"),
    ] {
        map.insert(
            id.to_string(),
            FrameworkSpec {
                name: name.to_string(),
                kind: "ui_framework".to_string(),
                notes: None,
            },
        );
    }

    for (id, name) in [
        ("truce-slint", "truce-slint"),
        ("lx-slint-editor", "lx-slint-editor"),
        ("aura-editor", "aura-editor"),
        ("aura-baseview", "aura-baseview"),
    ] {
        map.insert(
            id.to_string(),
            FrameworkSpec {
                name: name.to_string(),
                kind: "editor_adapter".to_string(),
                notes: Some(format!("{} editor adapter.", name)),
            },
        );
    }

    for (id, name) in [("clap", "CLAP"), ("vst3", "VST3"), ("lv2", "LV2")] {
        map.insert(
            id.to_string(),
            FrameworkSpec {
                name: name.to_string(),
                kind: "plugin_format".to_string(),
                notes: None,
            },
        );
    }

    map
}

pub fn resolve_framework_id(name: &str) -> Option<String> {
    match name {
        "aura" | "aura-core" | "aura-params" | "aura-dsp" | "aura-midi"
        | "aura-build" | "aura-test" | "aura-shm" | "aura-derive" => Some("aura".to_string()),
        "aura-clap" => Some("clap".to_string()),
        "aura-lv2" => Some("lv2".to_string()),
        "aura-vst3" => Some("vst3".to_string()),
        "aura-editor" => Some("aura-editor".to_string()),
        "aura-baseview" => Some("aura-baseview".to_string()),
        "truce" | "truce-core" | "truce-params" | "truce-clap" | "truce-vst3" | "truce-lv2"
        | "truce-loader" | "truce-test" | "truce-slint-build" => Some("truce".to_string()),
        "truce-slint" => Some("truce-slint".to_string()),
        "lx-slint-editor" => Some("lx-slint-editor".to_string()),
        "nih-plug" => Some("nih-plug".to_string()),
        "clack" | "clack-plugin" | "clack-host" => Some("clack".to_string()),
        "juce" => Some("juce".to_string()),
        "slint" | "slint-build" | "slint-baseview" => Some("slint".to_string()),
        "egui" => Some("egui".to_string()),
        "iced" => Some("iced".to_string()),
        "vizia" => Some("vizia".to_string()),
        "wry" => Some("wry".to_string()),
        "baseview" => Some("baseview".to_string()),
        "raw-window-handle" => Some("raw-window-handle".to_string()),
        "clap-sys" | "clap" => Some("clap".to_string()),
        "vst3" => Some("vst3".to_string()),
        // lv2 crate name + common sys bindings (aura-lv2 uses lv2-sys)
        "lv2" | "lv2-sys" | "lv2_raw" | "lv2-raw" => Some("lv2".to_string()),
        _ => None,
    }
}

pub fn crate_name_from_use_path(path: &str) -> Option<String> {
    let first = path.split("::").next()?;
    if first.is_empty()
        || first == "crate"
        || first == "self"
        || first == "super"
        || first == "std"
        || first == "core"
        || first == "alloc"
    {
        return None;
    }
    Some(first.replace('_', "-"))
}

pub fn detect_framework_ids(
    taxonomy: &BTreeMap<String, FrameworkSpec>,
    dependency_names: &BTreeSet<String>,
    imported_crates: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut detected = BTreeSet::new();

    for dep in dependency_names {
        if let Some(id) = resolve_framework_id(dep)
            && taxonomy.contains_key(&id)
        {
            detected.insert(id);
        }
        if taxonomy.contains_key(dep) {
            detected.insert(dep.clone());
        }
    }

    for imp in imported_crates {
        if let Some(id) = resolve_framework_id(imp)
            && taxonomy.contains_key(&id)
        {
            detected.insert(id);
        }
        if taxonomy.contains_key(imp) {
            detected.insert(imp.clone());
        }
    }

    detected
}

/// Collapse external Cargo deps into short flags for token efficiency.
pub fn summarize_external_deps(
    dep_names: &BTreeSet<String>,
    internal: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut flags = BTreeSet::new();
    let mut truce_stack = false;
    for d in dep_names {
        if internal.contains(d) {
            continue;
        }
        if d.starts_with("truce") {
            truce_stack = true;
            continue;
        }
        if matches!(d.as_str(), "clap-sys" | "clap") {
            flags.insert("clap".to_string());
            continue;
        }
        // Keep signal-bearing externals only.
        if matches!(
            d.as_str(),
            "realfft"
                | "serde"
                | "serde_json"
                | "tracing"
                | "tracing-subscriber"
                | "tracing-appender"
                | "ebur128"
                | "atomic_float"
                | "num-complex"
                | "shared_memory"
                | "baseview"
                | "raw-window-handle"
                | "slint"
                | "slint-build"
                | "slint-baseview"
                | "log"
                | "paste"
        ) {
            flags.insert(d.clone());
        }
    }
    if truce_stack {
        flags.insert("truce_stack".to_string());
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_framework_aliases() {
        assert_eq!(
            resolve_framework_id("truce-params").as_deref(),
            Some("truce")
        );
        assert_eq!(
            resolve_framework_id("lx-slint-editor").as_deref(),
            Some("lx-slint-editor")
        );
        assert_eq!(resolve_framework_id("clap-sys").as_deref(), Some("clap"));
        assert_eq!(resolve_framework_id("lv2-sys").as_deref(), Some("lv2"));
        assert_eq!(resolve_framework_id("lv2").as_deref(), Some("lv2"));
        assert_eq!(resolve_framework_id("serde"), None);
    }

    #[test]
    fn default_migrations_truce_slint() {
        let m = default_migrations(false);
        let t = m.get("truce-slint").expect("migration");
        assert_eq!(t.from, "truce-slint");
        assert_eq!(t.to, "lx-slint-editor");
    }

    #[test]
    fn default_migrations_aura() {
        let m = default_migrations(true);
        let t = m.get("truce-slint").expect("migration");
        assert_eq!(t.from, "truce-slint");
        assert_eq!(t.to, "aura-editor");
    }
}
