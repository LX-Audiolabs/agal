//! Integration: scan a tiny on-disk workspace fixture.

use agal_core::findings::{Health, Severity, health};
use agal_core::{GenerateOptions, generate, scan};
use std::path::PathBuf;
use std::sync::Mutex;

/// Serialise all fixture tests: shared mini_ws + tests that rewrite agal.toml.
static FIXTURE_LOCK: Mutex<()> = Mutex::new(());

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini_ws")
}

fn lock_fixture() -> std::sync::MutexGuard<'static, ()> {
    FIXTURE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[test]
fn scan_discovers_plugins_and_crate() {
    let _guard = lock_fixture();
    let root = fixture_root();
    let g = scan(&root, false).expect("scan fixture");
    let names: Vec<_> = g.nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(names.contains(&"demo"), "nodes: {:?}", names);
    assert!(names.contains(&"legacy_ed"), "nodes: {:?}", names);
    assert!(names.contains(&"shared"), "nodes: {:?}", names);
    assert_eq!(g.nodes.iter().filter(|n| n.kind == "plugin").count(), 2);
}

#[test]
fn integrity_orphan_plugin_and_missing_member() {
    let _guard = lock_fixture();
    let root = fixture_root();
    let g = scan(&root, false).expect("scan");
    let codes: Vec<_> = g.findings.iter().map(|f| f.code.as_str()).collect();
    assert!(
        codes.contains(&"plugin_not_in_workspace"),
        "expected ghost plugin finding, got {:?}",
        codes
    );
    assert!(
        codes.contains(&"workspace_member_missing"),
        "expected missing-member error, got {:?}",
        codes
    );
    assert_eq!(health(&g.findings), Health::Blocked);
}

#[test]
fn migration_legacy_on_truce_slint_plugin() {
    let _guard = lock_fixture();
    let root = fixture_root();
    let g = scan(&root, false).expect("scan");
    let legacy: Vec<_> = g
        .findings
        .iter()
        .filter(|f| f.code == "migration_legacy")
        .collect();
    assert_eq!(legacy.len(), 1, "findings: {:?}", g.findings);
    assert_eq!(legacy[0].severity, Severity::Error);
    assert!(legacy[0].fix.is_some());
    assert!(legacy[0].path.is_some());
}

#[test]
fn tool_hints_include_clippy_and_clap() {
    let _guard = lock_fixture();
    let root = fixture_root();
    let g = scan(&root, false).expect("scan");
    assert!(
        g.findings.iter().any(|f| f.code == "tool_hint_clippy"),
        "clippy hint missing"
    );
    assert!(
        g.findings
            .iter()
            .any(|f| f.code == "tool_hint_clap_validator"
                && f.node.as_deref() == Some("plugins/demo")),
        "clap hint for demo missing: {:?}",
        g.findings
            .iter()
            .filter(|f| f.code.contains("tool_hint"))
            .map(|f| (&f.code, &f.node))
            .collect::<Vec<_>>()
    );
}

