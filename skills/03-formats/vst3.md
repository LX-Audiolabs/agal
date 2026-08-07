---
source: global
copied_by: template
date: 2026-07-29
adapted: false
reason: "vst3 plugin format rules and conventions"
---

# VST3 Format

**Summary:** Rules for building VST3 plugins.
Covers component interfaces, kSingleComponent, and process context.
Read this when targeting VST3 hosts.

## Key Rules

- Implement `IComponent` and `IEditController`
- Use `Steinberg::Vst::kSingleComponent` when applicable
- Respect `ProcessContext` tempo and transport

## See Also

- [registry/frameworks.toml](../../registry/frameworks.toml) → `formats.vst3`
