use crate::{CapabilityId, ContractError, DashboardRequirement, ExtensionRequirement};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::str::FromStr;

pub const PACKAGE_FORMAT_VERSION: &str = "1.0.0";
pub const SOURCE_API_VERSION: &str = "1.0.0";
pub const SOURCE_BATCH_CONTRACT_VERSION: &str = "1.0.0";
pub const DASHBOARD_API_VERSION: &str = "1.0.0";
pub const LOCALE_API_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContractVersion(pub Version);

impl ContractVersion {
    pub fn new(version: Version) -> Self {
        Self(version)
    }

    pub fn as_semver(&self) -> &Version {
        &self.0
    }
}

impl FromStr for ContractVersion {
    type Err = ContractError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Version::parse(value)
            .map(Self)
            .map_err(|error| ContractError::new("invalid_contract_version", error.to_string()))
    }
}

impl TryFrom<String> for ContractVersion {
    type Error = ContractError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl TryFrom<&str> for ContractVersion {
    type Error = ContractError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl fmt::Display for ContractVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModuleId(String);

impl ModuleId {
    pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ContractError::new(
                "blank_module_id",
                "module identifiers cannot be blank",
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ModuleId {
    type Error = ContractError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for ModuleId {
    type Error = ContractError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl AsRef<str> for ModuleId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ModuleId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleType {
    Source,
    Dashboard,
    Locale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceManifest {
    pub module_type: ModuleType,
    pub module_id: ModuleId,
    pub module_version: ContractVersion,
    pub package_format_version: ContractVersion,
    pub source_api_version: ContractVersion,
    pub mapping_version: ContractVersion,
    pub compatible_app_versions: Vec<String>,
    #[serde(alias = "capabilities")]
    pub provided_capabilities: Vec<CapabilityId>,
    pub accepted_file_patterns: Vec<String>,
    pub artifact_signatures: Vec<String>,
    pub extension_contracts: Vec<SourceExtensionContract>,
    pub settings_schema: Value,
    pub entrypoint_hash: String,
    pub localization_namespace: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceExtensionContract {
    pub namespace: String,
    pub contract_version: ContractVersion,
    pub payload_schema: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardManifest {
    pub module_type: ModuleType,
    pub module_id: ModuleId,
    pub module_version: ContractVersion,
    pub package_format_version: ContractVersion,
    pub dashboard_api_version: ContractVersion,
    pub entrypoint_hash: String,
    pub compatible_app_versions: Vec<String>,
    pub required_capabilities: Vec<DashboardRequirement>,
    pub required_extension_contracts: Vec<ExtensionRequirement>,
    pub localization_namespace: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocaleFile {
    pub path: String,
    pub sha256: String,
    pub executable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocaleManifest {
    pub module_type: ModuleType,
    pub module_id: ModuleId,
    pub locale: String,
    pub display_name: String,
    pub module_version: ContractVersion,
    pub package_format_version: ContractVersion,
    pub compatible_app_versions: Vec<String>,
    pub localization_namespace: String,
    pub files: Vec<LocaleFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "manifest", rename_all = "snake_case")]
pub enum ModuleManifest {
    Source(SourceManifest),
    Dashboard(DashboardManifest),
    Locale(LocaleManifest),
}

impl ModuleManifest {
    pub fn module_id(&self) -> &ModuleId {
        match self {
            Self::Source(manifest) => &manifest.module_id,
            Self::Dashboard(manifest) => &manifest.module_id,
            Self::Locale(manifest) => &manifest.module_id,
        }
    }

    pub fn module_type(&self) -> ModuleType {
        match self {
            Self::Source(_) => ModuleType::Source,
            Self::Dashboard(_) => ModuleType::Dashboard,
            Self::Locale(_) => ModuleType::Locale,
        }
    }
}
