use std::fs;
use std::path::Path;

use crate::error::{CliError, Result};

pub(crate) mod manifest;
pub(crate) mod scaffold;
pub(crate) mod tfvars;

pub(crate) fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    let parent = path.parent().ok_or_else(|| CliError::Io {
        source: std::io::Error::new(std::io::ErrorKind::NotFound, "no parent directory"),
        path: path.to_path_buf(),
    })?;

    fs::create_dir_all(parent).map_err(|source| CliError::Io {
        source,
        path: parent.to_path_buf(),
    })?;

    let tmp_name = format!(
        ".{}.tmp.{}.{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("evm-cloud"),
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|err| CliError::SystemClock(err.to_string()))?
            .as_nanos(),
    );

    let tmp_path = parent.join(tmp_name);
    fs::write(&tmp_path, contents).map_err(|source| CliError::Io {
        source,
        path: tmp_path.clone(),
    })?;

    fs::rename(&tmp_path, path).map_err(|source| CliError::Io {
        source,
        path: path.to_path_buf(),
    })?;

    Ok(())
}
