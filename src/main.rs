mod mcp;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "agal",
    version,
    about = "Audio-plugin workspace orientation: graph, notes, curated skills"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Workspace root (when generating without subcommand)
    #[arg(default_value = ".")]
    project_root: PathBuf,

    #[arg(short, long)]
    watch: bool,

    #[arg(long)]
    install_hook: bool,

    #[arg(short, long)]
    output: Option<String>,

    #[arg(short, long)]
    plugin: Option<String>,

    #[arg(short, long)]
    verbose: bool,

    #[arg(long)]
    agent_only: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Curated skill packs (live in the tool; sync into workspace on demand)
    Skills {
        #[command(subcommand)]
        action: SkillsCmd,
    },
    /// Check Clippy / clap-validator on PATH and print recommended commands
    Doctor {
        #[arg(default_value = ".")]
        project_root: PathBuf,
    },
    /// List workspace nodes + health (valid `context --focus` names)
    Nodes {
        #[arg(default_value = ".")]
        project_root: PathBuf,
    },
    /// Load one synced skill by stem or frontmatter id
    Skill {
        /// Skill stem or id (e.g. clap, dsp-realtime, aura)
        query: String,
        #[arg(default_value = ".")]
        project_root: PathBuf,
    },
    /// Reverse-dependency impact report for a crate/plugin
    Impact {
        /// Crate or plugin name (matches name, id, or suffix)
        name: String,
        #[arg(default_value = ".")]
        project_root: PathBuf,
    },
    /// Coverage map: note presence, atom count, and skill match per node
    Coverage {
        #[arg(default_value = ".")]
        project_root: std::path::PathBuf,
    },
    /// Append an [ATOM] entry to agal/notes/_workspace.md
    Atom {
        #[command(subcommand)]
        action: AtomCmd,
    },
    /// Aggregate [ATOM] entries from notes by type
    Findings {
        /// Atom types to include, comma-separated (default: failure,lesson)
        #[arg(long, default_value = "failure,lesson")]
        types: String,
        /// Pass --all to include every type except `fact`
        #[arg(long)]
        all: bool,
        /// Output dir under project root (default: agal)
        #[arg(short, long)]
        output: Option<String>,
        #[arg(default_value = ".")]
        project_root: std::path::PathBuf,
    },
    /// Search notes and synced skills for keywords
    Search {
        /// Query string (space-separated keywords, all must match per line)
        query: String,
        /// Max files to return (default: 10)
        #[arg(short, long, default_value = "10")]
        max: usize,
        /// Output dir under project root (default: agal)
        #[arg(short, long)]
        output: Option<String>,
        #[arg(default_value = ".")]
        project_root: std::path::PathBuf,
    },
    /// Pre-flight gate: scan workspace and exit non-zero on error/warn findings
    Check {
        /// Exit non-zero on warnings too (default: only errors)
        #[arg(long)]
        strict: bool,
        /// Output dir under project root (default: agal)
        #[arg(short, long)]
        output: Option<String>,
        #[arg(default_value = ".")]
        project_root: PathBuf,
    },
    /// Manage architecture proposals (pending decisions, lessons, constraints)
    Proposal {
        #[command(subcommand)]
        action: ProposalCmd,
    },
    /// Harvest lessons from recent git commits into pending proposals
    Harvest {
        /// Git date spec (e.g. "7 days ago", "2 weeks ago", "2026-08-01")
        #[arg(long, default_value = "7 days ago")]
        since: String,
        /// Output dir under project root (default: agal)
        #[arg(short, long)]
        output: Option<String>,
        #[arg(default_value = ".")]
        project_root: PathBuf,
    },
    /// Show DAW/host compatibility matrix (formats, notes dialect, sidechain, quirks)
    HostMatrix {
        #[arg(default_value = ".")]
        project_root: PathBuf,
    },
    /// Show the process() I/O signal map for one plugin/crate node
    ProcessMap {
        /// Crate or plugin name (e.g. smoke-synth, aura-clap)
        node: String,
        #[arg(default_value = ".")]
        project_root: PathBuf,
    },
    /// Start an MCP server over stdio (for Claude Code / AI tool integration)
    Serve {
        #[arg(default_value = ".")]
        project_root: PathBuf,
    },
    /// Build a focused, token-budgeted context pack for one node
    Context {
        /// Crate or plugin name to focus on (optional when --diff is given)
        #[arg(short, long)]
        focus: Option<String>,
        /// Optional git ref to center the pack on changed files
        #[arg(short, long)]
        diff: Option<String>,
        /// Approximate token budget (default: 8000)
        #[arg(short, long, default_value = "8000")]
        budget: usize,
        /// Output format: md or json (default: md)
        #[arg(long, default_value = "md")]
        format: String,
        /// Show which triggers fired per skill instead of full skill content
        #[arg(long)]
        explain: bool,
        #[arg(default_value = ".")]
        project_root: PathBuf,
    },
}

