use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{
    Fields, ImplItemFn, ItemEnum, ItemFn, ItemImpl, ItemMacro, ItemStruct, ItemTrait, ItemUse,
    Visibility,
};

/// One parameter field on a Params struct.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ParamField {
    pub name: String,
    pub ty: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// True when #[param] has flags containing hidden/bypass-only scaffolding.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    /// True when #[param] has an explicit `id = N` (AURA required, truce optional).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub has_explicit_id: bool,
}

/// Ranked public API surface entry: signature + file (line when available).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiSymbol {
    pub kind: String,
    pub name: String,
    pub signature: String,
    pub file: String,
    /// 1-based line when span locations are available; `0` = file only.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub line: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AstSummary {
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    pub plugin_impls: BTreeSet<String>,
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    pub plugin_logic_impls: BTreeSet<String>,
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    pub plugin_macro_types: BTreeSet<String>,
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    pub params_impls: BTreeSet<String>,
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    pub params_structs: BTreeSet<String>,
    /// Params struct name → fields (name, type, optional #[param] display name).
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub params_fields: BTreeMap<String, Vec<ParamField>>,
    /// Param field names referenced in editor / process / presets / .slint (not only Params def).
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    pub params_referenced: BTreeSet<String>,
    /// Param field names never seen outside the Params struct definition.
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    pub params_unbound: BTreeSet<String>,
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    pub process_functions: BTreeSet<String>,
    /// Distinct process hooks only (PluginLogic / free fn), not every DSP method named process.
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    pub process_hooks: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub process_method_count: usize,
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    pub editor_functions: BTreeSet<String>,
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    pub imported_editor_adapters: BTreeSet<String>,
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    pub imported_crates: BTreeSet<String>,
    /// Public crate surface: `struct Biquad`, `enum Mode`, `fn foo`, `mod bar`.
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    pub public_api: BTreeSet<String>,
    /// Aider-style ranked public signatures (file:line). Capped at render/json time.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub api_surface: Vec<ApiSymbol>,
    /// Total public symbols before capping (rest = count only).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub api_surface_total: usize,
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    pub slint_components: BTreeSet<String>,
    /// Components this crate *exports* (export component X in .slint).
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    pub slint_exports: BTreeSet<String>,
    /// Runtime / IPC hints: shm, relay, shared_state, seqlock, …
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    pub ipc_signals: BTreeSet<String>,
    /// Relative path → role (entry, audio, ui, state, ipc, slint, build, source, …).
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub file_roles: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    pub files: BTreeSet<String>,
    /// Relative path → unique symbols. Empty unless `include_symbols` is true.
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub symbols_by_file: BTreeMap<String, BTreeSet<String>>,
    /// Cargo [features] — feature name → list of sub-features/deps (ponytail: stored on ast for simplicity).
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub features: BTreeMap<String, Vec<String>>,
    /// Detected plugin export formats: clap, vst3, lv2, etc.
    #[serde(skip_serializing_if = "BTreeSet::is_empty", default)]
    pub plugin_formats: BTreeSet<String>,
}

impl AstSummary {
    /// Strong IPC (peer-relevant), not just shared_state UI glue.
    pub fn has_strong_ipc(&self) -> bool {
        self.ipc_signals
            .iter()
            .any(|s| matches!(s.as_str(), "shm" | "relay" | "seqlock"))
    }

