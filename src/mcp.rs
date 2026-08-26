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
        Self { workspace, output_dir }
    }
}

#[tool(tool_box)]
impl AgalServer {
    #[tool(description = "Build a focused, token-budgeted context pack for a workspace node. Returns markdown with graph neighbours, matched skills, atoms, and findings.")]
    fn context(
        &self,
        #[tool(param)]
        #[schemars(description = "Crate or plugin name to focus on")]
        focus: String,
        #[tool(param)]
        #[schemars(description = "Token budget (default 8000)")]
        budget: Option<u32>,
        #[tool(param)]
        #[schemars(description = "Explain mode: trigger table instead of full skill content")]
        explain: Option<bool>,
    ) -> Result<CallToolResult, rmcp::Error> {
        let opts = ContextPackOptions {
            focus: Some(focus),
            budget_tokens: budget.unwrap_or(8000) as usize,
            explain: explain.unwrap_or(false),
            format: ContextPackFormat::Markdown,
            ..Default::default()
        };
        agal_core::context_pack(&self.workspace, &opts)
            .map(|text| CallToolResult::success(vec![Content::text(text)]))
            .map_err(|e| rmcp::Error::internal_error(e, None))
    }

    #[tool(description = "Search workspace notes, skills, and atoms. All space-separated terms must match per result.")]
    fn search(
        &self,
        #[tool(param)]
        #[schemars(description = "Space-separated keywords")]
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

    #[tool(description = "Append an [ATOM] knowledge entry to workspace notes.")]
    fn atom_add(
        &self,
        #[tool(param)]
        #[schemars(description = "Atom type: lesson | failure | decision | constraint")]
        atom_type: String,
        #[tool(param)]
        #[schemars(description = "The knowledge detail to record")]
        detail: String,
        #[tool(param)]
        #[schemars(description = "Target note stem (e.g. aura-dsp); omit for _workspace.md")]
        note: Option<String>,
    ) -> Result<CallToolResult, rmcp::Error> {
        agal_core::atom_add(
            &self.workspace,
            &self.output_dir,
            &atom_type,
            &detail,
            note.as_deref(),
        )
        .map(|msg| CallToolResult::success(vec![Content::text(msg)]))
        .map_err(|e| rmcp::Error::internal_error(e, None))
    }
}

#[tool(tool_box)]
impl ServerHandler for AgalServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            instructions: Some(format!(
                "AGAL workspace at {}. Tools: context (node context pack), search (keyword search), findings (atom entries), atom_add (record knowledge).",
                self.workspace.display()
            )),
            ..Default::default()
        }
    }
}
