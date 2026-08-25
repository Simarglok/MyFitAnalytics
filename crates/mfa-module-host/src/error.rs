use std::path::PathBuf;
use thiserror::Error;
use zip::result::ZipError;

#[derive(Debug, Error)]
pub enum PackageError {
    #[error("package I/O failed: {detail}")]
    Io { detail: String },
    #[error("package archive is invalid: {detail}")]
    Zip { detail: String },
    #[error("package extension does not match module type")]
    ModuleExtensionMismatch,
    #[error("archive path is absolute: {path}")]
    AbsolutePath { path: String },
    #[error("archive path traverses its root: {path}")]
    PathTraversal { path: String },
    #[error("archive path is not canonical: {path}")]
    InvalidPath { path: String },
    #[error("archive contains duplicate entry: {path}")]
    DuplicateEntry { path: String },
    #[error("archive contains duplicate module manifest")]
    DuplicateManifest,
    #[error("archive contains a symbolic link: {path}")]
    SymlinkEntry { path: String },
    #[error("archive contains an unexpected executable entry: {path}")]
    UnexpectedExecutable { path: String },
    #[error("locale archive contains executable entry: {path}")]
    ExecutableLocaleEntry { path: String },
    #[error("archive exceeds uncompressed size limit")]
    UncompressedSizeLimit,
    #[error("module manifest is missing")]
    ManifestMissing,
    #[error("module manifest is not valid JSON: {detail}")]
    ManifestInvalidJson { detail: String },
    #[error("module manifest is invalid: {detail}")]
    ManifestInvalid { detail: String },
    #[error("module manifest does not satisfy its schema: {detail}")]
    ManifestSchemaInvalid { detail: String },
    #[error("module entrypoint is missing")]
    EntrypointMissing,
    #[error("module entrypoint hash does not match manifest")]
    EntrypointHashMismatch,
    #[error("module entrypoint payload hash is invalid")]
    EntrypointHashInvalid,
    #[error("source API version is incompatible with this host")]
    IncompatibleSourceApi,
    #[error("dashboard API version is incompatible with this host")]
    IncompatibleDashboardApi,
    #[error("package format version is incompatible with this host")]
    IncompatiblePackageFormat,
    #[error("locale payload hash does not match manifest: {path}")]
    LocalePayloadHashMismatch { path: String },
    #[error("module is not installed: {module_id}")]
    ModuleNotFound { module_id: String },
    #[error("module has no active installed version: {module_id}")]
    NoActiveModule { module_id: String },
    #[error("installed module is corrupt: {path}")]
    InstalledModuleCorrupt { path: PathBuf },
    #[error("atomic installation failed: {detail}")]
    AtomicInstall { detail: String },
    #[error("mutable module state is invalid: {detail}")]
    StateInvalid { detail: String },
}

impl PackageError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io { .. } => "package_io_error",
            Self::Zip { .. } => "package_zip_error",
            Self::ModuleExtensionMismatch => "module_extension_mismatch",
            Self::AbsolutePath { .. } => "absolute_path",
            Self::PathTraversal { .. } => "path_traversal",
            Self::InvalidPath { .. } => "invalid_archive_path",
            Self::DuplicateEntry { .. } => "duplicate_entry",
            Self::DuplicateManifest => "duplicate_manifest",
            Self::SymlinkEntry { .. } => "symlink_entry",
            Self::UnexpectedExecutable { .. } => "unexpected_executable",
            Self::ExecutableLocaleEntry { .. } => "executable_locale_entry",
            Self::UncompressedSizeLimit => "uncompressed_size_limit",
            Self::ManifestMissing => "manifest_missing",
            Self::ManifestInvalidJson { .. } => "manifest_invalid_json",
            Self::ManifestInvalid { .. } => "manifest_invalid",
            Self::ManifestSchemaInvalid { .. } => "manifest_schema_invalid",
            Self::EntrypointMissing => "entrypoint_missing",
            Self::EntrypointHashMismatch => "entrypoint_hash_mismatch",
            Self::EntrypointHashInvalid => "entrypoint_hash_invalid",
            Self::IncompatibleSourceApi => "incompatible_source_api",
            Self::IncompatibleDashboardApi => "incompatible_dashboard_api",
            Self::IncompatiblePackageFormat => "incompatible_package_format",
            Self::LocalePayloadHashMismatch { .. } => "locale_payload_hash_mismatch",
            Self::ModuleNotFound { .. } => "module_not_found",
            Self::NoActiveModule { .. } => "no_active_module",
            Self::InstalledModuleCorrupt { .. } => "installed_module_corrupt",
            Self::AtomicInstall { .. } => "atomic_install_failed",
            Self::StateInvalid { .. } => "state_invalid",
        }
    }
}

impl From<std::io::Error> for PackageError {
    fn from(error: std::io::Error) -> Self {
        Self::Io {
            detail: error.to_string(),
        }
    }
}

impl From<ZipError> for PackageError {
    fn from(error: ZipError) -> Self {
        Self::Zip {
            detail: error.to_string(),
        }
    }
}