    /// Unique role tags for agent/notes one-liners.
    /// Path-based `file_roles` plus `ipc` when content signals imply it
    /// (so packages like lucent-relay without `*relay*.rs` still get `ipc`).
    pub fn role_tags(&self) -> BTreeSet<String> {
        let mut roles: BTreeSet<String> = self.file_roles.values().cloned().collect();
        if self.has_strong_ipc() {
            roles.insert("ipc".into());
        }
        roles
    }
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AnalyzeOptions {
    /// Keep per-file symbol lists (token-heavy). Default off.
    pub include_symbols: bool,
}

pub fn analyze_crate_with_options(crate_path: &Path, opts: AnalyzeOptions) -> AstSummary {
    let mut summary = AstSummary::default();
    // Sources used for param-binding checks (editor/process/ui — not Params def alone).
    let mut binding_corpus = String::new();
    let mut lib_corpus = String::new();

    let walker = walkdir::WalkDir::new(crate_path)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.starts_with("target") && !name.starts_with('.')
        });

    for entry in walker.filter_map(|e| e.ok()) {
        let path = entry.path();
        let rel = match path.strip_prefix(crate_path) {
            Ok(r) => r.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };
        if rel.is_empty() || rel.starts_with("target/") {
            continue;
        }
        summary.files.insert(rel.clone());
        if let Some(role) = infer_file_role(&rel) {
            summary.file_roles.insert(rel.clone(), role.to_string());
        }

        if path.extension().and_then(|s| s.to_str()) == Some("slint") {
            if let Ok(content) = fs::read_to_string(path) {
                analyze_slint_file(&content, &mut summary);
                binding_corpus.push('\n');
                binding_corpus.push_str(&content);
            }
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        if let Ok(content) = fs::read_to_string(path) {
            scan_ipc_text(&content, &rel, &mut summary);
            analyze_file(crate_path, path, &content, &mut summary, opts);

            let lower = rel.to_ascii_lowercase();
            if lower == "src/lib.rs" || lower == "src/main.rs" {
                lib_corpus.push('\n');
                lib_corpus.push_str(&content);
            } else if lower.ends_with(".rs") {
                // editor, process, presets, relay_state, modules…
                binding_corpus.push('\n');
                binding_corpus.push_str(&content);
            }
        }
    }

    finalize_param_binding(&mut summary, &binding_corpus, &lib_corpus);
    summary
}

/// Mark params as referenced if their field name (or PascalCase enum variant form)
/// appears in non-lib sources / slint. Unbound = only live in Params definition.
fn finalize_param_binding(summary: &mut AstSummary, binding_corpus: &str, lib_corpus: &str) {
    let all_fields: Vec<(String, bool)> = summary
        .params_fields
        .values()
        .flat_map(|fields| {
            fields
                .iter()
                .map(|f| (f.name.clone(), f.hidden || is_internal_param_name(&f.name)))
        })
        .collect();

    if all_fields.is_empty() {
        return;
    }

    let _ = lib_corpus;
    for (name, internal) in &all_fields {
        if *internal {
            // Hidden / _prefixed scaffolding — not expected in UI bindings.
            continue;
        }
        let pascal = snake_to_pascal(name);
        let in_binding = contains_ident(binding_corpus, name)
            || contains_ident(binding_corpus, &pascal)
            || contains_ident(binding_corpus, &format!("{}_text", name))
            || contains_ident(binding_corpus, &format!("{}_committed", name))
            || binding_corpus.contains(&pascal);

        if in_binding {
            summary.params_referenced.insert(name.clone());
        } else {
            summary.params_unbound.insert(name.clone());
        }
    }
}

fn is_internal_param_name(name: &str) -> bool {
    name.starts_with('_') || name.starts_with("internal_")
}

fn contains_ident(hay: &str, needle: &str) -> bool {
    if needle.is_empty() || !hay.contains(needle) {
        return false;
    }
    // Cheap boundary check: not mid-identifier on either side.
    let bytes = hay.as_bytes();
    let n = needle.as_bytes();
    let mut start = 0;
    while let Some(rel) = hay[start..].find(needle) {
        let i = start + rel;
        let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
        let after = i + n.len();
        let after_ok = after >= bytes.len() || !is_ident_byte(bytes[after]);
        if before_ok && after_ok {
            return true;
        }
        start = i + 1;
    }
    false
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn snake_to_pascal(s: &str) -> String {
    s.split('_')
        .filter(|p| !p.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect()
}

fn infer_file_role(rel: &str) -> Option<&'static str> {
    let lower = rel.to_ascii_lowercase();
    if lower == "cargo.toml" {
        return Some("manifest");
    }
    if lower == "build.rs" {
        return Some("build");
    }
    if lower.ends_with(".slint") {
        return Some("slint");
    }
    if lower == "src/lib.rs" || lower == "src/main.rs" {
        return Some("entry");
    }
    if lower.contains("relay") || lower.contains("shm") || lower.contains("shared_state") {
        return Some("ipc");
    }
    if lower.ends_with("process.rs") || lower.contains("/dsp/") || lower.contains("/audio/") {
        return Some("audio");
    }
    if lower.ends_with("editor.rs") || lower.contains("/ui/") || lower.contains("/editor/") {
        return Some("ui");
    }
    if lower.ends_with("presets.rs") || lower.contains("preset") || lower.contains("state") {
        return Some("state");
    }
    if lower.starts_with("src/") && lower.ends_with(".rs") {
        return Some("source");
    }
    None
}

fn scan_ipc_text(content: &str, rel: &str, summary: &mut AstSummary) {
    let patterns: &[(&str, &str)] = &[
        ("lx_analysis::shm", "shm"),
        ("lx_analysis::relay", "relay"),
        ("relay_hub", "relay"),
        ("SharedState", "shared_state"),
        ("shared_memory", "shm"),
        ("lx_shm", "shm"),
        ("seqlock", "seqlock"),
        ("shm_slot", "shm"),
        ("relay_active_mask", "relay"),
        ("RelayUi", "relay"),
    ];
    for (needle, signal) in patterns {
        if content.contains(needle) {
            summary.ipc_signals.insert((*signal).to_string());
        }
    }
    if rel.contains("relay") {
        summary.ipc_signals.insert("relay".to_string());
    }
}

fn analyze_file(
    crate_path: &Path,
    path: &Path,
    content: &str,
    summary: &mut AstSummary,
    opts: AnalyzeOptions,
) {
    let file = match syn::parse_file(content) {
        Ok(f) => f,
        Err(_) => return,
    };

    let rel = path
        .strip_prefix(crate_path)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");

    let mut visitor = AudioPluginVisitor {
        summary,
        current_file: rel,
        include_symbols: opts.include_symbols,
        in_plugin_logic_impl: false,
        in_plugin_impl: false,
    };
    visitor.visit_file(&file);
}

struct AudioPluginVisitor<'a> {
    summary: &'a mut AstSummary,
    current_file: String,
    include_symbols: bool,
    in_plugin_logic_impl: bool,
    in_plugin_impl: bool,
}

