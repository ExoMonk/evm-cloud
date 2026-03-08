use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use serde::Deserialize;
use serde_json::Value;

use crate::error::{CliError, Result};

const BASELINE_MIN_VERSION: (u32, u32, u32) = (1, 14, 6);
pub(crate) const REQUIRED_VERSION_CONSTRAINT: &str = ">= 1.14.6";
const DEFAULT_TERRAFORM_PARALLELISM: &str = "-parallelism=3";
type VersionTuple = (u32, u32, u32);
type VersionFloor = (VersionTuple, String);

pub(crate) struct TerraformRunner {
    binary_path: PathBuf,
    extra_env: Vec<(String, String)>,
}

/// Find a single `.tfbackend` file in `dir` (non-recursive).
/// Returns the **filename only** (not full path) since Terraform runs with
/// `current_dir(dir)` and resolves `-backend-config` relative to its cwd.
/// Returns `None` if zero or 2+ files found (ambiguous).
pub(crate) fn find_tfbackend(dir: &Path) -> Option<PathBuf> {
    let (found, count) = find_tfbackend_inner(dir);
    if count > 1 {
        return None;
    }
    found
}

/// Returns (filename_only, total_count) of `.tfbackend` files in `dir`.
fn find_tfbackend_inner(dir: &Path) -> (Option<PathBuf>, usize) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return (None, 0),
    };
    let mut first: Option<PathBuf> = None;
    let mut count = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("tfbackend") && path.is_file() {
            count += 1;
            if first.is_none() {
                // Store filename only — terraform runs with current_dir(dir).
                first = path.file_name().map(PathBuf::from);
            }
        }
    }
    (first, count)
}

#[derive(Deserialize)]
struct TerraformVersionJson {
    terraform_version: String,
}

impl TerraformRunner {
    pub(crate) fn check_installed(root: &Path) -> Result<Self> {
        let binary_path = which::which("terraform").map_err(|_| CliError::TerraformNotFound)?;

        let version = probe_terraform_version(&binary_path)?;
        let found_tuple =
            parse_version(&version).ok_or_else(|| CliError::TerraformVersionProbeFailed {
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
            extra_env: Vec::new(),
        })
    }

    /// Configure the runner with environment-specific TF_DATA_DIR.
    pub(crate) fn with_env(mut self, env_ctx: &crate::env::EnvContext) -> Self {
        self.extra_env.push((
            "TF_DATA_DIR".into(),
            env_ctx.tf_data_dir.display().to_string(),
        ));
        self
    }

    pub(crate) fn init(&self, dir: &Path, passthrough_args: &[String]) -> Result<()> {
        let mut args = vec!["init".to_string()];
        args.extend_from_slice(passthrough_args);
        self.run_captured(dir, &args)
    }

    /// Skip init if `.terraform/` already exists and no passthrough args.
    /// Auto-injects `-backend-config=<file>` on first init or reconfigure.
    ///
    /// When `env_ctx` is `Some`, uses the env's `tf_data_dir` for existence
    /// checks and `tfbackend` for backend config injection.
    pub(crate) fn init_if_needed(
        &self,
        dir: &Path,
        env_ctx: Option<&crate::env::EnvContext>,
        passthrough_args: &[String],
    ) -> Result<bool> {
        let terraform_exists = match env_ctx {
            Some(ctx) => ctx.tf_data_dir.is_dir(),
            None => dir.join(".terraform").is_dir(),
        };

        // Fast path: already initialized and caller has no special flags.
        if terraform_exists && passthrough_args.is_empty() {
            return Ok(false);
        }

        let mut args = Vec::new();

        // Auto-add -backend-config on first init or reconfigure/migrate.
        let has_reconfigure = passthrough_args.iter().any(|a| {
            a == "-reconfigure"
                || a == "--reconfigure"
                || a == "-migrate-state"
                || a == "--migrate-state"
        });

        if let Some(ctx) = env_ctx {
            // Env-aware: always inject -backend-config so Terraform uses the
            // correct per-env backend, even on -upgrade or other passthrough args.
            args.push(format!("-backend-config={}", ctx.tfbackend.display()));
        } else if !terraform_exists || has_reconfigure {
            let (found, count) = find_tfbackend_inner(dir);
            match count.cmp(&1) {
                std::cmp::Ordering::Equal => {
                    if let Some(tfbackend) = found {
                        args.push(format!("-backend-config={}", tfbackend.display()));
                    }
                }
                std::cmp::Ordering::Greater => {
                    eprintln!(
                        "     ⚠ Found {} .tfbackend files — skipping auto-injection. \
                         Pass --tf-args='-backend-config=<file>' to select one.",
                        count
                    );
                }
                std::cmp::Ordering::Less => {}
            }
        }

        args.extend_from_slice(passthrough_args);
        self.init(dir, &args)?;
        Ok(true)
    }

