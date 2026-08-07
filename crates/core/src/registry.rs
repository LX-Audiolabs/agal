//! Embedded framework registry.
//!
//! Reads `registry/frameworks.toml` at compile time so the installed binary
//! carries the framework index without runtime file dependencies.

pub const FRAMEWORKS_TOML: &str = include_str!("../../../registry/frameworks.toml");

pub fn frameworks_toml() -> &'static str {
    FRAMEWORKS_TOML
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Registry {
    #[serde(default)]
    pub frameworks: std::collections::BTreeMap<String, RegistryFramework>,
    #[serde(default)]
    pub ui: std::collections::BTreeMap<String, RegistryUi>,
    #[serde(default)]
    pub formats: std::collections::BTreeMap<String, RegistryFormat>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RegistryFramework {
    pub name: String,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub docs: Option<String>,
    #[serde(default)]
    pub skill_source: Option<String>,
    #[serde(default)]
    pub migrates_from: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RegistryUi {
    pub name: String,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub docs: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RegistryFormat {
    #[serde(default)]
    pub spec: Option<String>,
    #[serde(default)]
    pub changelog: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
}

pub fn load() -> Option<Registry> {
    FRAMEWORKS_TOML.parse::<toml::Table>().map(toml::Value::Table).ok()?.try_into().ok()
}
