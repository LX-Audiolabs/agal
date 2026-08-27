use crate::{Node, ast::ProcessIo};

pub fn render_node(node: &Node) -> String {
    let io = node
        .ast_summary
        .as_ref()
        .map(|a| &a.process_io)
        .cloned()
        .unwrap_or_default();
    render(&node.name, &io)
}

pub fn render(name: &str, io: &ProcessIo) -> String {
    if io.is_empty() {
        return format!(
            "## Process I/O: {name}\n\nNo process() hook detected (or no I/O patterns found).\n"
        );
    }

    let mut out = format!("## Process I/O: {name}\n\n");
    out.push_str("```\n");

    if io.audio_in || io.audio_out {
        let arrow = match (io.audio_in, io.audio_out) {
            (true, true) => "[Audio In] ──────────────────→ [Audio Out]",
            (true, false) => "[Audio In] (read-only, no output)",
            (false, true) => "            ──synthesize──→ [Audio Out]",
            _ => unreachable!(),
        };
        out.push_str(arrow);
        out.push('\n');
    }
    if io.sidechain_in {
        out.push_str("[Sidechain In] ──────────────→ (modulates output)\n");
    }
    if io.midi_in || io.midi_out {
        let arrow = match (io.midi_in, io.midi_out) {
            (true, true) => "[MIDI In] ────────────────→ [MIDI Out]",
            (true, false) => "[MIDI In] (consumed, no MIDI out)",
            (false, true) => "          ──generate──→ [MIDI Out]",
            _ => unreachable!(),
        };
        out.push_str(arrow);
        out.push('\n');
    }
    if io.notes_in || io.notes_out {
        let arrow = match (io.notes_in, io.notes_out) {
            (true, true) => "[Notes In] ───────────────→ [Notes Out]",
            (true, false) => "[Notes In] (consumed, no notes out)",
            (false, true) => "           ──generate──→ [Notes Out]",
            _ => unreachable!(),
        };
        out.push_str(arrow);
        out.push('\n');
    }
    if io.transport {
        out.push_str("[Transport] (read: play/bpm/pos)\n");
    }

    out.push_str("```\n\n");

    // Detail table
    out.push_str("| Signal | In | Out |\n|---|---|---|\n");
    out.push_str(&format!(
        "| Audio | {} | {} |\n",
        tick(io.audio_in), tick(io.audio_out)
    ));
    out.push_str(&format!(
        "| Sidechain | {} | — |\n",
        tick(io.sidechain_in)
    ));
    out.push_str(&format!(
        "| MIDI | {} | {} |\n",
        tick(io.midi_in), tick(io.midi_out)
    ));
    out.push_str(&format!(
        "| Notes (CLAP) | {} | {} |\n",
        tick(io.notes_in), tick(io.notes_out)
    ));
    out.push_str(&format!("| Transport | {} | — |\n", tick(io.transport)));

    out
}

fn tick(b: bool) -> &'static str {
    if b { "✓" } else { "—" }
}
