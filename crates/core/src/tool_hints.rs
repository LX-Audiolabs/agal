//! External tool hints (Clippy, clap-validator, optional symbol tools).
//!
//! **Not** executed on `agal .` — only PATH probes + info findings with `fix` commands.
//! Run tools yourself (or CI). Use `agal doctor` for a human checklist.
//!
//! Optional symbol/call-graph tools are **doctor-only** (no generate findings) —
//! they complement agal, they do not replace it.

use std::env;
use std::path::{Path, PathBuf};

use crate::Node;
use crate::findings::{Finding, Severity};

/// Binaries we recognize as optional code-intelligence tools (not required).
const OPTIONAL_SYMBOL_TOOLS: &[&str] = &["codegraph", "codebase-memory-mcp", "graphify"];

/// Result of probing common audio/Rust quality tools.
#[derive(Debug, Clone)]
pub struct ToolStatus {
    pub clippy: ToolProbe,
    pub clap_validator: ToolProbe,
    /// Optional local symbol / call-graph tools (found or not — never required).
    pub symbol_tools: Vec<ToolProbe>,
}

#[derive(Debug, Clone)]
pub struct ToolProbe {
    pub name: &'static str,
    pub found: bool,
    pub path: Option<PathBuf>,
}

impl ToolProbe {
    fn probe(name: &'static str) -> Self {
        let path = find_on_path(name);
        Self {
            name,
            found: path.is_some(),
            path,
        }
    }
}

/// Check whether `cargo-clippy`, `clap-validator`, and optional symbol tools are on PATH.
pub fn probe_tools() -> ToolStatus {
    ToolStatus {
        // `cargo clippy` is provided by the `cargo-clippy` binary.
        clippy: ToolProbe::probe("cargo-clippy"),
        clap_validator: ToolProbe::probe("clap-validator"),
        symbol_tools: OPTIONAL_SYMBOL_TOOLS
            .iter()
            .map(|n| ToolProbe::probe(n))
            .collect(),
    }
}

/// Append workspace/plugin **info** hints (never error/warn).
pub fn append_hints(project_root: &Path, nodes: &[Node], out: &mut Vec<Finding>) {
    let status = probe_tools();

    // One workspace-level Clippy hint (not per crate).
    let clippy_fix = if status.clippy.found {
        "cargo clippy --workspace --all-targets -- -D warnings"
    } else {
        "install rustup component: `rustup component add clippy`, then \
         `cargo clippy --workspace --all-targets -- -D warnings`"
    };
    let clippy_msg = if status.clippy.found {
        "run Clippy on the workspace before shipping (static lint; not a substitute for graph findings)"
            .to_string()
    } else {
        "Clippy not found on PATH — install via rustup, then lint the workspace".to_string()
    };
    out.push(
        Finding::new(Severity::Info, "tool_hint_clippy", clippy_msg)
            .with_path("Cargo.toml")
            .with_fix(clippy_fix),
    );

    // AURA workspace: hint cargo aura if aura.toml or aura-* crates present
    let is_aura_ws = crate::config::is_aura_workspace(project_root);
    let has_aura_nodes = nodes
        .iter()
        .any(|n| n.frameworks.iter().any(|f| f == "aura"));
    if is_aura_ws || has_aura_nodes {
        let aura_msg = if has_aura_nodes {
            "AURA framework detected — use `cargo aura` for build, install, doctor"
        } else {
            "AURA workspace (`aura.toml`) detected — use `cargo aura` for build, install, doctor"
        };
        let plugin_names: Vec<&str> = nodes
            .iter()
            .filter(|n| n.kind == "plugin")
            .map(|n| n.name.as_str())
            .collect();
        let aura_fix = if plugin_names.is_empty() {
            "cargo aura build --clap && cargo aura doctor".to_string()
        } else {
            format!(
                "cargo aura build --clap -plug {} && cargo aura doctor",
                plugin_names.join(" ")
            )
        };
        out.push(
            Finding::new(Severity::Info, "tool_hint_cargo_aura", aura_msg)
                .with_path("aura.toml")
                .with_fix(aura_fix),
        );
    }

    // Per-plugin CLAP validator hint when format includes CLAP.
    for n in nodes.iter().filter(|n| n.kind == "plugin") {
        let has_clap = n
            .ast_summary
            .as_ref()
            .map(|a| {
                a.plugin_formats.iter().any(|f| {
                    let u = f.to_ascii_uppercase();
                    u == "CLAP" || u.contains("CLAP")
                })
            })
            .unwrap_or(false)
            || n.frameworks
                .iter()
                .any(|f| f == "clap" || f == "truce" || f == "aura");

        if !has_clap {
            continue;
        }

        let (msg, fix) = if status.clap_validator.found {
            (
                format!(
                    "{} exports CLAP — validate the **built** binary with clap-validator after compile",
                    n.name
                ),
                format!(
                    "after build: clap-validator validate path/to/{}.clap  \
                     (or the host-exported .clap from your install step)",
                    n.name
                ),
            )
        } else {
            (
                format!(
                    "{} exports CLAP — clap-validator not on PATH; install to validate built plugins",
                    n.name
                ),
                "install clap-validator (https://github.com/free-audio/clap-validator), then \
                 `clap-validator validate path/to/plugin.clap`"
                    .to_string(),
            )
        };

        out.push(
            Finding::new(Severity::Info, "tool_hint_clap_validator", msg)
                .at_node(n)
                .with_fix(fix),
        );
    }
}

