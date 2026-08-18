// Style nits from large scan pipeline — correctness still enforced via -D warnings in CI.
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_match)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::redundant_closure)]

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

pub mod agent;
pub mod ast;
pub mod config;
pub mod delta;
pub mod findings;
pub mod guide;
pub mod html;
pub mod notes;
pub mod registry;
pub mod skills;
pub mod tool_hints;

const AGAL_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Default workspace output folder (`agal/`). Override via `agal.toml` `output_dir` or CLI `-o`.
pub const DEFAULT_OUTPUT_DIR: &str = "agal";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Framework {
    id: String,
    name: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    migration_target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyDetail {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub path: String,
    pub frameworks: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub migration_status: Option<String>,
    /// Workspace-internal dependency node ids (path form).
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    pub internal_deps: BTreeSet<String>,
    /// Compact external dep flags (e.g. truce_stack, realfft) — not the full Cargo list.
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    pub external_flags: BTreeSet<String>,
    /// Full Cargo dep names — only when --verbose.
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    pub dependency_names: BTreeSet<String>,
    /// Cargo dependency details (name, version, source hint) for display.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub dependency_details: Vec<DependencyDetail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ast_summary: Option<ast::AstSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationDetail {
    pub from: String,
    pub to: String,
    pub legacy_count: usize,
    pub migrated_count: usize,
    pub legacy_plugins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationSummary {
    pub total_plugins: usize,
    pub total_legacy: usize,
    pub total_migrated: usize,
    pub migrations: BTreeMap<String, MigrationDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Audiolabs {
    pub version: String,
    pub generated_at: String,
    pub project_root: String,
    pub project_name: String,
    /// Framework ids detected on at least one node (deps/imports). Migration
    /// endpoints that are *not* present are only in `migration_summary`, not here.
    #[serde(default)]
    pub used_frameworks: Vec<String>,
    /// Full taxonomy entries for used frameworks only (unless --verbose catalog).
    pub frameworks: Vec<Framework>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub findings: Vec<findings::Finding>,
    /// Findings matched by `[[suppress]]` and omitted from `findings`.
    #[serde(default)]
    pub findings_suppressed: usize,
    pub migration_summary: MigrationSummary,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub rules: BTreeMap<String, String>,
    /// Diff vs previous agal.json (if present at generate time).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub delta: Option<delta::GraphDelta>,
}

#[derive(Debug, Clone, Default)]
pub struct GenerateOptions {
    pub watch_mode: bool,
    pub install_hook: bool,
    pub output_dir_override: Option<String>,
    pub verbose: bool,
    pub agent_only: bool,
    pub plugin_filter: Option<String>,
}

fn parse_cargo_toml(path: &Path) -> Result<toml::Value, String> {
    let mut content = fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
    // toml v1 requires LF line endings; normalize CRLF on Windows.
    content.retain(|c| c != '\r');
    content
        .parse::<toml::Table>()
        .map(toml::Value::Table)
        .map_err(|e| format!("failed to parse {}: {}", path.display(), e))
}

fn extract_string(value: &toml::Value) -> Option<String> {
    value.as_str().map(|s| s.to_string())
}

fn extract_workspace_dependencies(root_cargo: &toml::Value) -> BTreeMap<String, DependencyInfo> {
    let mut map = BTreeMap::new();
    if let Some(ws_deps) = root_cargo
        .get("workspace")
        .and_then(|v| v.as_table())
        .and_then(|t| t.get("dependencies"))
        .and_then(|v| v.as_table())
    {
        for (name, value) in ws_deps {
            map.insert(name.clone(), DependencyInfo::from_value(value));
        }
    }
    map
}

fn extract_dependencies(table: &toml::Table) -> Vec<(String, DependencyInfo)> {
    let mut deps = Vec::new();
    if let Some(deps_table) = table.get("dependencies").and_then(|v| v.as_table()) {
        for (name, value) in deps_table {
            deps.push((name.clone(), DependencyInfo::from_value(value)));
        }
    }
    if let Some(deps_table) = table.get("build-dependencies").and_then(|v| v.as_table()) {
        for (name, value) in deps_table {
            let mut info = DependencyInfo::from_value(value);
            info.kind = "build".to_string();
            deps.push((name.clone(), info));
        }
    }
    if let Some(deps_table) = table.get("dev-dependencies").and_then(|v| v.as_table()) {
        for (name, value) in deps_table {
            let mut info = DependencyInfo::from_value(value);
            info.kind = "dev".to_string();
            deps.push((name.clone(), info));
        }
    }
    deps
}

#[derive(Debug, Clone, Default)]
struct DependencyInfo {
    kind: String,
    #[allow(dead_code)]
    optional: bool,
    path: Option<String>,
    #[allow(dead_code)]
    workspace: bool,
    version: Option<String>,
    git: Option<String>,
}

impl DependencyInfo {
    fn from_value(value: &toml::Value) -> Self {
        let mut info = DependencyInfo::default();
        info.kind = "normal".to_string();

        if let Some(version) = value.as_str() {
            info.version = Some(version.to_string());
        } else if let Some(table) = value.as_table() {
            info.optional = table
                .get("optional")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            info.path = table.get("path").and_then(extract_string);
            info.workspace = table
                .get("workspace")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            info.version = table.get("version").and_then(extract_string);
            info.git = table.get("git").and_then(extract_string);
        }
        info
    }
}

fn discover_members(project_root: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let root_cargo = project_root.join("Cargo.toml");
    let root = parse_cargo_toml(&root_cargo)?;
    let workspace = root
        .get("workspace")
        .and_then(|v| v.as_table())
        .ok_or_else(|| format!("no [workspace] in {}", root_cargo.display()))?;

    let members = workspace
        .get("members")
        .and_then(|v| v.as_array())
        .ok_or_else(|| format!("no workspace.members in {}", root_cargo.display()))?;

    let mut result = Vec::new();
    for member in members {
        let pattern = member
            .as_str()
            .ok_or_else(|| "workspace member is not a string".to_string())?;
        let expanded = glob_member_paths(project_root, pattern)?;
        for (name, path) in expanded {
            result.push((name, path));
        }
    }
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result.dedup_by(|a, b| a.0 == b.0);
    Ok(result)
}

fn glob_member_paths(project_root: &Path, pattern: &str) -> Result<Vec<(String, PathBuf)>, String> {
    let mut result = Vec::new();
    if pattern.ends_with("/*") || pattern.ends_with("/**") {
        let base = &pattern[..pattern.len() - if pattern.ends_with("/**") { 3 } else { 2 }];
        let base_path = project_root.join(base);
        if base_path.exists() {
            for entry in fs::read_dir(&base_path)
                .map_err(|e| format!("failed to read dir {}: {}", base_path.display(), e))?
            {
                let entry = entry.map_err(|e| format!("dir entry error: {}", e))?;
                let path = entry.path();
                if path.is_dir() && path.join("Cargo.toml").exists() {
                    let cargo = parse_cargo_toml(&path.join("Cargo.toml"))?;
                    let name = cargo
                        .get("package")
                        .and_then(|v| v.as_table())
                        .and_then(|t| t.get("name"))
                        .and_then(extract_string)
                        .unwrap_or_else(|| path.file_name().unwrap().to_string_lossy().to_string());
                    result.push((name, path));
                }
            }
        }
    } else {
        let path = project_root.join(pattern);
        if path.join("Cargo.toml").exists() {
            let cargo = parse_cargo_toml(&path.join("Cargo.toml"))?;
            let name = cargo
                .get("package")
                .and_then(|v| v.as_table())
                .and_then(|t| t.get("name"))
                .and_then(extract_string)
                .unwrap_or_else(|| pattern.to_string());
            result.push((name, path));
        }
    }
    Ok(result)
}

fn build_frameworks_from_taxonomy(
    taxonomy: &BTreeMap<String, config::FrameworkSpec>,
    migrations: &BTreeMap<String, config::MigrationSpec>,
    used: &BTreeSet<String>,
    include_all: bool,
) -> Vec<Framework> {
    taxonomy
        .iter()
        .filter(|(id, _)| include_all || used.contains(*id))
        .map(|(id, spec)| Framework {
            id: id.clone(),
            name: spec.name.clone(),
            kind: spec.kind.clone(),
            migration_target: migrations.get(id).map(|m| m.to.clone()),
            notes: spec.notes.clone(),
        })
        .collect()
}

fn compute_migration_status(
    frameworks: &[String],
    migrations: &BTreeMap<String, config::MigrationSpec>,
) -> Option<String> {
    // No migrations configured → don't invent status/noise findings.
    if migrations.is_empty() {
        return None;
    }
    if frameworks.is_empty() {
        return Some("unknown".to_string());
    }

    let mut saw_relevant = false;
    for (from_id, migration) in migrations {
        let to_id = &migration.to;
        let has_from = frameworks.contains(from_id);
        let has_to = frameworks.contains(to_id);
        if has_from || has_to {
            saw_relevant = true;
        }
        if has_to && !has_from {
            return Some("migrated".to_string());
        }
        if has_from && !has_to {
            return Some("legacy".to_string());
        }
        // Both sides present (mixed) — treat as unknown so findings can warn.
        if has_from && has_to {
            return Some("unknown".to_string());
        }
    }

    // Plugin doesn't use any of the migration adapters → no status (not a finding).
    if !saw_relevant {
        return None;
    }
    Some("unknown".to_string())
}

fn auto_detect_project_name(project_root: &Path) -> String {
    let root_cargo_path = project_root.join("Cargo.toml");
    if let Ok(root) = parse_cargo_toml(&root_cargo_path) {
        if let Some(name) = root
            .get("workspace")
            .and_then(|v| v.as_table())
            .and_then(|t| t.get("package"))
            .and_then(|v| v.as_table())
            .and_then(|t| t.get("name"))
            .and_then(extract_string)
        {
            return name;
        }
        if let Some(name) = root
            .get("package")
            .and_then(|v| v.as_table())
            .and_then(|t| t.get("name"))
            .and_then(extract_string)
        {
            return name;
        }
    }
    project_root
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string())
}

fn resolve_dep_path(member_dir: &Path, dep_path: &str) -> Option<PathBuf> {
    let candidate = member_dir.join(dep_path);
    let resolved = candidate.canonicalize().unwrap_or_else(|_| {
        let mut components = Vec::new();
        for c in candidate.components() {
            match c {
                std::path::Component::ParentDir => {
                    components.pop();
                }
                std::path::Component::CurDir => {}
                other => components.push(other),
            }
        }
        components.iter().collect()
    });
    if resolved.is_dir() && resolved.join("Cargo.toml").exists() {
        Some(resolved)
    } else {
        None
    }
}

fn edge_exists(edges: &[Edge], from: &str, to: &str, kind: &str) -> bool {
    edges
        .iter()
        .any(|e| e.from == from && e.to == to && e.kind == kind)
}

fn build_audiolabs(
    project_root: &Path,
    project_config: &config::ProjectConfig,
    verbose: bool,
) -> Result<Audiolabs, String> {
    let members = discover_members(project_root)?;
    let migrations = project_config.migrations.clone();
    let config_internal: HashSet<String> = project_config.internal_crates.iter().cloned().collect();
    let taxonomy = {
        let mut t = config::default_taxonomy();
        t.extend(project_config.frameworks.clone());
        t
    };

    let project_name = project_config
        .project_name
        .clone()
        .unwrap_or_else(|| auto_detect_project_name(project_root));

    let member_paths: HashSet<PathBuf> = members
        .iter()
        .map(|(_, p)| p.canonicalize().unwrap_or_else(|_| p.clone()))
        .collect();

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut all_member_names: HashSet<String> = HashSet::new();

    for (name, path) in &members {
        all_member_names.insert(name.clone());
        let _ = path;
    }

    let name_to_id: BTreeMap<String, String> = members
        .iter()
        .map(|(n, p)| {
            let rp = p
                .strip_prefix(project_root)
                .unwrap_or(p)
                .to_string_lossy()
                .replace('\\', "/");
            (n.clone(), rp)
        })
        .collect();

    // Package name → node id for UI/IPC linking.
    let mut name_to_node_id = name_to_id.clone();

    let mut auto_internal: BTreeMap<PathBuf, String> = BTreeMap::new();

    let root_cargo = parse_cargo_toml(&project_root.join("Cargo.toml"))?;
    let workspace_deps = extract_workspace_dependencies(&root_cargo);

    let analyze_opts = ast::AnalyzeOptions {
        include_symbols: verbose,
    };

    for (name, path) in &members {
        let cargo_path = path.join("Cargo.toml");
        let cargo = parse_cargo_toml(&cargo_path)?;
        let package = cargo
            .get("package")
            .and_then(|v| v.as_table())
            .ok_or_else(|| format!("no [package] in {}", cargo_path.display()))?;

        let description = package.get("description").and_then(extract_string);
        let version = package.get("version").and_then(extract_string);
        let package_name = package
            .get("name")
            .and_then(extract_string)
            .unwrap_or_else(|| name.clone());

        let relative_path = path
            .strip_prefix(project_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        let kind = if relative_path.starts_with("plugins/") {
            "plugin"
        } else if relative_path.starts_with("crates/") {
            "crate"
        } else {
            "member"
        };

        let deps = cargo
            .as_table()
            .map(|t| extract_dependencies(t))
            .unwrap_or_default();

        let mut dep_names = BTreeSet::new();
        let mut internal_deps = BTreeSet::new();
        let mut dependency_details = Vec::new();

        for (dep_name, info) in &deps {
            dep_names.insert(dep_name.clone());

            let source = if info.workspace {
                Some("workspace".to_string())
            } else if info.git.is_some() {
                Some("git".to_string())
            } else if info.path.is_some() {
                Some("path".to_string())
            } else {
                None
            };
            let resolved_version = if info.workspace {
                workspace_deps
                    .get(dep_name)
                    .and_then(|d| d.version.clone())
                    .or_else(|| info.version.clone())
            } else {
                info.version.clone()
            };
            dependency_details.push(DependencyDetail {
                name: dep_name.clone(),
                version: resolved_version,
                source,
            });

            let resolved_internal = info
                .path
                .as_ref()
                .and_then(|p| resolve_dep_path(path, p))
                .and_then(|resolved| {
                    if member_paths.contains(&resolved) {
                        None
                    } else {
                        let pkg_name = parse_cargo_toml(&resolved.join("Cargo.toml"))
                            .ok()
                            .and_then(|c| {
                                c.get("package")
                                    .and_then(|v| v.as_table())
                                    .and_then(|t| t.get("name"))
                                    .and_then(extract_string)
                            })
                            .unwrap_or_else(|| dep_name.clone());
                        auto_internal.insert(resolved, pkg_name.clone());
                        Some(pkg_name)
                    }
                });

            let effective_dep_name = resolved_internal.as_deref().unwrap_or(dep_name);
            let is_internal = all_member_names.contains(effective_dep_name)
                || config_internal.contains(effective_dep_name)
                || auto_internal.values().any(|n| n == effective_dep_name);

            if is_internal {
                if let Some(target_id) = name_to_id.get(effective_dep_name) {
                    let edge_kind = match info.kind.as_str() {
                        "build" => "build_depends_on",
                        "dev" => "dev_depends_on",
                        _ => "depends_on",
                    };
                    if !edge_exists(&edges, &relative_path, target_id, edge_kind) {
                        edges.push(Edge {
                            from: relative_path.clone(),
                            to: target_id.clone(),
                            kind: edge_kind.to_string(),
                            feature: None,
                            note: None,
                        });
                    }
                    if info.kind == "normal" || info.kind.is_empty() {
                        internal_deps.insert(target_id.clone());
                    } else if info.kind == "build" {
                        // keep build deps visible too
                        internal_deps.insert(target_id.clone());
                    }
                }
            }
        }

        let mut ast_summary = ast::analyze_crate_with_options(path, analyze_opts);

        // Extract [features] from Cargo.toml
        if let Some(features_table) = cargo.get("features").and_then(|v| v.as_table()) {
            for (feat_name, feat_val) in features_table {
                let deps: Vec<String> = feat_val
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                ast_summary.features.insert(feat_name.clone(), deps);
            }
        }

        let imported_crates = ast_summary.imported_crates.clone();

        let mut detected_frameworks =
            config::detect_framework_ids(&taxonomy, &dep_names, &imported_crates);

        if taxonomy.contains_key(&package_name) && !detected_frameworks.contains(&package_name) {
            detected_frameworks.insert(package_name.clone());
        }

        let mut frameworks_vec: Vec<String> = detected_frameworks.into_iter().collect();
        frameworks_vec.sort();

        // Detect plugin export formats
        if kind == "plugin" {
            let feature_keys: BTreeSet<String> = ast_summary.features.keys().cloned().collect();
            let fw_lower: BTreeSet<String> =
                frameworks_vec.iter().map(|f| f.to_lowercase()).collect();

            if fw_lower.contains("truce")
                || feature_keys.contains("clap")
                || fw_lower.contains("clap")
                || fw_lower.contains("clack")
            {
                ast_summary.plugin_formats.insert("CLAP".into());
            }
            if feature_keys.contains("vst3") || fw_lower.contains("vst3") {
                ast_summary.plugin_formats.insert("VST3".into());
            }
            if feature_keys.contains("lv2") || fw_lower.contains("lv2") {
                ast_summary.plugin_formats.insert("LV2".into());
            }
            // nih-plug style feature-gated exports
            if feature_keys.contains("nih_export_vst3") || feature_keys.contains("nih-export-vst3")
            {
                ast_summary.plugin_formats.insert("VST3".into());
            }
            if feature_keys.contains("nih_export_clap") || feature_keys.contains("nih-export-clap")
            {
                ast_summary.plugin_formats.insert("CLAP".into());
            }
        }

        let migration_status = if kind == "plugin" {
            compute_migration_status(&frameworks_vec, &migrations)
        } else {
            None
        };

        dependency_details.sort_by(|a, b| a.name.cmp(&b.name));

        // Internal name set for external flag collapse.
        let internal_names: BTreeSet<String> = all_member_names.iter().cloned().collect();
        let external_flags = config::summarize_external_deps(&dep_names, &internal_names);

        name_to_node_id.insert(package_name.clone(), relative_path.clone());

        nodes.push(Node {
            id: relative_path.clone(),
            name: package_name,
            kind: kind.to_string(),
            description,
            version,
            path: relative_path,
            frameworks: frameworks_vec,
            migration_status,
            internal_deps,
            external_flags,
            dependency_names: if verbose { dep_names } else { BTreeSet::new() },
            dependency_details,
            ast_summary: Some(ast_summary),
        });
    }

    // --- Semantic edges: uses_ui ---
    let ui_crate_ids: Vec<String> = {
        let mut ids = Vec::new();
        for n in &nodes {
            let is_ui = project_config.ui_crates.iter().any(|c| c == &n.name)
                || n.ast_summary
                    .as_ref()
                    .map(|a| !a.slint_exports.is_empty())
                    .unwrap_or(false);
            if is_ui {
                ids.push(n.id.clone());
            }
        }
        ids
    };

    let ui_exports: BTreeMap<String, BTreeSet<String>> = nodes
        .iter()
        .filter(|n| ui_crate_ids.contains(&n.id))
        .map(|n| {
            (
                n.id.clone(),
                n.ast_summary
                    .as_ref()
                    .map(|a| a.slint_exports.clone())
                    .unwrap_or_default(),
            )
        })
        .collect();

    for n in &nodes {
        if n.kind != "plugin" {
            continue;
        }
        let Some(ast) = &n.ast_summary else { continue };
        if ast.slint_components.is_empty() {
            continue;
        }
        for (ui_id, exports) in &ui_exports {
            if ui_id == &n.id {
                continue;
            }
            let overlap = ast.slint_components.intersection(exports).count();
            if overlap == 0 {
                continue;
            }
            if !edge_exists(&edges, &n.id, ui_id, "uses_ui")
                && !edge_exists(&edges, &n.id, ui_id, "depends_on")
            {
                edges.push(Edge {
                    from: n.id.clone(),
                    to: ui_id.clone(),
                    kind: "uses_ui".to_string(),
                    feature: None,
                    note: Some(format!("{} shared Lx* components", overlap)),
                });
            } else if edge_exists(&edges, &n.id, ui_id, "depends_on")
                && !edge_exists(&edges, &n.id, ui_id, "uses_ui")
            {
                // Cargo dep already present; still add semantic uses_ui for clarity.
                edges.push(Edge {
                    from: n.id.clone(),
                    to: ui_id.clone(),
                    kind: "uses_ui".to_string(),
                    feature: None,
                    note: Some(format!("{} shared Lx* components", overlap)),
                });
            }
        }
    }

    // --- Semantic edges: ipc_peer + runtime_depends_on lx-shm ---
    let ipc_hub_names: HashSet<String> = {
        let mut s: HashSet<String> = project_config.ipc_hubs.iter().cloned().collect();
        if s.is_empty() {
            s.insert("lx-shm".to_string());
            s.insert("lx-analysis".to_string());
        }
        s
    };

    let shm_node_id = name_to_node_id.get("lx-shm").cloned();
    let analysis_node_id = name_to_node_id.get("lx-analysis").cloned();

    // Strong IPC signals only for peer/runtime edges (SharedState alone is UI-local everywhere).
    fn strong_ipc(sigs: &BTreeSet<String>) -> BTreeSet<String> {
        sigs.iter()
            .filter(|s| matches!(s.as_str(), "shm" | "relay" | "seqlock"))
            .cloned()
            .collect()
    }

    let ipc_plugins: Vec<(String, BTreeSet<String>)> = nodes
        .iter()
        .filter(|n| n.kind == "plugin")
        .filter_map(|n| {
            let sigs = strong_ipc(&n.ast_summary.as_ref()?.ipc_signals);
            if sigs.is_empty() {
                None
            } else {
                Some((n.id.clone(), sigs))
            }
        })
        .collect();

    for (pid, sigs) in &ipc_plugins {
        // runtime edge to lx-shm when plugin uses shm/relay but may only depend via lx-analysis
        if let Some(ref shm) = shm_node_id {
            if !edge_exists(&edges, pid, shm, "depends_on")
                && !edge_exists(&edges, pid, shm, "runtime_depends_on")
            {
                edges.push(Edge {
                    from: pid.clone(),
                    to: shm.clone(),
                    kind: "runtime_depends_on".to_string(),
                    feature: None,
                    note: Some(format!(
                        "via {}",
                        sigs.iter().cloned().collect::<Vec<_>>().join("+")
                    )),
                });
            }
        }
        let _ = &ipc_hub_names;
        let _ = &analysis_node_id;
    }

    // Peer edges between plugins that share strong IPC signals.
    for i in 0..ipc_plugins.len() {
        for j in (i + 1)..ipc_plugins.len() {
            let (a, sa) = &ipc_plugins[i];
            let (b, sb) = &ipc_plugins[j];
            let shared: Vec<&String> = sa.intersection(sb).collect();
            if shared.is_empty() {
                continue;
            }
            if !edge_exists(&edges, a, b, "ipc_peer") && !edge_exists(&edges, b, a, "ipc_peer") {
                edges.push(Edge {
                    from: a.clone(),
                    to: b.clone(),
                    kind: "ipc_peer".to_string(),
                    feature: None,
                    note: Some(format!(
                        "shared: {}",
                        shared
                            .iter()
                            .map(|s| s.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )),
                });
            }
        }
    }

    // Migration summary
    let plugin_nodes: Vec<&Node> = nodes.iter().filter(|n| n.kind == "plugin").collect();
    let total_plugins = plugin_nodes.len();
    let mut migration_details = BTreeMap::new();
    let mut total_legacy = 0usize;
    let mut total_migrated = 0usize;

    for (migration_id, migration_spec) in &migrations {
        let legacy_plugins: Vec<String> = plugin_nodes
            .iter()
            .filter(|n| {
                let fw = &n.frameworks;
                fw.contains(&migration_spec.from) && !fw.contains(&migration_spec.to)
            })
            .map(|n| n.name.clone())
            .collect();
        let migrated_count = plugin_nodes
            .iter()
            .filter(|n| {
                let fw = &n.frameworks;
                fw.contains(&migration_spec.to) && !fw.contains(&migration_spec.from)
            })
            .count();
        total_legacy += legacy_plugins.len();
        total_migrated += migrated_count;
        migration_details.insert(
            migration_id.clone(),
            MigrationDetail {
                from: migration_spec.from.clone(),
                to: migration_spec.to.clone(),
                legacy_count: legacy_plugins.len(),
                migrated_count,
                legacy_plugins,
            },
        );
    }

    // Frameworks actually detected on nodes (not migration config endpoints).
    let mut used: BTreeSet<String> = BTreeSet::new();
    for n in &nodes {
        for f in &n.frameworks {
            used.insert(f.clone());
        }
    }

    let frameworks = build_frameworks_from_taxonomy(&taxonomy, &migrations, &used, verbose);
    let used_frameworks: Vec<String> = used.iter().cloned().collect();

    let mut raw_findings = findings::analyze(project_root, &nodes, &edges, &project_config.rules);
    tool_hints::append_hints(project_root, &nodes, &mut raw_findings);

    let (findings, findings_suppressed) =
        findings::apply_suppressions(raw_findings, &project_config.suppress);

    // Stable edge order
    edges.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| a.from.cmp(&b.from))
            .then_with(|| a.to.cmp(&b.to))
    });

    Ok(Audiolabs {
        version: AGAL_VERSION.to_string(),
        generated_at: now_rfc3339(),
        project_root: clean_project_root(project_root),
        project_name,
        used_frameworks,
        frameworks,
        nodes,
        edges,
        findings,
        findings_suppressed,
        migration_summary: MigrationSummary {
            total_plugins,
            total_legacy,
            total_migrated,
            migrations: migration_details,
        },
        rules: project_config.rules.clone(),
        delta: None, // filled in generate() after loading previous JSON
    })
}

/// Rank and cap API surfaces after the full graph is known.
///
/// - Plugins get no public API strip (their surface is plugin hooks / params, already noted).
/// - Crates are sorted by symbol kind priority and capped per `[map]` config.
/// - `cfg.rank` is reserved (`inbound_deps` not applied yet); kind priority only.
fn rank_and_cap_api_surfaces(graph: &mut Audiolabs, cfg: &config::MapConfig) {
    // First pass: per-crate cap + total count.
    let mut max_per_crate = cfg.max_symbols_per_crate;
    for n in &mut graph.nodes {
        let Some(ast) = n.ast_summary.as_mut() else {
            continue;
        };
        if n.kind == "plugin" {
            ast.public_api.clear();
            ast.api_surface.clear();
            ast.api_surface_total = 0;
            continue;
        }
        // Keep the legacy name set capped for JSON size.
        if ast.public_api.len() > 80 {
            let truncated: BTreeSet<String> = ast.public_api.iter().take(80).cloned().collect();
            ast.public_api = truncated;
        }
        sort_api_surface(ast);
        ast.api_surface_total = ast.api_surface.len();
        if ast.api_surface.len() > max_per_crate {
            ast.api_surface.truncate(max_per_crate);
        }
    }

    // Second pass: global approximate token cap (char budget = tokens * 4).
    // If over budget, lower the per-crate limit uniformly until it fits or hits a floor.
    let budget_chars = cfg.api_tokens.saturating_mul(4);
    loop {
        let total_chars: usize = graph
            .nodes
            .iter()
            .filter_map(|n| n.ast_summary.as_ref())
            .map(|a| {
                a.api_surface
                    .iter()
                    .map(|s| s.signature.len())
                    .sum::<usize>()
            })
            .sum();
        if total_chars <= budget_chars || max_per_crate <= 3 {
            break;
        }
        max_per_crate -= 1;
        for n in &mut graph.nodes {
            let Some(ast) = n.ast_summary.as_mut() else {
                continue;
            };
            if ast.api_surface_total > max_per_crate {
                ast.api_surface.truncate(max_per_crate);
            }
        }
    }
}

fn sort_api_surface(ast: &mut ast::AstSummary) {
    ast.api_surface.sort_by(|a, b| {
        api_symbol_priority(a)
            .cmp(&api_symbol_priority(b))
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| a.line.cmp(&b.line))
            .then_with(|| a.name.cmp(&b.name))
    });
}