    pub(crate) fn apply_captured_with_log(
        &self,
        dir: &Path,
        auto_approve: bool,
        passthrough_args: &[String],
        log_path: &Path,
    ) -> Result<()> {
        let mut args = vec!["apply".to_string()];
        if auto_approve {
            args.push("-auto-approve".to_string());
        }
        args.extend_from_slice(passthrough_args);
        ensure_default_parallelism(&mut args);
        self.run_captured_with_log(dir, &args, Some(log_path))
    }

    pub(crate) fn plan_with_log(
        &self,
        dir: &Path,
        passthrough_args: &[String],
        log_path: &Path,
    ) -> Result<()> {
        let mut args = vec!["plan".to_string()];
        args.extend_from_slice(passthrough_args);
        ensure_default_parallelism(&mut args);
        self.run_captured_with_log(dir, &args, Some(log_path))
    }

    pub(crate) fn fmt(&self, dir: &Path) -> Result<()> {
        self.run_captured(dir, &["fmt".to_string()])
    }

    pub(crate) fn validate(&self, dir: &Path) -> Result<()> {
        self.run_captured(dir, &["validate".to_string()])
    }

    pub(crate) fn destroy_captured(
        &self,
        dir: &Path,
        auto_approve: bool,
        passthrough_args: &[String],
    ) -> Result<()> {
        let mut args = vec!["destroy".to_string()];
        if auto_approve {
            args.push("-auto-approve".to_string());
        }
        args.extend_from_slice(passthrough_args);
        ensure_default_parallelism(&mut args);
        self.run_captured(dir, &args)
    }

    pub(crate) fn output_json(&self, dir: &Path) -> Result<Value> {
        self.output_json_internal(dir, &["output", "-json"])
    }

    pub(crate) fn output_named_json(&self, dir: &Path, output_name: &str) -> Result<Value> {
        let output = Command::new(&self.binary_path)
            .args(["output", "-json", output_name])
            .current_dir(dir)
            .envs(self.extra_env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::inherit())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|source| CliError::CommandSpawn {
                command: "terraform".to_string(),
                source,
            })?;

        if output.status.success() {
            let parsed = serde_json::from_slice::<Value>(&output.stdout)?;
            return Ok(parsed);
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains(&format!("Output \"{}\" not found", output_name)) {
            return Err(CliError::TerraformOutputMissing {
                output: output_name.to_string(),
            });
        }

        if let Some(code) = output.status.code() {
            return Err(CliError::TerraformFailed { code });
        }

        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            Err(CliError::TerraformSignaled {
                signal: output.status.signal(),
            })
        }

        #[cfg(not(unix))]
        {
            Err(CliError::TerraformSignaled { signal: None })
        }
    }

    fn output_json_internal(&self, dir: &Path, args: &[&str]) -> Result<Value> {
        let output = Command::new(&self.binary_path)
            .args(args)
            .current_dir(dir)
            .envs(self.extra_env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::inherit())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|source| CliError::CommandSpawn {
                command: "terraform".to_string(),
                source,
            })?;

        map_status(output.status)?;
        let parsed = serde_json::from_slice::<Value>(&output.stdout)?;
        Ok(parsed)
    }

    fn run_captured(&self, dir: &Path, args: &[String]) -> Result<()> {
        self.run_captured_with_log(dir, args, None)
    }

    fn run_captured_with_log(
        &self,
        dir: &Path,
        args: &[String],
        log_path: Option<&Path>,
    ) -> Result<()> {
        let output = Command::new(&self.binary_path)
            .args(args)
            .current_dir(dir)
            .envs(self.extra_env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::inherit())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|source| CliError::CommandSpawn {
                command: "terraform".to_string(),
                source,
            })?;

        if let Some(path) = log_path {
            append_terraform_log(path, args, &output.stdout, &output.stderr)?;
        }

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let excerpt = if !stderr.trim().is_empty() {
            stderr.trim()
        } else {
            stdout.trim()
        };

        if !excerpt.is_empty() {
            eprintln!("{}", excerpt);
        }

        map_status(output.status)
    }
}