impl<'a, 'ast> Visit<'ast> for AudioPluginVisitor<'a> {
    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        let prev_logic = self.in_plugin_logic_impl;
        let prev_plugin = self.in_plugin_impl;

        if let Some((path, _)) = &node.trait_ {
            let trait_name = path
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .unwrap_or_default();
            if let syn::Type::Path(tp) = &*node.self_ty {
                let self_name = tp
                    .path
                    .segments
                    .last()
                    .map(|s| s.ident.to_string())
                    .unwrap_or_default();
                match trait_name.as_str() {
                    "Plugin" => {
                        self.summary.plugin_impls.insert(self_name.clone());
                        self.record_symbol(&format!("impl Plugin for {}", self_name));
                        self.in_plugin_impl = true;
                    }
                    "PluginLogic" => {
                        self.summary.plugin_logic_impls.insert(self_name.clone());
                        self.record_symbol(&format!("impl PluginLogic for {}", self_name));
                        self.in_plugin_logic_impl = true;
                    }
                    "Params" => {
                        self.summary.params_impls.insert(self_name.clone());
                        self.record_symbol(&format!("impl Params for {}", self_name));
                    }
                    _ => {}
                }
            }
        }

        // Surface trait impls only; inherent impls are noisy and their methods are captured separately.
        if node.trait_.is_some() {
            self.record_api(
                "impl",
                &signature_for_impl(node),
                signature_for_impl(node),
                item_line(node),
            );
        }

