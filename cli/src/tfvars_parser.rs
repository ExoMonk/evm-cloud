use std::collections::HashMap;
use std::path::Path;

use crate::error::{CliError, Result};

/// Parse a `.tfvars` file into key-value pairs.
///
/// Handles: blank lines, `#` comments, inline comments after values,
/// single/double quoted values, and whitespace around `=`.
/// Quoted values preserve `#` characters (only unquoted values strip inline comments).
/// Keys with empty values after parsing are omitted from the result.
pub(crate) fn parse_tfvars(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((lhs, rhs)) = line.split_once('=') else {
            continue;
        };
        let key = lhs.trim().to_string();
        let value = extract_value(rhs.trim());
        if !value.is_empty() {
            map.insert(key, value);
        }
    }
    map
}

/// Extract a value from the RHS of a tfvars `key = value` line.
///
/// If the value is quoted, returns the content between matching quotes
/// (preserving `#` and other special characters). If unquoted, strips
/// inline `# comments` first.
fn extract_value(raw: &str) -> String {
    if let Some(inner) = raw.strip_prefix('"') {
        // Double-quoted: take everything up to the closing quote.
        return inner.split_once('"').map_or(inner, |(v, _)| v).to_string();
    }
    if let Some(inner) = raw.strip_prefix('\'') {
        // Single-quoted: take everything up to the closing quote.
        return inner.split_once('\'').map_or(inner, |(v, _)| v).to_string();
    }
    // Unquoted: strip inline comments.
    raw.split('#').next().unwrap_or("").trim().to_string()
}

/// Read and parse the first existing tfvars file from `candidates`.
pub(crate) fn parse_first_existing(candidates: &[std::path::PathBuf]) -> Result<HashMap<String, String>> {
    for path in candidates {
        if !path.is_file() {
            continue;
        }
        let raw = std::fs::read_to_string(path).map_err(|source| CliError::Io {
            source,
            path: path.clone(),
        })?;
        return Ok(parse_tfvars(&raw));
    }
    Ok(HashMap::new())
}

/// Read and merge all existing tfvars files from `candidates` (first-wins per key).
pub(crate) fn parse_all_existing(candidates: &[std::path::PathBuf]) -> Result<HashMap<String, String>> {
    let mut merged = HashMap::new();
    for path in candidates {
        if !path.is_file() {
            continue;
        }
        let raw = std::fs::read_to_string(path).map_err(|source| CliError::Io {
            source,
            path: path.clone(),
        })?;
        for (key, value) in parse_tfvars(&raw) {
            merged.entry(key).or_insert(value);
        }
    }
    Ok(merged)
}

/// Look up a single value from a tfvars file at `path`.
pub(crate) fn lookup(path: &Path, key: &str) -> Result<Option<String>> {
    if !path.is_file() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path).map_err(|source| CliError::Io {
        source,
        path: path.to_path_buf(),
    })?;
    Ok(parse_tfvars(&raw).remove(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_key_value() {
        let input = r#"
            key1 = "value1"
            key2 = 'value2'
            key3 = bare_value
        "#;
        let map = parse_tfvars(input);
        assert_eq!(map.get("key1").unwrap(), "value1");
        assert_eq!(map.get("key2").unwrap(), "value2");
        assert_eq!(map.get("key3").unwrap(), "bare_value");
    }

    #[test]
    fn strips_inline_comments() {
        let input = r#"password = "s3cret" # my password"#;
        let map = parse_tfvars(input);
        assert_eq!(map.get("password").unwrap(), "s3cret");
    }

    #[test]
    fn skips_comment_and_empty_lines() {
        let input = "# comment\n\n  \nkey = val\n# another comment\n";
        let map = parse_tfvars(input);
        assert_eq!(map.len(), 1);
        assert_eq!(map.get("key").unwrap(), "val");
    }

    #[test]
    fn skips_empty_values() {
        let input = "key = \"\"\nkey2 = ''";
        let map = parse_tfvars(input);
        assert!(map.is_empty());
    }

    #[test]
    fn preserves_hash_in_quoted_values() {
        let input = "password = \"my#secret\"\ntoken = 'abc#def'";
        let map = parse_tfvars(input);
        assert_eq!(map.get("password").unwrap(), "my#secret");
        assert_eq!(map.get("token").unwrap(), "abc#def");
    }

    #[test]
    fn strips_hash_from_bare_values() {
        let input = "key = bare_value # inline comment";
        let map = parse_tfvars(input);
        assert_eq!(map.get("key").unwrap(), "bare_value");
    }

    #[test]
    fn handles_equals_in_value() {
        let input = r#"url = "postgres://user:pass@host:5432/db?sslmode=require""#;
        let map = parse_tfvars(input);
        assert_eq!(
            map.get("url").unwrap(),
            "postgres://user:pass@host:5432/db?sslmode=require"
        );
    }
}
