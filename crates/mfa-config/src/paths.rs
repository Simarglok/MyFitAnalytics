use mfa_contracts::ModuleId;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub app_data: PathBuf,
    pub database: PathBuf,
    pub recovery: PathBuf,
    pub quarantine: PathBuf,
    pub module_store: PathBuf,
    pub logs: PathBuf,
    pub tmp: PathBuf,
}

impl AppPaths {
    pub fn new(app_data: impl Into<PathBuf>) -> Self {
        let app_data = app_data.into();
        Self {
            database: app_data.join("myfitanalytics.duckdb"),
            recovery: app_data.join("recovery"),
            quarantine: app_data.join("quarantine"),
            module_store: app_data.join("module-store"),
            logs: app_data.join("logs"),
            tmp: app_data.join("tmp"),
            app_data,
        }
    }

    pub fn ensure_directories(&self) -> Result<(), std::io::Error> {
        for directory in [
            &self.app_data,
            &self.recovery,
            &self.quarantine,
            &self.module_store,
            &self.logs,
            &self.tmp,
        ] {
            fs::create_dir_all(directory)?;
        }
        for directory in [
            self.module_store.join("sources"),
            self.module_store.join("dashboards"),
            self.module_store.join("locales"),
        ] {
            fs::create_dir_all(directory)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePaths {
    pub root: PathBuf,
    source_inbox_roots: BTreeMap<ModuleId, PathBuf>,
}

impl WorkspacePaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            source_inbox_roots: BTreeMap::new(),
        }
    }

    pub fn with_source_inbox_roots(
        root: impl Into<PathBuf>,
        source_inbox_roots: BTreeMap<ModuleId, PathBuf>,
    ) -> Self {
        Self {
            root: root.into(),
            source_inbox_roots,
        }
    }

    pub fn source_inbox(&self, source: &ModuleId) -> PathBuf {
        self.source_inbox_roots
            .get(source)
            .cloned()
            .unwrap_or_else(|| self.root.join("inbox").join(source.as_str()))
    }

    pub fn source_archive(&self, source: &ModuleId) -> PathBuf {
        self.root.join("archive").join(source.as_str())
    }

    pub fn enable_source(&self, source: &ModuleId) -> Result<(), std::io::Error> {
        fs::create_dir_all(self.source_inbox(source))?;
        fs::create_dir_all(self.source_archive(source))?;
        Ok(())
    }

    pub fn validate(
        workspace_root: impl AsRef<Path>,
        app_paths: &AppPaths,
    ) -> Result<(), PathPolicyError> {
        let workspace = comparable_path(workspace_root.as_ref());
        let app_data = comparable_path(&app_paths.app_data);
        if workspace == app_data || workspace.starts_with(&app_data) {
            return Err(PathPolicyError::WorkspaceInsideAppData {
                workspace,
                app_data,
            });
        }
        if app_data.starts_with(&workspace) {
            return Err(PathPolicyError::AppDataInsideWorkspace {
                workspace,
                app_data,
            });
        }
        Ok(())
    }

    pub fn validate_against(&self, app_paths: &AppPaths) -> Result<(), PathPolicyError> {
        Self::validate(&self.root, app_paths)
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PathPolicyError {
    #[error("workspace root cannot be equal to or nested inside application data")]
    WorkspaceInsideAppData {
        workspace: PathBuf,
        app_data: PathBuf,
    },
    #[error("application data cannot be nested inside workspace root")]
    AppDataInsideWorkspace {
        workspace: PathBuf,
        app_data: PathBuf,
    },
}

impl PathPolicyError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::WorkspaceInsideAppData { .. } => "workspace_inside_app_data",
            Self::AppDataInsideWorkspace { .. } => "app_data_inside_workspace",
        }
    }
}

fn comparable_path(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    let mut existing = absolute.clone();
    let mut suffix = Vec::new();
    while !existing.exists() {
        let Some(name) = existing.file_name() else {
            break;
        };
        suffix.push(name.to_owned());
        let Some(parent) = existing.parent() else {
            break;
        };
        existing = parent.to_path_buf();
    }
    let mut result = fs::canonicalize(&existing).unwrap_or(existing);
    for component in suffix.into_iter().rev() {
        result.push(component);
    }
    normalize_components(&result)
}

fn normalize_components(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(value) => normalized.push(value),
            Component::RootDir | Component::Prefix(_) => normalized.push(component.as_os_str()),
        }
    }
    normalized
}