        syn::visit::visit_item_impl(self, node);
        self.in_plugin_logic_impl = prev_logic;
        self.in_plugin_impl = prev_plugin;
    }

    fn visit_item_macro(&mut self, node: &'ast ItemMacro) {
        let macro_path = node
            .mac
            .path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>()
            .join("::");
        if macro_path == "truce::plugin" || macro_path.ends_with("::plugin") {
            let tokens = node.mac.tokens.to_string();
            if let Some(logic) = extract_macro_arg(&tokens, "logic") {
                self.summary.plugin_macro_types.insert(logic.clone());
                self.record_symbol(&format!("truce::plugin! logic: {}", logic));
            }
            if let Some(params) = extract_macro_arg(&tokens, "params") {
                self.summary.params_structs.insert(params.clone());
                self.record_symbol(&format!("truce::plugin! params: {}", params));
            }
        }
        syn::visit::visit_item_macro(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        let fn_name = node.sig.ident.to_string();
        let is_pub = matches!(node.vis, Visibility::Public(_));
        if is_pub {
            self.record_api("fn", &fn_name, signature_for_fn(&node.sig), item_line(node));
        }
        match fn_name.as_str() {
            "process" => {
                self.summary.process_functions.insert(fn_name.clone());
                self.summary
                    .process_hooks
                    .insert(format!("fn process @ {}", self.current_file));
                self.record_symbol("fn process");
            }
            "editor" => {
                self.summary.editor_functions.insert(fn_name.clone());
                self.record_symbol("fn editor");
            }
            _ => {}
        }
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast ImplItemFn) {
        let fn_name = node.sig.ident.to_string();
        match fn_name.as_str() {
            "process" => {
                self.summary.process_method_count += 1;
                if self.in_plugin_logic_impl || self.in_plugin_impl {
                    self.summary.process_functions.insert(fn_name.clone());
                    self.summary
                        .process_hooks
                        .insert(format!("PluginLogic::process @ {}", self.current_file));
                    self.record_symbol("fn process (plugin hook)");
                }
                // Do not spam symbols for every DSP process method.
            }
            "editor" if self.in_plugin_logic_impl || self.in_plugin_impl => {
                self.summary.editor_functions.insert(fn_name.clone());
                self.record_symbol("fn editor (plugin hook)");
            }
            "editor" => {}
            _ => {}
        }
        syn::visit::visit_impl_item_fn(self, node);
    }

    fn visit_item_struct(&mut self, node: &'ast ItemStruct) {
        let name = node.ident.to_string();
        if matches!(node.vis, Visibility::Public(_)) {
            self.record_api("struct", &name, signature_for_struct(node), item_line(node));
        }
        if name.ends_with("Params") {
            self.summary.params_structs.insert(name.clone());
            self.record_symbol(&format!("struct {}", name));
            let fields = extract_param_fields(node);
            if !fields.is_empty() {
                self.summary.params_fields.insert(name.clone(), fields);
            }
        }
        for attr in &node.attrs {
            let attr_path = attr
                .path()
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            if attr_path == "derive"
                && let Ok(list) = attr.parse_args_with(
                    syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
                )
            {
                for path in list {
                    let derive_name = path
                        .segments
                        .last()
                        .map(|s| s.ident.to_string())
                        .unwrap_or_default();
                    if derive_name == "Params" && name.ends_with("Params") {
                        self.summary.params_structs.insert(name.clone());
                        self.record_symbol(&format!("#[derive(Params)] {}", name));
                    }
                }
            }
        }
        syn::visit::visit_item_struct(self, node);
    }

    fn visit_item_enum(&mut self, node: &'ast ItemEnum) {
        let name = node.ident.to_string();
        if matches!(node.vis, Visibility::Public(_)) {
            let generics = generics_to_compact(&node.generics);
            self.record_api(
                "enum",
                &name,
                format!("enum {}{}", name, generics),
                item_line(node),
            );
        }
        syn::visit::visit_item_enum(self, node);
    }

    fn visit_item_trait(&mut self, node: &'ast ItemTrait) {
        let name = node.ident.to_string();
        if matches!(node.vis, Visibility::Public(_)) {
            let generics = generics_to_compact(&node.generics);
            self.record_api(
                "trait",
                &name,
                format!("trait {}{}", name, generics),
                item_line(node),
            );
        }
        syn::visit::visit_item_trait(self, node);
    }

    fn visit_item_type(&mut self, node: &'ast syn::ItemType) {
        let name = node.ident.to_string();
        if matches!(node.vis, Visibility::Public(_)) {
            self.record_api(
                "type",
                &name,
                format!("type {} = {}", name, type_to_compact(&node.ty)),
                item_line(node),
            );
        }
        syn::visit::visit_item_type(self, node);
    }

    fn visit_item_const(&mut self, node: &'ast syn::ItemConst) {
        let name = node.ident.to_string();
        if matches!(node.vis, Visibility::Public(_)) {
            self.record_api(
                "const",
                &name,
                format!("const {}: {}", name, type_to_compact(&node.ty)),
                item_line(node),
            );
        }
        syn::visit::visit_item_const(self, node);
    }

    fn visit_item_static(&mut self, node: &'ast syn::ItemStatic) {
        let name = node.ident.to_string();
        if matches!(node.vis, Visibility::Public(_)) {
            self.record_api(
                "static",
                &name,
                format!("static {}: {}", name, type_to_compact(&node.ty)),
                item_line(node),
            );
        }
        syn::visit::visit_item_static(self, node);
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let name = node.ident.to_string();
        if matches!(node.vis, Visibility::Public(_)) {
            self.record_api("mod", &name, format!("pub mod {}", name), item_line(node));
        }
        syn::visit::visit_item_mod(self, node);
    }

    fn visit_item_use(&mut self, node: &'ast ItemUse) {
        let path = use_tree_to_string(&node.tree);

        if let Some(crate_name) = crate::config::crate_name_from_use_path(&path) {
            self.summary.imported_crates.insert(crate_name.clone());
            self.record_symbol(&format!("use {}", crate_name));
            // Any imported crate whose name ends with -editor is treated as an
            // editor adapter (e.g. lx-slint-editor, aura-editor).
            if crate_name.ends_with("-editor") {
                self.summary.imported_editor_adapters.insert(crate_name);
            }
        }

        if path.contains("truce_slint") || path.contains("truce-slint") {
            self.summary
                .imported_editor_adapters
                .insert("truce-slint".to_string());
            self.record_symbol("use truce-slint");
        }
        if path.contains("lx_slint_editor") || path.contains("lx-slint-editor") {
            self.summary
                .imported_editor_adapters
                .insert("lx-slint-editor".to_string());
            self.record_symbol("use lx-slint-editor");
        }

        // IPC from use paths
        if path.contains("shm") || path.contains("shared_memory") {
            self.summary.ipc_signals.insert("shm".to_string());
        }
        if path.contains("relay") {
            self.summary.ipc_signals.insert("relay".to_string());
        }

        syn::visit::visit_item_use(self, node);
    }
}

