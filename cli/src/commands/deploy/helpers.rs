use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{fs, time};

use crate::config::schema::ComputeEngine;
use crate::deployer::{self, Action, InvokeOptions};
use crate::error::{CliError, Result};
use crate::handoff;
use crate::output::ColorMode;
use crate::preflight::ProjectKind;

pub(super) fn invoke_with_optional_timeout(
    handoff: &handoff::WorkloadHandoff,
    deployer_args: &[String],
    timeout_secs: Option<u64>,
    json: bool,
    color: ColorMode,
) -> Result<()> {
    let effective_color = if json { ColorMode::Never } else { color };

    match timeout_secs {
        Some(secs) => {
            use std::sync::{atomic::AtomicU32, Arc};

            let child_pid = Arc::new(AtomicU32::new(0));
            let pid_for_timeout = Arc::clone(&child_pid);

            let (tx, rx) = std::sync::mpsc::channel();
            let handoff_clone = handoff.clone();
            let args_clone = deployer_args.to_vec();
            std::thread::spawn(move || {
                let result = deployer::invoke_deployer(
                    &handoff_clone,
                    Action::Deploy,
                    InvokeOptions {
                        passthrough_args: &args_clone,
                        quiet_output: true,
                        color: effective_color,
                        compute_engine: handoff_clone.compute_engine,
                        child_pid: Some(child_pid),
                    },
                );
                let _ = tx.send(result);
            });

            match rx.recv_timeout(time::Duration::from_secs(secs)) {
                Ok(result) => result,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    kill_child(pid_for_timeout.load(std::sync::atomic::Ordering::Relaxed));
                    Err(CliError::DeployerTimedOut { seconds: secs })
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    Err(CliError::DeployerThreadPanicked)
                }
            }
        }
        None => deployer::invoke_deployer(
            handoff,
            Action::Deploy,
            InvokeOptions {
                passthrough_args: deployer_args,
                quiet_output: true,
                color: effective_color,
                compute_engine: handoff.compute_engine,
                child_pid: None,
            },
        ),
    }
}

/// Best-effort kill of a child process on timeout.
#[cfg(unix)]
fn kill_child(pid: u32) {
    if pid != 0 {
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGTERM);
        }
    }
}

#[cfg(not(unix))]
fn kill_child(_pid: u32) {
    // On non-Unix, we can't portably kill the child.
}

pub(super) fn backfill_inline_clickhouse_password(
    handoff: &mut handoff::WorkloadHandoff,
    project_root: &Path,
    project_kind: &ProjectKind,
    env_ctx: Option<&crate::env::EnvContext>,
) -> Result<()> {
    if handoff.compute_engine != ComputeEngine::K3s {
        return Ok(());
    }

    let secrets_mode = handoff
        .secrets
        .mode
        .as_deref()
        .map(str::trim)
        .unwrap_or("inline");
    if secrets_mode != "inline" {
        return Ok(());
    }

    let backend = handoff.data.backend.as_deref().map(str::trim).unwrap_or("");
    if backend != "clickhouse" {
        return Ok(());
    }

    let has_password = handoff
        .data
        .clickhouse
        .as_ref()
        .and_then(|ch| ch.password.as_ref())
        .map(|password| !password.trim().is_empty())
        .unwrap_or(false);
    if has_password {
        return Ok(());
    }

    // Check env-specific paths first, then fall back to project root.
    let mut candidates = match project_kind {
        ProjectKind::EasyToml => vec![
            project_root.join(".evm-cloud").join("secrets.auto.tfvars"),
            project_root.join("secrets.auto.tfvars"),
        ],
        ProjectKind::RawTerraform => vec![project_root.join("secrets.auto.tfvars")],
    };
    if let Some(ctx) = env_ctx {
        for path in ctx.auto_tfvars.iter().rev() {
            candidates.insert(0, path.clone());
        }
    }

    let secrets_path = match candidates.iter().find(|p| p.is_file()) {
        Some(p) => p,
        None => return Ok(()),
    };

    let Some(password) =
        crate::tfvars_parser::lookup(secrets_path, "indexer_clickhouse_password")?
    else {
        return Ok(());
    };

    let ch = handoff.data.clickhouse.get_or_insert_with(Default::default);
    ch.password = Some(password);
    Ok(())
}