#[test]
fn generate_writes_agent_without_info_noise() {
    let _guard = lock_fixture();
    let root = fixture_root();
    let out = root.join("_test_out");
    let _ = std::fs::remove_dir_all(&out);
    generate(
        &root,
        &GenerateOptions {
            output_dir_override: Some("_test_out".into()),
            agent_only: true,
            ..Default::default()
        },
    )
    .expect("generate");

    let agent = std::fs::read_to_string(out.join("agal.agent.md")).expect("agent.md");
    assert!(agent.contains("health: **blocked**"), "agent:\n{agent}");
    assert!(agent.contains("migration_legacy"), "should list errors");
    assert!(
        !agent.contains("tool_hint_clippy"),
        "info tool hints must stay out of agent.md"
    );
    assert!(
        !agent.contains("tool_hint_clap_validator"),
        "info clap hints must stay out of agent.md"
    );

    let json = std::fs::read_to_string(out.join("agal.json")).expect("json");
    assert!(json.contains("tool_hint_clippy"));
    assert!(json.contains("\"fix\""));

    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn suppress_rules_remove_matching_findings() {
    let _guard = lock_fixture();
    let root = fixture_root();
    // Write a one-off config next to fixture (don't clobber if present)
    let cfg_path = root.join("agal.toml");
    let had_cfg = cfg_path.exists();
    let backup = if had_cfg {
        Some(std::fs::read_to_string(&cfg_path).unwrap())
    } else {
        None
    };
    std::fs::write(
        &cfg_path,
        r#"
[[suppress]]
code = "migration_legacy"
node = "legacy_ed"
reason = "test mute"
"#,
    )
    .unwrap();

    let g = scan(&root, false).expect("scan with suppress");
    assert!(
        !g.findings.iter().any(|f| f.code == "migration_legacy"),
        "migration_legacy should be suppressed"
    );
    assert!(
        g.findings_suppressed >= 1,
        "suppressed count should surface: {}",
        g.findings_suppressed
    );
    // integrity errors still present → still blocked or degraded depending
    assert!(
        g.findings
            .iter()
            .any(|f| f.code == "workspace_member_missing" || f.code == "plugin_not_in_workspace")
    );

    if let Some(b) = backup {
        std::fs::write(&cfg_path, b).unwrap();
    } else {
        let _ = std::fs::remove_file(&cfg_path);
    }
}

#[test]
fn notes_preserve_human_body() {
    let _guard = lock_fixture();
    let root = fixture_root();
    let out = root.join("_test_notes");
    let _ = std::fs::remove_dir_all(&out);
    let opts = GenerateOptions {
        output_dir_override: Some("_test_notes".into()),
        agent_only: true,
        ..Default::default()
    };
    generate(&root, &opts).expect("gen1");

    let note_path = out.join("notes/demo.md");
    let mut body = std::fs::read_to_string(&note_path).expect("note");
    // append unique human text below marker
    body.push_str("\n## Intent\n\nKEEP_THIS_HUMAN_LINE\n");
    std::fs::write(&note_path, &body).unwrap();

    generate(&root, &opts).expect("gen2");
    let body2 = std::fs::read_to_string(&note_path).expect("note2");
    assert!(
        body2.contains("KEEP_THIS_HUMAN_LINE"),
        "human body not preserved:\n{body2}"
    );
    assert!(body2.contains("AGAL:AUTO-START"));

    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn notes_workspace_memory_seeded_once() {
    let _guard = lock_fixture();
    let root = fixture_root();
    let out = root.join("_test_workspace_note");
    let _ = std::fs::remove_dir_all(&out);
    let opts = GenerateOptions {
        output_dir_override: Some("_test_workspace_note".into()),
        agent_only: true,
        ..Default::default()
    };
    generate(&root, &opts).expect("gen1");
    let ws = out.join("notes/_workspace.md");
    assert!(ws.is_file(), "expected _workspace.md seed");
    let mut body = std::fs::read_to_string(&ws).unwrap();
    body.push_str("\nKEEP_WORKSPACE_LINE\n");
    std::fs::write(&ws, &body).unwrap();

    generate(&root, &opts).expect("gen2");
    let body2 = std::fs::read_to_string(&ws).unwrap();
    assert!(
        body2.contains("KEEP_WORKSPACE_LINE"),
        "_workspace.md must not be overwritten:\n{body2}"
    );

    let index = std::fs::read_to_string(out.join("notes/_index.md")).unwrap();
    assert!(index.contains("_workspace.md"));

    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn notes_include_graph_atoms() {
    let _guard = lock_fixture();
    let root = fixture_root();
    let out = root.join("_test_notes_atoms");
    let _ = std::fs::remove_dir_all(&out);
    let opts = GenerateOptions {
        output_dir_override: Some("_test_notes_atoms".into()),
        agent_only: true,
        ..Default::default()
    };
    generate(&root, &opts).expect("gen");

    let demo = std::fs::read_to_string(out.join("notes/demo.md")).expect("demo note");
    assert!(
        demo.contains("## Graph atoms (auto)"),
        "missing graph atoms section:\n{demo}"
    );
    assert!(
        demo.contains("[ATOM] type=fact | detail=kind=plugin"),
        "missing kind atom:\n{demo}"
    );
    assert!(
        demo.contains("depends_on=shared")
            || demo.contains("[ATOM] type=fact | detail=depends_on="),
        "expected workspace dep atom:\n{demo}"
    );
    // info tool hints must not appear inside the graph-atoms fence (error/warn only)
    let atoms_block = demo
        .split("## Graph atoms (auto)")
        .nth(1)
        .and_then(|rest| rest.split("```").nth(1))
        .expect("atoms fenced block");
    assert!(
        !atoms_block.contains("tool_hint"),
        "info findings leaked into graph atoms:\n{atoms_block}"
    );

    let legacy = std::fs::read_to_string(out.join("notes/legacy_ed.md")).expect("legacy note");
    assert!(
        legacy.contains("migration=legacy") || legacy.contains("migration_legacy"),
        "legacy plugin should surface migration constraint:\n{legacy}"
    );
    assert!(
        legacy.contains("[ATOM] type=constraint"),
        "expected constraint atom:\n{legacy}"
    );

    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn shared_note_includes_api_surface() {
    let _guard = lock_fixture();
    let root = fixture_root();
    let out = root.join("_test_notes_api");
    let _ = std::fs::remove_dir_all(&out);
    generate(
        &root,
        &GenerateOptions {
            output_dir_override: Some("_test_notes_api".into()),
            agent_only: true,
            ..Default::default()
        },
    )
    .expect("generate");

    let shared = std::fs::read_to_string(out.join("notes/shared.md")).expect("shared note");
    // mini_ws shared crate should expose a public surface strip when any pub items exist
    let g = scan(&root, false).expect("scan");
    let shared_node = g.nodes.iter().find(|n| n.name == "shared").expect("shared");
    if let Some(ast) = &shared_node.ast_summary
        && !ast.api_surface.is_empty()
    {
        assert!(
            shared.contains("## api surface"),
            "missing api surface section:\n{shared}"
        );
    }

    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn impact_report_lists_inbound_edges() {
    let _guard = lock_fixture();
    let root = fixture_root();
    let report = agal_core::impact_report(&root, "shared").expect("impact report");
    assert!(
        report.contains("# agal impact: shared"),
        "missing header:\n{report}"
    );
    assert!(
        report.contains("demo") || report.contains("legacy_ed"),
        "expected inbound plugin references:\n{report}"
    );
    assert!(
        report.contains("direct cargo dependencies"),
        "expected direct deps section:\n{report}"
    );
}

#[test]
fn context_pack_focuses_node() {
    let _guard = lock_fixture();
    let root = fixture_root();
    let opts = agal_core::ContextPackOptions {
        focus: Some("demo".into()),
        diff: None,
        budget_tokens: 4000,
        format: agal_core::ContextPackFormat::Markdown,
    };
    let pack = agal_core::context_pack(&root, &opts).expect("context pack");
    assert!(
        pack.contains("# agal context pack: demo"),
        "missing header:\n{pack}"
    );
    assert!(
        pack.contains("shared"),
        "expected neighbor crate in pack:\n{pack}"
    );
}

#[test]
fn context_pack_json_format() {
    let _guard = lock_fixture();
    let root = fixture_root();
    let opts = agal_core::ContextPackOptions {
        focus: Some("shared".into()),
        diff: None,
        budget_tokens: 2000,
        format: agal_core::ContextPackFormat::Json,
    };
    let pack = agal_core::context_pack(&root, &opts).expect("context pack json");
    let value: serde_json::Value = serde_json::from_str(&pack).expect("valid json");
    assert_eq!(value["focus"]["name"], "shared");
    assert!(value["neighbors"].is_array());
}