impl<'a> AudioPluginVisitor<'a> {
    fn record_symbol(&mut self, symbol: &str) {
        if !self.include_symbols {
            return;
        }
        self.summary
            .symbols_by_file
            .entry(self.current_file.clone())
            .or_default()
            .insert(symbol.to_string());
    }

    fn record_api(&mut self, kind: &str, name: &str, signature: String, line: usize) {
        let label = if kind == "impl" {
            name.into()
        } else {
            format!("{} {}", kind, name)
        };
        self.summary.public_api.insert(label);
        self.summary.api_surface.push(ApiSymbol {
            kind: kind.into(),
            name: name.into(),
            signature,
            file: self.current_file.clone(),
            line,
        });
    }
}

fn extract_param_fields(node: &ItemStruct) -> Vec<ParamField> {
    let fields = match &node.fields {
        Fields::Named(n) => &n.named,
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    for field in fields {
        let Some(ident) = &field.ident else { continue };
        let name = ident.to_string();
        let ty = type_to_compact(&field.ty);
        let meta = field.attrs.iter().find_map(extract_param_meta);
        let display_name = meta.as_ref().and_then(|m| m.display_name.clone());
        let hidden =
            meta.as_ref().map(|m| m.hidden).unwrap_or(false) || is_internal_param_name(&name);
        let has_explicit_id = meta.as_ref().map(|m| m.has_explicit_id).unwrap_or(false);
        // Only keep fields that look like params: have #[param] or type ends with Param.
        let looks_like_param = meta.is_some() || ty.ends_with("Param") || ty.contains("Param<");
        if !looks_like_param {
            continue;
        }
        out.push(ParamField {
            name,
            ty,
            display_name,
            hidden,
            has_explicit_id,
        });
    }
    out
}

struct ParamMeta {
    display_name: Option<String>,
    hidden: bool,
    has_explicit_id: bool,
}

fn extract_param_meta(attr: &syn::Attribute) -> Option<ParamMeta> {
    let path = attr
        .path()
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>()
        .join("::");
    if path != "param" {
        return None;
    }
    let tokens = attr.meta.require_list().ok()?.tokens.to_string();
    let display_name = extract_string_assign(&tokens, "name");
    let flags = extract_string_assign(&tokens, "flags").unwrap_or_default();
    let flags_l = flags.to_ascii_lowercase();
    let hidden = flags_l.contains("hidden")
        || flags_l.split('|').any(|f| f.trim() == "bypass")
        || display_name
            .as_deref()
            .map(|n| n.starts_with('_'))
            .unwrap_or(false);
    let has_explicit_id = extract_int_assign(&tokens, "id").is_some();
    Some(ParamMeta {
        display_name,
        hidden,
        has_explicit_id,
    })
}

fn extract_string_assign(tokens: &str, key: &str) -> Option<String> {
    let key_pat = format!("{} =", key);
    let idx = tokens.find(&key_pat)?;
    let rest = tokens[idx + key_pat.len()..].trim_start();
    if let Some(stripped) = rest.strip_prefix('"') {
        let end = stripped.find('"')?;
        return Some(stripped[..end].to_string());
    }
    None
}

fn extract_int_assign(tokens: &str, key: &str) -> Option<i64> {
    let key_pat = format!("{} =", key);
    let idx = tokens.find(&key_pat)?;
    let rest = tokens[idx + key_pat.len()..].trim_start();
    let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if num_str.is_empty() {
        return None;
    }
    num_str.parse().ok()
}

fn type_to_compact(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(p) => path_to_compact(&p.path),
        syn::Type::Reference(r) => {
            let mut s = String::from("&");
            if let Some(lt) = &r.lifetime {
                s.push_str(&lt.ident.to_string());
                s.push(' ');
            }
            if r.mutability.is_some() {
                s.push_str("mut ");
            }
            s.push_str(&type_to_compact(&r.elem));
            s
        }
        syn::Type::Tuple(t) => {
            let inner: Vec<_> = t.elems.iter().map(type_to_compact).collect();
            format!("({})", inner.join(", "))
        }
        syn::Type::Array(a) => format!("[{}]", type_to_compact(&a.elem)),
        syn::Type::Slice(s) => format!("[{}]", type_to_compact(&s.elem)),
        syn::Type::Ptr(p) => {
            let mut s = match &p.mutability {
                syn::PointerMutability::Const(_) => String::from("*const "),
                syn::PointerMutability::Mut(_) => String::from("*mut "),
            };
            s.push_str(&type_to_compact(&p.elem));
            s
        }
        syn::Type::Paren(p) => type_to_compact(&p.elem),
        syn::Type::Group(g) => type_to_compact(&g.elem),
        syn::Type::TraitObject(t) => {
            let bounds: Vec<String> = t
                .bounds
                .iter()
                .filter_map(|b| match b {
                    syn::TypeParamBound::Trait(tb) => Some(path_to_compact(&tb.path)),
                    syn::TypeParamBound::Lifetime(lt) => Some(format!("'{}", lt.ident)),
                    _ => None,
                })
                .collect();
            if bounds.is_empty() {
                "dyn …".into()
            } else {
                format!("dyn {}", bounds.join(" + "))
            }
        }
        syn::Type::ImplTrait(i) => {
            let bounds: Vec<String> = i
                .bounds
                .iter()
                .filter_map(|b| match b {
                    syn::TypeParamBound::Trait(tb) => Some(path_to_compact(&tb.path)),
                    syn::TypeParamBound::Lifetime(lt) => Some(format!("'{}", lt.ident)),
                    _ => None,
                })
                .collect();
            if bounds.is_empty() {
                "impl …".into()
            } else {
                format!("impl {}", bounds.join(" + "))
            }
        }
        syn::Type::FnPtr(f) => {
            let args: Vec<String> = f
                .inputs
                .iter()
                .map(|arg| type_to_compact(&arg.ty))
                .collect();
            let ret = match &f.output {
                syn::ReturnType::Default => String::new(),
                syn::ReturnType::Type(_, ty) => format!(" -> {}", type_to_compact(ty)),
            };
            format!("fn({}){}", args.join(", "), ret)
        }
        _ => "…".to_string(),
    }
}

