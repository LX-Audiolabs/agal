#![allow(clippy::collapsible_if)]

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::Edge;
use crate::Node;
use crate::config::SuppressRule;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warn,
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warn => write!(f, "warn"),
            Severity::Info => write!(f, "info"),
        }
    }
}

/// Project health from findings (Projucer-style gate).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    /// No error/warn.
    Ok,
    /// Warnings only.
    Degraded,
    /// Any error.
    Blocked,
}

impl Health {
    pub fn as_str(self) -> &'static str {
        match self {
            Health::Ok => "ok",
            Health::Degraded => "degraded",
            Health::Blocked => "blocked",
        }
    }
}

impl std::fmt::Display for Health {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Finding {
    pub severity: Severity,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    pub message: String,
    /// Relative path for navigation (crate root, Cargo.toml, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Short actionable remediation (Projucer-style).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

impl Finding {
    pub(crate) fn new(
        severity: Severity,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            node: None,
            message: message.into(),
            path: None,
            fix: None,
        }
    }

    pub(crate) fn at_node(mut self, n: &Node) -> Self {
        self.node = Some(n.id.clone());
        self.path = Some(n.path.clone());
        self
    }

    pub(crate) fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub(crate) fn with_fix(mut self, fix: impl Into<String>) -> Self {
        self.fix = Some(fix.into());
        self
    }
}

/// Stable key for delta matching (severity+code+node).
pub fn finding_key(f: &Finding) -> String {
    format!(
        "{}|{}|{}",
        f.severity,
        f.code,
        f.node.as_deref().unwrap_or("")
    )
}

pub fn health(findings: &[Finding]) -> Health {
    if findings.iter().any(|f| f.severity == Severity::Error) {
        Health::Blocked
    } else if findings.iter().any(|f| f.severity == Severity::Warn) {
        Health::Degraded
    } else {
        Health::Ok
    }
}

/// Actionable findings for agent.md (error + warn only).
pub fn actionable(findings: &[Finding]) -> impl Iterator<Item = &Finding> {
    findings
        .iter()
        .filter(|f| matches!(f.severity, Severity::Error | Severity::Warn))
}

/// Drop findings matched by `[[suppress]]` rules in `agal.toml`.
/// Returns `(kept, suppressed_count)`.
pub fn apply_suppressions(
    findings: Vec<Finding>,
    rules: &[SuppressRule],
) -> (Vec<Finding>, usize) {
    if rules.is_empty() {
        return (findings, 0);
    }
    let before = findings.len();
    let kept: Vec<Finding> = findings
        .into_iter()
        .filter(|f| !is_suppressed(f, rules))
        .collect();
    let suppressed = before.saturating_sub(kept.len());
    (kept, suppressed)
}

fn is_suppressed(f: &Finding, rules: &[SuppressRule]) -> bool {
    rules.iter().any(|r| {
        if r.code != "*" && r.code != f.code {
            return false;
        }
        match r.node.as_deref() {
            None | Some("*") | Some("") => true,
            Some(want) => node_matches(f, want),
        }
    })
}

fn node_matches(f: &Finding, want: &str) -> bool {
    let want = want.trim().trim_matches('`');
    if want.is_empty() {
        return true;
    }
    let Some(id) = f.node.as_deref() else {
        // Workspace-level finding (no node): only match explicit empty/"*" handled above
        // or want that looks like a root path.
        return f.path.as_deref() == Some(want) || want == ".";
    };
    if id == want {
        return true;
    }
    // package name / short id
    let short = id.rsplit('/').next().unwrap_or(id);
    if short == want {
        return true;
    }
    // path prefix / suffix
    id.ends_with(want) || id.ends_with(&format!("/{want}")) || f.path.as_deref() == Some(want)
}

/// Run structural + integrity checks over the built graph.
pub fn analyze(
    project_root: &Path,
    nodes: &[Node],
    edges: &[Edge],
    rules: &BTreeMap<String, String>,
) -> Vec<Finding> {
    let mut out = Vec::new();

    integrity_checks(project_root, nodes, &mut out);

    let plugins: Vec<&Node> = nodes.iter().filter(|n| n.kind == "plugin").collect();

    let ui_export_set: BTreeSet<String> = nodes
        .iter()
        .filter_map(|n| n.ast_summary.as_ref())
        .flat_map(|a| a.slint_exports.iter().cloned())
        .collect();

    let shm_node = nodes.iter().find(|n| n.name == "lx-shm");
    // Default LX target; override only if rules text names another *-editor crate.
    let editor_target = rules
        .get("plugin_target_editor")
        .and_then(|s| {
            s.split(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
                .find(|t| t.ends_with("-editor") && *t != "truce-slint")
        })
        .unwrap_or("lx-slint-editor");

    // Incoming edge counts for unused-crate detection.
    let mut inbound: BTreeMap<&str, usize> = BTreeMap::new();
    for e in edges {
        *inbound.entry(e.to.as_str()).or_default() += 1;
    }

    for p in &plugins {
        // --- Metadata ---
        if p.version.is_none() {
            out.push(
                Finding::new(
                    Severity::Warn,
                    "missing_version",
                    format!(
                        "{} has no version in Cargo.toml — add `version = \"...\"` under [package]",
                        p.name
                    ),
                )
                .at_node(p)
                .with_path(format!("{}/Cargo.toml", p.path))
                .with_fix(format!(
                    "set `version` in `{}/Cargo.toml` [package]",
                    p.path
                )),
            );
        }
        if p.description.is_none() {
            out.push(
                Finding::new(
                    Severity::Info,
                    "missing_description",
                    format!(
                        "{} has no description in Cargo.toml — add `description = \"...\"` under [package]",
                        p.name
                    ),
                )
                .at_node(p)
                .with_path(format!("{}/Cargo.toml", p.path))
                .with_fix(format!(
                    "set `description` in `{}/Cargo.toml` [package]",
                    p.path
                )),
            );
        }

        let Some(ast) = p.ast_summary.as_ref() else {
            continue;
        };

        // --- Plugin format ---
        if ast.plugin_formats.is_empty() {
            out.push(
                Finding::new(
                    Severity::Warn,
                    "missing_format",
                    format!(
                        "{} has no detected export format (CLAP/VST3/LV2) — check [features] or framework config",
                        p.name
                    ),
                )
                .at_node(p)
                .with_path(format!("{}/Cargo.toml", p.path))
                .with_fix(format!(
                    "enable format features in `{}/Cargo.toml` (e.g. clap/vst3)",
                    p.path
                )),
            );
        }

        // --- Migration ---
        match p.migration_status.as_deref() {
            Some("legacy") => out.push(
                Finding::new(
                    Severity::Error,
                    "migration_legacy",
                    format!(
                        "{} still on legacy editor adapter — {}",
                        p.name,
                        rules
                            .get("plugin_target_editor")
                            .cloned()
                            .unwrap_or_else(|| "migrate editor adapter".into())
                    ),
                )
                .at_node(p)
                .with_fix(format!(
                    "switch `{}` to `{}` (Cargo dep + editor imports)",
                    p.name, editor_target
                )),
            ),
            Some("unknown") => out.push(
                Finding::new(
                    Severity::Info,
                    "migration_unknown",
                    format!(
                        "{} migration status unknown (mixed or unmatched editor adapters)",
                        p.name
                    ),
                )
                .at_node(p)
                .with_fix(format!(
                    "use only `{}` as editor adapter in {}",
                    editor_target, p.name
                )),
            ),
            _ => {}
        }

        // Mixed adapters
        if ast.imported_editor_adapters.len() > 1 {
            let adapters: Vec<String> = ast.imported_editor_adapters.iter().cloned().collect();
            out.push(
                Finding::new(
                    Severity::Warn,
                    "mixed_editor_adapters",
                    format!(
                        "{} imports multiple editor adapters: {}",
                        p.name,
                        adapters.join(", ")
                    ),
                )
                .at_node(p)
                .with_fix(format!(
                    "keep a single editor adapter (prefer `{}`)",
                    editor_target
                )),
            );
        }

        // Has Slint / truce UI path but no editor adapter detected
        let has_slint =
            p.frameworks.iter().any(|f| f == "slint") || !ast.slint_components.is_empty();
        if has_slint
            && ast.imported_editor_adapters.is_empty()
            && !p
                .frameworks
                .iter()
                .any(|f| f == "lx-slint-editor" || f == "truce-slint" || f.ends_with("-editor"))
        {
            out.push(
                Finding::new(
                    Severity::Warn,
                    "missing_editor_adapter",
                    format!(
                        "{} uses Slint UI but no editor adapter import detected",
                        p.name
                    ),
                )
                .at_node(p)
                .with_fix(format!(
                    "depend on and import `{}` in {}",
                    editor_target, p.name
                )),
            );
        }

        // Integrity: required editor crate when Slint + target crate exists in workspace
        let has_editor_target = nodes.iter().any(|n| n.name == editor_target);
        let links_editor = p.internal_deps.iter().any(|d| d.contains(editor_target))
            || p.frameworks.iter().any(|f| f == editor_target)
            || edges.iter().any(|e| {
                e.from == p.id
                    && (e.kind == "depends_on" || e.kind == "build_depends_on")
                    && e.to.contains(editor_target)
            });
        if has_slint && has_editor_target && !links_editor {
            // Avoid double-fire if already missing_editor_adapter / migration_legacy
            let already = out.iter().any(|f| {
                f.node.as_deref() == Some(p.id.as_str())
                    && (f.code == "missing_editor_adapter" || f.code == "migration_legacy")
            });
            if !already {
                out.push(
                    Finding::new(
                        Severity::Warn,
                        "required_dep_missing",
                        format!(
                            "{} uses Slint but has no workspace dep on `{}`",
                            p.name, editor_target
                        ),
                    )
                    .at_node(p)
                    .with_path(format!("{}/Cargo.toml", p.path))
                    .with_fix(format!(
                        "add path dep `{}` in `{}/Cargo.toml`",
                        editor_target, p.path
                    )),
                );
            }
        }

        // Integrity: IPC signals without lx-shm link when hub exists
        let has_ipc = ast.ipc_signals.contains("shm")
            || ast.ipc_signals.contains("relay")
            || ast.ipc_signals.contains("seqlock");
        if has_ipc && let Some(shm) = shm_node {
            let links_shm = p.internal_deps.iter().any(|d| d == &shm.id || d.ends_with("lx-shm"))
                || edges.iter().any(|e| {
                    e.from == p.id
                        && e.to == shm.id
                        && (e.kind == "depends_on"
                            || e.kind == "runtime_depends_on"
                            || e.kind == "build_depends_on")
                })
                // transitive via analysis crate is OK for LX stack
                || p.internal_deps.iter().any(|d| d.contains("lx-analysis"))
                || edges.iter().any(|e| {
                    e.from == p.id && e.to.contains("lx-analysis") && e.kind == "depends_on"
                });
            if !links_shm {
                out.push(
                    Finding::new(
                        Severity::Warn,
                        "required_dep_missing",
                        format!(
                            "{} has shm/relay signals but no link to `lx-shm` (direct or via lx-analysis)",
                            p.name
                        ),
                    )
                    .at_node(p)
                    .with_path(format!("{}/Cargo.toml", p.path))
                    .with_fix(format!(
                        "depend on `lx-shm` or `lx-analysis` from `{}`",
                        p.path
                    )),
                );
            }
        }

        // PluginLogic without process hook
        if !ast.plugin_logic_impls.is_empty() && ast.process_hooks.is_empty() {
            out.push(
                Finding::new(
                    Severity::Warn,
                    "missing_process_hook",
                    format!("{} has PluginLogic but no process hook detected", p.name),
                )
                .at_node(p)
                .with_fix(format!(
                    "implement process/audio callback on PluginLogic in `{}`",
                    p.path
                )),
            );
        }

        // PluginLogic type vs truce::plugin! logic mismatch
        if !ast.plugin_logic_impls.is_empty() && !ast.plugin_macro_types.is_empty() {
            let logic = &ast.plugin_logic_impls;
            let macros = &ast.plugin_macro_types;
            if logic.intersection(macros).next().is_none() {
                out.push(
                    Finding::new(
                        Severity::Error,
                        "logic_macro_mismatch",
                        format!(
                            "{} PluginLogic {:?} does not match truce::plugin! logic {:?}",
                            p.name, logic, macros
                        ),
                    )
                    .at_node(p)
                    .with_fix(format!(
                        "align `truce::plugin!(logic = …)` with PluginLogic type in `{}`",
                        p.path
                    )),
                );
            }
        }

        // Params without plugin! wiring
        if !ast.params_structs.is_empty() && ast.plugin_macro_types.is_empty() {
            out.push(
                Finding::new(
                    Severity::Info,
                    "params_without_plugin_macro",
                    format!(
                        "{} has Params struct(s) but no truce::plugin! logic type seen",
                        p.name
                    ),
                )
                .at_node(p)
                .with_fix(format!(
                    "wire Params via framework plugin macro in `{}`",
                    p.path
                )),
            );
        }

        // process without editor
        if !ast.process_hooks.is_empty() && ast.editor_functions.is_empty() {
            out.push(
                Finding::new(
                    Severity::Info,
                    "process_without_editor",
                    format!(
                        "{} has process hook but no editor() — headless/pass-through?",
                        p.name
                    ),
                )
                .at_node(p),
            );
        }

        // editor without process
        if !ast.editor_functions.is_empty() && ast.process_hooks.is_empty() {
            out.push(
                Finding::new(
                    Severity::Warn,
                    "editor_without_process",
                    format!(
                        "{} has editor() but no process hook — audio path missing?",
                        p.name
                    ),
                )
                .at_node(p)
                .with_fix(format!(
                    "add process hook or mark plugin headless intentionally in `{}`",
                    p.path
                )),
            );
        }

        // Has UI file role but zero slint components (maybe pure Rust UI or empty shell)
        let has_ui_role = ast.file_roles.values().any(|r| r == "ui" || r == "slint");
        if has_ui_role && ast.slint_components.is_empty() && has_slint {
            out.push(
                Finding::new(
                    Severity::Info,
                    "ui_files_without_lx_components",
                    format!(
                        "{} has ui/slint files but no Lx* components detected",
                        p.name
                    ),
                )
                .at_node(p),
            );
        }

        // Shared UI components without uses_ui edge
        if !ast.slint_components.is_empty() && !ui_export_set.is_empty() {
            let uses_shared = ast
                .slint_components
                .iter()
                .any(|c| ui_export_set.contains(c));
            let has_ui_edge = edges.iter().any(|e| e.from == p.id && e.kind == "uses_ui");
            if uses_shared && !has_ui_edge {
                out.push(
                    Finding::new(
                        Severity::Info,
                        "ui_coupling_implicit",
                        format!(
                            "{} uses shared Lx* components but no uses_ui edge was created",
                            p.name
                        ),
                    )
                    .at_node(p),
                );
            }
        }

        // IPC file without strong signal
        if ast.file_roles.values().any(|r| r == "ipc")
            && !ast.ipc_signals.contains("shm")
            && !ast.ipc_signals.contains("relay")
        {
            out.push(
                Finding::new(
                    Severity::Info,
                    "ipc_file_without_signal",
                    format!(
                        "{} has ipc-role file but no shm/relay signal detected in sources",
                        p.name
                    ),
                )
                .at_node(p),
            );
        }

        // Visible (non-hidden) param surface
        let visible_param_count: usize = ast
            .params_fields
            .values()
            .flat_map(|v| v.iter())
            .filter(|f| !f.hidden)
            .count();
        let param_count: usize = ast.params_fields.values().map(|v| v.len()).sum();
        if visible_param_count > 40 {
            out.push(
                Finding::new(
                    Severity::Info,
                    "large_param_surface",
                    format!(
                        "{} exposes {} visible params ({} total) — state migration / UI binding cost is high",
                        p.name, visible_param_count, param_count
                    ),
                )
                .at_node(p),
            );
        }

        // Unbound params (never referenced in editor/process/slint); excludes hidden
        if !ast.params_unbound.is_empty() {
            let total = visible_param_count.max(1);
            let unbound_n = ast.params_unbound.len();
            let sample: Vec<&str> = ast
                .params_unbound
                .iter()
                .take(8)
                .map(|s| s.as_str())
                .collect();
            let more = if unbound_n > 8 {
                format!(" (+{} more)", unbound_n - 8)
            } else {
                String::new()
            };
            let severity = if unbound_n * 100 / total >= 25 {
                Severity::Warn
            } else {
                Severity::Info
            };
            out.push(
                Finding::new(
                    severity,
                    "params_unbound",
                    format!(
                        "{} has {}/{} visible params never referenced outside Params def: {}{}",
                        p.name,
                        unbound_n,
                        total,
                        sample.join(", "),
                        more
                    ),
                )
                .at_node(p)
                .with_fix(format!(
                    "bind or hide unused params in `{}` editor/process/UI",
                    p.path
                )),
            );
        }

        // Many params, few shared UI components → possible under-wired UI
        if visible_param_count >= 20 && !ast.slint_components.is_empty() {
            let comps = ast.slint_components.len();
            if comps > 0 && visible_param_count / comps >= 4 {
                out.push(
                    Finding::new(
                        Severity::Info,
                        "params_heavy_ui_light",
                        format!(
                            "{} has {} visible params but only {} Lx* components — check binding density",
                            p.name, visible_param_count, comps
                        ),
                    )
                    .at_node(p),
                );
            }
        }

        // Plugin with process hook but no audio-role file (everything crammed in lib.rs)
        if !ast.process_hooks.is_empty()
            && !ast.file_roles.values().any(|r| r == "audio")
            && ast.files.iter().any(|f| f.ends_with("lib.rs"))
        {
            out.push(
                Finding::new(
                    Severity::Info,
                    "process_inlined_in_lib",
                    format!(
                        "{} process hook lives without process.rs — audio path may be inlined in lib.rs",
                        p.name
                    ),
                )
                .at_node(p)
                .with_path(format!("{}/src/lib.rs", p.path)),
            );
        }
    }

    // Crates: DSP process method name collision
    for n in nodes.iter().filter(|n| n.kind == "crate") {
        if let Some(ast) = &n.ast_summary
            && ast.process_method_count >= 5
        {
            out.push(
                Finding::new(
                    Severity::Info,
                    "dsp_process_methods",
                    format!(
                        "{} has {} methods named process (DSP units, not plugin hooks)",
                        n.name, ast.process_method_count
                    ),
                )
                .at_node(n),
            );
        }

        // Workspace crate with zero inbound edges (possible dead / not wired).
        // Skip nested package examples — demos are path-included, not dependents.
        let in_count = inbound.get(n.id.as_str()).copied().unwrap_or(0);
        let is_nested_example = n.id.contains("/examples/") || n.path.contains("/examples/");
        if in_count == 0 && n.name != "lx-slint-build" && !is_nested_example {
            out.push(
                Finding::new(
                    Severity::Info,
                    "crate_no_dependents",
                    format!(
                        "{} has no inbound workspace edges — unused or only path-included?",
                        n.name
                    ),
                )
                .at_node(n)
                .with_fix(format!(
                    "wire `{}` as a path dep from a plugin/crate, or remove from workspace",
                    n.name
                )),
            );
        }
    }

    // Orphan strong IPC
    let ipc_plugins: Vec<&Node> = plugins
        .iter()
        .copied()
        .filter(|p| {
            p.ast_summary
                .as_ref()
                .map(|a| {
                    a.ipc_signals.contains("shm")
                        || a.ipc_signals.contains("relay")
                        || a.ipc_signals.contains("seqlock")
                })
                .unwrap_or(false)
        })
        .collect();
    if ipc_plugins.len() == 1 {
        out.push(
            Finding::new(
                Severity::Warn,
                "ipc_single_peer",
                format!(
                    "{} is the only plugin with shm/relay signals — peer missing or not detected",
                    ipc_plugins[0].name
                ),
            )
            .at_node(ipc_plugins[0])
            .with_fix("add peer plugin with matching shm/relay, or remove dead IPC code"),
        );
    }

    // Relay publisher without consumer peer edge
    for p in &ipc_plugins {
        let is_relay_named = p.name.contains("relay");
        let has_relay = p
            .ast_summary
            .as_ref()
            .map(|a| a.ipc_signals.contains("relay"))
            .unwrap_or(false);
        if is_relay_named || has_relay {
            let has_peer = edges
                .iter()
                .any(|e| e.kind == "ipc_peer" && (e.from == p.id || e.to == p.id));
            if !has_peer && ipc_plugins.len() > 1 {
                let peers: Vec<_> = ipc_plugins
                    .iter()
                    .filter(|o| o.id != p.id)
                    .map(|o| o.name.as_str())
                    .collect();
                if !peers.is_empty() {
                    out.push(
                        Finding::new(
                            Severity::Info,
                            "relay_no_ipc_peer_edge",
                            format!(
                                "{} has relay signals but no ipc_peer edge (candidates: {})",
                                p.name,
                                peers.join(", ")
                            ),
                        )
                        .at_node(p)
                        .with_fix(
                            "ensure both peers share shm/relay signals so ipc_peer edge appears",
                        ),
                    );
                }
            }
        }
    }

    // External tool hints (info only; never block health)
    crate::tool_hints::append_hints(nodes, &mut out);

    out.sort_by(|a, b| {
        severity_rank(&a.severity)
            .cmp(&severity_rank(&b.severity))
            .then_with(|| a.node.cmp(&b.node))
            .then_with(|| a.code.cmp(&b.code))
    });
    out
}

/// Projucer-style integrity: broken members, packages outside workspace.
fn integrity_checks(project_root: &Path, nodes: &[Node], out: &mut Vec<Finding>) {
    let known: BTreeSet<&str> = nodes.iter().map(|n| n.path.as_str()).collect();

    // Explicit workspace.members that failed to resolve
    let root_cargo = project_root.join("Cargo.toml");
    if let Ok(mut content) = fs::read_to_string(&root_cargo) {
        content.retain(|c| c != '\r');
        if let Ok(toml) = content.parse::<toml::Table>().map(toml::Value::Table) {
            if let Some(members) = toml
                .get("workspace")
                .and_then(|w| w.get("members"))
                .and_then(|m| m.as_array())
            {
                for m in members {
                    let Some(pattern) = m.as_str() else {
                        continue;
                    };
                    if pattern.contains('*') {
                        let base = pattern
                            .trim_end_matches("/**")
                            .trim_end_matches("/*")
                            .trim_end_matches('*')
                            .trim_end_matches('/');
                        if base.is_empty() {
                            continue;
                        }
                        let base_path = project_root.join(base);
                        if !base_path.is_dir() {
                            out.push(
                                Finding::new(
                                    Severity::Error,
                                    "workspace_member_missing",
                                    format!(
                                        "workspace member glob `{pattern}` base `{base}` does not exist"
                                    ),
                                )
                                .with_path("Cargo.toml")
                                .with_fix(format!(
                                    "create `{base}/` or remove `{pattern}` from workspace.members"
                                )),
                            );
                        }
                    } else {
                        let path = project_root.join(pattern);
                        let cargo = path.join("Cargo.toml");
                        if !cargo.exists() {
                            out.push(
                                Finding::new(
                                    Severity::Error,
                                    "workspace_member_missing",
                                    format!(
                                        "workspace member `{pattern}` has no Cargo.toml"
                                    ),
                                )
                                .with_path(pattern.to_string())
                                .with_fix(format!(
                                    "add `{pattern}/Cargo.toml` or remove member from root Cargo.toml"
                                )),
                            );
                        }
                    }
                }
            }
        }
    }

    // packages under plugins/ and crates/ not in the workspace graph
    for base in ["plugins", "crates"] {
        let dir = project_root.join(base);
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if !path.is_dir() || !path.join("Cargo.toml").exists() {
                continue;
            }
            let rel = path
                .strip_prefix(project_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if known.contains(rel.as_str()) {
                continue;
            }
            let code = if base == "plugins" {
                "plugin_not_in_workspace"
            } else {
                "package_not_in_workspace"
            };
            out.push(
                Finding::new(
                    Severity::Warn,
                    code,
                    format!("`{rel}` has Cargo.toml but is not a workspace member"),
                )
                .with_path(rel.clone())
                .with_fix(format!(
                    "add `{rel}` (or `{base}/*`) to workspace.members in root Cargo.toml"
                )),
            );
        }
    }
}

fn severity_rank(s: &Severity) -> u8 {
    match s {
        Severity::Error => 0,
        Severity::Warn => 1,
        Severity::Info => 2,
    }
}

pub fn count_by_severity(findings: &[Finding]) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for f in findings {
        *m.entry(f.severity.to_string()).or_default() += 1;
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SuppressRule;

    #[test]
    fn health_gate_ranks_error_over_warn() {
        assert_eq!(health(&[]), Health::Ok);
        let w = Finding::new(Severity::Warn, "w", "warn");
        assert_eq!(health(std::slice::from_ref(&w)), Health::Degraded);
        let e = Finding::new(Severity::Error, "e", "err");
        assert_eq!(health(&[w, e]), Health::Blocked);
    }

    #[test]
    fn suppress_by_code_and_node_name() {
        let mut f = Finding::new(Severity::Info, "large_param_surface", "many");
        f.node = Some("plugins/aurum-slint".into());
        f.path = Some("plugins/aurum-slint".into());
        let other = Finding::new(Severity::Info, "large_param_surface", "many2");
        let mut other = other;
        other.node = Some("plugins/meridian".into());

        let rules = vec![SuppressRule {
            code: "large_param_surface".into(),
            node: Some("aurum-slint".into()),
            reason: Some("intentional".into()),
        }];
        let (out, n) = apply_suppressions(vec![f, other.clone()], &rules);
        assert_eq!(out.len(), 1);
        assert_eq!(n, 1);
        assert_eq!(out[0].node.as_deref(), Some("plugins/meridian"));

        let (all, n_all) = apply_suppressions(
            vec![other],
            &[SuppressRule {
                code: "large_param_surface".into(),
                node: None,
                reason: None,
            }],
        );
        assert!(all.is_empty());
        assert_eq!(n_all, 1);

        let (noop, n0) = apply_suppressions(
            vec![Finding::new(Severity::Info, "x", "y")],
            &[],
        );
        assert_eq!(noop.len(), 1);
        assert_eq!(n0, 0);
    }
}