/// Kind priority for API strip: hooks → traits/types → free fns → impl/mod noise.
fn api_symbol_priority(s: &ast::ApiSymbol) -> u8 {
    match s.kind.as_str() {
        "fn" if s.name == "process" => 0,
        "fn" if s.name == "editor" => 1,
        "trait" => 2,
        "struct" => 3,
        "enum" => 4,
        "type" => 5,
        "fn" => 6,
        "const" => 7,
        "static" => 8,
        "impl" => 9,
        "mod" => 10,
        _ => 11,
    }
}

fn now_rfc3339() -> String {
    let now = std::time::SystemTime::now();
    let duration = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    let days = secs / 86400;
    let rem = secs % 86400;
    let hours = rem / 3600;
    let mins = (rem % 3600) / 60;
    let seconds = rem % 60;

    let (year, month, day) = unix_days_to_ymd(days as i64);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, mins, seconds
    )
}

fn unix_days_to_ymd(days: i64) -> (i64, i64, i64) {
    let mut y = 1970;
    let mut d = days;
    loop {
        let ydays = if is_leap_year(y) { 366 } else { 365 };
        if d >= ydays {
            d -= ydays;
            y += 1;
        } else {
            break;
        }
    }
    let month_days = if is_leap_year(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 1;
    for (i, md) in month_days.iter().enumerate() {
        if d < *md as i64 {
            m = (i + 1) as i64;
            break;
        }
        d -= *md as i64;
        m = (i + 2) as i64;
    }
    (y, m, d + 1)
}

fn is_leap_year(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

fn clean_project_root(path: &Path) -> String {
    let mut s = path.to_string_lossy().replace('\\', "/");
    if s.starts_with("//?/") {
        s = s[4..].to_string();
    }
    s
}

fn install_git_hook(project_root: &Path) -> Result<(), String> {
    let hook_path = project_root.join(".git/hooks/post-commit");
    let parent = hook_path.parent().unwrap();
    fs::create_dir_all(parent)
        .map_err(|e| format!("cannot create hooks dir {}: {}", parent.display(), e))?;

    let hook_content = format!(
        "#!/bin/sh\n# Auto-generated by agal install-hook\nagal \"{}\"\n",
        clean_project_root(project_root)
    );

    fs::write(&hook_path, hook_content)
        .map_err(|e| format!("cannot write hook {}: {}", hook_path.display(), e))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&hook_path)
            .map_err(|e| format!("cannot stat hook {}: {}", hook_path.display(), e))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&hook_path, perms)
            .map_err(|e| format!("cannot chmod hook {}: {}", hook_path.display(), e))?;
    }

    println!("installed post-commit hook at {}", hook_path.display());
    Ok(())
}

