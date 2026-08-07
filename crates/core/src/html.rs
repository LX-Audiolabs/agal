use serde::Serialize;
use std::fs;
use std::path::Path;

use crate::config::ProjectConfig;
use crate::findings::Finding;

#[derive(Serialize)]
struct VizDependency {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
}

#[derive(Serialize)]
struct VizNode {
    id: String,
    label: String,
    kind: String,
    frameworks: Vec<String>,
    migration_status: Option<String>,
    color: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    border_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    internal_deps: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    external_flags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dependency_names: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dependency_details: Option<Vec<VizDependency>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ast_summary: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct VizEdge {
    source: String,
    target: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

/// Metadata shown in the HTML chrome (title, sidebar meta line).
pub struct HtmlMeta<'a> {
    pub project_name: &'a str,
    pub generated_at: &'a str,
    pub graph_version: &'a str,
    /// View default from config (None = auto-detect).
    pub view_default: Option<&'a str>,
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn render_html(
    _project_root: &Path,
    nodes: &[super::Node],
    edges: &[super::Edge],
    _frameworks: &[super::Framework],
    _project_config: &ProjectConfig,
    findings: &[Finding],
    meta: &HtmlMeta<'_>,
) -> Result<String, String> {
    let viz_nodes: Vec<VizNode> = nodes
        .iter()
        .map(|n| {
            let (color, border_color) = node_colors(n);
            VizNode {
                id: n.id.clone(),
                label: n.name.clone(),
                kind: n.kind.clone(),
                frameworks: n.frameworks.clone(),
                migration_status: n.migration_status.clone(),
                color,
                border_color,
                version: n.version.clone(),
                description: n.description.clone(),
                internal_deps: if n.internal_deps.is_empty() {
                    None
                } else {
                    Some(n.internal_deps.iter().cloned().collect())
                },
                external_flags: if n.external_flags.is_empty() {
                    None
                } else {
                    Some(n.external_flags.iter().cloned().collect())
                },
                dependency_names: if n.dependency_names.is_empty() {
                    None
                } else {
                    Some(n.dependency_names.iter().cloned().collect())
                },
                dependency_details: if n.dependency_details.is_empty() {
                    None
                } else {
                    Some(
                        n.dependency_details
                            .iter()
                            .map(|d| VizDependency {
                                name: d.name.clone(),
                                version: d.version.clone(),
                                source: d.source.clone(),
                            })
                            .collect(),
                    )
                },
                ast_summary: n
                    .ast_summary
                    .as_ref()
                    .map(|s| serde_json::to_value(s).unwrap_or_default()),
            }
        })
        .collect();

    let viz_edges: Vec<VizEdge> = edges
        .iter()
        .map(|e| VizEdge {
            source: e.from.clone(),
            target: e.to.clone(),
            kind: e.kind.clone(),
            note: e.note.clone(),
        })
        .collect();

    let nodes_json = serde_json::to_string(&viz_nodes)
        .map_err(|e| format!("failed to serialize viz nodes: {}", e))?;
    let edges_json = serde_json::to_string(&viz_edges)
        .map_err(|e| format!("failed to serialize viz edges: {}", e))?;
    let findings_json = serde_json::to_string(findings)
        .map_err(|e| format!("failed to serialize findings: {}", e))?;

    let html = HTML_TEMPLATE
        .replace("{{NODES_JSON}}", &nodes_json)
        .replace("{{EDGES_JSON}}", &edges_json)
        .replace("{{FINDINGS_JSON}}", &findings_json)
        .replace("{{PROJECT_NAME}}", &html_escape(meta.project_name))
        .replace("{{GENERATED_AT}}", &html_escape(meta.generated_at))
        .replace("{{GRAPH_VERSION}}", &html_escape(meta.graph_version))
        .replace(
            "{{VIEW_CONFIG}}",
            &serde_json::to_string(&serde_json::json!({
                "default": meta.view_default
            }))
            .unwrap_or_default(),
        );

    Ok(html)
}

pub fn write_html(
    project_root: &Path,
    nodes: &[super::Node],
    edges: &[super::Edge],
    frameworks: &[super::Framework],
    project_config: &ProjectConfig,
    findings: &[Finding],
    meta: &HtmlMeta<'_>,
) -> Result<(), String> {
    let html = render_html(
        project_root,
        nodes,
        edges,
        frameworks,
        project_config,
        findings,
        meta,
    )?;
    let output_dir = project_config.output_dir.as_deref().unwrap_or("agal");
    let output_path = project_root.join(output_dir).join("agal.html");
    fs::write(&output_path, html)
        .map_err(|e| format!("failed to write {}: {}", output_path.display(), e))?;
    Ok(())
}

fn node_colors(node: &super::Node) -> (String, Option<String>) {
    let fill = match node.kind.as_str() {
        "plugin" => "#8b5cf6",
        "crate" => "#64748b",
        "member" => "#475569",
        _ => "#475569",
    };
    let border = match node.migration_status.as_deref() {
        Some("migrated") => Some("#10b981".to_string()),
        Some("legacy") => Some("#f59e0b".to_string()),
        _ => None,
    };
    (fill.to_string(), border)
}

const HTML_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta name="color-scheme" content="dark">
  <title>{{PROJECT_NAME}} · Agentic Audiolabs</title>
  <script src="https://unpkg.com/cytoscape@3.26.0/dist/cytoscape.min.js"></script>
  <style>
    :root {
      --bg: #0b0c10;
      --bg-elevated: #111318;
      --panel: #151922;
      --panel-2: #1c212e;
      --panel-3: #252b3a;
      --text: #e2e8f0;
      --text-bright: #f8fafc;
      --text-dim: #94a3b8;
      --text-muted: #64748b;
      --accent: #6366f1;
      --accent-hover: #818cf8;
      --border: #252b3a;
      --border-soft: #1c212e;
      --ok: #10b981;
      --warn: #f59e0b;
      --err: #ef4444;
      --info: #38bdf8;
      --plugin: #8b5cf6;
      --crate: #64748b;
      --edge-dep: #334155;
      --edge-ui: #6366f1;
      --edge-ipc: #a855f7;
      --edge-rt: #06b6d4;
      --radius: 12px;
      --radius-sm: 8px;
      --font: "Inter", "Segoe UI", system-ui, -apple-system, sans-serif;
      --mono: "JetBrains Mono", "Cascadia Code", "SF Mono", ui-monospace, monospace;
      --shadow: 0 10px 40px rgba(0,0,0,.45);
      --shadow-sm: 0 4px 16px rgba(0,0,0,.3);
    }
    * { box-sizing: border-box; }
    html, body {
      margin: 0;
      padding: 0;
      font-family: var(--font);
      background: var(--bg);
      color: var(--text);
      height: 100vh;
      width: 100vw;
      overflow: hidden;
      -webkit-font-smoothing: antialiased;
    }
    #app {
      position: relative;
      width: 100%;
      height: 100%;
    }
    #cy {
      position: absolute;
      inset: 0;
      background:
        radial-gradient(ellipse 70% 50% at 50% 30%, #13161d 0%, transparent 60%),
        var(--bg);
      background-image:
        linear-gradient(rgba(99,102,241,0.025) 1px, transparent 1px),
        linear-gradient(90deg, rgba(99,102,241,0.025) 1px, transparent 1px);
      background-size: 32px 32px;
      z-index: 1;
    }