#[derive(Subcommand)]
enum SkillsCmd {
    /// List embedded skills and groups
    List,
    /// Draft a workspace skill file for one node (AST + notes → template)
    Draft {
        /// Crate or plugin name (e.g. smoke-synth, aura-dsp)
        node: String,
        /// Overwrite existing skill file
        #[arg(long)]
        force: bool,
        /// Output dir under project root (default: agal)
        #[arg(short, long)]
        output: Option<String>,
        #[arg(default_value = ".")]
        project_root: PathBuf,
    },
    /// Copy selected groups/skills into <workspace>/<output>/skills/
    Sync {
        /// Comma list: groups (`policy`, `ui`), singles (`ui/slint`), presets (`slint-ui`); default `core`
        #[arg(long, default_value = "core")]
        only: String,
        /// Task loadout preset (overrides `--only`): `dsp-fix`, `slint-ui`, `clap-ship`, …
        #[arg(long, value_name = "NAME")]
        preset: Option<String>,
        /// Overwrite existing skill files
        #[arg(long)]
        force: bool,
        /// Output dir under project root (default: agal)
        #[arg(short, long)]
        output: Option<String>,
        #[arg(default_value = ".")]
        project_root: PathBuf,
    },
}

#[derive(Subcommand)]
enum ProposalCmd {
    /// List proposals
    List {
        /// Filter by status: pending | approved | rejected | all (default: pending)
        #[arg(long)]
        status: Option<String>,
        #[arg(short, long)]
        output: Option<String>,
        #[arg(default_value = ".")]
        project_root: PathBuf,
    },
    /// Show one proposal in detail
    Show {
        /// Proposal ID (e.g. p-001)
        id: String,
        #[arg(short, long)]
        output: Option<String>,
        #[arg(default_value = ".")]
        project_root: PathBuf,
    },
    /// Create a new pending proposal
    Add {
        /// Kind: decision | lesson | constraint | task
        #[arg(long, default_value = "decision")]
        kind: String,
        /// Short title
        title: String,
        /// Detailed description
        detail: String,
        #[arg(long)]
        rationale: Option<String>,
        #[arg(short, long)]
        output: Option<String>,
        #[arg(default_value = ".")]
        project_root: PathBuf,
    },
    /// Approve a proposal
    Approve {
        /// Proposal ID (e.g. p-001)
        id: String,
        /// Also write as [ATOM] to _workspace.md
        #[arg(long)]
        promote: bool,
        #[arg(short, long)]
        output: Option<String>,
        #[arg(default_value = ".")]
        project_root: PathBuf,
    },
    /// Reject a proposal
    Reject {
        /// Proposal ID (e.g. p-001)
        id: String,
        #[arg(long)]
        reason: Option<String>,
        #[arg(short, long)]
        output: Option<String>,
        #[arg(default_value = ".")]
        project_root: PathBuf,
    },
}

#[derive(Subcommand)]
enum AtomCmd {
    /// Add an [ATOM] entry to a note (default: _workspace.md)
    Add {
        /// Atom type: lesson, failure, decision, constraint
        #[arg(long, short, default_value = "lesson")]
        r#type: String,
        /// The detail text
        detail: String,
        /// Target note stem (e.g. aura-dsp → notes/aura-dsp.md); default: _workspace.md
        #[arg(long, short)]
        note: Option<String>,
        /// Output dir under project root (default: agal)
        #[arg(short, long)]
        output: Option<String>,
        #[arg(default_value = ".")]
        project_root: std::path::PathBuf,
    },
}

fn canonicalize_root(project_root: &std::path::Path) -> PathBuf {
    let root = match project_root.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "error: cannot canonicalize {}: {}",
                project_root.display(),
                e
            );
            std::process::exit(1);
        }
    };
    if !root.join("Cargo.toml").exists() {
        eprintln!("error: no Cargo.toml in {}", root.display());
        std::process::exit(1);
    }
    root
}

