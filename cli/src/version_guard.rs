use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{CliError, Result};

const VERSION_FILE: &str = ".evm-cloud-version";

pub(crate) fn enforce_pinned_version_from_cwd() -> Result<()> {
    let cwd = env::current_dir().map_err(|source| CliError::Io {
        source,
        path: PathBuf::from("."),
    })?;

    let Some(version_file) = find_version_file(&cwd) else {
        return Ok(());
    };

    let raw = fs::read_to_string(&version_file).map_err(|source| CliError::Io {
        source,
        path: version_file.clone(),
    })?;

    let required = normalize_version(raw.trim()).ok_or_else(|| CliError::PinnedVersionInvalid {
        path: version_file.clone(),
        value: raw.trim().to_string(),
    })?;
    let current = normalize_version(env!("CARGO_PKG_VERSION")).expect("pkg version must be valid");

    if required != current {
        return Err(CliError::PinnedVersionMismatch {
            path: version_file,
            required,
            current,
        });
    }

    Ok(())
}

fn find_version_file(start: &Path) -> Option<PathBuf> {
    let mut cursor = Some(start);
    while let Some(path) = cursor {
        let candidate = path.join(VERSION_FILE);
        if candidate.is_file() {
            return Some(candidate);
        }
        cursor = path.parent();
    }

    None
}

fn normalize_version(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let without_prefix = trimmed.strip_prefix('v').unwrap_or(trimmed);
    if without_prefix.is_empty() {
        return None;
    }

    let mut chars = without_prefix.chars();
    let starts_with_digit = chars
        .next()
        .map(|character| character.is_ascii_digit())
        .unwrap_or(false);
    if !starts_with_digit {
        return None;
    }

    let looks_like_semver = without_prefix.contains('.');
    if !looks_like_semver {
        return None;
    }

    if !without_prefix.chars().all(|character| {
        character.is_ascii_alphanumeric()
            || character == '.'
            || character == '-'
            || character == '+'
    }) {
        return None;
    }

    Some(without_prefix.to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{find_version_file, normalize_version};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let base = std::env::temp_dir().join(format!(
            "evm-cloud-version-guard-tests-{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock before unix epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("create temp dir");
        base
    }

    #[test]
    fn normalizes_optional_v_prefix() {
        assert_eq!(normalize_version("v0.1.0").as_deref(), Some("0.1.0"));
        assert_eq!(normalize_version("0.1.0").as_deref(), Some("0.1.0"));
        assert_eq!(normalize_version("v0.1.0-alpha2").as_deref(), Some("0.1.0-alpha2"));
    }

    #[test]
    fn rejects_invalid_or_empty_values() {
        assert!(normalize_version("").is_none());
        assert!(normalize_version("v").is_none());
        assert!(normalize_version("latest").is_none());
        assert_eq!(normalize_version("0.1.0+build").as_deref(), Some("0.1.0+build"));
    }

    #[test]
    fn finds_nearest_file_while_walking_up() {
        let root = temp_dir("find-nearest");
        let project = root.join("project");
        let nested = project.join("nested/inner");
        fs::create_dir_all(&nested).expect("create nested path");

        fs::write(project.join(".evm-cloud-version"), "v0.1.0\n").expect("write version file");

        let found = find_version_file(&nested).expect("must find version file");
        assert_eq!(found, project.join(".evm-cloud-version"));
    }
}
