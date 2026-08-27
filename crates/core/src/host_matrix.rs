use crate::config::HostEntry;

pub fn render_md(hosts: &[HostEntry]) -> String {
    if hosts.is_empty() {
        return "## Host Compatibility Matrix\n\n\
            No entries yet. Add `[[host_matrix]]` blocks to `agal.toml`.\n\
            \n\
            Example:\n\
            ```toml\n\
            [[host_matrix]]\n\
            name = \"Bitwig Studio\"\n\
            version = \"5.x\"\n\
            formats = [\"clap\", \"vst3\"]\n\
            notes_dialect = \"clap\"\n\
            sidechain = true\n\
            param_mod = true\n\
            voice_stack = true\n\
            quirks = [\"Voice Stack sends PARAM_MOD before NOTE_ON\"]\n\
            ```\n"
            .to_string();
    }

    let mut out = String::from("## Host Compatibility Matrix\n\n");
    out.push_str("| Host | Formats | Dialect | SC | ParamMod | VoiceStack | MIDI CC | Quirks |\n");
    out.push_str("|---|---|---|---|---|---|---|---|\n");

    let mut all_quirks: Vec<(&str, &str)> = Vec::new();

    for h in hosts {
        let name = if let Some(v) = &h.version {
            format!("{} {}", h.name, v)
        } else {
            h.name.clone()
        };
        let formats = if h.formats.is_empty() {
            "—".to_string()
        } else {
            h.formats.iter().map(|s| s.to_uppercase()).collect::<Vec<_>>().join(" ")
        };
        let dialect = h.notes_dialect.as_deref().unwrap_or("—");
        let sc = if h.sidechain { "✓" } else { "✗" };
        let pm = if h.param_mod { "✓" } else { "✗" };
        let vs = if h.voice_stack { "✓" } else { "✗" };
        let cc = if h.midi_cc { "✓" } else { "✗" };
        let qcount = if h.quirks.is_empty() {
            "—".to_string()
        } else {
            h.quirks.len().to_string()
        };
        out.push_str(&format!(
            "| {name} | {formats} | {dialect} | {sc} | {pm} | {vs} | {cc} | {qcount} |\n"
        ));
        for q in &h.quirks {
            all_quirks.push((&h.name, q.as_str()));
        }
    }

    if !all_quirks.is_empty() {
        out.push_str("\n### Known Quirks\n");
        for (host, quirk) in all_quirks {
            out.push_str(&format!("- **{host}**: {quirk}\n"));
        }
    }

    out
}