fn ensure_default_parallelism(args: &mut Vec<String>) {
    let has_parallelism = args.iter().any(|arg| {
        arg == "-parallelism"
            || arg == "--parallelism"
            || arg.starts_with("-parallelism=")
            || arg.starts_with("--parallelism=")
    });

    if !has_parallelism {
        args.push(DEFAULT_TERRAFORM_PARALLELISM.to_string());
    }
}

fn append_terraform_log(path: &Path, args: &[String], stdout: &[u8], stderr: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::Io {
            source,
            path: parent.to_path_buf(),
        })?;
    }

    let mut payload = String::new();
    payload.push_str("\n=== terraform ");
    payload.push_str(&args.join(" "));
    payload.push_str(" ===\n");

    if !stdout.is_empty() {
        payload.push_str("--- stdout ---\n");
        payload.push_str(&String::from_utf8_lossy(stdout));
        if !payload.ends_with('\n') {
            payload.push('\n');
        }
    }

    if !stderr.is_empty() {
        payload.push_str("--- stderr ---\n");
        payload.push_str(&String::from_utf8_lossy(stderr));
        if !payload.ends_with('\n') {
            payload.push('\n');
        }
    }

    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| CliError::Io {
            source,
            path: path.to_path_buf(),
        })?;
    file.write_all(payload.as_bytes())
        .map_err(|source| CliError::Io {
            source,
            path: path.to_path_buf(),
        })?;

    Ok(())
}

pub(crate) fn map_status(status: ExitStatus) -> Result<()> {
    crate::error::map_exit_status(
        status,
        |code| CliError::TerraformFailed { code },
        |signal| CliError::TerraformSignaled { signal },
    )
}

fn probe_terraform_version(binary_path: &Path) -> Result<String> {
    let json_attempt = Command::new(binary_path)
        .args(["version", "-json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|source| CliError::CommandSpawn {
            command: "terraform".to_string(),
            source,
        })?;

    if json_attempt.status.success() {
        let parsed: TerraformVersionJson =
            serde_json::from_slice(&json_attempt.stdout).map_err(|err| {
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
        .map_err(|source| CliError::CommandSpawn {
            command: "terraform".to_string(),
            source,
        })?;

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

    parse_required_version_floor(&contents).map_err(|details| {
        CliError::TerraformVersionProbeFailed {
            details: format!("{} ({})", details, versions_path.display()),
        }
    })
}

fn parse_required_version_floor(
    contents: &str,
) -> std::result::Result<Option<VersionFloor>, String> {
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
        let second_quote = rest
            .find('"')
            .ok_or_else(|| "`required_version` found but closing quote is missing".to_string())?;

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

#[cfg(test)]
mod tests {
    use super::*;

    fn tfbackend_test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("evm-cloud-tf-test")
            .join(name)
            .join(format!("{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn find_tfbackend_none_when_empty_dir() {
        let dir = tfbackend_test_dir("empty");
        assert!(find_tfbackend(&dir).is_none());
    }

    #[test]
    fn find_tfbackend_returns_single_file() {
        let dir = tfbackend_test_dir("single");
        std::fs::write(dir.join("project.s3.tfbackend"), "bucket = \"b\"\n").unwrap();
        let result = find_tfbackend(&dir);
        assert!(result.is_some());
        assert!(result.unwrap().ends_with("project.s3.tfbackend"));
    }

    #[test]
    fn find_tfbackend_none_when_multiple_files() {
        let dir = tfbackend_test_dir("multiple");
        std::fs::write(dir.join("dev.s3.tfbackend"), "bucket = \"dev\"\n").unwrap();
        std::fs::write(dir.join("prod.s3.tfbackend"), "bucket = \"prod\"\n").unwrap();
        assert!(find_tfbackend(&dir).is_none());
    }

    #[test]
    fn find_tfbackend_ignores_non_tfbackend_files() {
        let dir = tfbackend_test_dir("non-tfbackend");
        std::fs::write(dir.join("main.tf"), "terraform {}\n").unwrap();
        std::fs::write(dir.join("project.s3.tfbackend"), "bucket = \"b\"\n").unwrap();
        let result = find_tfbackend(&dir);
        assert!(result.is_some());
    }
}
