//! Auto-harvest lessons from git commit history into proposals.

use std::path::Path;
use std::process::Command;

/// Scan recent git commits for lesson-bearing patterns and create pending proposals.
///
/// `since`: git date spec (e.g. "7 days ago", "2 weeks ago", "2026-08-01").
pub fn harvest(workspace: &Path, output_dir: &str, since: &str) -> Result<String, String> {
    let output = Command::new("git")
        .args(["log", &format!("--since={since}"), "--format=%s", "HEAD"])
        .current_dir(workspace)
        .output()
        .map_err(|e| format!("git log failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git log error: {stderr}"));
    }

    let log = String::from_utf8_lossy(&output.stdout);
    let mut proposed: Vec<String> = Vec::new();

    for subject in log.lines() {
        let subject = subject.trim();
        if subject.is_empty() {
            continue;
        }
        let Some(lesson) = extract_lesson(subject) else {
            continue;
        };
        let id = crate::proposals::propose(
            workspace,
            output_dir,
            "lesson",
            subject,
            &lesson,
            None,
            "git-harvest",
        )?;
        proposed.push(format!("- {id}: {subject}"));
    }

    if proposed.is_empty() {
        return Ok(format!(
            "# agal harvest\n\nno lesson-bearing commits since `{since}`.\n"
        ));
    }

    Ok(format!(
        "# agal harvest\n\n{} proposals created from commits since `{since}`:\n\n{}\n\nReview: `agal proposal list` → approve/reject each.\n",
        proposed.len(),
        proposed.join("\n")
    ))
}

/// Returns a lesson string if the commit subject signals a lesson, else None.
fn extract_lesson(subject: &str) -> Option<String> {
    let lc = subject.to_ascii_lowercase();

    // Fix/hotfix/revert/workaround commits always carry a lesson.
    if lc.starts_with("fix:")
        || lc.starts_with("fix(")
        || lc.starts_with("hotfix:")
        || lc.starts_with("revert:")
        || lc.starts_with("workaround:")
    {
        return Some(subject.to_string());
    }

    // Subjects containing explicit lesson signals.
    if lc.contains("must not")
        || lc.contains("never ")
        || lc.contains("don't ")
        || lc.contains("do not ")
        || lc.contains("panic")
        || lc.contains("crash")
        || lc.contains("deadlock")
        || lc.contains("race condition")
        || lc.contains("regression")
    {
        return Some(subject.to_string());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fix_commit_is_lesson() {
        assert!(extract_lesson("fix: CLAP process must not heap-alloc").is_some());
        assert!(extract_lesson("fix(clap): correct event cap").is_some());
        assert!(extract_lesson("hotfix: SVF NaN guard").is_some());
    }

    #[test]
    fn feat_commit_is_not_lesson() {
        assert!(extract_lesson("feat: add new oscillator").is_none());
        assert!(extract_lesson("docs: update README").is_none());
    }

    #[test]
    fn signal_words_trigger_lesson() {
        assert!(extract_lesson("chore: ensure_current must not return Err on dead DC").is_some());
        assert!(extract_lesson("perf: avoid panic in biquad when q=0").is_some());
    }
}
