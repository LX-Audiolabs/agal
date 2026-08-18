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
    /// Reverse-dependency impact report for a crate/plugin
    Impact {
        /// Crate or plugin name (matches name, id, or suffix)
        name: String,
        #[arg(default_value = ".")]
        project_root: PathBuf,
    },
    /// Build a focused, token-budgeted context pack for one node
    Context {
        /// Crate or plugin name to focus on
        #[arg(short, long)]
        focus: String,
        /// Approximate token budget (default: 8000)
        #[arg(short, long, default_value = "8000")]
        budget: usize,
        /// Output format: md or json (default: md)
        #[arg(long, default_value = "md")]
        format: String,
        #[arg(default_value = ".")]
        project_root: PathBuf,
    },
}

#[derive(Subcommand)]
enum SkillsCmd {
    /// List embedded skills and groups
    List,
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
            Commands::Skills { action } => match action {
                SkillsCmd::List => {
                    agal_core::skills::print_list();
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
            Commands::Context {
                focus,
                budget,
                format,
                project_root,
            } => {
                let root = canonicalize_root(&project_root);
                let fmt = match agal_core::ContextPackFormat::parse(&format) {
                    Ok(f) => f,
                    Err(e) => {
                        eprintln!("error: {e}");
                        std::process::exit(1);
                    }
                };
                let opts = agal_core::ContextPackOptions {
                    focus,
                    budget_tokens: budget,
                    format: fmt,
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
