//! Keyword search over agal notes and synced skills.
//!
//! Walks `<output_dir>/notes/` and `<output_dir>/skills/` in the workspace,
//! matches lines case-insensitively, and returns a ranked markdown report.
//! No index needed — walkdir over ~50 files is instant.

use std::fmt::Write as _;
use std::path::Path;
use walkdir::WalkDir;

/// One matching file with its relevant lines.
struct FileMatch {
    /// Relative path from workspace root.
    rel_path: String,
    /// (line_number, line_content) for each matching line.
    hits: Vec<(usize, String)>,
}

/// Search `<workspace>/<output_dir>/{notes,skills}` for `query` keywords.
///
/// Returns a markdown report with file matches ranked by hit count.
/// Returns an empty report if no files match or output_dir does not exist.
pub fn search(workspace: &Path, output_dir: &str, query: &str, max_results: usize) -> String {
    let terms: Vec<String> = query
        .split_whitespace()
        .map(|t| t.to_ascii_lowercase())
        .filter(|t| !t.is_empty())
        .collect();

    if terms.is_empty() {
        return "error: empty query\n".to_string();
    }

    let agal_dir = workspace.join(output_dir);
    let search_dirs = [agal_dir.join("notes"), agal_dir.join("skills")];

    let mut matches: Vec<FileMatch> = Vec::new();

    for dir in &search_dirs {
        if !dir.exists() {
            continue;
        }
        for entry in WalkDir::new(dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().is_file()
                    && e.path()
                        .extension()
                        .and_then(|s| s.to_str())
                        .map(|ext| ext == "md")
                        .unwrap_or(false)
            })
        {
            let path = entry.path();
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let mut hits: Vec<(usize, String)> = Vec::new();
            for (idx, line) in content.lines().enumerate() {
                let lower = line.to_ascii_lowercase();
                if terms.iter().all(|t| lower.contains(t.as_str())) {
                    hits.push((idx + 1, line.to_string()));
                }
            }

            if !hits.is_empty() {
                let rel = path
                    .strip_prefix(workspace)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .replace('\\', "/");
                matches.push(FileMatch { rel_path: rel, hits });
            }
        }
    }

    // Rank by hit count (most matches first).
    matches.sort_by(|a, b| b.hits.len().cmp(&a.hits.len()));
    matches.truncate(max_results);

    if matches.is_empty() {
        return format!(
            "# agal search: `{}`\n\nno matches in `{}/notes/` or `{}/skills/`\n",
            query, output_dir, output_dir
        );
    }

    let mut out = String::new();
    let _ = writeln!(
        out,
        "# agal search: `{}`\n{} file(s) matched\n",
        query,
        matches.len()
    );

    for m in &matches {
        let _ = writeln!(out, "## `{}`  ({} hit(s))\n", m.rel_path, m.hits.len());
        for (lineno, text) in &m.hits {
            let trimmed = text.trim();
            if trimmed.is_empty() || trimmed.starts_with("---") {
                continue;
            }
            let _ = writeln!(out, "- **L{}:** {}", lineno, trimmed);
        }
        let _ = writeln!(out);
    }

    out
}