    /* Floating cards */
    .floating-card {
      position: absolute;
      z-index: 10;
      background: rgba(21, 25, 34, 0.92);
      backdrop-filter: blur(14px);
      -webkit-backdrop-filter: blur(14px);
      border: 1px solid var(--border);
      border-radius: var(--radius);
      box-shadow: var(--shadow);
      padding: 16px;
      min-width: 220px;
      max-width: 340px;
      transition: transform .18s ease, opacity .18s ease;
    }
    .floating-card.collapsed {
      opacity: 0.85;
    }
    .floating-card.collapsed .card-body {
      display: none;
    }
    .card-header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
      margin-bottom: 12px;
    }
    .floating-card.collapsed .card-header {
      margin-bottom: 0;
    }
    .card-title {
      font-size: 12px;
      font-weight: 700;
      letter-spacing: 0.08em;
      text-transform: uppercase;
      color: var(--text-bright);
    }
    .card-toggle {
      background: transparent;
      border: none;
      color: var(--text-muted);
      cursor: pointer;
      font-size: 14px;
      padding: 0;
      width: 20px;
      height: 20px;
      display: flex;
      align-items: center;
      justify-content: center;
      border-radius: 4px;
    }
    .card-toggle:hover { color: var(--text); background: var(--panel-2); }

    #controls-card { top: 18px; left: 18px; }
    #overview-card { top: 18px; right: 18px; text-align: right; }
    #findings-card { bottom: 18px; left: 18px; max-height: 32vh; display: flex; flex-direction: column; }
    #findings-card .card-body { overflow-y: auto; flex: 1; }
    #legend-card { bottom: 18px; right: 18px; }

    .brand {
      font-size: 15px;
      font-weight: 800;
      letter-spacing: -0.03em;
      color: var(--text-bright);
      margin-bottom: 2px;
    }
    .brand span { color: var(--accent); }
    .project-name {
      font-size: 13px;
      font-weight: 600;
      color: var(--text);
      word-break: break-word;
      line-height: 1.3;
      margin-bottom: 6px;
    }
    .meta-line {
      font-size: 10px;
      color: var(--text-muted);
    }

    .metric-grid {
      display: grid;
      grid-template-columns: repeat(2, 1fr);
      gap: 10px;
      margin-top: 12px;
    }
    .metric {
      background: var(--panel-2);
      border: 1px solid var(--border-soft);
      border-radius: var(--radius-sm);
      padding: 10px 12px;
      text-align: left;
    }
    .metric-label {
      font-size: 10px;
      text-transform: uppercase;
      letter-spacing: 0.06em;
      color: var(--text-muted);
      margin-bottom: 4px;
    }
    .metric-value {
      font-size: 18px;
      font-weight: 700;
      color: var(--text-bright);
      font-family: var(--mono);
    }
    .metric-value.err { color: var(--err); }
    .metric-value.warn { color: var(--warn); }
    .metric-value.ok { color: var(--ok); }

    .chip-row { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 6px; margin-top: 12px; }
    .chip {
      font-size: 10px;
      font-weight: 700;
      letter-spacing: 0.04em;
      text-transform: uppercase;
      padding: 4px 8px;
      border-radius: 999px;
      border: 1px solid var(--border);
      color: var(--text-dim);
      background: var(--panel-2);
    }
    .chip-ok { border-color: rgba(16,185,129,0.35); color: var(--ok); background: rgba(16,185,129,0.08); }
    .chip-warn { border-color: rgba(245,158,11,0.35); color: var(--warn); background: rgba(245,158,11,0.08); }
    .chip-err { border-color: rgba(239,68,68,0.35); color: var(--err); background: rgba(239,68,68,0.08); }

    .filter-group { margin-bottom: 12px; }
    .filter-group label {
      display: block;
      font-size: 10px;
      color: var(--text-muted);
      margin-bottom: 5px;
      text-transform: uppercase;
      letter-spacing: 0.08em;
      font-weight: 700;
    }
    select, input[type="text"] {
      width: 100%;
      padding: 9px 11px;
      background: var(--panel-2);
      border: 1px solid var(--border);
      color: var(--text-bright);
      border-radius: var(--radius-sm);
      font-family: inherit;
      font-size: 13px;
      outline: none;
      transition: border-color .15s, box-shadow .15s;
    }
    select:focus, input[type="text"]:focus {
      border-color: var(--accent);
      box-shadow: 0 0 0 3px rgba(99,102,241,.12);
    }
    select option { background: var(--panel-2); color: var(--text-bright); }
    .check-row {
      display: flex;
      flex-direction: column;
      gap: 6px;
      margin-bottom: 12px;
    }
    .check-row label {
      display: flex;
      align-items: center;
      gap: 8px;
      font-size: 12px;
      color: var(--text-dim);
      text-transform: none;
      letter-spacing: 0;
      font-weight: 500;
      cursor: pointer;
      margin: 0;
    }
    .check-row input { width: auto; accent-color: var(--accent); }
    .btn {
      width: 100%;
      padding: 9px 12px;
      background: var(--panel-2);
      border: 1px solid var(--border);
      color: var(--text-dim);
      border-radius: var(--radius-sm);
      font-family: inherit;
      font-size: 12px;
      font-weight: 600;
      cursor: pointer;
      transition: all .15s;
    }
    .btn:hover { border-color: var(--accent); color: var(--text-bright); background: var(--panel-3); }
    .btn-primary {
      background: var(--accent);
      border-color: var(--accent);
      color: #fff;
    }
    .btn-primary:hover { background: var(--accent-hover); border-color: var(--accent-hover); color: #fff; }

    .legend { display: flex; flex-wrap: wrap; gap: 8px; }
    .legend-item {
      display: flex;
      align-items: center;
      gap: 6px;
      font-size: 11px;
      color: var(--text-dim);
      background: var(--panel-2);
      border: 1px solid var(--border-soft);
      border-radius: 999px;
      padding: 4px 10px;
    }
    .legend-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
    .legend-line {
      width: 14px;
      height: 0;
      border-top: 2px solid;
      flex-shrink: 0;
    }
    .legend-line.dashed { border-top-style: dashed; }
    .legend-line.dotted { border-top-style: dotted; }

    /* Findings list */
    .findings-summary {
      display: flex;
      gap: 8px;
      margin-bottom: 10px;
    }
    .severity-pill {
      font-size: 10px;
      font-weight: 700;
      padding: 3px 8px;
      border-radius: 999px;
      text-transform: uppercase;
      letter-spacing: 0.04em;
    }
    .finding-group { margin-bottom: 10px; }
    .finding-group-title {
      font-size: 10px;
      text-transform: uppercase;
      letter-spacing: 0.08em;
      color: var(--text-muted);
      font-weight: 700;
      margin-bottom: 6px;
    }
    .finding-item {
      font-size: 11px;
      margin-bottom: 6px;
      line-height: 1.4;
      padding: 8px 10px;
      border-radius: var(--radius-sm);
      background: var(--panel-2);
      border: 1px solid var(--border-soft);
      color: var(--text-dim);
      cursor: pointer;
      transition: border-color .12s, background .12s;
    }
    .finding-item:hover { border-color: var(--border); background: var(--panel-3); }
    .finding-item b { color: var(--text-bright); font-weight: 600; }
    .finding-item .node-ref { color: var(--text-muted); }

    /* Detail drawer */
    #detail-drawer {
      position: absolute;
      top: 0;
      right: 0;
      width: 420px;
      max-width: 92vw;
      height: 100%;
      background: rgba(17, 19, 24, 0.97);
      backdrop-filter: blur(18px);
      -webkit-backdrop-filter: blur(18px);
      border-left: 1px solid var(--border);
      box-shadow: var(--shadow);
      z-index: 20;
      transform: translateX(100%);
      transition: transform .22s cubic-bezier(.2,.8,.2,1);
      display: flex;
      flex-direction: column;
    }
    #detail-drawer.open { transform: translateX(0); }
    #drawer-resizer {
      position: absolute;
      top: 0;
      left: -4px;
      width: 8px;
      height: 100%;
      cursor: ew-resize;
      z-index: 30;
      touch-action: none;
    }
    #drawer-resizer::after {
      content: "";
      position: absolute;
      top: 50%;
      left: 3px;
      width: 2px;
      height: 36px;
      margin-top: -18px;
      border-radius: 2px;
      background: var(--border);
      transition: background .12s;
    }
    #drawer-resizer:hover::after,
    body.drawer-resizing #drawer-resizer::after {
      background: var(--accent);
    }
    body.drawer-resizing {
      cursor: ew-resize;
      user-select: none;
      -webkit-user-select: none;
    }
    #drawer-header {
      padding: 20px 22px 14px;
      border-bottom: 1px solid var(--border-soft);
      flex-shrink: 0;
    }
    #drawer-header .name {
      font-size: 18px;
      font-weight: 700;
      color: var(--text-bright);
      letter-spacing: -0.02em;
      word-break: break-word;
      line-height: 1.25;
      margin-bottom: 10px;
    }
    #drawer-header .actions {
      display: flex;
      gap: 8px;
      flex-wrap: wrap;
    }
    #drawer-header .btn { width: auto; flex: 1 1 auto; }
    #drawer-close {
      position: absolute;
      top: 16px;
      right: 16px;
      background: var(--panel-2);
      border: 1px solid var(--border);
      color: var(--text-dim);
      cursor: pointer;
      font-size: 14px;
      width: 30px;
      height: 30px;
      border-radius: var(--radius-sm);
      display: flex;
      align-items: center;
      justify-content: center;
    }
    #drawer-close:hover { border-color: var(--accent); color: var(--text-bright); }

    #drawer-tabs {
      display: flex;
      flex-shrink: 0;
      gap: 0;
      padding: 0 16px;
      border-bottom: 1px solid var(--border-soft);
      background: var(--panel);
      overflow-x: auto;
    }
    #drawer-tabs button {
      appearance: none;
      background: transparent;
      border: none;
      border-bottom: 2px solid transparent;
      color: var(--text-muted);
      font-family: inherit;
      font-size: 12px;
      font-weight: 600;
      letter-spacing: 0.03em;
      padding: 12px 14px;
      cursor: pointer;
      white-space: nowrap;
      transition: color .12s, border-color .12s;
    }
    #drawer-tabs button:hover { color: var(--text); }
    #drawer-tabs button.active {
      color: var(--accent);
      border-bottom-color: var(--accent);
    }
    #drawer-tabs button .tab-count {
      display: inline-block;
      margin-left: 6px;
      font-size: 10px;
      font-weight: 700;
      background: var(--panel-2);
      color: var(--text-muted);
      padding: 1px 7px;
      border-radius: 999px;
    }
    #drawer-tabs button.active .tab-count { background: rgba(99,102,241,.18); color: var(--accent); }
    #drawer-tabs button:disabled { opacity: 0.32; cursor: default; }
    #drawer-tabs button.hidden { display: none; }
    #drawer-body {
      flex: 1 1 auto;
      min-height: 0;
      overflow: hidden;
      position: relative;
    }
    .drawer-tab-panel {
      display: none;
      height: 100%;
      overflow-y: auto;
      overflow-x: hidden;
      padding: 18px 22px 26px;
    }
    .drawer-tab-panel.active { display: block; }

    .section-title {
      font-size: 10px;
      text-transform: uppercase;
      color: var(--text-muted);
      letter-spacing: 0.1em;
      margin: 16px 0 8px 0;
      font-weight: 700;
    }
    .section-title:first-child { margin-top: 0; }
    .info-row {
      font-size: 13px;
      margin-bottom: 7px;
      word-break: break-word;
      overflow-wrap: anywhere;
      color: var(--text-bright);
      line-height: 1.45;
    }
    .info-row .key {
      color: var(--text-muted);
      display: inline-block;
      min-width: 96px;
      vertical-align: top;
      font-size: 10px;
      text-transform: uppercase;
      letter-spacing: 0.06em;
      font-weight: 700;
      padding-top: 2px;
    }
    .info-row code {
      font-family: var(--mono);
      font-size: 12px;
      color: var(--accent-hover);
      background: var(--panel-2);
      padding: 2px 6px;
      border-radius: 4px;
    }
    .badge {
      display: inline-block;
      padding: 3px 8px;
      border-radius: 999px;
      font-size: 10px;
      text-transform: uppercase;
      font-weight: 700;
      margin-right: 4px;
      margin-bottom: 3px;
      letter-spacing: 0.04em;
      vertical-align: middle;
    }
    .badge-legacy { background: rgba(245,158,11,.12); color: var(--warn); border: 1px solid rgba(245,158,11,.25); }
    .badge-migrated { background: rgba(16,185,129,.12); color: var(--ok); border: 1px solid rgba(16,185,129,.25); }
    .badge-framework { background: rgba(56,189,248,.10); color: var(--info); border: 1px solid rgba(56,189,248,.20); }
    .badge-plugin { background: rgba(139,92,246,.12); color: #c4b5fd; border: 1px solid rgba(139,92,246,.25); }
    .badge-crate { background: rgba(100,116,139,.12); color: #cbd5e1; border: 1px solid rgba(100,116,139,.25); }
    .badge-member { background: rgba(71,85,105,.12); color: #94a3b8; border: 1px solid rgba(71,85,105,.25); }
    .badge-error { background: rgba(239,68,68,.12); color: var(--err); border: 1px solid rgba(239,68,68,.25); }
    .badge-warn { background: rgba(245,158,11,.12); color: var(--warn); border: 1px solid rgba(245,158,11,.25); }
    .badge-info { background: rgba(56,189,248,.10); color: var(--info); border: 1px solid rgba(56,189,248,.20); }
    .badge-build { background: rgba(100,116,139,.10); color: #94a3b8; border: 1px dashed rgba(100,116,139,.35); }

    .comp-wrap { display: flex; flex-wrap: wrap; gap: 6px; }
    .comp-tag {
      display: inline-block;
      padding: 4px 10px;
      border-radius: 999px;
      font-size: 11px;
      background: rgba(139,92,246,.10);
      color: #c4b5fd;
      border: 1px solid rgba(139,92,246,.20);
      font-weight: 600;
      word-break: break-word;
    }
    .comp-tag.missing { background: rgba(239,68,68,.10); color: var(--err); text-decoration: line-through; border-color: rgba(239,68,68,.20); }

    .file-list { margin-top: 4px; }
    .file-group { margin-bottom: 12px; }
    .file-group-title {
      color: var(--text-muted);
      font-size: 10px;
      text-transform: uppercase;
      letter-spacing: 0.08em;
      margin-bottom: 5px;
      font-weight: 700;
    }
    .file-item {
      font-size: 12px;
      color: var(--text-bright);
      font-family: var(--mono);
      opacity: 0.92;
      word-break: break-all;
      padding: 3px 0;
    }
    .file-ext {
      display: inline-block;
      min-width: 30px;
      font-size: 9px;
      padding: 1px 5px;
      border-radius: 4px;
      text-transform: uppercase;
      text-align: center;
      margin-right: 6px;
      font-weight: 700;
      letter-spacing: 0.04em;
      font-family: var(--font);
    }
    .ext-rs { background: rgba(56,189,248,.12); color: var(--info); }
    .ext-slint { background: rgba(245,158,11,.12); color: var(--warn); }
    .ext-toml { background: rgba(100,116,139,.12); color: var(--text-dim); }
    .ext-other { background: var(--panel-3); color: var(--text-muted); }
    .dim { opacity: 0.6; color: var(--text-muted); }

    .focus-cards { display: flex; flex-wrap: wrap; gap: 12px; }
    .focus-cards .focus-card { min-width: 220px; max-width: 320px; flex: 1 1 240px; }
    .focus-card {
      background: var(--panel-2);
      border: 1px solid var(--border);
      border-radius: var(--radius-sm);
      padding: 14px;
      font-size: 12px;
      box-shadow: var(--shadow-sm);
    }
    .focus-card .fc-header {
      font-weight: 600;
      color: var(--text-bright);
      margin-bottom: 10px;
      display: flex;
      align-items: center;
      gap: 6px;
    }

    /* Scrollbars */
    .drawer-tab-panel::-webkit-scrollbar,
    #findings-card .card-body::-webkit-scrollbar,
    #drawer-tabs::-webkit-scrollbar {
      width: 8px;
      height: 8px;
    }
    .drawer-tab-panel::-webkit-scrollbar-thumb,
    #findings-card .card-body::-webkit-scrollbar-thumb,
    #drawer-tabs::-webkit-scrollbar-thumb {
      background: var(--panel-3);
      border-radius: 4px;
    }
    .drawer-tab-panel::-webkit-scrollbar-thumb:hover,
    #findings-card .card-body::-webkit-scrollbar-thumb:hover {
      background: var(--accent);
    }

    .graph-hint {
      position: absolute;
      left: 18px;
      bottom: 18px;
      z-index: 5;
      font-size: 11px;
      color: var(--text-muted);
      pointer-events: none;
      letter-spacing: 0.03em;
    }

    @media (max-width: 760px) {
      .floating-card { max-width: 86vw; }
      #controls-card { top: 12px; left: 12px; }
      #overview-card { top: auto; bottom: 12px; right: 12px; left: 12px; text-align: left; }
      #overview-card .metric-grid { grid-template-columns: repeat(4, 1fr); }
      #overview-card .chip-row { justify-content: flex-start; }
      #findings-card { display: none; }
      #legend-card { display: none; }
      #detail-drawer { width: 100%; }
      #drawer-resizer { display: none; }
    }
  </style>
</head>
<body>
  <div id="app">
    <div id="cy"></div>
    <div class="graph-hint">click node · drag · scroll zoom · esc closes details</div>

    <div class="floating-card" id="controls-card">
      <div class="card-header">
        <div class="brand"><span>Agentic</span> Audiolabs</div>
        <button class="card-toggle" data-target="controls-card" aria-label="collapse">−</button>
      </div>
      <div class="card-body">
        <div class="project-name">{{PROJECT_NAME}}</div>
        <div class="meta-line">v{{GRAPH_VERSION}} · {{GENERATED_AT}}</div>
        <div class="filter-group" style="margin-top:14px">
          <label>Search</label>
          <input id="search" type="text" placeholder="node or crate…">
        </div>
        <div class="filter-group">
          <label>View</label>
          <select id="kind-filter">
            <option value="overview">overview (hubs)</option>
            <option value="all">all nodes</option>
            <option value="plugin">plugins</option>
            <option value="crate">crates</option>
          </select>
        </div>
        <div class="filter-group">
          <label>Framework</label>
          <select id="framework-filter"><option value="all">all</option></select>
        </div>
        <div class="check-row">
          <label><input type="checkbox" id="show-build-edges"> show build edges</label>
          <label><input type="checkbox" id="show-info-findings"> show info findings</label>
        </div>
        <button class="btn" id="reset-btn">Reset view</button>
      </div>
    </div>

    <div class="floating-card" id="overview-card">
      <div class="card-header">
        <div class="card-title">Overview</div>
        <button class="card-toggle" data-target="overview-card" aria-label="collapse">−</button>
      </div>
      <div class="card-body">
        <div class="metric-grid" id="overview-metrics"></div>
        <div class="chip-row" id="overview-chips"></div>
      </div>
    </div>

    <div class="floating-card collapsed" id="findings-card">
      <div class="card-header">
        <div class="card-title">Findings</div>
        <button class="card-toggle" data-target="findings-card" aria-label="expand">+</button>
      </div>
      <div class="card-body" id="findings-body"></div>
    </div>

    <div class="floating-card collapsed" id="legend-card">
      <div class="card-header">
        <div class="card-title">Legend</div>
        <button class="card-toggle" data-target="legend-card" aria-label="expand">+</button>
      </div>
      <div class="card-body">
        <div class="legend" id="legend"></div>
        <div class="legend" id="edge-legend" style="margin-top:10px"></div>
      </div>
    </div>

    <div id="detail-drawer">
      <div id="drawer-resizer" title="Drag to resize"></div>
      <button id="drawer-close" aria-label="close">✕</button>
      <div id="drawer-header"></div>
      <div id="drawer-tabs" role="tablist"></div>
      <div id="drawer-body"></div>
    </div>
  </div>

  <script>
    function initGraph() {
      const nodes = {{NODES_JSON}};
      const edges = {{EDGES_JSON}};
      const findings = {{FINDINGS_JSON}};
      const viewConfig = {{VIEW_CONFIG}};
      let cy = null;
      let selectedId = null;
      let activeTab = 'info';

      const tabConfig = [
        { key: 'info', label: 'Info' },
        { key: 'files', label: 'Files' },
        { key: 'comps', label: 'Components' },
        { key: 'deps', label: 'Dependencies' },
        { key: 'params', label: 'Params' },
        { key: 'focus', label: 'Focus' },
      ];

      function qs(sel) { return document.querySelector(sel); }
      function qsa(sel) { return document.querySelectorAll(sel); }

      function setTabCounts(counts) {
        document.querySelectorAll('#drawer-tabs button[data-tab]').forEach(btn => {
          const key = btn.dataset.tab;
          const n = counts[key];
          const base = tabConfig.find(t => t.key === key).label;
          const wasActive = btn.classList.contains('active');
          btn.innerHTML = (n == null || n === '') ? base : `${base}<span class="tab-count">${n}</span>`;
          if (wasActive) btn.classList.add('active');
        });
      }

      function switchTab(tab) {
        activeTab = tab;
        qsa('#drawer-tabs button[data-tab]').forEach(btn => {
          btn.classList.toggle('active', btn.dataset.tab === tab);
        });
        qsa('.drawer-tab-panel').forEach(panel => {
          panel.classList.toggle('active', panel.id === 'tab-' + tab);
        });
      }

      document.getElementById('drawer-tabs').addEventListener('click', (ev) => {
        const btn = ev.target.closest('button[data-tab]');
        if (!btn || btn.disabled || btn.classList.contains('hidden')) return;
        switchTab(btn.dataset.tab);
      });

      function relevantFiles(n) {
        const files = (n.ast_summary && n.ast_summary.files) || [];
        return files.filter(f =>
          f.startsWith('src/') ||
          f.startsWith('ui/') ||
          f.startsWith('assets/') ||
          f === 'Cargo.toml' ||
          f.endsWith('.rs') ||
          f.endsWith('.slint')
        );
      }

      function fileExtBadge(filename) {
        if (filename.endsWith('.rs')) return '<span class="file-ext ext-rs">rs</span>';
        if (filename.endsWith('.slint')) return '<span class="file-ext ext-slint">slt</span>';
        if (filename.endsWith('.toml')) return '<span class="file-ext ext-toml">toml</span>';
        const dot = filename.lastIndexOf('.');
        if (dot > 0 && filename.length - dot <= 6) {
          return '<span class="file-ext ext-other">' + filename.slice(dot + 1) + '</span>';
        }
        return '';
      }

      function groupFiles(files) {
        const groups = {};
        files.forEach(f => {
          const slash = f.indexOf('/');
          const dir = slash >= 0 ? f.slice(0, slash) : '(root)';
          (groups[dir] = groups[dir] || []).push(f);
        });
        return groups;
      }

      function renderFileTree(files) {
        if (!files.length) return '<div class="dim">no files indexed</div>';
        const groups = groupFiles(files);
        let html = '<div class="file-list">';
        Object.keys(groups).sort().forEach(dir => {
          const dirCount = groups[dir].length;
          html += `<div class="file-group"><div class="file-group-title">${dir}/ <span style="opacity:0.5">(${dirCount})</span></div>`;
          groups[dir].sort().forEach(f => {
            const sub = f.includes('/') ? f.slice(f.indexOf('/') + 1) : f;
            const badge = fileExtBadge(f);
            html += `<div class="file-item">${badge}${sub}</div>`;
          });
          html += '</div>';
        });
        html += '</div>';
        return html;
      }

      function createTabPanels() {
        const body = document.getElementById('drawer-body');
        body.innerHTML = '';
        tabConfig.forEach(t => {
          const div = document.createElement('div');
          div.id = 'tab-' + t.key;
          div.className = 'drawer-tab-panel';
          div.setAttribute('role', 'tabpanel');
          body.appendChild(div);
        });
      }

      function createTabButtons() {
        const tabs = document.getElementById('drawer-tabs');
        tabs.innerHTML = '';
        tabConfig.forEach(t => {
          const btn = document.createElement('button');
          btn.type = 'button';
          btn.setAttribute('role', 'tab');
          btn.dataset.tab = t.key;
          btn.textContent = t.label;
          if (t.key === 'info') btn.classList.add('active');
          tabs.appendChild(btn);
        });
      }

      function openDrawer() {
        document.getElementById('detail-drawer').classList.add('open');
      }

      function closeDrawer() {
        document.getElementById('detail-drawer').classList.remove('open');
        selectedId = null;
        if (cy) cy.nodes().unselect();
      }

      function showDetail(n) {
        selectedId = n.id;
        const ast = n.ast_summary || {};
        const comps = ast.slint_components || [];
        const files = relevantFiles(n);
        const outgoing = edges.filter(e => e.source === n.id).map(e => e.target);
        const incoming = edges.filter(e => e.target === n.id).map(e => e.source);
        const internal = n.internal_deps || [];
        const external = n.external_flags || [];
        const depDetails = n.dependency_details || [];
        const depCount = internal.length + external.length + depDetails.length;

        const paramRows = [];
        const pf = ast.params_fields || {};
        Object.keys(pf).sort().forEach(structName => {
          (pf[structName] || []).forEach(f => {
            paramRows.push({ struct: structName, ...f });
          });
        });

        createTabPanels();
        createTabButtons();

        // Header
        const header = document.getElementById('drawer-header');
        header.innerHTML = `
          <div class="name">${n.label}</div>
          <div class="actions">
            <span class="badge badge-${n.kind}">${n.kind}</span>
            ${n.migration_status ? `<span class="badge badge-${n.migration_status}">${n.migration_status}</span>` : ''}
            ${n.frameworks.map(f => `<span class="badge badge-framework">${f}</span>`).join('')}
            <button class="btn btn-primary" id="focus-btn">Focus + deps</button>
          </div>
        `;
        document.getElementById('focus-btn').addEventListener('click', () => focusNode(n.id));

        // Info tab
        const roles = ast.file_roles || {};
        const roleList = Object.keys(roles).map(f => `${f} → ${roles[f]}`);
        const ipc = (ast.ipc_signals || []).slice().sort();
        const hooks = (ast.process_hooks || []).slice();
        const formats = (ast.plugin_formats || []).slice().sort();
        const features = ast.features || {};
        const featureKeys = Object.keys(features).sort();
        document.getElementById('tab-info').innerHTML = `
          <div class="info-row"><span class="key">ID</span> ${n.id}</div>
          <div class="info-row"><span class="key">Version</span> ${n.version || '—'}</div>
          ${n.description ? `<div class="info-row"><span class="key">Desc</span> ${n.description}</div>` : ''}
          ${formats.length ? `<div class="info-row"><span class="key">Formats</span> ${formats.map(f => `<span class="badge badge-info">${f}</span>`).join(' ')}</div>` : ''}
          <div class="info-row"><span class="key">Depends on</span> ${outgoing.length ? outgoing.map(t => {
            const tgt = nodes.find(x => x.id === t);
            return tgt ? `<span class="badge badge-${tgt.kind}">${tgt.label}</span>` : t;
          }).join(' ') : '<span class="dim">none</span>'}</div>
          <div class="info-row"><span class="key">Used by</span> ${incoming.length ? incoming.map(t => {
            const tgt = nodes.find(x => x.id === t);
            return tgt ? `<span class="badge badge-${tgt.kind}">${tgt.label}</span>` : t;
          }).join(' ') : '<span class="dim">none</span>'}</div>
          ${ipc.length ? `<div class="info-row"><span class="key">IPC</span> ${ipc.map(s => `<span class="badge badge-framework">${s}</span>`).join(' ')}</div>` : ''}
          ${hooks.length ? `<div class="info-row"><span class="key">Process</span> ${hooks.join(', ')}</div>` : ''}
          ${(ast.plugin_logic_impls || []).length ? `<div class="info-row"><span class="key">Logic</span> ${(ast.plugin_logic_impls || []).join(', ')}</div>` : ''}
          ${featureKeys.length ? `<div class="section-title">Features</div>${featureKeys.map(k => `<div class="info-row"><code>${k}</code> → ${(features[k]||[]).join(', ') || '<span class="dim">—</span>'}</div>`).join('')}` : ''}
          ${roleList.length ? `<div class="section-title">File roles</div>${roleList.map(r => `<div class="info-row">${r}</div>`).join('')}` : ''}
        `;

        // Files tab
        document.getElementById('tab-files').innerHTML = renderFileTree(files);

        // Components tab
        document.getElementById('tab-comps').innerHTML = comps.length
          ? `<div class="comp-wrap">${comps.slice().sort().map(c => `<span class="comp-tag">${c}</span>`).join('')}</div>`
          : '<span class="dim">none detected</span>';

        // Deps tab
        const depLines = [];
        if (internal.length) {
          depLines.push('<div class="section-title">Internal</div>');
          const buildTargets = new Set(edges
            .filter(e => e.source === n.id && (e.kind === 'build_depends_on' || e.kind === 'dev_depends_on'))
            .map(e => e.target));
          internal.slice().sort().forEach(d => {
            const short = d.replace(/^plugins\//, '').replace(/^crates\//, '');
            const badge = buildTargets.has(d) ? ' <span class="badge badge-build">build</span>' : '';
            depLines.push(`<div class="info-row">${short}${badge}</div>`);
          });
        }
        if (external.length) {
          depLines.push('<div class="section-title">External flags</div>');
          external.slice().sort().forEach(d => {
            depLines.push(`<div class="info-row"><span class="badge badge-framework">${d}</span></div>`);
          });
        }
        if (depDetails.length) {
          depLines.push('<div class="section-title">Cargo deps</div>');
          depDetails.forEach(d => {
            const version = d.version ? ` <code>${d.version}</code>` : '';
            const source = d.source ? ` <span class="dim">(${d.source})</span>` : '';
            depLines.push(`<div class="info-row">${d.name}${version}${source}</div>`);
          });
        }
        document.getElementById('tab-deps').innerHTML = depLines.length
          ? depLines.join('')
          : '<span class="dim">none</span>';

        // Params tab
        if (paramRows.length) {
          let html = '';
          let lastStruct = '';
          paramRows.forEach(p => {
            if (p.struct !== lastStruct) {
              html += `<div class="section-title">${p.struct}</div>`;
              lastStruct = p.struct;
            }
            const hidden = p.hidden ? ' <span class="dim">(hidden)</span>' : '';
            const dn = p.display_name ? ` — ${p.display_name}` : '';
            html += `<div class="info-row"><code>${p.name}</code>: ${p.ty || '…'}${dn}${hidden}</div>`;
          });
          document.getElementById('tab-params').innerHTML = html;
        } else {
          document.getElementById('tab-params').innerHTML = '<span class="dim">no params surface</span>';
        }

        // Focus tab initial
        document.getElementById('tab-focus').innerHTML = '<span class="dim">Click “Focus + deps” to load neighbor summary</span>';

        // Hide tabs with no content
        qsa('#drawer-tabs button[data-tab]').forEach(btn => {
          const key = btn.dataset.tab;
          let has = true;
          if (key === 'files') has = files.length > 0;
          else if (key === 'comps') has = comps.length > 0;
          else if (key === 'deps') has = depCount > 0;
          else if (key === 'params') has = paramRows.length > 0;
          else if (key === 'focus') has = false; // starts disabled
          btn.classList.toggle('hidden', !has);
          btn.disabled = key === 'focus';
        });

        setTabCounts({
          info: null,
          files: files.length,
          comps: comps.length,
          deps: depCount,
          params: paramRows.length,
          focus: null,
        });

        const prefer = activeTab === 'focus' ? 'info' : activeTab;
        switchTab(prefer);
        openDrawer();
        if (cy) cy.resize();
      }

      function focusNode(id) {
        const n = nodes.find(x => x.id === id);
        if (!n || !cy) return;
        showDetail(n);
        cy.nodes().unselect();
        const node = cy.getElementById(id);
        if (node && node.length) node.select();
        const neighborIds = new Set([id]);
        edges.forEach(e => {
          if (e.source === id) neighborIds.add(e.target);
          if (e.target === id) neighborIds.add(e.source);
        });
        const focusedNodes = nodes.filter(x => neighborIds.has(x.id));
        const toShow = cy.nodes().filter(n => neighborIds.has(n.data().id));
        cy.nodes().not(toShow).style('display', 'none');
        toShow.style('display', 'element');
        // Show every edge inside the focus neighborhood (incl. build/dev deps) —
        // applyFilters may have hidden them, leaving focused nodes visually orphaned.
        cy.edges().forEach(e => {
          const ok = neighborIds.has(e.data('source')) && neighborIds.has(e.data('target'));
          e.style('display', ok ? 'element' : 'none');
        });

        let cardsHtml = '<div class="focus-cards">';
        let cardCount = 0;
        focusedNodes.sort((a,b) => a.kind === b.kind ? a.label.localeCompare(b.label) : (a.kind === 'plugin' ? -1 : 1));
        focusedNodes.forEach(fn => {
          if (fn.id === id) return;
          cardCount++;
          const ff = relevantFiles(fn);
          const fc = (fn.ast_summary && fn.ast_summary.slint_components) || [];
          const dot = `<span style="display:inline-block;width:8px;height:8px;border-radius:50%;background:${fn.color};margin-right:4px;${fn.border_color ? 'border:2px solid '+fn.border_color : ''}"></span>`;
          cardsHtml += `
            <div class="focus-card">
              <div class="fc-header">${dot}${fn.label} <span class="badge badge-${fn.kind}">${fn.kind}</span></div>
              ${fc.length ? `<div style="margin-bottom:6px">${fc.slice().sort().map(c => `<span class="comp-tag">${c}</span>`).join('')}</div>` : ''}
              ${renderFileTree(ff)}
            </div>`;
        });
        cardsHtml += '</div>';
        document.getElementById('tab-focus').innerHTML = cardCount ? cardsHtml : '<span class="dim">no neighbors</span>';

        const focusBtn = qs('#drawer-tabs button[data-tab="focus"]');
        if (focusBtn) {
          focusBtn.disabled = false;
          focusBtn.classList.remove('hidden');
          focusBtn.innerHTML = `Focus<span class="tab-count">${cardCount}</span>`;
        }
        switchTab('focus');

        if (toShow.length > 0) {
          runDiskLayout(toShow, 'focus');
        }
      }

      // Compact square around viewport center. Side scales with node count
      // (never fill whole window — that made 12 nodes look like dust in a void).
      function compactBB(n) {
        const w = container.clientWidth || 800;
        const h = container.clientHeight || 600;
        // ~90–220px radius cluster; grow gently with n
        const side = Math.max(220, Math.min(480, 160 + n * 22));
        const s = Math.min(side, Math.min(w, h) * 0.62);
        return {
          x1: (w - s) / 2,
          y1: (h - s) / 2,
          w: s,
          h: s
        };
      }

      /**
       * Compact disk (not full-viewport explosion):
       * - overview/all: 2-ring concentric — crates center, plugins outer
       * - plugin|crate only: tight circle
       * - focus/search: cose in same compact box
       */
      function runDiskLayout(eles, mode) {
        if (!eles || eles.length === 0) return;
        const n = eles.length;
        const bb = compactBB(n);
        const pad = 36;

        if (mode === 'overview' || mode === 'all') {
          eles.layout({
            name: 'concentric',
            boundingBox: bb,
            fit: true,
            padding: pad,
            avoidOverlap: true,
            minNodeSpacing: 28,
            equidistant: false,
            spacingFactor: 0.9,
            startAngle: -Math.PI / 2,
            clockwise: true,
            nodeDimensionsIncludeLabels: true,
            // exactly two rings: hubs in, plugins out
            concentric: function(node) {
              return node.data('kind') === 'crate' ? 2 : 1;
            },
            levelWidth: function() { return 1; }
          }).run();
          return;
        }

        if (mode === 'plugin' || mode === 'crate') {
          const radius = Math.max(90, Math.min(200, 40 + n * 14));
          eles.layout({
            name: 'circle',
            boundingBox: bb,
            radius: radius,
            fit: true,
            padding: pad,
            avoidOverlap: true,
            spacingFactor: 0.95,
            startAngle: -Math.PI / 2,
            clockwise: true,
            nodeDimensionsIncludeLabels: true
          }).run();
          return;
        }

        // focus / search: compact force cloud
        eles.layout({
          name: 'cose',
          boundingBox: bb,
          fit: true,
          padding: pad,
          animate: false,
          randomize: false,
          componentSpacing: 24,
          nodeRepulsion: 450000,
          idealEdgeLength: 72,
          edgeElasticity: 80,
          nestingFactor: 1.2,
          gravity: 2.4,
          numIter: 1800,
          initialTemp: 120,
          coolingFactor: 0.95,
          minTemp: 1.0
        }).run();
      }

      function hubCrateIds() {
        // Crates that plugins/members depend on (not build-only noise).
        const showBuild = document.getElementById('show-build-edges').checked;
        const rootIds = new Set(nodes.filter(n => n.kind === 'plugin' || n.kind === 'member').map(n => n.id));
        const hubs = new Set();
        edges.forEach(e => {
          if ((e.kind === 'build_depends_on' || e.kind === 'dev_depends_on') && !showBuild) return;
          const srcRoot = rootIds.has(e.source);
          const tgtRoot = rootIds.has(e.target);
          if (srcRoot || tgtRoot) {
            const other = srcRoot ? e.target : e.source;
            const n = nodes.find(x => x.id === other);
            if (n && (n.kind === 'crate' || n.kind === 'member')) hubs.add(other);
          }
          // ipc_peer is plugin-plugin; runtime_depends_on to hubs
          if (e.kind === 'runtime_depends_on' || e.kind === 'uses_ui') {
            const n = nodes.find(x => x.id === e.target);
            if (n && n.kind === 'crate') hubs.add(e.target);
          }
        });
        return hubs;
      }

      function persistShowBuild() {
        try {
          localStorage.setItem('agal.showBuildEdges',
            document.getElementById('show-build-edges').checked ? '1' : '0');
        } catch (_) { /* file:// or restricted webview — ignore */ }
      }

      function restoreShowBuild() {
        try {
          const saved = localStorage.getItem('agal.showBuildEdges');
          if (saved !== null) document.getElementById('show-build-edges').checked = saved === '1';
        } catch (_) { /* ignore */ }
      }

      function edgeAllowed(kind) {
        const showBuild = document.getElementById('show-build-edges').checked;
        if (kind === 'build_depends_on' || kind === 'dev_depends_on') return showBuild;
        return true;
      }

      function resetFocus() {
        selectedId = null;
        if (!cy) return;
        closeDrawer();
        document.getElementById('kind-filter').value = defaultView();
        document.getElementById('framework-filter').value = 'all';
        document.getElementById('search').value = '';
        document.getElementById('show-build-edges').checked = false;
        persistShowBuild();
        applyFilters(true);
      }

      function defaultView() {
        if (viewConfig && viewConfig.default) return viewConfig.default;
        return nodes.some(n => n.kind === 'plugin') ? 'overview' : 'all';
      }

      function applyFilters(doLayout) {
        if (!cy) return;
        const kind = document.getElementById('kind-filter').value;
        const fw = document.getElementById('framework-filter').value;
        const term = document.getElementById('search').value.toLowerCase();
        const hubs = hubCrateIds();
        const pluginCount = nodes.filter(n => n.kind === 'plugin').length;

        const matchedIds = new Set();
        cy.nodes().forEach(n => {
          const d = n.data();
          let kindMatch = true;
          if (kind === 'plugin') kindMatch = d.kind === 'plugin';
          else if (kind === 'crate') kindMatch = d.kind === 'crate' || d.kind === 'member';
          else if (kind === 'overview') {
            if (pluginCount === 0) {
              // Framework repos with no plugins: overview = all.
              kindMatch = true;
            } else {
              kindMatch = d.kind === 'plugin' || hubs.has(d.id);
            }
          }
          // kind === 'all' → all kinds
          const fwMatch = fw === 'all' || (d.frameworks || []).includes(fw);
          const termMatch = !term || d.label.toLowerCase().includes(term) || d.id.toLowerCase().includes(term);
          if (kindMatch && fwMatch && termMatch) matchedIds.add(d.id);
        });

        // Expand 1-hop when searching so focus stays useful
        const idsToShow = new Set(matchedIds);
        if (term || fw !== 'all') {
          edges.forEach(e => {
            if (!edgeAllowed(e.kind)) return;
            if (matchedIds.has(e.source)) idsToShow.add(e.target);
            if (matchedIds.has(e.target)) idsToShow.add(e.source);
          });
        }

        const toShow = cy.nodes().filter(n => idsToShow.has(n.data().id));
        cy.nodes().not(toShow).style('display', 'none');
        toShow.style('display', 'element');
        cy.edges().forEach(e => {
          const s = e.data('source');
          const t = e.data('target');
          const k = e.data('kind');
          const ok = idsToShow.has(s) && idsToShow.has(t) && edgeAllowed(k);
          e.style('display', ok ? 'element' : 'none');
        });

        if (doLayout !== false && toShow.length > 0) {
          let mode = kind;
          if (term || fw !== 'all') mode = 'search';
          runDiskLayout(toShow, mode);
        } else if (toShow.length > 0) {
          cy.fit(toShow, 72);
        }
      }

      document.getElementById('reset-btn').addEventListener('click', resetFocus);
      document.getElementById('kind-filter').addEventListener('change', () => applyFilters(true));
      document.getElementById('framework-filter').addEventListener('change', () => applyFilters(true));
      document.getElementById('search').addEventListener('input', () => applyFilters(true));
      document.getElementById('show-build-edges').addEventListener('change', () => { persistShowBuild(); applyFilters(false); });
      document.getElementById('show-info-findings').addEventListener('change', () => renderFindings());
      document.getElementById('drawer-close').addEventListener('click', closeDrawer);

      document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape') closeDrawer();
      });

      document.getElementById('cy').addEventListener('click', (e) => {
        if (e.target.id === 'cy') closeDrawer();
      });

      // Floating card toggles
      document.querySelectorAll('.card-toggle').forEach(btn => {
        btn.addEventListener('click', () => {
          const card = document.getElementById(btn.dataset.target);
          card.classList.toggle('collapsed');
          btn.textContent = card.classList.contains('collapsed') ? '+' : '−';
          btn.setAttribute('aria-label', card.classList.contains('collapsed') ? 'expand' : 'collapse');
        });
      });

      // Framework options
      const frameworks = [...new Set(nodes.flatMap(n => n.frameworks))].sort();
      const frameworkSelect = document.getElementById('framework-filter');
      frameworks.forEach(f => {
        const opt = document.createElement('option');
        opt.value = f;
        opt.textContent = f;
        frameworkSelect.appendChild(opt);
      });

      // Legends
      const legend = document.getElementById('legend');
      const legendItems = [
        { label: 'plugin', color: '#8b5cf6' },
        { label: 'crate', color: '#64748b' },
        { label: 'migrated', color: '#10b981', ring: true },
        { label: 'legacy', color: '#f59e0b', ring: true },
      ];
      if (nodes.some(n => n.kind === 'member')) {
        legendItems.splice(2, 0, { label: 'member', color: '#475569' });
      }
      legendItems.forEach(item => {
        const div = document.createElement('div');
        div.className = 'legend-item';
        const style = item.ring ? `background:transparent;border:2px solid ${item.color}` : `background:${item.color}`;
        div.innerHTML = `<div class="legend-dot" style="${style}"></div><span>${item.label}</span>`;
        legend.appendChild(div);
      });

      const edgeLegend = document.getElementById('edge-legend');
      [
        { label: 'depends_on', color: '#334155', cls: '' },
        { label: 'build', color: '#334155', cls: 'dashed' },
        { label: 'uses_ui', color: '#6366f1', cls: 'dashed' },
        { label: 'ipc_peer', color: '#a855f7', cls: '' },
        { label: 'runtime', color: '#06b6d4', cls: 'dotted' },
      ].forEach(item => {
        const div = document.createElement('div');
        div.className = 'legend-item';
        div.innerHTML = `<div class="legend-line ${item.cls}" style="border-color:${item.color}"></div><span>${item.label}</span>`;
        edgeLegend.appendChild(div);
      });

      // Overview
      function renderOverview() {
        const plugins = nodes.filter(n => n.kind === 'plugin');
        const crates = nodes.filter(n => n.kind === 'crate');
        const members = nodes.filter(n => n.kind === 'member');
        const crateTotal = crates.length + members.length;
        const legacy = plugins.filter(n => n.migration_status === 'legacy').length;
        const migrated = plugins.filter(n => n.migration_status === 'migrated').length;
        const err = findings.filter(f => f.severity === 'error').length;
        const warn = findings.filter(f => f.severity === 'warn').length;
        const info = findings.filter(f => f.severity === 'info').length;
        const nodeLabel = plugins.length ? 'Plugins' : 'Nodes';

        document.getElementById('overview-metrics').innerHTML = `
          <div class="metric"><div class="metric-label">${nodeLabel}</div><div class="metric-value">${plugins.length || nodes.length}</div></div>
          <div class="metric"><div class="metric-label">Crates</div><div class="metric-value">${crateTotal}</div></div>
          <div class="metric"><div class="metric-label">Edges</div><div class="metric-value">${edges.length}</div></div>
          <div class="metric"><div class="metric-label">Findings</div><div class="metric-value ${err ? 'err' : warn ? 'warn' : ''}">${findings.length}</div></div>
        `;

        const chips = document.getElementById('overview-chips');
        let chipHtml = '';
        if (migrated) chipHtml += `<span class="chip chip-ok">${migrated} migrated</span>`;
        if (legacy) chipHtml += `<span class="chip chip-warn">${legacy} legacy</span>`;
        if (err) chipHtml += `<span class="chip chip-err">${err} errors</span>`;
        if (warn) chipHtml += `<span class="chip chip-warn">${warn} warnings</span>`;
        if (info) chipHtml += `<span class="chip">${info} infos</span>`;
        chips.innerHTML = chipHtml;
      }
      renderOverview();

      // Findings — error/warn first; info only if toggled (less noise)
      function renderFindings() {
        const el = document.getElementById('findings-body');
        if (!findings || !findings.length) {
          el.innerHTML = '<span class="dim">none</span>';
          return;
        }
        const showInfo = document.getElementById('show-info-findings').checked;
        const bySev = { error: [], warn: [], info: [] };
        findings.forEach(f => {
          const sev = (f.severity || 'info').toLowerCase();
          (bySev[sev] || bySev.info).push(f);
        });

        let html = '';
        const order = showInfo ? ['error', 'warn', 'info'] : ['error', 'warn'];
        order.forEach(sev => {
          const list = bySev[sev];
          if (!list.length) return;
          html += `<div class="finding-group"><div class="finding-group-title">${sev} (${list.length})</div>`;
          list.slice(0, 8).forEach(f => {
            const short = f.node ? f.node.replace(/^plugins\//, '').replace(/^crates\//, '') : '';
            const nodeRef = short ? ` <span class="node-ref">${short}</span>` : '';
            const pathLine = f.path ? `<br><span class="dim">${f.path}</span>` : '';
            const fixLine = f.fix ? `<br><span class="dim">fix: ${f.fix}</span>` : '';
            html += `<div class="finding-item" data-node="${f.node || ''}"><span class="badge badge-${sev}">${sev}</span><b>${f.code}</b>${nodeRef}<br>${f.message}${pathLine}${fixLine}</div>`;
          });
          if (list.length > 8) {
            html += `<div class="dim" style="font-size:11px;margin-top:4px">+ ${list.length - 8} more</div>`;
          }
          html += '</div>';
        });
        if (!showInfo && bySev.info.length) {
          html += `<div class="dim" style="font-size:11px;margin-top:8px">${bySev.info.length} info hidden — enable “show info findings”</div>`;
        }
        if (!html) {
          html = '<span class="dim">no error/warn — enable info if needed</span>';
        }
        el.innerHTML = html;

        el.querySelectorAll('.finding-item').forEach(item => {
          item.addEventListener('click', () => {
            const nodeId = item.dataset.node;
            if (!nodeId) return;
            const n = nodes.find(x => x.id === nodeId);
            if (n) showDetail(n);
          });
        });
      }
      renderFindings();

      // Cytoscape
      const container = document.getElementById('cy');
      const elements = [
        ...nodes.map(n => ({ data: { id: n.id, label: n.label, ...n } })),
        ...edges.map(e => ({
          data: {
            id: `${e.source}->${e.target}:${e.kind}`,
            source: e.source,
            target: e.target,
            kind: e.kind
          }
        }))
      ];

      cy = cytoscape({
        container: container,
        elements: elements,
        wheelSensitivity: 0.25,
        // Positions come from runDiskLayout (applyFilters) — avoid tall cose sausage.
        layout: { name: 'null' },
        style: [
          { selector: 'node', style: {
            'background-color': 'data(color)',
            'label': 'data(label)',
            'color': '#e2e8f0',
            'font-size': '13px',
            'font-weight': '600',
            'font-family': 'Inter, Segoe UI, system-ui, sans-serif',
            'text-valign': 'bottom',
            'text-halign': 'center',
            'text-margin-y': 8,
            'text-outline-color': '#0b0c10',
            'text-outline-width': 3,
            'width': 36,
            'height': 36,
            'border-width': 3,
            'border-color': '#0b0c10',
            'transition-property': 'background-color, border-color, width, height, opacity',
            'transition-duration': '0.18s'
          }},
          { selector: 'node[border_color]', style: {
            'border-color': 'data(border_color)',
            'border-width': 3
          }},
          { selector: 'node[kind = "plugin"]', style: { 'shape': 'round-rectangle' } },
          { selector: 'node[kind = "crate"]', style: { 'shape': 'ellipse' } },
          { selector: 'node[kind = "member"]', style: { 'shape': 'diamond' } },
          { selector: 'node:selected', style: {
            'border-color': '#6366f1',
            'border-width': 3,
            'width': 42,
            'height': 42,
            'color': '#f8fafc'
          }},
          { selector: 'node:active', style: {
            'overlay-opacity': 0.08,
            'overlay-color': '#6366f1'
          }},
          { selector: 'edge', style: {
            'width': 1.4,
            'line-color': '#334155',
            'target-arrow-color': '#334155',
            'target-arrow-shape': 'triangle',
            'curve-style': 'bezier',
            'arrow-scale': 0.75,
            'opacity': 0.9
          }},
          { selector: 'edge[kind = "build_depends_on"]', style: { 'line-style': 'dashed', 'line-color': '#334155', 'target-arrow-color': '#334155' } },
          { selector: 'edge[kind = "dev_depends_on"]', style: { 'line-style': 'dotted', 'line-color': '#334155', 'target-arrow-color': '#334155' } },
          { selector: 'edge[kind = "uses_ui"]', style: { 'line-color': '#6366f1', 'line-style': 'dashed', 'target-arrow-color': '#6366f1', 'width': 1.6 } },
          { selector: 'edge[kind = "ipc_peer"]', style: { 'line-color': '#a855f7', 'width': 2.4, 'target-arrow-color': '#a855f7' } },
          { selector: 'edge[kind = "runtime_depends_on"]', style: { 'line-color': '#06b6d4', 'line-style': 'dotted', 'target-arrow-color': '#06b6d4', 'width': 1.8 } },
          { selector: 'edge:selected', style: { 'opacity': 1, 'width': 2.5 } }
        ]
      });
      window._cy = cy;

      cy.ready(() => {
        // Restore persisted build-edge visibility before first filter pass
        restoreShowBuild();
        document.getElementById('kind-filter').value = defaultView();
        applyFilters(true);
      });

      // Keep disk aspect on window resize
      let resizeTimer = null;
      window.addEventListener('resize', () => {
        clearTimeout(resizeTimer);
        resizeTimer = setTimeout(() => applyFilters(true), 180);
      });

      cy.on('tap', 'node', function(evt){
        showDetail(evt.target.data());
      });

      cy.on('tap', function(evt){
        if (evt.target === cy) {
          closeDrawer();
        }
      });
    }

    // Drawer resize handle (drag left edge to widen/narrow)
    (function initDrawerResize() {
      const drawer = document.getElementById('detail-drawer');
      const handle = document.getElementById('drawer-resizer');
      if (!drawer || !handle) return;
      let startX = 0;
      let startW = 0;
      const MIN_W = 300;
      handle.addEventListener('pointerdown', (e) => {
        e.preventDefault();
        startX = e.clientX;
        startW = drawer.getBoundingClientRect().width;
        document.body.classList.add('drawer-resizing');
        handle.setPointerCapture(e.pointerId);
      });
      handle.addEventListener('pointermove', (e) => {
        if (!handle.hasPointerCapture(e.pointerId)) return;
        const maxW = window.innerWidth * 0.92;
        const w = Math.min(Math.max(startW + (startX - e.clientX), MIN_W), maxW);
        drawer.style.width = w + 'px';
      });
      handle.addEventListener('pointerup', (e) => {
        handle.releasePointerCapture(e.pointerId);
        document.body.classList.remove('drawer-resizing');
      });
      handle.addEventListener('pointercancel', () => {
        document.body.classList.remove('drawer-resizing');
      });
    })();

    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', initGraph);
    } else {
      initGraph();
    }
  </script>
</body>
</html>
"#;