fn path_to_compact(path: &syn::Path) -> String {
    let segs: Vec<String> = path
        .segments
        .iter()
        .map(|s| {
            let mut name = s.ident.to_string();
            if let syn::PathArguments::AngleBracketed(ab) = &s.arguments {
                let args: Vec<String> = ab
                    .args
                    .iter()
                    .filter_map(|a| match a {
                        syn::GenericArgument::Type(t) => Some(type_to_compact(t)),
                        _ => None,
                    })
                    .collect();
                if !args.is_empty() {
                    name.push('<');
                    name.push_str(&args.join(","));
                    name.push('>');
                }
            }
            name
        })
        .collect();
    // Prefer last segment for readability: truce::params::FloatParam → FloatParam
    segs.last().cloned().unwrap_or_default()
}

fn signature_for_fn(sig: &syn::Signature) -> String {
    let name = sig.ident.to_string();
    let generics = generics_to_compact(&sig.generics);
    let args: Vec<String> = sig
        .inputs
        .iter()
        .map(|arg| match arg {
            syn::FnArg::Receiver(r) => {
                let mut s = String::new();
                if let syn::ReceiverKind::Reference(_, lt, mutability) = &r.kind {
                    s.push('&');
                    if let Some(lt) = lt {
                        s.push_str(&lt.ident.to_string());
                        s.push(' ');
                    }
                    if mutability.is_some() {
                        s.push_str("mut ");
                    }
                }
                s.push_str("self");
                s
            }
            syn::FnArg::Typed(p) => {
                let pat = pat_to_compact(&p.pat);
                let ty = type_to_compact(&p.ty);
                format!("{}: {}", pat, ty)
            }
        })
        .collect();
    let ret = match &sig.output {
        syn::ReturnType::Default => String::new(),
        syn::ReturnType::Type(_, ty) => format!(" -> {}", type_to_compact(ty)),
    };
    format!("fn {}{}({}){}", name, generics, args.join(", "), ret)
}