fn watch_and_regenerate(project_root: &Path, options: &GenerateOptions) -> Result<(), String> {
    use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc::channel;
    use std::time::{Duration, Instant};

    let (tx, rx) = channel::<notify::Result<Event>>();
    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = tx.send(res);
        },
        Config::default(),
    )
    .map_err(|e| format!("failed to create watcher: {}", e))?;

    watcher
        .watch(project_root, RecursiveMode::Recursive)
        .map_err(|e| format!("failed to watch {}: {}", project_root.display(), e))?;

    println!(
        "watching {} for changes; regenerate on .rs/Cargo.toml/slint changes (Ctrl-C to stop)",
        clean_project_root(project_root)
    );

    let mut last_event: Option<Instant> = None;
    let debounce = Duration::from_millis(500);

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(event)) => {
                if is_interesting_event(&event) {
                    last_event = Some(Instant::now());
                }
            }
            Ok(Err(e)) => {
                eprintln!("watch error: {}", e);
            }
            Err(_) => {}
        }

        if let Some(t) = last_event {
            if t.elapsed() >= debounce {
                last_event = None;
                if let Err(e) = generate(project_root, options) {
                    eprintln!("regeneration failed: {}", e);
                }
            }
        }
    }
}

fn is_interesting_event(event: &notify::Event) -> bool {
    event.paths.iter().any(|p| {
        p.extension()
            .and_then(|s| s.to_str())
            .map(|ext| ext == "rs" || ext == "toml" || ext == "slint")
            .unwrap_or(false)
    }) && !matches!(event.kind, notify::EventKind::Access(_))
}