/// Human-readable doctor report (stdout for `agal doctor`).
pub fn render_doctor(project_root: &Path, nodes: &[Node], status: &ToolStatus) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    let _ = writeln!(s, "# agal doctor\n");
    let _ = writeln!(s, "project: `{}`\n", project_root.display());

    let _ = writeln!(s, "## external tools (required quality stack)\n");
    for p in [&status.clippy, &status.clap_validator] {
        if p.found {
            let path = p
                .path
                .as_ref()
                .map(|x| x.display().to_string())
                .unwrap_or_else(|| "?".into());
            let _ = writeln!(s, "- **{}**: ok — `{}`", p.name, path);
        } else {
            let _ = writeln!(s, "- **{}**: missing on PATH", p.name);
        }
    }

    let _ = writeln!(
        s,
        "\n## optional symbol / call-graph tools\n\n\
         agal is the **structure** layer (map, health, notes, skills).  \n\
         These tools answer callers / impact / symbols — use when needed, not as a substitute for `AGAL.md`.\n"
    );
    let any_symbol = status.symbol_tools.iter().any(|p| p.found);
    for p in &status.symbol_tools {
        if p.found {
            let path = p
                .path
                .as_ref()
                .map(|x| x.display().to_string())
                .unwrap_or_else(|| "?".into());
            let _ = writeln!(s, "- **{}**: found — `{}`", p.name, path);
        } else {
            let _ = writeln!(s, "- **{}**: not on PATH", p.name);
        }
    }
    if !any_symbol {
        let _ = writeln!(
            s,
            "\n_None found — fine. Install only if you want local call-graph / symbol queries \
             (e.g. [codegraph](https://github.com/colbymchenry/codegraph), \
             [codebase-memory-mcp](https://github.com/DeusData/codebase-memory-mcp))._\n"
        );
    } else {
        let _ = writeln!(s);
    }

    let _ = writeln!(s, "\n## recommended commands\n");
    let _ = writeln!(s, "### Clippy (workspace lint)\n");
    if status.clippy.found {
        let _ = writeln!(
            s,
            "```bash\ncargo clippy --workspace --all-targets -- -D warnings\n```\n"
        );
    } else {
        let _ = writeln!(
            s,
            "```bash\nrustup component add clippy\ncargo clippy --workspace --all-targets -- -D warnings\n```\n"
        );
    }

    let clap_plugins: Vec<&Node> = nodes
        .iter()
        .filter(|n| n.kind == "plugin")
        .filter(|n| {
            n.ast_summary
                .as_ref()
                .map(|a| {
                    a.plugin_formats
                        .iter()
                        .any(|f| f.to_ascii_uppercase().contains("CLAP"))
                })
                .unwrap_or(false)
                || n.frameworks.iter().any(|f| f == "clap" || f == "truce")
        })
        .collect();

    let _ = writeln!(s, "### clap-validator (after you **build** the plugin)\n");
    if clap_plugins.is_empty() {
        let _ = writeln!(s, "_no CLAP plugins detected in graph_\n");
    } else {
        if !status.clap_validator.found {
            let _ = writeln!(
                s,
                "Install: [clap-validator](https://github.com/free-audio/clap-validator) and put it on PATH.\n"
            );
        }
        let _ = writeln!(
            s,
            "agal does **not** run the validator (needs a built `.clap`, not sources).\n"
        );
        for n in clap_plugins {
            let _ = writeln!(
                s,
                "- **{}** (`{}`): `clap-validator validate path/to/{}.clap`",
                n.name, n.path, n.name
            );
        }
        let _ = writeln!(s);
    }

    let _ = writeln!(
        s,
        "## note\n\
         **agal** = structure (migration, params, IPC, health).  \n\
         **Clippy** = Rust lints.  \n\
         **clap-validator** = CLAP ABI/spec on the **binary**.  \n\
         **codegraph / codebase-memory / …** = optional symbols & impact (not agal).  \n\
         Clippy + clap-validator hints appear as **info** findings on generate; symbol tools are doctor-only.\n"
    );
    s
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path_os = env::var_os("PATH")?;
    for dir in env::split_paths(&path_os) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let exe = dir.join(format!("{name}.exe"));
            if exe.is_file() {
                return Some(exe);
            }
            let cmd = dir.join(format!("{name}.cmd"));
            if cmd.is_file() {
                return Some(cmd);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_does_not_panic() {
        let s = probe_tools();
        // We only assert structure; PATH content varies by machine.
        assert_eq!(s.clippy.name, "cargo-clippy");
        assert_eq!(s.clap_validator.name, "clap-validator");
        assert_eq!(s.symbol_tools.len(), OPTIONAL_SYMBOL_TOOLS.len());
        assert!(s.symbol_tools.iter().any(|p| p.name == "codegraph"));
    }

    #[test]
    fn doctor_mentions_symbol_tools_section() {
        let status = ToolStatus {
            clippy: ToolProbe {
                name: "cargo-clippy",
                found: false,
                path: None,
            },
            clap_validator: ToolProbe {
                name: "clap-validator",
                found: false,
                path: None,
            },
            symbol_tools: vec![ToolProbe {
                name: "codegraph",
                found: false,
                path: None,
            }],
        };
        let report = render_doctor(Path::new("."), &[], &status);
        assert!(report.contains("optional symbol"));
        assert!(report.contains("codegraph"));
        assert!(report.contains("not on PATH"));
    }

    #[test]
    fn append_hints_does_not_emit_symbol_tool_findings() {
        let mut out = Vec::new();
        append_hints(Path::new("."), &[], &mut out);
        assert!(
            out.iter()
                .all(|f| !f.code.contains("symbol") && !f.code.contains("codegraph"))
        );
    }

    #[test]
    fn clap_hint_only_for_clapish_plugins() {
        let mut clap = Node {
            id: "plugins/demo".into(),
            name: "demo".into(),
            kind: "plugin".into(),
            description: None,
            version: Some("0.1.0".into()),
            path: "plugins/demo".into(),
            frameworks: vec!["truce".into(), "clap".into()],
            migration_status: None,
            internal_deps: Default::default(),
            external_flags: Default::default(),
            dependency_names: Default::default(),
            dependency_details: vec![],
            ast_summary: Some({
                let mut a = crate::ast::AstSummary::default();
                a.plugin_formats.insert("CLAP".into());
                a
            }),
        };
        let other = Node {
            id: "plugins/headless".into(),
            name: "headless".into(),
            kind: "plugin".into(),
            description: None,
            version: Some("0.1.0".into()),
            path: "plugins/headless".into(),
            frameworks: vec![],
            migration_status: None,
            internal_deps: Default::default(),
            external_flags: Default::default(),
            dependency_names: Default::default(),
            dependency_details: vec![],
            ast_summary: Some(crate::ast::AstSummary::default()),
        };
        let mut out = Vec::new();
        append_hints(Path::new("."), std::slice::from_ref(&clap), &mut out);
        let clap_hints: Vec<_> = out
            .iter()
            .filter(|f| f.code == "tool_hint_clap_validator")
            .collect();
        assert_eq!(clap_hints.len(), 1);
        assert!(clap_hints[0].fix.is_some());

        out.clear();
        clap.frameworks.clear();
        if let Some(a) = clap.ast_summary.as_mut() {
            a.plugin_formats.clear();
        }
        append_hints(Path::new("."), &[clap, other], &mut out);
        assert!(out.iter().all(|f| f.code != "tool_hint_clap_validator"));
    }
}
