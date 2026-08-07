---
source: global
copied_by: template
date: 2026-07-29
adapted: false
reason: "lv2 plugin format rules and conventions"
---

# LV2 Format

**Summary:** Rules for building LV2 plugins.
Covers TTL manifests, port definitions, and worker extensions.
Read this when targeting LV2 hosts.

## Key Rules

- Provide `manifest.ttl` and plugin `.ttl`
- Define input/output audio ports clearly
- Use `worker` extension for background tasks

## See Also

- [registry/frameworks.toml](../../registry/frameworks.toml) → `formats.lv2`