fn generics_to_compact(generics: &syn::Generics) -> String {
    if generics.params.is_empty() {
        return String::new();
    }
    let parts: Vec<String> = generics
        .params
        .iter()
        .map(|p| match p {
            syn::GenericParam::Type(tp) => tp.ident.to_string(),
            syn::GenericParam::Lifetime(_) => String::new(),
            syn::GenericParam::Const(cp) => {
                format!("const {}: {}", cp.ident, type_to_compact(&cp.ty))
            }
        })
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        String::new()
    } else {
        format!("<{}>", parts.join(", "))
    }
}

fn pat_to_compact(pat: &syn::Pat) -> String {
    match pat {
        syn::Pat::Ident(i) => i.ident.to_string(),
        syn::Pat::Wild(_) => "_".into(),
        syn::Pat::Reference(r) => pat_to_compact(&r.pat),
        syn::Pat::Tuple(t) => {
            let inner: Vec<_> = t.elems.iter().map(pat_to_compact).collect();
            format!("({})", inner.join(", "))
        }
        _ => "_".into(),
    }
}

fn signature_for_struct(node: &ItemStruct) -> String {
    let name = node.ident.to_string();
    let generics = generics_to_compact(&node.generics);
    match &node.fields {
        Fields::Unit => format!("struct {}{}", name, generics),
        fields => {
            let field_str = fields
                .iter()
                .filter_map(|f| {
                    let ident = f.ident.as_ref()?;
                    Some(format!("{}: {}", ident, type_to_compact(&f.ty)))
                })
                .collect::<Vec<_>>()
                .join(", ");
            if field_str.len() > 60 {
                format!("struct {}{} {{ … }}", name, generics)
            } else {
                format!("struct {}{} {{ {} }}", name, generics, field_str)
            }
        }
    }
}

fn signature_for_impl(node: &ItemImpl) -> String {
    let self_ty = type_to_compact(&node.self_ty);
    if let Some((trait_, _)) = &node.trait_ {
        let trait_name = path_to_compact(trait_);
        format!("impl {} for {}", trait_name, self_ty)
    } else {
        format!("impl {}", self_ty)
    }
}

fn item_line<T: Spanned>(_node: &T) -> usize {
    // ponytail: proc_macro2 span-locations not enabled (crashed on Windows in tests).
    // Notes render `file` only when line is 0; re-enable later with span-locations feature.
    let _ = _node;
    0
}

/// Scan a .slint file for LX component usage and exports.
fn analyze_slint_file(content: &str, summary: &mut AstSummary) {
    for line in content.lines() {
        let trimmed = line.trim();
        // export component LxKnob …
        if let Some(rest) = trimmed
            .strip_prefix("export component ")
            .or_else(|| trimmed.strip_prefix("export global "))
        {
            let name: String = rest
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
                .unwrap_or("")
                .to_string();
            if name.starts_with("Lx") || name == "Lx" {
                summary.slint_exports.insert(name.clone());
                summary.slint_components.insert(name);
            }
        }
    }

    for word in content.split(|c: char| !c.is_alphanumeric() && c != '-') {
        let word = word.trim();
        if word.starts_with("Lx")
            && word.len() > 2
            && word.chars().nth(2).is_some_and(|c| c.is_uppercase())
        {
            let end = &word[2..];
            if end != "Result"
                && end != "Error"
                && end != "Value"
                && end != "Ptr"
                && end != "Str"
                && end != "Type"
                && end != "Data"
                && end != "Info"
                && end != "Mode"
            {
                summary.slint_components.insert(word.to_string());
            }
        }
        if word.starts_with("lx-") && word.len() > 3 {
            let kebab = &word[3..];
            if kebab.chars().all(|c| c.is_alphanumeric() || c == '-') && !kebab.starts_with('-') {
                let pascal: String = kebab
                    .split('-')
                    .filter(|s| !s.is_empty())
                    .map(|w| {
                        let mut c = w.chars();
                        match c.next() {
                            None => String::new(),
                            Some(f) => f.to_uppercase().to_string() + c.as_str(),
                        }
                    })
                    .collect();
                if !pascal.is_empty() {
                    summary.slint_components.insert(format!("Lx{}", pascal));
                }
            }
        }
    }
}

