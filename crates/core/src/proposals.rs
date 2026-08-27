//! Proposal lifecycle: pending → approved / rejected.
//! State lives in `<output_dir>/proposals.json`.

use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProposalStatus {
    Pending,
    Approved,
    Rejected,
}

impl std::fmt::Display for ProposalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Approved => write!(f, "approved"),
            Self::Rejected => write!(f, "rejected"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: String,
    pub status: ProposalStatus,
    /// "decision" | "lesson" | "constraint" | "task"
    pub kind: String,
    pub title: String,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    /// "agent" | "human" | "git-harvest"
    pub proposed_by: String,
    pub proposed_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution_note: Option<String>,
}

fn path(workspace: &Path, output_dir: &str) -> std::path::PathBuf {
    workspace.join(output_dir).join("proposals.json")
}

pub fn load(workspace: &Path, output_dir: &str) -> Vec<Proposal> {
    let p = path(workspace, output_dir);
    let Ok(content) = std::fs::read_to_string(&p) else {
        return Vec::new();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

fn save(workspace: &Path, output_dir: &str, proposals: &[Proposal]) -> Result<(), String> {
    let p = path(workspace, output_dir);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("cannot create dir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(proposals).map_err(|e| format!("serialize: {e}"))?;
    std::fs::write(&p, json).map_err(|e| format!("write proposals.json: {e}"))
}

fn next_id(proposals: &[Proposal]) -> String {
    let max = proposals
        .iter()
        .filter_map(|p| p.id.strip_prefix("p-").and_then(|n| n.parse::<u32>().ok()))
        .max()
        .unwrap_or(0);
    format!("p-{:03}", max + 1)
}

pub fn propose(
    workspace: &Path,
    output_dir: &str,
    kind: &str,
    title: &str,
    detail: &str,
    rationale: Option<&str>,
    proposed_by: &str,
) -> Result<String, String> {
    let mut proposals = load(workspace, output_dir);
    let id = next_id(&proposals);
    proposals.push(Proposal {
        id: id.clone(),
        status: ProposalStatus::Pending,
        kind: kind.to_string(),
        title: title.to_string(),
        detail: detail.to_string(),
        rationale: rationale.map(str::to_string),
        proposed_by: proposed_by.to_string(),
        proposed_at: crate::now_rfc3339(),
        resolved_at: None,
        resolution_note: None,
    });
    save(workspace, output_dir, &proposals)?;
    Ok(id)
}

/// Approve a pending proposal. If `promote` is true, also writes an [ATOM] to notes.
pub fn approve(
    workspace: &Path,
    output_dir: &str,
    id: &str,
    promote: bool,
) -> Result<String, String> {
    let mut proposals = load(workspace, output_dir);
    let p = proposals
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("proposal `{id}` not found"))?;
    if p.status == ProposalStatus::Approved {
        return Ok(format!("{id} already approved"));
    }
    p.status = ProposalStatus::Approved;
    p.resolved_at = Some(crate::now_rfc3339());
    let detail = p.detail.clone();
    let kind = p.kind.clone();
    save(workspace, output_dir, &proposals)?;
    if promote {
        crate::atoms::append_atom(workspace, output_dir, &kind, &detail, None)?;
        Ok(format!("{id} approved → [ATOM] type={kind} written to _workspace.md"))
    } else {
        Ok(format!("{id} approved"))
    }
}

pub fn reject(
    workspace: &Path,
    output_dir: &str,
    id: &str,
    reason: Option<&str>,
) -> Result<String, String> {
    let mut proposals = load(workspace, output_dir);
    let p = proposals
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("proposal `{id}` not found"))?;
    p.status = ProposalStatus::Rejected;
    p.resolved_at = Some(crate::now_rfc3339());
    p.resolution_note = reason.map(str::to_string);
    save(workspace, output_dir, &proposals)?;
    Ok(format!("{id} rejected"))
}

/// `status_filter`: "pending" | "approved" | "rejected" | None (all)
pub fn list_md(workspace: &Path, output_dir: &str, status_filter: Option<&str>) -> String {
    let proposals = load(workspace, output_dir);
    if proposals.is_empty() {
        return "# agal proposals\n\nno proposals yet. Use `propose` to create one.\n".to_string();
    }
    let filtered: Vec<&Proposal> = proposals
        .iter()
        .filter(|p| match status_filter {
            None => true,
            Some(f) => p.status.to_string() == f,
        })
        .collect();

    let label = status_filter.unwrap_or("all");
    let mut out = format!(
        "# agal proposals\n\n{} total · {} {} shown\n\n",
        proposals.len(),
        filtered.len(),
        label
    );
    for p in &filtered {
        let _ = writeln!(
            out,
            "## {} — {} [{}]\n\n{}\n",
            p.id, p.title, p.status, p.detail
        );
        if let Some(r) = &p.rationale {
            let _ = writeln!(out, "*rationale:* {r}\n");
        }
        if let Some(note) = &p.resolution_note {
            let _ = writeln!(out, "*resolution:* {note}\n");
        }
    }
    if filtered.is_empty() {
        let _ = writeln!(out, "_no {label} proposals_");
    }
    out
}

pub fn show_md(workspace: &Path, output_dir: &str, id: &str) -> Result<String, String> {
    let proposals = load(workspace, output_dir);
    let p = proposals
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("proposal `{id}` not found"))?;
    let mut out = format!(
        "# proposal {}\n\n**status:** {}\n**kind:** {}\n**title:** {}\n**by:** {} at {}\n\n{}\n",
        p.id, p.status, p.kind, p.title, p.proposed_by, p.proposed_at, p.detail
    );
    if let Some(r) = &p.rationale {
        let _ = writeln!(out, "\n**rationale:** {r}");
    }
    if let Some(at) = &p.resolved_at {
        let _ = writeln!(out, "**resolved:** {at}");
    }
    if let Some(note) = &p.resolution_note {
        let _ = writeln!(out, "**note:** {note}");
    }
    Ok(out)
}
