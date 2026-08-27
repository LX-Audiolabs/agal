//! MCP server: exposes AGAL workspace tools over stdio (JSON-RPC 2.0).
//! Start with: `agal serve [project_root]`

use std::path::PathBuf;

use rmcp::{
    ServerHandler,
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    tool,
};

use agal_core::{self, ContextPackFormat, ContextPackOptions, DEFAULT_OUTPUT_DIR, config};

#[derive(Debug, Clone)]
pub struct AgalServer {
    workspace: PathBuf,
    output_dir: String,
}

impl AgalServer {
    pub fn new(workspace: PathBuf) -> Self {
        let output_dir = config::ProjectConfig::load(&workspace)
            .output_dir
            .unwrap_or_else(|| DEFAULT_OUTPUT_DIR.to_string());
        Self {
            workspace,
            output_dir,
        }
    }
}

fn ok_text(text: String) -> CallToolResult {
    CallToolResult::success(vec![Content::text(text)])
}

fn err_internal(e: String) -> rmcp::Error {
    rmcp::Error::internal_error(e, None)
}

/// Unknown-node (and similar lookup misses) are recoverable: return the
/// message as a normal tool result so agents read candidates instead of
/// treating JSON-RPC -32603 as a hard failure.
fn tool_result(r: Result<String, String>) -> Result<CallToolResult, rmcp::Error> {
    match r {
        Ok(text) => Ok(ok_text(text)),
        Err(e) if e.starts_with("node '") || e.starts_with("no skill matching") || e.starts_with("ambiguous skill") => {
            Ok(ok_text(format!(
                "{e}\n\nCall `nodes` for crate names, or `skill` with a stem from `coverage`."
            )))
        }
        Err(e) => Err(err_internal(e)),
    }
}

#[tool(tool_box)]
impl AgalServer {
    #[tool(description = "List workspace nodes (crate/plugin names) plus health. Call this first when you do not know a valid `focus` name.")]
    fn nodes(&self) -> Result<CallToolResult, rmcp::Error> {
        agal_core::nodes_report(&self.workspace)
            .map(ok_text)
            .map_err(err_internal)
    }

    #[tool(description = "Build a token-budgeted context pack. Omit `focus` (and `diff`) to get the node list. Default `explain=true` returns a skill trigger table instead of inlining skill files.")]
    fn context(
        &self,
        #[tool(param)]
        #[schemars(description = "Crate or plugin name (optional — omit for node list)")]
        focus: Option<String>,
        #[tool(param)]
        #[schemars(description = "Token budget (default 8000)")]
        budget: Option<u32>,
        #[tool(param)]
        #[schemars(description = "Trigger table instead of full skill text (default true)")]
        explain: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Git ref to center the pack on changed files (e.g. HEAD~1)")]
        diff: Option<String>,
    ) -> Result<CallToolResult, rmcp::Error> {
        let focus = focus.filter(|s| !s.trim().is_empty());
        let diff = diff.filter(|s| !s.trim().is_empty());
        if focus.is_none() && diff.is_none() {
            return agal_core::nodes_report(&self.workspace)
                .map(ok_text)
                .map_err(err_internal);
        }
        let opts = ContextPackOptions {
            focus,
            diff,
            budget_tokens: budget.unwrap_or(8000) as usize,
            explain: explain.unwrap_or(true),
            format: ContextPackFormat::Markdown,
        };
        tool_result(agal_core::context_pack(&self.workspace, &opts))
    }

    #[tool(description = "Reverse dependencies: who uses this crate/plugin.")]
    fn impact(
        &self,
        #[tool(param)]
        #[schemars(description = "Crate or plugin name")]
        name: String,
    ) -> Result<CallToolResult, rmcp::Error> {
        tool_result(agal_core::impact_report(&self.workspace, &name))
    }

    #[tool(description = "Per-node gap map: note presence, human atom count, matched skill count.")]
    fn coverage(&self) -> Result<CallToolResult, rmcp::Error> {
        agal_core::coverage_report(&self.workspace)
            .map(ok_text)
            .map_err(err_internal)
    }

    #[tool(description = "Load ONE synced skill by stem or frontmatter id (e.g. clap, dsp-realtime, aura, slint).")]
    fn skill(
        &self,
        #[tool(param)]
        #[schemars(description = "Skill stem or id")]
        query: String,
    ) -> Result<CallToolResult, rmcp::Error> {
        tool_result(agal_core::skill_get(&self.workspace, &query))
    }

    #[tool(description = "Search notes and synced skills (BM25). Space-separated terms are ranked, not strict AND.")]
    fn search(
        &self,
        #[tool(param)]
        #[schemars(description = "Search query")]
        query: String,
        #[tool(param)]
        #[schemars(description = "Max results (default 10)")]
        max: Option<u32>,
    ) -> String {
        agal_core::search_workspace(
            &self.workspace,
            &self.output_dir,
            &query,
            max.unwrap_or(10) as usize,
        )
    }

    #[tool(description = "Aggregate [ATOM] knowledge entries from notes. Types: failure, lesson, decision, constraint.")]
    fn findings(
        &self,
        #[tool(param)]
        #[schemars(description = "Comma-separated atom types (default: failure,lesson)")]
        types: Option<String>,
    ) -> String {
        let types_str = types.unwrap_or_else(|| "failure,lesson".to_string());
        let type_list: Vec<&str> = types_str.split(',').map(str::trim).collect();
        agal_core::findings_report(&self.workspace, &self.output_dir, &type_list)
    }

    #[tool(description = "Append an [ATOM] to a crate note. `note` is required: crate stem (aura-clap) or `_workspace` for cross-cutting decisions only.")]
    fn atom_add(
        &self,
        #[tool(param)]
        #[schemars(description = "Atom type: lesson | failure | decision | constraint")]
        atom_type: String,
        #[tool(param)]
        #[schemars(description = "The knowledge detail to record")]
        detail: String,
        #[tool(param)]
        #[schemars(description = "Target note stem (e.g. aura-dsp) or `_workspace`")]
        note: Option<String>,
    ) -> Result<CallToolResult, rmcp::Error> {
        let Some(stem) = note.filter(|s| !s.trim().is_empty()) else {
            return Ok(ok_text(
                "atom_add needs `note` = crate/plugin stem (e.g. aura-clap).\n\
                 For cross-cutting workspace decisions pass note=\"_workspace\".\n\
                 Call `nodes` if you do not know the stem. Nothing was written."
                    .to_string(),
            ));
        };
        let note_arg = if stem == "_workspace" {
            None
        } else {
            Some(stem.as_str())
        };
        agal_core::atom_add(
            &self.workspace,
            &self.output_dir,
            &atom_type,
            &detail,
            note_arg,
        )
        .map(|warn| {
            if warn.is_empty() {
                ok_text(format!("wrote [ATOM] type={atom_type} → {stem}"))
            } else {
                ok_text(format!("wrote [ATOM] type={atom_type} → {stem}\nwarn: {warn}"))
            }
        })
        .map_err(err_internal)
    }
}

#[tool(tool_box)]
impl ServerHandler for AgalServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            instructions: Some(format!(
                "AGAL orientation for {}. Disclosure: nodes (or context without focus) → context(focus, explain=true) → skill(id) for one file → notes atoms. Budget: 1 note, ≤1 inlined skill. Never dump skills/. Unknown focus returns candidate names.",
                self.workspace.display()
            )),
            ..Default::default()
        }
    }
}
