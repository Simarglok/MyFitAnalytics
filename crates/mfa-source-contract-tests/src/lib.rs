mod assertions;
pub mod harness;

use mfa_contracts::{AssetMetadata, ReadOnlyAsset};
use mfa_module_host::{
    ComponentRuntime, InstalledModule, PackageError, PackageInstaller, RuntimeError, RuntimeLimits,
};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeExpectation {
    pub confidence: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedResult {
    pub records: usize,
    pub source_records: usize,
    pub lineage: usize,
    pub extensions: usize,
    pub issues: usize,
    pub logical_snapshot_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCase {
    pub fixture: std::path::PathBuf,
    pub expected_probe: ProbeExpectation,
    pub expected_result: ExpectedResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConformanceReport {
    pub cases: usize,
    pub deterministic_probe: bool,
    pub deterministic_parse: bool,
    pub forbidden_imports_absent: bool,
}

#[derive(Debug, Error)]
pub enum ContractTestError {
    #[error("package inspection failed: {0}")]
    Package(#[from] PackageError),
    #[error("fixture could not be read: {0}")]
    Fixture(#[from] std::io::Error),
    #[error("source runtime failed: {0}")]
    Runtime(#[from] RuntimeError),
    #[error("source contract mismatch: {0}")]
    Mismatch(String),
}

pub struct ContractHarness {
    runtime: ComponentRuntime,
    package_installer: PackageInstaller,
    limits: RuntimeLimits,
}

impl ContractHarness {
    pub fn new(store_root: impl AsRef<Path>) -> Self {
        Self {
            runtime: ComponentRuntime::new(),
            package_installer: PackageInstaller::new(store_root.as_ref().to_path_buf()),
            limits: RuntimeLimits::default(),
        }
    }

    pub fn with_limits(mut self, limits: RuntimeLimits) -> Self {
        self.limits = limits;
        self
    }

    pub async fn assert_conforms(
        &self,
        package: &Path,
        cases: &[SourceCase],
    ) -> Result<ConformanceReport, ContractTestError> {
        if cases.is_empty() {
            return Err(ContractTestError::Mismatch(
                "at least one source case is required".to_owned(),
            ));
        }
        let inspected = self.package_installer.inspect(package)?;
        let module = self.package_installer.install(package)?;
        let forbidden_imports_absent = wasm_has_no_forbidden_import_markers(&module);
        if !forbidden_imports_absent {
            return Err(ContractTestError::Mismatch(
                "source component contains a forbidden import marker".to_owned(),
            ));
        }
        if inspected.manifest.module_type() != mfa_contracts::ModuleType::Source {
            return Err(ContractTestError::Mismatch(
                "conformance package is not a source module".to_owned(),
            ));
        }

        let mut deterministic_probe = true;
        let mut deterministic_parse = true;
        for (index, case) in cases.iter().enumerate() {
            let bytes = fs::read(&case.fixture)?;
            let first_asset = asset(index, &case.fixture, bytes.clone());
            let second_asset = asset(index, &case.fixture, bytes);
            let first_probe = self
                .runtime
                .probe_source(&module, first_asset, self.limits)
                .await?;
            let second_probe = self
                .runtime
                .probe_source(&module, second_asset, self.limits)
                .await?;
            deterministic_probe &= first_probe == second_probe;
            if first_probe != case.expected_probe.confidence {
                return Err(ContractTestError::Mismatch(format!(
                    "{} probe was {}, expected {}",
                    case.fixture.display(),
                    first_probe,
                    case.expected_probe.confidence
                )));
            }

            let first = self
                .runtime
                .invoke_source(
                    &module,
                    asset(index, &case.fixture, fs::read(&case.fixture)?),
                    self.limits,
                )
                .await?;
            let second = self
                .runtime
                .invoke_source(
                    &module,
                    asset(index, &case.fixture, fs::read(&case.fixture)?),
                    self.limits,
                )
                .await?;
            deterministic_parse &= first == second;
            assertions::assert_expected(&first, &case.expected_result)?;
        }
        if !deterministic_probe || !deterministic_parse {
            return Err(ContractTestError::Mismatch(
                "source probe or parse output is not deterministic".to_owned(),
            ));
        }
        Ok(ConformanceReport {
            cases: cases.len(),
            deterministic_probe,
            deterministic_parse,
            forbidden_imports_absent,
        })
    }
}

fn asset(index: usize, fixture: &Path, bytes: Vec<u8>) -> Arc<dyn ReadOnlyAsset> {
    Arc::new(MemoryAsset {
        bytes,
        asset_id: Uuid::from_u128((index + 1) as u128),
        file_name: fixture
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("fixture")
            .to_owned(),
    })
}

struct MemoryAsset {
    bytes: Vec<u8>,
    asset_id: Uuid,
    file_name: String,
}

impl ReadOnlyAsset for MemoryAsset {
    fn metadata(&self) -> AssetMetadata {
        AssetMetadata {
            asset_id: self.asset_id,
            file_name: self.file_name.clone(),
            media_type: "application/octet-stream".to_owned(),
            byte_len: self.bytes.len() as u64,
        }
    }

    fn read_at(
        &self,
        offset: u64,
        max_bytes: u32,
    ) -> Result<Vec<u8>, mfa_contracts::AssetReadError> {
        let offset = usize::try_from(offset)
            .map_err(|_| mfa_contracts::AssetReadError::InvalidRange { offset, max_bytes })?;
        if offset > self.bytes.len() {
            return Err(mfa_contracts::AssetReadError::InvalidRange {
                offset: offset as u64,
                max_bytes,
            });
        }
        let end = offset
            .saturating_add(max_bytes as usize)
            .min(self.bytes.len());
        Ok(self.bytes[offset..end].to_vec())
    }
}

fn wasm_has_no_forbidden_import_markers(module: &InstalledModule) -> bool {
    let Ok(bytes) = fs::read(module.root.join("module.wasm")) else {
        return false;
    };
    let forbidden = [
        "wasi",
        "filesystem",
        "network",
        "duckdb",
        "raw_sql",
        "credentials",
        "javascript",
    ];
    let parser = wasmparser::Parser::new(0);
    for payload in parser.parse_all(&bytes) {
        let Ok(payload) = payload else {
            return false;
        };
        match payload {
            wasmparser::Payload::ComponentImportSection(section) => {
                for import in section {
                    let Ok(import) = import else {
                        return false;
                    };
                    if forbidden
                        .iter()
                        .any(|marker| import.name.name.to_ascii_lowercase().contains(marker))
                    {
                        return false;
                    }
                }
            }
            wasmparser::Payload::ImportSection(section) => {
                for imports in section {
                    let Ok(imports) = imports else {
                        return false;
                    };
                    for import in imports {
                        let Ok((_, import)) = import else {
                            return false;
                        };
                        if forbidden.iter().any(|marker| {
                            import.module.to_ascii_lowercase().contains(marker)
                                || import.name.to_ascii_lowercase().contains(marker)
                        }) {
                            return false;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    true
}
