use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use base64::Engine;
use clap::Args;

use crate::error::{CliError, Result};
use crate::handoff::{self, WorkloadHandoff};
use crate::preflight::{self, ProjectKind};
use crate::terraform::TerraformRunner;

#[derive(Args)]
pub(crate) struct KubectlArgs {
    #[arg(short, long, default_value = ".")]
    dir: PathBuf,
    #[arg(long)]
    kubeconfig: Option<PathBuf>,
    #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
    kubectl_args: Vec<String>,
}

pub(crate) fn run(args: KubectlArgs) -> Result<()> {
    let args = normalize_embedded_wrapper_flags(args)?;

    if args.kubectl_args.is_empty() {
        return Err(CliError::Message(
            "missing kubectl arguments, e.g. `evm-cloud kubectl -- get nodes`".to_string(),
        ));
    }

    let kubectl_binary = which::which("kubectl").map_err(|_| CliError::PrerequisiteNotFound {
        tool: "kubectl".to_string(),
    })?;

    let canonical_dir = fs::canonicalize(&args.dir).map_err(|source| CliError::Io {
        source,
        path: args.dir.clone(),
    })?;

    let kubeconfig_path = resolve_or_generate_kubeconfig_path(&canonical_dir, args.kubeconfig)?;

    let status = Command::new(kubectl_binary)
        .args(&args.kubectl_args)
        .current_dir(&canonical_dir)
        .env("KUBECONFIG", &kubeconfig_path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|err| CliError::ToolFailed {
            tool: "kubectl".to_string(),
            details: err.to_string(),
        })?;

    if status.success() {
        return Ok(());
    }

    if let Some(code) = status.code() {
        return Err(CliError::ToolFailed {
            tool: "kubectl".to_string(),
            details: format!("exited with status code {code}"),
        });
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        return Err(CliError::ToolFailed {
            tool: "kubectl".to_string(),
            details: format!("terminated by signal {:?}", status.signal()),
        });
    }

    #[cfg(not(unix))]
    {
        Err(CliError::ToolFailed {
            tool: "kubectl".to_string(),
            details: "terminated unexpectedly".to_string(),
        })
    }
}

fn resolve_or_generate_kubeconfig_path(canonical_dir: &Path, explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        let target = absolutize_path(canonical_dir, path);
        if target.is_file() {
            return Ok(target);
        }

        generate_kubeconfig(canonical_dir, &target)?;
        return ensure_existing_kubeconfig(target);
    }

    let candidates = kubeconfig_candidates(canonical_dir);
    let preferred = candidates
        .first()
        .cloned()
        .unwrap_or_else(|| canonical_dir.join("kubeconfig.yaml"));

    generate_kubeconfig(canonical_dir, &preferred)?;
    ensure_existing_kubeconfig(preferred)
}

fn generate_kubeconfig(start_dir: &Path, target_path: &Path) -> Result<()> {
    let preflight = preflight::run_checks(start_dir, true)?;
    let project_root = preflight.resolved_root;
    let terraform_dir = match preflight.project_kind {
        ProjectKind::EasyToml => project_root.join(".evm-cloud"),
        ProjectKind::RawTerraform => project_root.clone(),
    };

    let runner = TerraformRunner::check_installed(&terraform_dir)?;
    let handoff = load_handoff(&runner, &terraform_dir)?;

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::Io {
            source,
            path: parent.to_path_buf(),
        })?;
    }

    match handoff.compute_engine.as_str() {
        "k3s" => {
            let encoded = handoff
                .runtime
                .k3s
                .as_ref()
                .and_then(|runtime| runtime.kubeconfig_base64.as_ref())
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| CliError::Message(
                    "workload_handoff.runtime.k3s.kubeconfig_base64 is missing; cannot generate kubeconfig"
                        .to_string(),
                ))?;

            let decoded = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|err| CliError::Message(format!("invalid base64 kubeconfig payload: {err}")))?;

            fs::write(target_path, decoded).map_err(|source| CliError::Io {
                source,
                path: target_path.to_path_buf(),
            })?;
            Ok(())
        }
        "eks" => generate_eks_kubeconfig(&handoff, &project_root, target_path),
        other => Err(CliError::Message(format!(
            "auto kubeconfig generation is only supported for k3s/eks; current compute_engine is `{other}`"
        ))),
    }
}