pub(super) struct SshVars {
    pub key_path: Option<String>,
    pub user: Option<String>,
    pub port: Option<String>,
}

pub(super) fn resolve_ssh_vars_from_tfvars(
    project_root: &Path,
    project_kind: &ProjectKind,
    env_ctx: Option<&crate::env::EnvContext>,
) -> Result<SshVars> {
    let mut candidates = match project_kind {
        ProjectKind::EasyToml => vec![
            project_root.join(".evm-cloud").join("secrets.auto.tfvars"),
            project_root.join("secrets.auto.tfvars"),
            project_root.join(".evm-cloud").join("terraform.tfvars"),
        ],
        ProjectKind::RawTerraform => vec![
            project_root.join("secrets.auto.tfvars"),
            project_root.join("terraform.tfvars"),
        ],
    };
    // Prepend env-specific auto.tfvars so they take precedence.
    if let Some(ctx) = env_ctx {
        for path in ctx.auto_tfvars.iter().rev() {
            candidates.insert(0, path.clone());
        }
    }

    let vars = crate::tfvars_parser::parse_all_existing(&candidates)?;

    let key_path = vars.get("ssh_private_key_path").cloned();

    Ok(SshVars {
        key_path,
        user: vars.get("bare_metal_ssh_user").cloned(),
        port: vars.get("bare_metal_ssh_port").cloned(),
    })
}

pub(super) fn has_flag_with_value(args: &[String], flag: &str) -> bool {
    args.iter().enumerate().any(|(index, arg)| {
        if arg == flag {
            return args.get(index + 1).is_some();
        }
        arg.starts_with(&format!("{flag}="))
    })
}

pub(super) fn ensure_config_dir(project_root: &Path) -> Result<PathBuf> {
    let explicit = project_root.join("config");
    if config_dir_ready(&explicit) {
        return Ok(explicit);
    }

    if config_dir_ready(project_root) {
        return Ok(project_root.to_path_buf());
    }

    let generated = project_root.join(".evm-cloud").join("config-bundle");
    fs::create_dir_all(generated.join("abis")).map_err(|source| CliError::Io {
        source,
        path: generated.join("abis"),
    })?;

    copy_required_with_fallback(project_root, &generated, "erpc.yaml")?;
    copy_required_with_fallback(project_root, &generated, "rindexer.yaml")?;

    Ok(generated)
}

fn config_dir_ready(path: &Path) -> bool {
    path.join("erpc.yaml").is_file()
        && path.join("rindexer.yaml").is_file()
        && path.join("abis").is_dir()
}

fn copy_required_with_fallback(
    source_root: &Path,
    destination_root: &Path,
    file: &str,
) -> Result<()> {
    let source = [
        source_root.join("config").join(file),
        source_root.join(file),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file());

    let Some(source) = source else {
        return Err(CliError::DeployConfigFileMissing {
            file: file.to_string(),
        });
    };

    let destination = destination_root.join(file);
    fs::copy(&source, &destination).map_err(|source_err| CliError::Io {
        source: source_err,
        path: destination,
    })?;

    Ok(())
}

pub(super) fn generate_env_file(
    config_dir: &Path,
    project_root: &Path,
    project_kind: &ProjectKind,
    handoff: &handoff::WorkloadHandoff,
    env_ctx: Option<&crate::env::EnvContext>,
) -> Result<()> {
    let mut candidates = match project_kind {
        ProjectKind::EasyToml => vec![
            project_root.join(".evm-cloud").join("secrets.auto.tfvars"),
            project_root.join("secrets.auto.tfvars"),
        ],
        ProjectKind::RawTerraform => vec![project_root.join("secrets.auto.tfvars")],
    };
    // Prepend env-specific auto.tfvars so they take precedence.
    if let Some(ctx) = env_ctx {
        for path in ctx.auto_tfvars.iter().rev() {
            candidates.insert(0, path.clone());
        }
    }

    let tfvars = crate::tfvars_parser::parse_first_existing(&candidates)?;
    build_and_write_env(config_dir, &tfvars, handoff)
}

