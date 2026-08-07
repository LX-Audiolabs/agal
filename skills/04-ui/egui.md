---
source: global
copied_by: template
date: 2026-07-29
adapted: false
reason: "egui ui stack rules for audio plugins"
---

# egui UI

**Summary:** UI patterns for egui-based plugin editors.
Covers immediate-mode layout, context, and frame loops.
Read this when using egui for plugin GUIs.

## Key Rules

- Build UI in the `update()` method
- Avoid blocking the audio thread
- Use `egui::Context` for repaint requests

## See Also

- [registry/frameworks.toml](../../registry/frameworks.toml) → `ui.egui`
