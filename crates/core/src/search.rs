//! Full-text search over agal notes and synced skills via Tantivy (BM25).
//!
//! Builds an in-RAM index per call — fast enough for ~200 markdown files
//! and avoids stale-index problems entirely.
//! ponytail: in-RAM rebuild, switch to persistent index when files > ~2k

use std::fmt::Write as _;
use std::path::Path;

use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Schema, STORED, TEXT, Value};
use tantivy::{Index, SnippetGenerator, TantivyDocument, doc};
use walkdir::WalkDir;

/// Search `<workspace>/<output_dir>/{notes,skills}` using BM25 full-text search.
///
/// Returns a markdown report ranked by relevance. Falls back to "no matches"
/// if the index build or query parsing fails.
pub fn search(workspace: &Path, output_dir: &str, query_str: &str, max_results: usize) -> String {
    if query_str.trim().is_empty() {
        return "error: empty query\n".to_string();
    }

    match search_inner(workspace, output_dir, query_str, max_results) {
        Ok(out) => out,
        Err(e) => format!("error: search failed: {e}\n"),
    }
}

fn search_inner(
    workspace: &Path,
    output_dir: &str,
    query_str: &str,
    max_results: usize,
) -> Result<String, Box<dyn std::error::Error>> {
    // Schema: path (display) + body (searchable + stored for snippets).
    let mut sb = Schema::builder();
    let f_path = sb.add_text_field("path", STORED);
    let f_body = sb.add_text_field("body", TEXT | STORED);
    let schema = sb.build();

    let index = Index::create_in_ram(schema);
    let mut writer = index.writer(16_000_000)?;

    let agal_dir = workspace.join(output_dir);
    let mut file_count = 0usize;
    for dir in &[agal_dir.join("notes"), agal_dir.join("skills")] {
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
            let Ok(content) = std::fs::read_to_string(path) else {
                continue;
            };
            let rel = path
                .strip_prefix(workspace)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            writer.add_document(doc!(f_path => rel, f_body => content))?;
            file_count += 1;
        }
    }
    writer.commit()?;

    if file_count == 0 {
        return Ok(format!(
            "# agal search: `{query_str}`\n\nno files in `{output_dir}/notes/` or `{output_dir}/skills/`\n"
        ));
    }

    let reader = index.reader()?;
    let searcher = reader.searcher();

    let mut qp = QueryParser::for_index(&index, vec![f_body]);
    qp.set_conjunction_by_default(); // all terms must match, like the old behaviour
    let query = match qp.parse_query(query_str) {
        Ok(q) => q,
        Err(_) => {
            return Ok(format!(
                "# agal search: `{query_str}`\n\ncould not parse query — try simpler terms\n"
            ));
        }
    };

    let top_docs = searcher.search(&query, &TopDocs::with_limit(max_results))?;

    if top_docs.is_empty() {
        return Ok(format!(
            "# agal search: `{query_str}`\n\nno matches in `{output_dir}/notes/` or `{output_dir}/skills/`\n"
        ));
    }

    let snippet_gen = SnippetGenerator::create(&searcher, &query, f_body)?;

    let mut out = String::new();
    let _ = writeln!(
        out,
        "# agal search: `{}`\n{} result(s)\n",
        query_str,
        top_docs.len()
    );

    for (_score, doc_addr) in &top_docs {
        let doc: TantivyDocument = searcher.doc(*doc_addr)?;
        let path = doc
            .get_first(f_path)
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let snippet = snippet_gen.snippet_from_doc(&doc);
        let fragment = snippet.fragment().trim();

        let _ = writeln!(out, "## `{path}`\n");
        if !fragment.is_empty() {
            let _ = writeln!(out, "> {}\n", fragment.replace('\n', " "));
        }
    }

    Ok(out)
}
