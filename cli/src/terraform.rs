use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use serde::Deserialize;
use serde_json::Value;

use crate::error::{CliError, Result};

const BASELINE_MIN_VERSION: (u32, u32, u32) = (1, 14, 6);
type VersionTuple = (u32, u32, u32);
type VersionFloor = (VersionTuple, String);

pub(crate) struct TerraformRunner {
    binary_path: PathBuf,
    version: String,
}

#[derive(Deserialize)]
struct TerraformVersionJson {
    terraform_version: String,
}

impl TerraformRunner {
    pub(crate) fn check_installed(root: &Path) -> Result<Self> {
        let binary_path = which::which("terraform").map_err(|_| CliError::TerraformNotFound)?;

        let version = probe_terraform_version(&binary_path)?;
        let found_tuple = parse_version(&version).ok_or_else(|| CliError::TerraformVersionProbeFailed {
            details: format!("unsupported terraform version format: {version}"),
        })?;

        let mut required = BASELINE_MIN_VERSION;
        let mut required_str = tuple_to_version(required);

        if let Some((floor, floor_str)) = root_floor_requirement(root)? {
            if floor > required {
                required = floor;
                required_str = floor_str;
            }
        }

        if found_tuple < required {
            return Err(CliError::TerraformVersionTooOld {
                found: version,
                minimum: required_str,
            });
        }

        Ok(Self {
            binary_path,
            version,
        })
    }

    pub(crate) fn init(&self, dir: &Path, passthrough_args: &[String]) -> Result<()> {
        let mut args = vec!["init".to_string()];
        args.extend_from_slice(passthrough_args);
        self.run_inherited(dir, &args)
    }

    pub(crate) fn apply(&self, dir: &Path, auto_approve: bool, passthrough_args: &[String]) -> Result<()> {
        let mut args = vec!["apply".to_string()];
        if auto_approve {
            args.push("-auto-approve".to_string());
        }
        args.extend_from_slice(passthrough_args);
        self.run_inherited(dir, &args)
    }

    pub(crate) fn destroy(&self, dir: &Path, auto_approve: bool, passthrough_args: &[String]) -> Result<()> {
        let mut args = vec!["destroy".to_string()];
        if auto_approve {
            args.push("-auto-approve".to_string());
        }
        args.extend_from_slice(passthrough_args);
        self.run_inherited(dir, &args)
    }

    #[allow(dead_code)]
    pub(crate) fn output_json(&self, dir: &Path) -> Result<Value> {
        let output = Command::new(&self.binary_path)
            .args(["output", "-json"])
            .current_dir(dir)
            .stdin(Stdio::inherit())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|err| CliError::Other(err.into()))?;

        map_status(output.status)?;
        let parsed = serde_json::from_slice::<Value>(&output.stdout)?;
        Ok(parsed)
    }

    pub(crate) fn version(&self) -> &str {
        &self.version
    }

    fn run_inherited(&self, dir: &Path, args: &[String]) -> Result<()> {
        let status = Command::new(&self.binary_path)
            .args(args)
            .current_dir(dir)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|err| CliError::Other(err.into()))?;

        map_status(status)
    }
}

pub(crate) fn map_status(status: ExitStatus) -> Result<()> {
    if status.success() {
        return Ok(());
    }

    if let Some(code) = status.code() {
        return Err(CliError::TerraformFailed { code });
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        Err(CliError::TerraformSignaled {
            signal: status.signal(),
        })
    }

    #[cfg(not(unix))]
    {
        Err(CliError::TerraformSignaled { signal: None })
    }
}