fn main() {
    let cli = Cli::parse();

    if let Some(cmd) = cli.command {
        match cmd {
            Commands::Check { strict, output, project_root } => {
                let root = canonicalize_root(&project_root);
                let output_dir = output.unwrap_or_else(|| {
                    agal_core::config::ProjectConfig::load(&root)
                        .output_dir
                        .unwrap_or_else(|| agal_core::DEFAULT_OUTPUT_DIR.to_string())
                });
                match agal_core::scan(&root, false) {
                    Err(e) => {
                        eprintln!("error: {e}");
                        std::process::exit(2);
                    }
                    Ok(graph) => {
                        use agal_core::findings::Severity;
                        let errors: Vec<_> = graph.findings.iter()
                            .filter(|f| f.severity == Severity::Error)
                            .collect();
                        let warns: Vec<_> = graph.findings.iter()
                            .filter(|f| f.severity == Severity::Warn)
                            .collect();
                        let pending = agal_core::proposals::load(&root, &output_dir)
                            .into_iter()
                            .filter(|p| p.status == agal_core::proposals::ProposalStatus::Pending)
                            .count();

                        for f in &errors {
                            eprintln!("error [{}] {}: {}", f.code, f.node.as_deref().unwrap_or("-"), f.message);
                        }
                        for f in &warns {
                            eprintln!("warn  [{}] {}: {}", f.code, f.node.as_deref().unwrap_or("-"), f.message);
                        }
                        if pending > 0 {
                            eprintln!("note: {} pending proposal(s) — review with `agal proposal list`", pending);
                        }

                        let fail = !errors.is_empty() || (strict && !warns.is_empty());
                        if fail {
                            eprintln!(
                                "\nagal check failed: {} error(s), {} warn(s)",
                                errors.len(), warns.len()
                            );
                            std::process::exit(1);
                        } else {
                            println!(
                                "agal check ok: {} node(s), {} warn(s)",
                                graph.nodes.len(), warns.len()
                            );
                        }
                    }
                }
                return;
            }
            Commands::Proposal { action } => {
                let (project_root, output_override) = match &action {
                    ProposalCmd::List { project_root, output, .. } => (project_root.clone(), output.clone()),
                    ProposalCmd::Show { project_root, output, .. } => (project_root.clone(), output.clone()),
                    ProposalCmd::Add { project_root, output, .. } => (project_root.clone(), output.clone()),
                    ProposalCmd::Approve { project_root, output, .. } => (project_root.clone(), output.clone()),
                    ProposalCmd::Reject { project_root, output, .. } => (project_root.clone(), output.clone()),
                };
                let root = canonicalize_root(&project_root);
                let output_dir = output_override.unwrap_or_else(|| {
                    agal_core::config::ProjectConfig::load(&root)
                        .output_dir
                        .unwrap_or_else(|| agal_core::DEFAULT_OUTPUT_DIR.to_string())
                });
                match action {
                    ProposalCmd::List { status, .. } => {
                        let filter = status.as_deref().filter(|&s| s != "all");
                        print!("{}", agal_core::proposal_list(&root, &output_dir, filter));
                    }
                    ProposalCmd::Show { id, .. } => {
                        match agal_core::proposal_show(&root, &output_dir, &id) {
                            Ok(r) => print!("{r}"),
                            Err(e) => { eprintln!("error: {e}"); std::process::exit(1); }
                        }
                    }
                    ProposalCmd::Add { kind, title, detail, rationale, .. } => {
                        match agal_core::proposal_propose(&root, &output_dir, &kind, &title, &detail, rationale.as_deref(), "human") {
                            Ok(id) => println!("proposal created: {id}"),
                            Err(e) => { eprintln!("error: {e}"); std::process::exit(1); }
                        }
                    }
                    ProposalCmd::Approve { id, promote, .. } => {
                        match agal_core::proposal_approve(&root, &output_dir, &id, promote) {
                            Ok(r) => println!("{r}"),
                            Err(e) => { eprintln!("error: {e}"); std::process::exit(1); }
                        }
                    }
                    ProposalCmd::Reject { id, reason, .. } => {
                        match agal_core::proposal_reject(&root, &output_dir, &id, reason.as_deref()) {
                            Ok(r) => println!("{r}"),
                            Err(e) => { eprintln!("error: {e}"); std::process::exit(1); }
                        }
                    }
                }
                return;
            }
            Commands::Harvest { since, output, project_root } => {
                let root = canonicalize_root(&project_root);
                let output_dir = output.unwrap_or_else(|| {
                    agal_core::config::ProjectConfig::load(&root)
                        .output_dir
                        .unwrap_or_else(|| agal_core::DEFAULT_OUTPUT_DIR.to_string())
                });
                match agal_core::harvest_commits(&root, &output_dir, &since) {
                    Ok(r) => print!("{r}"),
                    Err(e) => { eprintln!("error: {e}"); std::process::exit(1); }
                }
                return;
            }
            Commands::HostMatrix { project_root } => {
                let root = canonicalize_root(&project_root);
                print!("{}", agal_core::host_matrix_report(&root));
                return;
            }
            Commands::ProcessMap { node, project_root } => {
                let root = canonicalize_root(&project_root);
                match agal_core::process_map_report(&root, &node) {
                    Ok(r) => print!("{r}"),
                    Err(e) => { eprintln!("error: {e}"); std::process::exit(1); }
                }
                return;
            }
            Commands::Serve { project_root } => {
                let root = canonicalize_root(&project_root);
                let server = mcp::AgalServer::new(root);
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("tokio runtime")
                    .block_on(async {
                        let transport = rmcp::transport::io::stdio();
                        rmcp::ServiceExt::serve(server, transport)
                            .await
                            .expect("MCP server error")
                            .waiting()
                            .await
                            .expect("MCP server join");
                    });
                return;
            }
            Commands::Skills { action } => match action {
                SkillsCmd::List => {
                    agal_core::skills::print_list();
                    return;
                }
                SkillsCmd::Draft { node, force, output, project_root } => {
                    let root = canonicalize_root(&project_root);
                    let output_dir = output.unwrap_or_else(|| {
                        agal_core::config::ProjectConfig::load(&root)
                            .output_dir
                            .unwrap_or_else(|| agal_core::DEFAULT_OUTPUT_DIR.to_string())
                    });
                    match agal_core::skill_draft_write(&root, &output_dir, &node, force) {
                        Ok(r) => println!("{r}"),
                        Err(e) => { eprintln!("error: {e}"); std::process::exit(1); }
                    }
                    return;
                }
                SkillsCmd::Sync {
                    only,
                    preset,
                    force,
                    output,
                    project_root,
                } => {
                    let root = match project_root.canonicalize() {
                        Ok(p) => p,
                        Err(e) => {
                            eprintln!(
                                "error: cannot canonicalize {}: {}",
                                project_root.display(),
                                e
                            );
                            std::process::exit(1);
                        }
                    };
                    if !root.join("Cargo.toml").exists() {
                        eprintln!("error: no Cargo.toml in {}", root.display());
                        std::process::exit(1);
                    }
                    let only_spec = if let Some(ref p) = preset {
                        match agal_core::skills::resolve_preset(p) {
                            Ok(expanded) => expanded.to_string(),
                            Err(e) => {
                                eprintln!("error: {}", e);
                                std::process::exit(1);
                            }
                        }
                    } else {
                        only
                    };
                    let selection = match agal_core::skills::parse_selection(&only_spec) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("error: {}", e);
                            std::process::exit(1);
                        }
                    };
                    // CLI -o wins; else agal.toml output_dir; else DEFAULT_OUTPUT_DIR ("agal").
                    let output_dir = output.unwrap_or_else(|| {
                        agal_core::config::ProjectConfig::load(&root)
                            .output_dir
                            .unwrap_or_else(|| agal_core::DEFAULT_OUTPUT_DIR.to_string())
                    });
                    let opts = agal_core::skills::SyncOptions {
                        selection,
                        force,
                        output_dir,
                    };
                    if let Err(e) = agal_core::skills::sync(&root, &opts) {
                        eprintln!("error: {}", e);
                        std::process::exit(1);
                    }
                    return;
                }
            },
            Commands::Doctor { project_root } => {
                let root = canonicalize_root(&project_root);
                match agal_core::doctor(&root) {
                    Ok(report) => {
                        print!("{report}");
                        return;
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        std::process::exit(1);
                    }
                }
            }
            Commands::Nodes { project_root } => {
                let root = canonicalize_root(&project_root);
                match agal_core::nodes_report(&root) {
                    Ok(report) => {
                        print!("{report}");
                        return;
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        std::process::exit(1);
                    }
                }
            }
            Commands::Skill { query, project_root } => {
                let root = canonicalize_root(&project_root);
                match agal_core::skill_get(&root, &query) {
                    Ok(report) => {
                        print!("{report}");
                        return;
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        std::process::exit(1);
                    }
                }
            }
            Commands::Impact { name, project_root } => {
                let root = canonicalize_root(&project_root);
                match agal_core::impact_report(&root, &name) {
                    Ok(report) => {
                        print!("{report}");
                        return;
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        std::process::exit(1);
                    }
                }
            }
            Commands::Coverage { project_root } => {
                let root = canonicalize_root(&project_root);
                match agal_core::coverage_report(&root) {
                    Ok(report) => {
                        print!("{report}");
                        return;
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        std::process::exit(1);
                    }
                }
            }
            Commands::Atom { action } => match action {
                AtomCmd::Add {
                    r#type,
                    detail,
                    note,
                    output,
                    project_root,
                } => {
                    let root = canonicalize_root(&project_root);
                    let output_dir = output.unwrap_or_else(|| {
                        agal_core::config::ProjectConfig::load(&root)
                            .output_dir
                            .unwrap_or_else(|| agal_core::DEFAULT_OUTPUT_DIR.to_string())
                    });
                    let note_ref = note.as_deref();
                    match agal_core::atom_add(&root, &output_dir, &r#type, &detail, note_ref) {
                        Ok(warn) => {
                            println!("[ATOM] type={} | detail={}", r#type, detail);
                            if !warn.is_empty() {
                                eprintln!("warn: {warn}");
                            }
                        }
                        Err(e) => {
                            eprintln!("error: {e}");
                            std::process::exit(1);
                        }
                    }
                    return;
                }
            },
            Commands::Findings {
                types,
                all,
                output,
                project_root,
            } => {
                let root = canonicalize_root(&project_root);
                let output_dir = output.unwrap_or_else(|| {
                    agal_core::config::ProjectConfig::load(&root)
                        .output_dir
                        .unwrap_or_else(|| agal_core::DEFAULT_OUTPUT_DIR.to_string())
                });
                let type_vec: Vec<String> = if all {
                    vec![]
                } else {
                    types.split(',').map(|s| s.trim().to_string()).collect()
                };
                let type_refs: Vec<&str> = type_vec.iter().map(String::as_str).collect();
                let report = agal_core::findings_report(&root, &output_dir, &type_refs);
                print!("{report}");
                return;
            }
            Commands::Search {
                query,
                max,
                output,
                project_root,
            } => {
                let root = canonicalize_root(&project_root);
                let output_dir = output.unwrap_or_else(|| {
                    agal_core::config::ProjectConfig::load(&root)
                        .output_dir
                        .unwrap_or_else(|| agal_core::DEFAULT_OUTPUT_DIR.to_string())
                });
                let report = agal_core::search_workspace(&root, &output_dir, &query, max);
                print!("{report}");
                return;
            }
            Commands::Context {
                focus,
                diff,
                budget,
                format,
                explain,
                project_root,
            } => {
                let root = canonicalize_root(&project_root);
                if focus.is_none() && diff.is_none() {
                    match agal_core::nodes_report(&root) {
                        Ok(report) => {
                            print!("{report}");
                            return;
                        }
                        Err(e) => {
                            eprintln!("error: {e}");
                            std::process::exit(1);
                        }
                    }
                }
                let fmt = match agal_core::ContextPackFormat::parse(&format) {
                    Ok(f) => f,
                    Err(e) => {
                        eprintln!("error: {e}");
                        std::process::exit(1);
                    }
                };
                let opts = agal_core::ContextPackOptions {
                    focus,
                    diff,
                    budget_tokens: budget,
                    format: fmt,
                    explain,
                };
                match agal_core::context_pack(&root, &opts) {
                    Ok(report) => {
                        print!("{report}");
                        return;
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        std::process::exit(1);
                    }
                }
            }
        }
    }

    let project_root = match cli.project_root.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "error: cannot canonicalize {}: {}",
                cli.project_root.display(),
                e
            );
            std::process::exit(1);
        }
    };

    if !project_root.join("Cargo.toml").exists() {
        eprintln!("error: no Cargo.toml found in {}", project_root.display());
        std::process::exit(1);
    }

    let options = agal_core::GenerateOptions {
        watch_mode: cli.watch,
        install_hook: cli.install_hook,
        output_dir_override: cli.output,
        verbose: cli.verbose,
        agent_only: cli.agent_only,
        plugin_filter: cli.plugin,
    };

    if let Err(e) = agal_core::generate(&project_root, &options) {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}
