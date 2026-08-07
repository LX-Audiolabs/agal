---
source: global
copied_by: template
date: 2026-07-29
adapted: false
reason: "iced ui stack rules for audio plugins"
---

# Iced UI

**Summary:** UI patterns for Iced-based plugin editors.
Covers Elm architecture, messages, and subscriptions.
Read this when using Iced for plugin GUIs.

## Key Rules

- Separate `view()` from `update()`
- Use `Subscription` for host sync
- Keep widget state minimal

## See Also

- [registry/frameworks.toml](../../registry/frameworks.toml) → `ui.iced`
