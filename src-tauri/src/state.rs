use mfa_config::{AppSettings, SettingsError, SettingsStore};
use mfa_module_host::{
    CapabilityError, CapabilityRegistry, InstalledModule, LocaleError, LocaleResolver,
    ModuleRegistry, PackageError, PackageInstaller, ProviderResolution,
};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppStateError {
    #[error("settings could not be loaded: {0}")]
    Settings(#[from] SettingsError),
    #[error("installed modules could not be loaded: {0}")]
    Modules(#[from] PackageError),
    #[error("capability providers could not be resolved: {0}")]
    Capabilities(#[from] CapabilityError),
    #[error("locale catalogs could not be loaded: {0}")]
    Locales(#[from] LocaleError),
}

impl AppStateError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Settings(error) => error.code(),
            Self::Modules(error) => error.code(),
            Self::Capabilities(error) => error.code(),
            Self::Locales(error) => error.code(),
        }
    }
}

pub struct AppState {
    pub(crate) settings: AppSettings,
    pub(crate) modules: Vec<InstalledModule>,
    pub(crate) providers: ProviderResolution,
    pub(crate) locale: LocaleResolver,
}

impl AppState {
    pub fn from_roots(
        config_root: impl AsRef<Path>,
        module_root: impl AsRef<Path>,
        core_locale_root: impl AsRef<Path>,
    ) -> Result<Self, AppStateError> {
        let settings = SettingsStore::new(config_root.as_ref().join("settings.json")).load()?;
        let installer = PackageInstaller::new(module_root.as_ref());
        let modules = installer.list()?;
        let providers = CapabilityRegistry::new().resolve(&modules, &settings)?;
        let locale = LocaleResolver::new(core_locale_root, modules.clone())?;
        Ok(Self {
            settings,
            modules,
            providers,
            locale,
        })
    }

    pub fn from_roots_with_core_catalog(
        config_root: impl AsRef<Path>,
        module_root: impl AsRef<Path>,
        core_catalog: &[u8],
    ) -> Result<Self, AppStateError> {
        let settings = SettingsStore::new(config_root.as_ref().join("settings.json")).load()?;
        let installer = PackageInstaller::new(module_root.as_ref());
        let modules = installer.list()?;
        let providers = CapabilityRegistry::new().resolve(&modules, &settings)?;
        let locale = LocaleResolver::from_core_json(core_catalog, modules.clone())?;
        Ok(Self {
            settings,
            modules,
            providers,
            locale,
        })
    }

    pub(crate) fn settings(&self) -> &AppSettings {
        &self.settings
    }

    pub(crate) fn modules(&self) -> &[InstalledModule] {
        &self.modules
    }

    pub(crate) fn providers(&self) -> &ProviderResolution {
        &self.providers
    }

    #[allow(dead_code)]
    pub(crate) fn locale(&self) -> &LocaleResolver {
        &self.locale
    }
}
