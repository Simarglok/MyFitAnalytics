use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AtomicFileError {
    #[error("atomic file I/O failed: {0}")]
    Io(#[from] std::io::Error),
}

pub fn temporary_path(path: &Path) -> PathBuf {
    path.with_extension(format!(
        "{}tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .map(|value| format!("{value}."))
            .unwrap_or_default()
    ))
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), AtomicFileError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let temporary = temporary_path(path);
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    sync_parent(parent)?;
    Ok(())
}

pub fn recover_temporary(path: &Path) -> Result<bool, AtomicFileError> {
    let temporary = temporary_path(path);
    if !temporary.exists() {
        return Ok(false);
    }
    if path.exists() {
        fs::remove_file(temporary)?;
    } else {
        fs::rename(temporary, path)?;
        sync_parent(path.parent().unwrap_or_else(|| Path::new(".")))?;
    }
    Ok(true)
}

fn sync_parent(parent: &Path) -> Result<(), AtomicFileError> {
    #[cfg(unix)]
    {
        OpenOptions::new().read(true).open(parent)?.sync_all()?;
    }
    Ok(())
}