fn generate_one_shot(project_root: &Path, options: &GenerateOptions) -> Result<(), String> {
    let project_config = config::ProjectConfig::load(project_root);
    let mut graph = build_audiolabs(project_root, &project_config, options.verbose)?;
    rank_and_cap_api_surfaces(&mut graph, &project_config.map);

    let output_dir = options
        .output_dir_override
        .clone()
        .or_else(|| project_config.output_dir.clone())
        .unwrap_or_else(|| DEFAULT_OUTPUT_DIR.to_string());

    let output_path = project_root.join(&output_dir);
    fs::create_dir_all(&output_path)
        .map_err(|e| format!("cannot create directory {}: {}", output_path.display(), e))?;

    // Delta vs previous JSON (load before overwrite).
    let json_path = output_path.join("agal.json");
    let previous = delta::load_previous(&json_path);
    // Strip nested delta from previous so we don't recursively store deltas.
    let previous = previous.map(|mut g| {
        g.delta = None;
        g
    });
    let d = delta::compute(previous.as_ref(), &graph);
    graph.delta = Some(d.clone());

    // JSON: slim by default (HTML/notes still see full node dep details).
    let json_graph = if options.verbose {
        graph.clone()
    } else {
        let mut g = graph.clone();
        for n in &mut g.nodes {
            n.dependency_details.clear();
            n.dependency_names.clear();
        }
        g
    };
    let json = serde_json::to_string_pretty(&json_graph)
        .map_err(|e| format!("failed to serialize graph: {}", e))?;
    fs::write(&json_path, &json)
        .map_err(|e| format!("failed to write {}: {}", json_path.display(), e))?;

    // Structural agent map + agal-owned orientation entry (AGAL.md).
    let skills_dir = output_path.join("skills");
    let agent_md = agent::render_agent_md(
        &graph,
        if skills_dir.is_dir() {
            Some(skills_dir.as_path())
        } else {
            None
        },
    );
    let agent_path = output_path.join("agal.agent.md");
    fs::write(&agent_path, &agent_md)
        .map_err(|e| format!("failed to write {}: {}", agent_path.display(), e))?;
    agent::write_agal_md(&output_path, Some(&graph), &output_dir)?;

    // Delta markdown
    let delta_md = delta::render_md(&d);
    let delta_path = output_path.join("agal.delta.md");
    fs::write(&delta_path, &delta_md)
        .map_err(|e| format!("failed to write {}: {}", delta_path.display(), e))?;

    // Hybrid notes (auto header + preserved human body) — not skills (use `agal skills sync`).
    let notes_n = notes::write_notes(&output_path, &graph)?;

    // Folder guide for humans (Obsidian / VS Code) — CLI cheatsheet in one place.
    guide::write_readme(&output_path, &graph, &output_dir)?;

    // Plugin slices: write --plugin target; also refresh any existing *.slice.json
    // so plain `agal .` does not leave stale one-hop files around.
    let mut slice_names: BTreeSet<String> = BTreeSet::new();
    if let Some(ref name) = options.plugin_filter {
        slice_names.insert(name.clone());
    }
    if let Ok(entries) = fs::read_dir(&output_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(fname) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if let Some(stem) = fname.strip_suffix(".slice.json") {
                if !stem.is_empty() {
                    slice_names.insert(stem.to_string());
                }
            }
        }
    }
    for name in &slice_names {
        match agent::plugin_slice(&graph, name) {
            Some(slice) => {
                let slice_path = output_path.join(format!("{}.slice.json", name));
                let body = serde_json::to_string_pretty(&slice)
                    .map_err(|e| format!("failed to serialize slice: {}", e))?;
                fs::write(&slice_path, body)
                    .map_err(|e| format!("failed to write {}: {}", slice_path.display(), e))?;
                println!("  wrote plugin slice {}", slice_path.display());
            }
            None => {
                if options.plugin_filter.as_deref() == Some(name.as_str()) {
                    eprintln!("warning: plugin '{}' not found in graph", name);
                }
                // Orphan slice for removed plugin: leave file (user may delete).
            }
        }
    }

    if !options.agent_only {
        // HTML uses config output_dir; pass override by temp-muting config
        let mut html_config = project_config.clone();
        if options.output_dir_override.is_some() {
            html_config.output_dir = options.output_dir_override.clone();
        }
        html::write_html(
            project_root,
            &graph.nodes,
            &graph.edges,
            &graph.frameworks,
            &html_config,
            &graph.findings,
            &html::HtmlMeta {
                project_name: &graph.project_name,
                generated_at: &graph.generated_at,
                graph_version: &graph.version,
                view_default: project_config.view.default.as_deref(),
            },
        )?;
    }

    let display_root = clean_project_root(project_root);
    println!(
        "agal v{} generated for {} in {}/{}",
        AGAL_VERSION, graph.project_name, display_root, output_dir
    );
    println!(
        "  {} nodes, {} edges, {} plugins, {} findings",
        graph.nodes.len(),
        graph.edges.len(),
        graph.migration_summary.total_plugins,
        graph.findings.len(),
    );
    let counts = findings::count_by_severity(&graph.findings);
    let health = findings::health(&graph.findings);
    println!("  health: {}", health);
    if !counts.is_empty() {
        println!(
            "  findings: error={} warn={} info={}",
            counts.get("error").copied().unwrap_or(0),
            counts.get("warn").copied().unwrap_or(0),
            counts.get("info").copied().unwrap_or(0),
        );
    }
    if graph.findings_suppressed > 0 || !project_config.suppress.is_empty() {
        println!(
            "  suppress: {} finding(s) muted ({} rule(s) in config)",
            graph.findings_suppressed,
            project_config.suppress.len()
        );
    }
    if graph.migration_summary.total_legacy > 0 {
        for (migration_id, detail) in &graph.migration_summary.migrations {
            if detail.legacy_count > 0 {
                println!(
                    "  {}: {} legacy ({}), {} migrated ({})",
                    migration_id,
                    detail.legacy_count,
                    detail.from,
                    detail.migrated_count,
                    detail.to,
                );
                for p in &detail.legacy_plugins {
                    println!("    - {}", p);
                }
            }
        }
    } else if graph.migration_summary.total_migrated > 0 {
        println!(
            "  migrations: complete ({} plugins on target adapters)",
            graph.migration_summary.total_migrated
        );
    }
    println!("  agal: {}/AGAL.md", output_dir);
    println!("  agent map: {}/agal.agent.md", output_dir);
    println!("  notes: {}/notes/ ({} files)", output_dir, notes_n);
    println!("  guide: {}/Cheatsheet.md", output_dir);
    println!("  skills: not auto-copied — run `agal skills sync` (default: core)");
    if let Some(ref d) = graph.delta {
        if d.first_run {
            println!("  delta: first run (no previous graph)");
        } else if delta::is_empty(d) {
            println!("  delta: no structural changes");
        } else {
            println!(
                "  delta: +{} nodes -{} nodes ~{} nodes · +{} edges -{} edges · +{} findings -{} resolved",
                d.added_nodes.len(),
                d.removed_nodes.len(),
                d.changed_nodes.len(),
                d.added_edges.len(),
                d.removed_edges.len(),
                d.new_findings.len(),
                d.resolved_findings.len(),
            );
        }
    }
    Ok(())
}

