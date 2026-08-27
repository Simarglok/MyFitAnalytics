use crate::error::PackageError;
use crate::package::{InstalledModule, PackageInstaller};
use mfa_contracts::ModuleId;

pub trait ModuleRegistry {
    fn list(&self) -> Result<Vec<InstalledModule>, PackageError>;
    fn resolve_active(&self, id: &ModuleId) -> Result<InstalledModule, PackageError>;
}

impl ModuleRegistry for PackageInstaller {
    fn list(&self) -> Result<Vec<InstalledModule>, PackageError> {
        self.current_registry()
    }

    fn resolve_active(&self, id: &ModuleId) -> Result<InstalledModule, PackageError> {
        let mut candidates: Vec<_> = self
            .current_registry()?
            .into_iter()
            .filter(|module| &module.module_id == id && module.enabled)
            .collect();
        candidates.sort_by(|left, right| {
            left.module_version
                .cmp(&right.module_version)
                .then_with(|| left.package_hash.cmp(&right.package_hash))
        });
        candidates
            .pop()
            .ok_or_else(|| PackageError::NoActiveModule {
                module_id: id.to_string(),
            })
    }
}