fn generate_eks_kubeconfig(handoff: &WorkloadHandoff, project_root: &Path, target_path: &Path) -> Result<()> {
    let cluster_name = handoff
        .runtime
        .eks
        .as_ref()
        .and_then(|runtime| runtime.cluster_name.as_ref())
        .map(|name| name.trim())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| CliError::Message(
            "workload_handoff.runtime.eks.cluster_name is missing; cannot generate kubeconfig"
                .to_string(),
        ))?;

    let aws = which::which("aws").map_err(|_| CliError::PrerequisiteNotFound {
        tool: "aws".to_string(),
    })?;

    let mut command = Command::new(aws);
    command
        .args(["eks", "update-kubeconfig", "--name", cluster_name, "--kubeconfig"])
        .arg(target_path)
        .current_dir(project_root)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    if let Some(region) = handoff
        .extra
        .get("aws_region")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        command.args(["--region", region]);
    }

    let status = command.status().map_err(|err| CliError::ToolFailed {
        tool: "aws eks update-kubeconfig".to_string(),
        details: err.to_string(),
    })?;

    if status.success() {
        return Ok(());
    }

    if let Some(code) = status.code() {
        return Err(CliError::ToolFailed {
            tool: "aws eks update-kubeconfig".to_string(),
            details: format!("exited with status code {code}"),
        });
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        return Err(CliError::ToolFailed {
            tool: "aws eks update-kubeconfig".to_string(),
            details: format!("terminated by signal {:?}", status.signal()),
        });
    }

    #[cfg(not(unix))]
    {
        Err(CliError::ToolFailed {
            tool: "aws eks update-kubeconfig".to_string(),
            details: "terminated unexpectedly".to_string(),
        })
    }
}

fn load_handoff(runner: &TerraformRunner, terraform_dir: &Path) -> Result<WorkloadHandoff> {
    match runner.output_named_json(terraform_dir, "workload_handoff") {
        Ok(value) => handoff::parse_handoff_value(value),
        Err(CliError::TerraformOutputMissing { .. }) => {
            let full_output = runner.output_json(terraform_dir)?;
            handoff::parse_from_full_output(full_output, "evm_cloud")
        }
        Err(err) => Err(err),
    }
}

fn absolutize_path(base_dir: &Path, candidate: PathBuf) -> PathBuf {
    if candidate.is_absolute() {
        candidate
    } else {
        base_dir.join(candidate)
    }
}

fn normalize_embedded_wrapper_flags(mut args: KubectlArgs) -> Result<KubectlArgs> {
    let mut passthrough = Vec::with_capacity(args.kubectl_args.len());
    let mut index = 0usize;

    while index < args.kubectl_args.len() {
        let current = &args.kubectl_args[index];

        if current == "--dir" {
            let value = args
                .kubectl_args
                .get(index + 1)
                .ok_or_else(|| CliError::Message("`--dir` requires a value".to_string()))?;

            if args.dir != PathBuf::from(".") {
                return Err(CliError::Message(
                    "duplicate `--dir` provided (use it only once)".to_string(),
                ));
            }

            args.dir = PathBuf::from(value);
            index += 2;
            continue;
        }

        if let Some(value) = current.strip_prefix("--dir=") {
            if args.dir != PathBuf::from(".") {
                return Err(CliError::Message(
                    "duplicate `--dir` provided (use it only once)".to_string(),
                ));
            }

            args.dir = PathBuf::from(value);
            index += 1;
            continue;
        }

        if current == "--kubeconfig" {
            let value = args
                .kubectl_args
                .get(index + 1)
                .ok_or_else(|| CliError::Message("`--kubeconfig` requires a value".to_string()))?;

            if args.kubeconfig.is_some() {
                return Err(CliError::Message(
                    "duplicate `--kubeconfig` provided (use it only once)".to_string(),
                ));
            }

            args.kubeconfig = Some(PathBuf::from(value));
            index += 2;
            continue;
        }

        if let Some(value) = current.strip_prefix("--kubeconfig=") {
            if args.kubeconfig.is_some() {
                return Err(CliError::Message(
                    "duplicate `--kubeconfig` provided (use it only once)".to_string(),
                ));
            }

            args.kubeconfig = Some(PathBuf::from(value));
            index += 1;
            continue;
        }

        passthrough.push(current.clone());
        index += 1;
    }

    args.kubectl_args = passthrough;
    Ok(args)
}

