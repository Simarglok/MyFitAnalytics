use mfa_contracts::ModuleManifest;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LocaleError {
    #[error("locale catalog I/O failed: {detail}")]
    Io { detail: String },
    #[error("locale catalog is invalid: {detail}")]
    InvalidCatalog { detail: String },
    #[error(
        "locale namespace {locale}/{namespace} is provided by both {first_module} and {second_module}"
    )]
    NamespaceCollision {
        locale: String,
        namespace: String,
        first_module: String,
        second_module: String,
    },
}

impl LocaleError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io { .. } => "locale_io",
            Self::InvalidCatalog { .. } => "invalid_locale_catalog",
            Self::NamespaceCollision { .. } => "namespace_collision",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedMessage {
    Found(String),
    Missing(String),
    InvalidPlaceholders { missing: Vec<String> },
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogFile {
    locale: String,
    namespace: String,
    messages: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
struct Catalog {
    module_id: String,
    locale: String,
    namespace: String,
    messages: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct LocaleResolver {
    core: Catalog,
    locale_modules: BTreeMap<(String, String), Catalog>,
    executable_english: BTreeMap<String, Catalog>,
}

impl LocaleResolver {
    pub fn new(
        core_root: impl AsRef<Path>,
        modules: Vec<crate::InstalledModule>,
    ) -> Result<Self, LocaleError> {
        let core = read_catalog(core_root.as_ref().join("messages.json"), "core")?;
        if core.locale != "en" || core.namespace != "core" {
            return Err(LocaleError::InvalidCatalog {
                detail: "core catalog must declare locale en and namespace core".to_owned(),
            });
        }
        let mut locale_modules = BTreeMap::new();
        let mut executable_english = BTreeMap::new();

        for module in modules.into_iter().filter(|module| module.enabled) {
            match &module.manifest {
                ModuleManifest::Locale(manifest) => {
                    let catalog =
                        read_catalog(module.root.join("messages.json"), module.module_id.as_str())?;
                    if catalog.locale != manifest.locale
                        || catalog.namespace != manifest.localization_namespace
                    {
                        return Err(LocaleError::InvalidCatalog {
                            detail: format!(
                                "locale catalog metadata does not match {}",
                                module.module_id
                            ),
                        });
                    }
                    let key = (
                        manifest.locale.clone(),
                        manifest.localization_namespace.clone(),
                    );
                    insert_locale_catalog(
                        &mut locale_modules,
                        key,
                        catalog,
                        module.module_id.as_str(),
                    )?;
                }
                ModuleManifest::Source(manifest) => {
                    insert_executable_catalog(
                        &mut executable_english,
                        &module,
                        &manifest.localization_namespace,
                    )?;
                }
                ModuleManifest::Dashboard(manifest) => {
                    insert_executable_catalog(
                        &mut executable_english,
                        &module,
                        &manifest.localization_namespace,
                    )?;
                }
            }
        }

        Ok(Self {
            core,
            locale_modules,
            executable_english,
        })
    }

    pub fn message(
        &self,
        locale: &str,
        namespace: &str,
        key: &str,
        args: &Value,
    ) -> ResolvedMessage {
        let template = self
            .locale_modules
            .get(&(locale.to_owned(), namespace.to_owned()))
            .and_then(|catalog| catalog.messages.get(key))
            .or_else(|| {
                self.executable_english
                    .get(namespace)
                    .and_then(|catalog| catalog.messages.get(key))
            })
            .or_else(|| self.core.messages.get(key));

        let Some(template) = template else {
            return ResolvedMessage::Missing(format!("⟦missing:{namespace}.{key}⟧"));
        };
        format_template(template, args)
    }
}

fn insert_locale_catalog(
    catalogs: &mut BTreeMap<(String, String), Catalog>,
    key: (String, String),
    catalog: Catalog,
    module_id: &str,
) -> Result<(), LocaleError> {
    if let Some(existing) = catalogs.get(&key)
        && existing.module_id != module_id
    {
        return Err(LocaleError::NamespaceCollision {
            locale: key.0,
            namespace: key.1,
            first_module: existing.module_id.clone(),
            second_module: module_id.to_owned(),
        });
    }
    catalogs.insert(key, catalog);
    Ok(())
}

fn insert_executable_catalog(
    catalogs: &mut BTreeMap<String, Catalog>,
    module: &crate::InstalledModule,
    namespace: &str,
) -> Result<(), LocaleError> {
    let path = module.root.join("messages.json");
    if !path.exists() {
        return Ok(());
    }
    let catalog = read_catalog(path, module.module_id.as_str())?;
    if catalog.locale != "en" || catalog.namespace != namespace {
        return Err(LocaleError::InvalidCatalog {
            detail: format!(
                "executable catalog metadata does not match {}",
                module.module_id
            ),
        });
    }
    if let Some(existing) = catalogs.get(namespace)
        && existing.module_id != module.module_id.as_str()
    {
        return Err(LocaleError::NamespaceCollision {
            locale: "en".to_owned(),
            namespace: namespace.to_owned(),
            first_module: existing.module_id.clone(),
            second_module: module.module_id.to_string(),
        });
    }
    catalogs.insert(namespace.to_owned(), catalog);
    Ok(())
}

fn read_catalog(path: impl AsRef<Path>, module_id: &str) -> Result<Catalog, LocaleError> {
    let bytes = fs::read(path).map_err(|error| LocaleError::Io {
        detail: error.to_string(),
    })?;
    let file: CatalogFile =
        serde_json::from_slice(&bytes).map_err(|error| LocaleError::InvalidCatalog {
            detail: error.to_string(),
        })?;
    if file.locale.trim().is_empty() || file.namespace.trim().is_empty() {
        return Err(LocaleError::InvalidCatalog {
            detail: "locale and namespace are required".to_owned(),
        });
    }
    Ok(Catalog {
        module_id: module_id.to_owned(),
        locale: file.locale,
        namespace: file.namespace,
        messages: file.messages,
    })
}

fn format_template(template: &str, args: &Value) -> ResolvedMessage {
    let mut output = String::with_capacity(template.len());
    let mut remaining = template;
    let mut missing = Vec::new();
    while let Some(open) = remaining.find('{') {
        if remaining[..open].contains('}') {
            return ResolvedMessage::InvalidPlaceholders {
                missing: vec!["<unmatched-close>".to_owned()],
            };
        }
        output.push_str(&remaining[..open]);
        let after_open = &remaining[open + 1..];
        let Some(close) = after_open.find('}') else {
            return ResolvedMessage::InvalidPlaceholders {
                missing: vec!["<unclosed>".to_owned()],
            };
        };
        let name = &after_open[..close];
        if name.is_empty()
            || !name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            return ResolvedMessage::InvalidPlaceholders {
                missing: vec![name.to_owned()],
            };
        }
        match args.get(name) {
            Some(value) => output.push_str(&value_text(value)),
            None => missing.push(name.to_owned()),
        }
        remaining = &after_open[close + 1..];
    }
    if remaining.contains('}') {
        return ResolvedMessage::InvalidPlaceholders {
            missing: vec!["<unmatched-close>".to_owned()],
        };
    }
    output.push_str(remaining);
    missing.sort();
    missing.dedup();
    if missing.is_empty() {
        ResolvedMessage::Found(output)
    } else {
        ResolvedMessage::InvalidPlaceholders { missing }
    }
}

fn value_text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}