fn probe_terraform_version(binary_path: &Path) -> Result<String> {
    let json_attempt = Command::new(binary_path)
        .args(["version", "-json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| CliError::Other(err.into()))?;

    if json_attempt.status.success() {
        let parsed: TerraformVersionJson = serde_json::from_slice(&json_attempt.stdout).map_err(|err| {
            CliError::TerraformVersionProbeFailed {
                details: format!("invalid JSON from `terraform version -json`: {err}"),
            }
        })?;
        return Ok(parsed.terraform_version);
    }

    let text_attempt = Command::new(binary_path)
        .arg("version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| CliError::Other(err.into()))?;

    if !text_attempt.status.success() {
        return Err(CliError::TerraformVersionProbeFailed {
            details: format!(
                "failed to run `terraform version`: {}",
                String::from_utf8_lossy(&text_attempt.stderr).trim()
            ),
        });
    }

    let stdout = String::from_utf8_lossy(&text_attempt.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(version) = trimmed.strip_prefix("Terraform v") {
            if parse_version(version).is_some() {
                return Ok(version.to_string());
            }
        }
    }

    Err(CliError::TerraformVersionProbeFailed {
        details: "could not parse terraform version from plain-text output".to_string(),
    })
}

fn root_floor_requirement(root: &Path) -> Result<Option<VersionFloor>> {
    let versions_path = root.join("versions.tf");
    if !versions_path.exists() {
        return Ok(None);
    }

    let contents = fs::read_to_string(&versions_path).map_err(|source| CliError::Io {
        source,
        path: versions_path.clone(),
    })?;

    parse_required_version_floor(&contents).map_err(|details| CliError::TerraformVersionProbeFailed {
        details: format!("{} ({})", details, versions_path.display()),
    })
}

fn parse_required_version_floor(contents: &str) -> std::result::Result<Option<VersionFloor>, String> {
    let mut required_expr: Option<String> = None;

    for line in contents.lines() {
        let trimmed = line.trim();
        if !trimmed.contains("required_version") {
            continue;
        }

        let first_quote = trimmed.find('"').ok_or_else(|| {
            "`required_version` found but expression is not a quoted string".to_string()
        })?;
        let rest = &trimmed[(first_quote + 1)..];
        let second_quote = rest.find('"').ok_or_else(|| {
            "`required_version` found but closing quote is missing".to_string()
        })?;

        required_expr = Some(rest[..second_quote].to_string());
        break;
    }

    let Some(expr) = required_expr else {
        return Ok(None);
    };

    let mut best_floor: Option<(u32, u32, u32)> = None;
    let mut best_floor_str: Option<String> = None;

    for part in expr.split(',') {
        let constraint = part.trim();
        let normalized = if let Some(rest) = constraint.strip_prefix(">=") {
            Some(rest.trim())
        } else {
            constraint.strip_prefix('>').map(|rest| rest.trim())
        };

        let Some(version_str) = normalized else {
            continue;
        };

        let parsed = parse_version(version_str).ok_or_else(|| {
            format!("unable to parse version constraint `{constraint}` in `{expr}`")
        })?;

        if best_floor.is_none() || parsed > best_floor.unwrap() {
            best_floor = Some(parsed);
            best_floor_str = Some(version_str.trim_start_matches('v').to_string());
        }
    }

    match (best_floor, best_floor_str) {
        (Some(floor), Some(version)) => Ok(Some((floor, version))),
        _ => Err(format!(
            "no parseable lower-bound constraint found in required_version expression `{expr}`"
        )),
    }
}

fn tuple_to_version(v: (u32, u32, u32)) -> String {
    format!("{}.{}.{}", v.0, v.1, v.2)
}

fn parse_version(version_str: &str) -> Option<(u32, u32, u32)> {
    let cleaned = version_str.trim().trim_start_matches('v');
    let parts: Vec<&str> = cleaned.split('.').collect();
    if parts.len() < 3 {
        return None;
    }

    let major = parts[0].parse::<u32>().ok()?;
    let minor = parts[1].parse::<u32>().ok()?;

    let patch_digits: String = parts[2]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();

    if patch_digits.is_empty() {
        return None;
    }

    let patch = patch_digits.parse::<u32>().ok()?;
    Some((major, minor, patch))
}