/// Scan workspace into an in-memory graph (no file writes). Useful for tests and `agal doctor`.
pub fn scan(project_root: &Path, verbose: bool) -> Result<Audiolabs, String> {
    let project_config = config::ProjectConfig::load(project_root);
    let mut graph = build_audiolabs(project_root, &project_config, verbose)?;
    rank_and_cap_api_surfaces(&mut graph, &project_config.map);
    Ok(graph)
}

/// External tool checklist (Clippy / clap-validator) + graph-aware hints.
pub fn doctor(project_root: &Path) -> Result<String, String> {
    let graph = scan(project_root, false)?;
    let status = tool_hints::probe_tools();
    Ok(tool_hints::render_doctor(
        project_root,
        &graph.nodes,
        &status,
    ))
}

/// Find a node by name, id, or suffix (e.g. `aether` matches `plugins/aether`).
fn resolve_node<'g>(graph: &'g Audiolabs, name: &str) -> Option<&'g Node> {
    graph
        .nodes
        .iter()
        .find(|n| n.name == name || n.id == name || n.id.ends_with(&format!("/{}", name)))
}

/// Reverse-dependency report: who depends on / uses the given node.
pub fn impact_report(project_root: &Path, name: &str) -> Result<String, String> {
    let graph = scan(project_root, false)?;
    let node = resolve_node(&graph, name)
        .ok_or_else(|| format!("node '{}' not found in workspace", name))?;
    let id = &node.id;

    let inbound: Vec<&Edge> = graph.edges.iter().filter(|e| e.to == *id).collect();
    let direct: Vec<&Edge> = inbound
        .iter()
        .copied()
        .filter(|e| e.kind == "depends_on")
        .collect();
    let semantic: Vec<&Edge> = inbound
        .iter()
        .copied()
        .filter(|e| e.kind != "depends_on")
        .collect();

    let mut s = String::new();
    let _ = writeln!(s, "# agal impact: {}", node.name);
    let _ = writeln!(s, "**id:** `{}`  ", id);
    let _ = writeln!(s, "**kind:** `{}`  ", node.kind);
    let _ = writeln!(s, "**path:** `{}`  ", node.path);
    let _ = writeln!(s, "**total inbound edges:** {}\n", inbound.len());

    if !direct.is_empty() {
        let _ = writeln!(s, "## direct cargo dependencies");
        for e in direct {
            let note = e
                .note
                .as_deref()
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
        let _ = writeln!(s);
    }

    if !semantic.is_empty() {
        let _ = writeln!(s, "## semantic edges");
        for e in semantic {
            let note = e
                .note
                .as_deref()
                .map(|n| format!(" — {}", n))
                .unwrap_or_default();
            let _ = writeln!(
                s,
                "- `{}` --[{}]→ `{}`{}",
                short_id(&e.from),
                e.kind,
                short_id(&e.to),
                note
            );
        }
        let _ = writeln!(s);
    }

    if inbound.is_empty() {
        let _ = writeln!(
            s,
            "_no inbound edges — this node is not referenced by any workspace member_"
        );
    }

    let related_findings: Vec<&findings::Finding> = graph
        .findings
        .iter()
        .filter(|f| f.node.as_deref() == Some(id.as_str()))
        .collect();
    if !related_findings.is_empty() {
        let _ = writeln!(s, "## findings");
        for f in related_findings {
            let _ = writeln!(s, "- **[{}]** `{}`: {}", f.severity, f.code, f.message);
        }
    }

    Ok(s)
}

/// Options for `context_pack`.
#[derive(Debug, Clone, Default)]
pub struct ContextPackOptions {
    pub focus: String,
    pub budget_tokens: usize,
    pub format: ContextPackFormat,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ContextPackFormat {
    #[default]
    Markdown,
    Json,
}

impl ContextPackFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_ascii_lowercase().as_str() {
            "md" | "markdown" => Ok(Self::Markdown),
            "json" => Ok(Self::Json),
            other => Err(format!("unknown context format '{}'; use md|json", other)),
        }
    }
}

