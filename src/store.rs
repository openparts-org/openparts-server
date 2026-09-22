//! Loads the `openparts-data` directory into an in-memory index at
//! startup. Architecture Specification section 22 describes an
//! Importer feeding PostgreSQL; this is a minimal first slice of that
//! role -- an in-memory "Derived Database" built directly from
//! Canonical Data, with no Canonical-Data write path (section 44:
//! "DBからGitへCanonical Dataを自動逆生成しない" holds trivially since
//! there is no DB write path at all yet). Swapping this for a real
//! PostgreSQL-backed importer is future work; the HTTP layer above it
//! does not need to change to make that swap.

use openparts_core::{Device, Package, Part, Source};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Default)]
pub struct Store {
    pub parts: HashMap<(String, String), Part>, // (manufacturer, mpn) -> Part
    pub devices: HashMap<String, Device>,
    pub packages: HashMap<String, Package>,
    pub sources: HashMap<String, Source>,
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse {path}: {source}")]
    Yaml {
        path: String,
        #[source]
        source: serde_yaml::Error,
    },
    #[error(transparent)]
    Data(#[from] openparts_data::DataError),
}

/// Recursively finds every `*.yaml` file under `dir`.
fn find_yaml_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            find_yaml_files(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "yaml") {
            out.push(path);
        }
    }
    Ok(())
}

/// Peeks at a YAML document's `kind` field without committing to a
/// specific target type, so the directory walk can dispatch to the
/// right typed loader (Canonical Data Specification section 5).
fn peek_kind(path: &Path, text: &str) -> Result<Option<String>, LoadError> {
    let value: serde_yaml::Value =
        serde_yaml::from_str(text).map_err(|source| LoadError::Yaml {
            path: path.display().to_string(),
            source,
        })?;
    Ok(value
        .get("kind")
        .and_then(|k| k.as_str())
        .map(|s| s.to_string()))
}

pub fn discover_and_load(data_dir: &Path) -> Result<Store, LoadError> {
    let mut paths = Vec::new();
    find_yaml_files(data_dir, &mut paths).map_err(|source| LoadError::Io {
        path: data_dir.display().to_string(),
        source,
    })?;

    let mut store = Store::default();
    for path in paths {
        let text = std::fs::read_to_string(&path).map_err(|source| LoadError::Io {
            path: path.display().to_string(),
            source,
        })?;
        let Some(kind) = peek_kind(&path, &text)? else {
            tracing::warn!(path = %path.display(), "skipping YAML file with no `kind` field");
            continue;
        };

        match kind.as_str() {
            "part" => {
                let part = openparts_data::load_part(&path)?;
                store
                    .parts
                    .insert((part.manufacturer.0.clone(), part.mpn.clone()), part);
            }
            "device" => {
                let device = openparts_data::load_device(&path)?;
                store.devices.insert(device.id.0.clone(), device);
            }
            "package" => {
                let package = openparts_data::load_package(&path)?;
                store.packages.insert(package.id.0.clone(), package);
            }
            "source" => {
                let source = openparts_data::load_source(&path)?;
                store.sources.insert(source.id.0.clone(), source);
            }
            // manufacturer/erratum/rejected aren't needed by any route
            // yet -- skip rather than error, this is a discovery walk
            // over a directory that legitimately holds more kinds than
            // this server currently serves.
            _ => {}
        }
    }

    tracing::info!(
        parts = store.parts.len(),
        devices = store.devices.len(),
        packages = store.packages.len(),
        sources = store.sources.len(),
        "loaded openparts-data"
    );

    Ok(store)
}