fn build_and_write_env(
    config_dir: &Path,
    tfvars: &HashMap<String, String>,
    handoff: &handoff::WorkloadHandoff,
) -> Result<()> {
    let backend = handoff.data.backend.as_deref().unwrap_or("");

    let mut env_lines: Vec<String> = Vec::new();

    if handoff.services.rpc_proxy.is_some() {
        env_lines.push("RPC_URL=http://erpc:4000".to_string());
    } else if let Some(url) = tfvars.get("indexer_rpc_url") {
        env_lines.push(format!("RPC_URL={url}"));
    }

    if backend == "postgres" || tfvars.contains_key("indexer_postgres_url") {
        if let Some(url) = tfvars.get("indexer_postgres_url") {
            env_lines.push(format!("DATABASE_URL={url}"));
        } else if let Some(pg) = &handoff.data.postgres {
            // Managed RDS: URL is constructed by Terraform and included in handoff
            if let Some(url) = &pg.url {
                env_lines.push(format!("DATABASE_URL={url}"));
            }
        }
    }

    if backend == "clickhouse" || tfvars.contains_key("indexer_clickhouse_url") {
        if let Some(url) = tfvars.get("indexer_clickhouse_url") {
            env_lines.push(format!("CLICKHOUSE_URL={url}"));
        }
        let user = tfvars
            .get("indexer_clickhouse_user")
            .map(|s| s.as_str())
            .unwrap_or("default");
        env_lines.push(format!("CLICKHOUSE_USER={user}"));
        if let Some(password) = tfvars.get("indexer_clickhouse_password") {
            env_lines.push(format!("CLICKHOUSE_PASSWORD={password}"));
        }
        let db = tfvars
            .get("indexer_clickhouse_db")
            .map(|s| s.as_str())
            .unwrap_or("rindexer");
        env_lines.push(format!("CLICKHOUSE_DB={db}"));
    }

    if env_lines.is_empty() {
        return Ok(());
    }

    let env_path = config_dir.join(".env");
    let content = format!("{}\n", env_lines.join("\n"));

    // Write with restricted permissions from the start — .env contains secrets
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&env_path)
            .map_err(|source| CliError::Io {
                source,
                path: env_path.clone(),
            })?;
        f.write_all(content.as_bytes())
            .map_err(|source| CliError::Io {
                source,
                path: env_path,
            })?;
    }
    #[cfg(not(unix))]
    {
        fs::write(&env_path, &content).map_err(|source| CliError::Io {
            source,
            path: env_path,
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // -----------------------------------------------------------------------
    // has_flag_with_value
    // -----------------------------------------------------------------------

    #[test]
    fn flag_with_separate_value() {
        let args: Vec<String> = vec!["--config-dir".into(), "/tmp/cfg".into()];
        assert!(has_flag_with_value(&args, "--config-dir"));
    }

    #[test]
    fn flag_with_equals_value() {
        let args: Vec<String> = vec!["--config-dir=/tmp/cfg".into()];
        assert!(has_flag_with_value(&args, "--config-dir"));
    }

    #[test]
    fn flag_without_value_at_end() {
        let args: Vec<String> = vec!["--config-dir".into()];
        assert!(!has_flag_with_value(&args, "--config-dir"));
    }

    #[test]
    fn flag_absent() {
        let args: Vec<String> = vec!["--other".into(), "val".into()];
        assert!(!has_flag_with_value(&args, "--config-dir"));
    }

    // -----------------------------------------------------------------------
    // build_and_write_env
    // -----------------------------------------------------------------------

    fn minimal_handoff() -> handoff::WorkloadHandoff {
        serde_json::from_value(serde_json::json!({
            "version": "1",
            "mode": "easy",
            "compute_engine": "ec2",
            "project_name": "test",
            "runtime": {},
            "services": {},
            "data": {},
            "secrets": {},
            "ingress": {}
        }))
        .unwrap()
    }

    #[test]
    fn env_file_includes_clickhouse_vars() {
        let dir = tempfile::tempdir().unwrap();
        let mut tfvars = HashMap::new();
        tfvars.insert("indexer_clickhouse_url".into(), "http://ch:8123".into());
        tfvars.insert("indexer_clickhouse_password".into(), "secret".into());

        let mut handoff = minimal_handoff();
        handoff.data.backend = Some("clickhouse".into());

        build_and_write_env(dir.path(), &tfvars, &handoff).unwrap();

        let content = fs::read_to_string(dir.path().join(".env")).unwrap();
        assert!(content.contains("CLICKHOUSE_URL=http://ch:8123"));
        assert!(content.contains("CLICKHOUSE_PASSWORD=secret"));
        assert!(content.contains("CLICKHOUSE_USER=default"));
        assert!(content.contains("CLICKHOUSE_DB=rindexer"));
    }

    #[test]
    fn env_file_includes_postgres_url() {
        let dir = tempfile::tempdir().unwrap();
        let mut tfvars = HashMap::new();
        tfvars.insert(
            "indexer_postgres_url".into(),
            "postgres://localhost/db".into(),
        );

        let mut handoff = minimal_handoff();
        handoff.data.backend = Some("postgres".into());

        build_and_write_env(dir.path(), &tfvars, &handoff).unwrap();

        let content = fs::read_to_string(dir.path().join(".env")).unwrap();
        assert!(content.contains("DATABASE_URL=postgres://localhost/db"));
    }

    #[test]
    fn env_file_adds_rpc_url_when_proxy_present() {
        let dir = tempfile::tempdir().unwrap();
        let mut tfvars = HashMap::new();
        tfvars.insert("indexer_clickhouse_url".into(), "http://ch:8123".into());

        let mut handoff = minimal_handoff();
        handoff.data.backend = Some("clickhouse".into());
        handoff.services.rpc_proxy = Some(handoff::RpcProxyService {
            internal_url: Some("http://erpc:4000".into()),
            extra: HashMap::new(),
        });

        build_and_write_env(dir.path(), &tfvars, &handoff).unwrap();

        let content = fs::read_to_string(dir.path().join(".env")).unwrap();
        assert!(content.contains("RPC_URL=http://erpc:4000"));
    }

    #[test]
    fn env_file_not_created_when_empty() {
        let dir = tempfile::tempdir().unwrap();
        let tfvars = HashMap::new();
        let handoff = minimal_handoff();

        build_and_write_env(dir.path(), &tfvars, &handoff).unwrap();

        assert!(!dir.path().join(".env").exists());
    }

    // -----------------------------------------------------------------------
    // resolve_ssh_vars_from_tfvars
    // -----------------------------------------------------------------------

    #[test]
    fn resolves_ssh_key_from_secrets_tfvars() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = dir.path().join(".evm-cloud").join("secrets.auto.tfvars");
        fs::create_dir_all(secrets.parent().unwrap()).unwrap();
        fs::write(
            &secrets,
            "ssh_private_key_path = \"/home/user/.ssh/id_rsa\"\n",
        )
        .unwrap();

        let vars = resolve_ssh_vars_from_tfvars(dir.path(), &ProjectKind::EasyToml, None).unwrap();
        assert_eq!(vars.key_path.as_deref(), Some("/home/user/.ssh/id_rsa"));
    }

    #[test]
    fn resolves_bare_metal_ssh_vars() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = dir.path().join("secrets.auto.tfvars");
        fs::write(
            &secrets,
            "ssh_private_key_path = \"/keys/bm\"\nbare_metal_ssh_user = \"deploy\"\nbare_metal_ssh_port = \"2222\"\n",
        )
        .unwrap();

        let vars = resolve_ssh_vars_from_tfvars(dir.path(), &ProjectKind::RawTerraform, None).unwrap();
        assert_eq!(vars.key_path.as_deref(), Some("/keys/bm"));
        assert_eq!(vars.user.as_deref(), Some("deploy"));
        assert_eq!(vars.port.as_deref(), Some("2222"));
    }

    // -----------------------------------------------------------------------
    // ensure_config_dir
    // -----------------------------------------------------------------------

    #[test]
    fn uses_explicit_config_dir_when_ready() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        fs::create_dir_all(config.join("abis")).unwrap();
        fs::write(config.join("erpc.yaml"), "").unwrap();
        fs::write(config.join("rindexer.yaml"), "").unwrap();

        let result = ensure_config_dir(dir.path()).unwrap();
        assert_eq!(result, config);
    }

    #[test]
    fn bundles_config_from_scattered_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("erpc.yaml"), "server:\n  port: 4000\n").unwrap();
        fs::write(dir.path().join("rindexer.yaml"), "name: test\n").unwrap();

        let result = ensure_config_dir(dir.path()).unwrap();
        let bundle = dir.path().join(".evm-cloud").join("config-bundle");
        assert_eq!(result, bundle);
        assert!(bundle.join("erpc.yaml").is_file());
        assert!(bundle.join("rindexer.yaml").is_file());
        assert!(bundle.join("abis").is_dir());
    }

    #[test]
    fn errors_when_required_config_files_missing() {
        let dir = tempfile::tempdir().unwrap();
        let result = ensure_config_dir(dir.path());
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("erpc.yaml") || err.contains("rindexer.yaml"));
    }

    // -----------------------------------------------------------------------
    // backfill_inline_clickhouse_password
    // -----------------------------------------------------------------------

    #[test]
    fn backfills_clickhouse_password_from_tfvars() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = dir.path().join(".evm-cloud").join("secrets.auto.tfvars");
        fs::create_dir_all(secrets.parent().unwrap()).unwrap();
        fs::write(&secrets, "indexer_clickhouse_password = \"ch_secret\"\n").unwrap();

        let mut handoff = minimal_handoff();
        handoff.compute_engine = ComputeEngine::K3s;
        handoff.secrets.mode = Some("inline".into());
        handoff.data.backend = Some("clickhouse".into());

        backfill_inline_clickhouse_password(&mut handoff, dir.path(), &ProjectKind::EasyToml, None)
            .unwrap();

        assert_eq!(
            handoff.data.clickhouse.unwrap().password.as_deref(),
            Some("ch_secret")
        );
    }

    #[test]
    fn skips_backfill_when_password_already_present() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = dir.path().join(".evm-cloud").join("secrets.auto.tfvars");
        fs::create_dir_all(secrets.parent().unwrap()).unwrap();
        fs::write(&secrets, "indexer_clickhouse_password = \"overwrite_me\"\n").unwrap();

        let mut handoff = minimal_handoff();
        handoff.compute_engine = ComputeEngine::K3s;
        handoff.secrets.mode = Some("inline".into());
        handoff.data.backend = Some("clickhouse".into());
        handoff.data.clickhouse = Some(handoff::ClickhouseData {
            password: Some("existing".into()),
            ..Default::default()
        });

        backfill_inline_clickhouse_password(&mut handoff, dir.path(), &ProjectKind::EasyToml, None)
            .unwrap();

        assert_eq!(
            handoff.data.clickhouse.unwrap().password.as_deref(),
            Some("existing")
        );
    }

    #[test]
    fn skips_backfill_for_non_k3s_engine() {
        let dir = tempfile::tempdir().unwrap();
        let mut handoff = minimal_handoff();
        handoff.compute_engine = ComputeEngine::Ec2;
        handoff.data.backend = Some("clickhouse".into());

        backfill_inline_clickhouse_password(&mut handoff, dir.path(), &ProjectKind::EasyToml, None)
            .unwrap();

        assert!(handoff.data.clickhouse.is_none());
    }
}