/// Build a focused, token-budgeted context pack for one node.
pub fn context_pack(project_root: &Path, opts: &ContextPackOptions) -> Result<String, String> {
    let graph = scan(project_root, false)?;
    let node = resolve_node(&graph, &opts.focus)
        .ok_or_else(|| format!("node '{}' not found in workspace", opts.focus))?;
    let id = &node.id;

    let related_edges: Vec<&Edge> = graph
        .edges
        .iter()
        .filter(|e| e.from == *id || e.to == *id)
        .collect();
    let neighbor_ids: BTreeSet<&str> = related_edges
        .iter()
        .flat_map(|e| [e.from.as_str(), e.to.as_str()])
        .filter(|n| *n != id.as_str())
        .collect();
    let neighbors: Vec<&Node> = graph
        .nodes
        .iter()
        .filter(|n| neighbor_ids.contains(n.id.as_str()))
        .collect();
    let findings: Vec<&findings::Finding> = graph
        .findings
        .iter()
        .filter(|f| f.node.as_deref() == Some(id.as_str()))
        .collect();

    match opts.format {
        ContextPackFormat::Json => {
            let value = serde_json::json!({
                "focus": node,
                "neighbors": neighbors,
                "edges": related_edges,
                "findings": findings,
                "budget_tokens": opts.budget_tokens,
            });
            serde_json::to_string_pretty(&value)
                .map_err(|e| format!("json serialization failed: {}", e))
        }
        ContextPackFormat::Markdown => Ok(render_context_markdown(
            &graph,
            node,
            &neighbors,
            &related_edges,
            &findings,
            opts.budget_tokens,
        )),
    }
}