fn extract_macro_arg(tokens: &str, key: &str) -> Option<String> {
    let key_end = tokens.find(key)? + key.len();
    let rest = &tokens[key_end..];
    let rest = rest.trim_start();
    if !rest.starts_with(':') {
        return None;
    }
    let rest = rest[1..].trim_start();
    let end = find_arg_end(rest);
    let value = rest[..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn find_arg_end(s: &str) -> usize {
    let mut depth = 0u32;
    for (i, c) in s.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            ',' | '}' | '\n' if depth == 0 => return i,
            _ => {}
        }
    }
    s.len()
}

fn use_tree_to_string(tree: &syn::UseTree) -> String {
    match tree {
        syn::UseTree::Path(p) => format!("{}::{}", p.ident, use_tree_to_string(&p.tree)),
        syn::UseTree::Name(n) => n.ident.to_string(),
        syn::UseTree::Rename(r) => r.ident.to_string(),
        syn::UseTree::Glob(_) => "*".to_string(),
        syn::UseTree::Group(g) => g
            .items
            .iter()
            .map(use_tree_to_string)
            .collect::<Vec<_>>()
            .join(","),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_tmp(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "agal_test_{label}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    #[test]
    fn detects_aura_editor_adapter_from_glob_use() {
        let tmp = unique_tmp("aura_editor");
        let _ = fs::remove_dir_all(&tmp);
        let src = tmp.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("editor.rs"), "use aura_editor::typed::*;\n").unwrap();

        let summary = analyze_crate_with_options(&tmp, AnalyzeOptions::default());
        let _ = fs::remove_dir_all(&tmp);

        assert!(
            summary.imported_editor_adapters.contains("aura-editor"),
            "expected aura-editor adapter, got {:?}",
            summary.imported_editor_adapters
        );
    }

    #[test]
    fn api_surface_keeps_dyn_trait_and_signatures() {
        let tmp = unique_tmp("api_surface");
        let _ = fs::remove_dir_all(&tmp);
        let src = tmp.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(
            src.join("lib.rs"),
            r#"
                pub trait Params {
                    fn set_plain(&self, id: u32, v: f64);
                }
                pub fn apply_event(params: &dyn Params, id: u32) {
                    let _ = (params, id);
                }
                pub fn touch(x: &mut i32, p: &dyn Params) {
                    let _ = (x, p);
                }
                pub struct Gain { pub amount: f32 }
            "#,
        )
        .unwrap();

        let summary = analyze_crate_with_options(&tmp, AnalyzeOptions::default());
        let _ = fs::remove_dir_all(&tmp);

        let sigs: Vec<&str> = summary
            .api_surface
            .iter()
            .map(|s| s.signature.as_str())
            .collect();

        assert!(
            sigs.iter().any(|s| s.contains("trait Params")),
            "trait missing: {sigs:?}"
        );
        assert!(
            sigs.iter()
                .any(|s| s.contains("fn apply_event") && s.contains("&dyn Params")),
            "dyn Params must not collapse to ellipsis: {sigs:?}"
        );
        assert!(
            sigs.iter()
                .any(|s| s.contains("fn touch") && s.contains("&mut i32")),
            "mut ref missing: {sigs:?}"
        );
        assert!(
            sigs.iter().any(|s| s.starts_with("struct Gain")),
            "struct missing: {sigs:?}"
        );
        assert!(
            summary.public_api.iter().any(|s| s == "fn apply_event"),
            "legacy public_api label: {:?}",
            summary.public_api
        );
    }

    #[test]
    fn type_to_compact_dyn_and_ref() {
        let ty: syn::Type = syn::parse_str("&dyn Params").unwrap();
        assert_eq!(type_to_compact(&ty), "&dyn Params");
        let ty: syn::Type = syn::parse_str("&mut Foo").unwrap();
        assert_eq!(type_to_compact(&ty), "&mut Foo");
        let ty: syn::Type = syn::parse_str("impl Iterator + Send").unwrap();
        assert_eq!(type_to_compact(&ty), "impl Iterator + Send");
    }
}
