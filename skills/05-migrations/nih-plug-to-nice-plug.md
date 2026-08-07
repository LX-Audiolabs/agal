---
source: global
copied_by: template
date: 2026-07-29
adapted: false
reason: "migration path from nih-plug to nice-plug"
---

# nih-plug → nice-plug Migration

**Summary:** Steps to migrate plugins from nih-plug to nice-plug.
Focus on params, editor adapters, and export macros.
Read this before editing legacy plugin code.

## Migration Steps

- Replace `nih_plug` imports with `nice_plug`
- Update `Params` derive and smoothing API
- Swap `nih_export_*` macros for `nice_plug` equivalents

## See Also

- [registry/frameworks.toml](../../registry/frameworks.toml) → `frameworks.nice-plug`
- [02-frameworks/framework-patterns.md](../02-frameworks/framework-patterns.md)
