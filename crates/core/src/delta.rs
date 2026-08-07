use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::Audiolabs;
use crate::findings::{Finding, finding_key};

/// Diff against previous `agal.json` (architectural change signal for agents).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GraphDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_generated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_version: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added_nodes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_nodes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_nodes: Vec<NodeChange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added_edges: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_edges: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub new_findings: Vec<Finding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resolved_findings: Vec<Finding>,
    /// True when no previous graph existed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub first_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeChange {
    pub id: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub changes: Vec<String>,
}

pub fn load_previous(json_path: &Path) -> Option<Audiolabs> {
    let content = fs::read_to_string(json_path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn compute(prev: Option<&Audiolabs>, curr: &Audiolabs) -> GraphDelta {
    let Some(prev) = prev else {
        return GraphDelta {
            first_run: true,
            ..Default::default()
        };
    };

    let prev_nodes: BTreeMap<&str, _> = prev.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let curr_nodes: BTreeMap<&str, _> = curr.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    let prev_ids: BTreeSet<&str> = prev_nodes.keys().copied().collect();
    let curr_ids: BTreeSet<&str> = curr_nodes.keys().copied().collect();

    let added_nodes: Vec<String> = curr_ids
        .difference(&prev_ids)
        .map(|s| (*s).to_string())
        .collect();
    let removed_nodes: Vec<String> = prev_ids
        .difference(&curr_ids)
        .map(|s| (*s).to_string())
        .collect();

    let mut changed_nodes = Vec::new();
    for id in prev_ids.intersection(&curr_ids) {
        let p = prev_nodes[*id];
        let c = curr_nodes[*id];
        let mut changes = Vec::new();

        if p.version != c.version {
            changes.push(format!(
                "version: {} → {}",
                p.version.as_deref().unwrap_or("?"),
                c.version.as_deref().unwrap_or("?")
            ));
        }
        if p.migration_status != c.migration_status {
            changes.push(format!(
                "migration: {} → {}",
                p.migration_status.as_deref().unwrap_or("?"),
                c.migration_status.as_deref().unwrap_or("?")
            ));
        }
        if p.frameworks != c.frameworks {
            changes.push(format!(
                "frameworks: {:?} → {:?}",
                p.frameworks, c.frameworks
            ));
        }
        if p.internal_deps != c.internal_deps {
            let added: Vec<_> = c.internal_deps.difference(&p.internal_deps).collect();
            let removed: Vec<_> = p.internal_deps.difference(&c.internal_deps).collect();
            if !added.is_empty() {
                changes.push(format!("deps+: {:?}", added));
            }
            if !removed.is_empty() {
                changes.push(format!("deps-: {:?}", removed));
            }
        }

        let p_params = param_count(p);
        let c_params = param_count(c);
        if p_params != c_params {
            changes.push(format!("params: {} → {}", p_params, c_params));
        }

        let p_ipc = ipc_sigs(p);
        let c_ipc = ipc_sigs(c);
        if p_ipc != c_ipc {
            changes.push(format!("ipc: {:?} → {:?}", p_ipc, c_ipc));
        }

        let p_files = file_set(p);
        let c_files = file_set(c);
        if p_files != c_files {
            let af: Vec<_> = c_files.difference(&p_files).take(6).collect();
            let rf: Vec<_> = p_files.difference(&c_files).take(6).collect();
            if !af.is_empty() {
                changes.push(format!("files+: {:?}", af));
            }
            if !rf.is_empty() {
                changes.push(format!("files-: {:?}", rf));
            }
        }

        if !changes.is_empty() {
            changed_nodes.push(NodeChange {
                id: (*id).to_string(),
                changes,
            });
        }
    }

    let prev_edges: BTreeSet<String> = prev.edges.iter().map(edge_key).collect();
    let curr_edges: BTreeSet<String> = curr.edges.iter().map(edge_key).collect();
    let added_edges: Vec<String> = curr_edges.difference(&prev_edges).cloned().collect();
    let removed_edges: Vec<String> = prev_edges.difference(&curr_edges).cloned().collect();

    let prev_f: BTreeMap<String, Finding> = prev
        .findings
        .iter()
        .map(|f| (finding_key(f), f.clone()))
        .collect();
    let curr_f: BTreeMap<String, Finding> = curr
        .findings
        .iter()
        .map(|f| (finding_key(f), f.clone()))
        .collect();

    let new_findings: Vec<Finding> = curr_f
        .iter()
        .filter(|(k, _)| !prev_f.contains_key(*k))
        .map(|(_, f)| f.clone())
        .collect();
    let resolved_findings: Vec<Finding> = prev_f
        .iter()
        .filter(|(k, _)| !curr_f.contains_key(*k))
        .map(|(_, f)| f.clone())
        .collect();

    GraphDelta {
        previous_generated_at: Some(prev.generated_at.clone()),
        previous_version: Some(prev.version.clone()),
        added_nodes,
        removed_nodes,
        changed_nodes,
        added_edges,
        removed_edges,
        new_findings,
        resolved_findings,
        first_run: false,
    }
}

fn edge_key(e: &crate::Edge) -> String {
    format!("{}|{}|{}", e.kind, e.from, e.to)
}

fn param_count(n: &crate::Node) -> usize {
    n.ast_summary
        .as_ref()
        .map(|a| a.params_fields.values().map(|v| v.len()).sum())
        .unwrap_or(0)
}

fn ipc_sigs(n: &crate::Node) -> BTreeSet<String> {
    n.ast_summary
        .as_ref()
        .map(|a| a.ipc_signals.clone())
        .unwrap_or_default()
}

fn file_set(n: &crate::Node) -> BTreeSet<String> {
    n.ast_summary
        .as_ref()
        .map(|a| a.files.clone())
        .unwrap_or_default()
}

pub fn is_empty(d: &GraphDelta) -> bool {
    !d.first_run
        && d.added_nodes.is_empty()
        && d.removed_nodes.is_empty()
        && d.changed_nodes.is_empty()
        && d.added_edges.is_empty()
        && d.removed_edges.is_empty()
        && d.new_findings.is_empty()
        && d.resolved_findings.is_empty()
}

/// Compact markdown for agents / PR notes.
pub fn render_md(d: &GraphDelta) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    if d.first_run {
        let _ = writeln!(
            s,
            "# agal delta\n\n\
             **Summary:** First run; no previous graph exists for comparison.  \n\
             The graph baseline has been written and future runs will show changes.  \n\
             Review the agent summary for the initial workspace structure.\n\n\
             _first run — no previous graph to compare._\n"
        );
        return s;
    }
    let _ = writeln!(
        s,
        "# agal delta\n\n\
         **Summary:** Structural changes since the previous generation.  \n\
         Tracks added/removed/changed nodes, edges, and new/resolved findings.  \n\
         Use to review architecture drift before editing code.\n\n\
         previous: `{}` (v{})\n",
        d.previous_generated_at.as_deref().unwrap_or("?"),
        d.previous_version.as_deref().unwrap_or("?")
    );

    if is_empty(d) {
        let _ = writeln!(s, "_no structural changes._\n");
        return s;
    }

    if !d.added_nodes.is_empty() {
        let _ = writeln!(s, "## added nodes");
        for n in &d.added_nodes {
            let _ = writeln!(s, "- `{}`", n);
        }
        let _ = writeln!(s);
    }
    if !d.removed_nodes.is_empty() {
        let _ = writeln!(s, "## removed nodes");
        for n in &d.removed_nodes {
            let _ = writeln!(s, "- `{}`", n);
        }
        let _ = writeln!(s);
    }
    if !d.changed_nodes.is_empty() {
        let _ = writeln!(s, "## changed nodes");
        for c in &d.changed_nodes {
            let _ = writeln!(s, "- **{}**", c.id);
            for ch in &c.changes {
                let _ = writeln!(s, "  - {}", ch);
            }
        }
        let _ = writeln!(s);
    }
    if !d.added_edges.is_empty() {
        let _ = writeln!(s, "## added edges ({})", d.added_edges.len());
        for e in d.added_edges.iter().take(30) {
            let _ = writeln!(s, "- `{}`", e.replace('|', " → "));
        }
        if d.added_edges.len() > 30 {
            let _ = writeln!(s, "- _…{} more_", d.added_edges.len() - 30);
        }
        let _ = writeln!(s);
    }
    if !d.removed_edges.is_empty() {
        let _ = writeln!(s, "## removed edges ({})", d.removed_edges.len());
        for e in d.removed_edges.iter().take(30) {
            let _ = writeln!(s, "- `{}`", e.replace('|', " → "));
        }
        let _ = writeln!(s);
    }
    if !d.new_findings.is_empty() {
        let _ = writeln!(s, "## new findings");
        for f in &d.new_findings {
            let _ = writeln!(
                s,
                "- [{}] **{}** {}: {}",
                f.severity,
                f.code,
                f.node.as_deref().unwrap_or("-"),
                f.message
            );
        }
        let _ = writeln!(s);
    }
    if !d.resolved_findings.is_empty() {
        let _ = writeln!(s, "## resolved findings");
        for f in &d.resolved_findings {
            let _ = writeln!(
                s,
                "- [{}] **{}** {}",
                f.severity,
                f.code,
                f.node.as_deref().unwrap_or("-")
            );
        }
        let _ = writeln!(s);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::findings::{Finding, Severity};
    use crate::{Audiolabs, MigrationSummary, Node};

    fn empty_graph(stamp: &str) -> Audiolabs {
        Audiolabs {
            version: "0.4.0".into(),
            generated_at: stamp.into(),
            project_root: "/tmp".into(),
            project_name: "t".into(),
            used_frameworks: vec![],
            frameworks: vec![],
            nodes: vec![],
            edges: vec![],
            findings: vec![],
            findings_suppressed: 0,
            migration_summary: MigrationSummary {
                total_plugins: 0,
                total_legacy: 0,
                total_migrated: 0,
                migrations: Default::default(),
            },
            rules: Default::default(),
            delta: None,
        }
    }

    fn node(id: &str, name: &str) -> Node {
        Node {
            id: id.into(),
            name: name.into(),
            kind: "plugin".into(),
            description: None,
            version: Some("0.1.0".into()),
            path: id.into(),
            frameworks: vec![],
            migration_status: Some("migrated".into()),
            internal_deps: Default::default(),
            external_flags: Default::default(),
            dependency_names: Default::default(),
            dependency_details: vec![],
            ast_summary: None,
        }
    }

    #[test]
    fn first_run_delta() {
        let curr = empty_graph("t1");
        let d = compute(None, &curr);
        assert!(d.first_run);
    }

    #[test]
    fn detects_added_node_and_finding() {
        let prev = empty_graph("t0");
        let mut curr = empty_graph("t1");
        curr.nodes.push(node("plugins/a", "a"));
        curr.findings
            .push(Finding::new(Severity::Warn, "missing_version", "x"));
        let d = compute(Some(&prev), &curr);
        assert!(!d.first_run);
        assert_eq!(d.added_nodes, vec!["plugins/a".to_string()]);
        assert_eq!(d.new_findings.len(), 1);
        assert_eq!(d.new_findings[0].code, "missing_version");
    }

    #[test]
    fn detects_resolved_finding() {
        let mut prev = empty_graph("t0");
        prev.findings.push(
            Finding::new(Severity::Error, "migration_legacy", "old")
                .at_node(&node("plugins/a", "a")),
        );
        let curr = empty_graph("t1");
        let d = compute(Some(&prev), &curr);
        assert_eq!(d.resolved_findings.len(), 1);
        assert_eq!(d.resolved_findings[0].code, "migration_legacy");
    }
}