fn render_context_markdown(
    graph: &Audiolabs,
    node: &Node,
    neighbors: &[&Node],
    edges: &[&Edge],
    findings: &[&findings::Finding],
    budget_tokens: usize,
) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    let _ = writeln!(s, "# agal context pack: {}", node.name);
    let _ = writeln!(s, "**budget:** ~{} tokens  ", budget_tokens);
    let _ = writeln!(s, "**focus:** `{}` (`{}`)  ", node.id, node.kind);
    if !node.frameworks.is_empty() {
        let _ = writeln!(s, "**frameworks:** {}  ", node.frameworks.join(", "));
    }
    if let Some(m) = &node.migration_status {
        let _ = writeln!(s, "**migration:** {}  ", m);
    }
    let _ = writeln!(s);

    // API surface (already capped by scan).
    if let Some(ast) = &node.ast_summary {
        if !ast.api_surface.is_empty() {
            let _ = writeln!(s, "## public API surface (top symbols)");
            for sym in &ast.api_surface {
                let _ = writeln!(s, "- `{}:{}` — `{}`", sym.file, sym.line, sym.signature);
            }
            if ast.api_surface_total > ast.api_surface.len() {
                let _ = writeln!(
                    s,
                    "- _… and {} more symbols_",
                    ast.api_surface_total - ast.api_surface.len()
                );
            }
            let _ = writeln!(s);
        }
    }

    if !edges.is_empty() {
        let _ = writeln!(s, "## edges (1-hop)");
        for kind in [
            "depends_on",
            "uses_ui",
            "ipc_peer",
            "runtime_depends_on",
            "build_depends_on",
            "dev_depends_on",
        ] {
            let group: Vec<&&Edge> = edges.iter().filter(|e| e.kind == kind).collect();
            if group.is_empty() {
                continue;
            }
            let _ = writeln!(s, "### {}", kind);
            for e in group {
                let note = e
                    .note
                    .as_deref()
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
    }

    if !neighbors.is_empty() {
        let _ = writeln!(s, "## neighbors");
        for n in neighbors {
            let _ = writeln!(
                s,
                "- `{}` (`{}`) — frameworks: {}",
                n.name,
                n.kind,
                n.frameworks.join(", ")
            );
        }
        let _ = writeln!(s);
    }

    if !findings.is_empty() {
        let _ = writeln!(s, "## findings");
        for f in findings {
            let fix = f
                .fix
                .as_deref()
                .map(|x| format!(" — fix: {}", x))
                .unwrap_or_default();
            let _ = writeln!(
                s,
                "- **[{}]** `{}`: {}{}",
                f.severity, f.code, f.message, fix
            );
        }
        let _ = writeln!(s);
    }

    let _ = writeln!(s, "## workspace summary");
    let _ = writeln!(
        s,
        "- nodes: {} · edges: {} · findings: {}",
        graph.nodes.len(),
        graph.edges.len(),
        graph.findings.len()
    );
    let _ = writeln!(s, "- generated: `{}`", graph.generated_at);

    s
}

fn short_id(id: &str) -> &str {
    id.rsplit('/').next().unwrap_or(id)
}

/// Public entry point used by the `agal` CLI.
pub fn generate(project_root: &Path, options: &GenerateOptions) -> Result<(), String> {
    if options.install_hook {
        return install_git_hook(project_root);
    }

    if options.watch_mode {
        return watch_and_regenerate(project_root, options);
    }

    generate_one_shot(project_root, options)
}