fn ensure_existing_kubeconfig(path: PathBuf) -> Result<PathBuf> {
    if path.is_file() {
        return Ok(path);
    }

    Err(CliError::Message(format!(
        "kubeconfig not found at {}",
        path.display()
    )))
}

fn kubeconfig_candidates(dir: &Path) -> Vec<PathBuf> {
    let is_evm_cloud_workdir = dir.file_name().and_then(|name| name.to_str()) == Some(".evm-cloud");

    if is_evm_cloud_workdir {
        if let Some(parent) = dir.parent() {
            return vec![
                parent.join("kubeconfig.yaml"),
                dir.join("kubeconfig.yaml"),
                parent.join(".evm-cloud").join("kubeconfig.yaml"),
            ];
        }

        return vec![dir.join("kubeconfig.yaml")];
    }

    vec![
        dir.join("kubeconfig.yaml"),
        dir.join(".evm-cloud").join("kubeconfig.yaml"),
    ]
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::{absolutize_path, kubeconfig_candidates, normalize_embedded_wrapper_flags, KubectlArgs};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let base = std::env::temp_dir().join(format!(
            "evm-cloud-kubectl-tests-{}-{}-{}",
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
    fn prefers_parent_kubeconfig_from_dot_evm_cloud_dir() {
        let root = temp_dir("dot-evm-cloud");
        let terraform_dir = root.join(".evm-cloud");
        fs::create_dir_all(&terraform_dir).expect("create terraform dir");

        let candidates = kubeconfig_candidates(&terraform_dir);

        assert_eq!(candidates.first(), Some(&root.join("kubeconfig.yaml")));
    }

    #[test]
    fn includes_project_and_generated_candidates_from_project_root() {
        let root = temp_dir("project-root");
        let candidates = kubeconfig_candidates(&root);

        assert_eq!(candidates.first(), Some(&root.join("kubeconfig.yaml")));
        assert_eq!(
            candidates.get(1),
            Some(&root.join(".evm-cloud").join("kubeconfig.yaml"))
        );
    }

    #[test]
    fn extracts_dir_and_kubeconfig_from_embedded_flags() {
        let args = KubectlArgs {
            dir: ".".into(),
            kubeconfig: None,
            kubectl_args: vec![
                "get".to_string(),
                "pods".to_string(),
                "--dir".to_string(),
                "sandbox/alpha-1".to_string(),
                "--kubeconfig=custom.yaml".to_string(),
            ],
        };

        let normalized = normalize_embedded_wrapper_flags(args).expect("must normalize args");

        assert_eq!(normalized.dir, std::path::PathBuf::from("sandbox/alpha-1"));
        assert_eq!(normalized.kubeconfig, Some(std::path::PathBuf::from("custom.yaml")));
        assert_eq!(normalized.kubectl_args, vec!["get".to_string(), "pods".to_string()]);
    }

    #[test]
    fn resolves_relative_explicit_path_against_selected_dir() {
        let root = temp_dir("absolute-path");
        let resolved = absolutize_path(&root, PathBuf::from("kubeconfig.yaml"));
        assert_eq!(resolved, root.join("kubeconfig.yaml"));
    }
}
