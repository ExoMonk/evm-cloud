//! CLI E2E integration tests.
//!
//! Spawns the `evm-cloud` binary against mock `terraform` and `kubectl` scripts.
//! Mock scripts log every invocation to a file so tests can assert on the exact
//! commands the CLI constructs — label selectors, namespaces, flags, service names.
//!
//! Platform: unix only (mocks are bash scripts).

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

// ===========================================================================
// Mock scripts
// ===========================================================================

/// Mock terraform: responds to version, init, validate, plan, apply, destroy,
/// fmt, output. Reads canned JSON from $MOCK_TF_STATE_DIR.
/// Logs every invocation to $MOCK_TF_STATE_DIR/terraform.log
const MOCK_TERRAFORM: &str = r#"#!/bin/bash
echo "terraform $@" >> "$MOCK_TF_STATE_DIR/terraform.log"
case "$1" in
  version)
    if [[ "$2" == "-json" ]]; then
      echo '{"terraform_version":"1.14.6","platform":"linux_amd64","provider_selections":{},"outdated":false}'
    else
      echo "Terraform v1.14.6"
    fi
    exit 0 ;;
  --version|-version)
    echo "Terraform v1.14.6"; exit 0 ;;
  init)     exit 0 ;;
  validate) echo '{"valid":true,"error_count":0,"warning_count":0,"diagnostics":[]}'; exit 0 ;;
  plan)     echo "Mock: terraform plan succeeded"; exit 0 ;;
  apply)    echo "Mock: terraform apply succeeded"; exit 0 ;;
  destroy)  echo "Mock: terraform destroy succeeded"; exit 0 ;;
  fmt)      exit 0 ;;
  output)
    if [[ "$2" == "-json" && -n "$3" ]]; then
      f="$MOCK_TF_STATE_DIR/output_${3}.json"
      if [[ -f "$f" ]]; then cat "$f"; exit 0
      else echo "Output \"${3}\" not found" >&2; exit 1; fi
    elif [[ "$2" == "-json" ]]; then
      f="$MOCK_TF_STATE_DIR/output_full.json"
      if [[ -f "$f" ]]; then cat "$f"; exit 0
      else echo '{}'; exit 0; fi
    fi
    exit 0 ;;
  *) echo "mock-terraform: unhandled: $@" >&2; exit 1 ;;
esac
"#;

/// Mock kubectl: logs every invocation to $MOCK_TF_STATE_DIR/kubectl.log
/// then exits 0. Tests read the log to verify the exact command constructed.
const MOCK_KUBECTL: &str = r#"#!/bin/bash
echo "kubectl $@" >> "$MOCK_TF_STATE_DIR/kubectl.log"
# Return empty output for logs commands
exit 0
"#;

/// Mock ssh: logs every invocation to $MOCK_TF_STATE_DIR/ssh.log
/// then exits 0. Tests read the log to verify the remote command.
const MOCK_SSH: &str = r#"#!/bin/bash
echo "ssh $@" >> "$MOCK_TF_STATE_DIR/ssh.log"
exit 0
"#;

/// Mock deployer: logs invocation and env, emits [evm-cloud] status lines.
/// Uses TMPDIR for log output (TMPDIR is in the sanitized env whitelist).
/// Exit code can be controlled via $TMPDIR/deployer_exit_code file.
const MOCK_DEPLOYER: &str = r#"#!/bin/bash
LOG_DIR="$TMPDIR"
echo "deployer $@" >> "$LOG_DIR/deployer.log"
env | sort >> "$LOG_DIR/deployer_env.log"

HANDOFF="$1"
if [[ ! -f "$HANDOFF" ]]; then
  echo "[evm-cloud] ERROR: handoff file not found" >&2
  exit 1
fi
# Validate handoff file is non-empty (production code already validates JSON)
if [[ ! -s "$HANDOFF" ]]; then
  echo "[evm-cloud] ERROR: handoff file is empty" >&2
  exit 1
fi
# Copy handoff content to a persistent location for test assertions
cp "$HANDOFF" "$LOG_DIR/deployer_handoff.json"

# Emit [evm-cloud] status lines for output streaming tests
echo "[evm-cloud] Cluster reachable."
echo "[evm-cloud] ESO is ready."
echo "[evm-cloud] Deploying eRPC (test-erpc)..."
echo "[evm-cloud] eRPC deployed."
echo "[evm-cloud] Deploying rindexer instance (test-indexer)..."
echo "[evm-cloud] test-indexer deployed."
echo "[evm-cloud] All workloads deployed successfully."

# Check for lock file and log its presence
if ls "$PROJECT_DIR/.evm-cloud-deploy"* 2>/dev/null; then
  echo "LOCK_EXISTS=true" >> "$LOG_DIR/deployer.log"
fi

# Configurable exit code via signal file
EXIT_FILE="$TMPDIR/deployer_exit_code"
if [[ -f "$EXIT_FILE" ]]; then
  exit "$(cat "$EXIT_FILE")"
fi
exit 0
"#;

// ===========================================================================
// Config fixtures
// ===========================================================================

const CONFIG_K3S: &str = r#"schema_version = 1

[project]
name = "test-project"

[compute]
engine = "k3s"

[database]
mode = "single"
provider = "bare_metal"

[indexer]
config_path = "config/rindexer.yaml"
chains = ["ethereum"]

[rpc]
endpoints = { ethereum = "https://rpc.example.com" }

[ingress]
mode = "none"

[secrets]
mode = "local"
"#;

const CONFIG_DOCKER_COMPOSE: &str = r#"schema_version = 1

[project]
name = "test-compose"

[compute]
engine = "docker_compose"

[database]
mode = "single"
provider = "bare_metal"

[indexer]
config_path = "config/rindexer.yaml"
chains = ["ethereum"]

[rpc]
endpoints = { ethereum = "https://rpc.example.com" }

[ingress]
mode = "none"

[secrets]
mode = "local"
"#;

const CONFIG_EC2: &str = r#"schema_version = 1

[project]
name = "test-ec2"

[compute]
engine = "ec2"

[database]
mode = "single"
provider = "aws"

[indexer]
config_path = "config/rindexer.yaml"
chains = ["ethereum"]

[rpc]
endpoints = { ethereum = "https://rpc.example.com" }

[ingress]
mode = "none"

[secrets]
mode = "local"
"#;

// ===========================================================================
// Handoff fixtures
// ===========================================================================

fn handoff_k3s() -> serde_json::Value {
    serde_json::json!({
        "version": "v1",
        "mode": "external",
        "compute_engine": "k3s",
        "project_name": "test-project",
        "runtime": {
            "k3s": {
                "kubeconfig_base64": "ZmFrZS1rdWJlY29uZmln",
                "host_ip": "10.0.0.1"
            }
        },
        "services": {
            "indexer": {
                "service_name": "test-project-indexer",
                "instances": [
                    {"name": "indexer", "config_key": "default"},
                    {"name": "backfill", "config_key": "backfill"}
                ]
            },
            "rpc_proxy": {"internal_url": "http://erpc:4000"},
            "custom_services": [
                {"name": "api", "image": "ghcr.io/test/api:v1", "port": 8080}
            ],
            "monitoring": {"grafana_hostname": "grafana.test"}
        },
        "data": {"backend": "clickhouse"},
        "secrets": {"mode": "inline"},
        "ingress": {"erpc_hostname": "rpc.test"}
    })
}

fn handoff_docker_compose() -> serde_json::Value {
    serde_json::json!({
        "version": "v1",
        "mode": "external",
        "compute_engine": "docker_compose",
        "project_name": "test-compose",
        "runtime": {
            "bare_metal": {
                "host_address": "10.0.0.2"
            }
        },
        "services": {
            "rpc_proxy": {"internal_url": "http://erpc:4000"},
            "custom_services": [
                {"name": "api", "image": "ghcr.io/test/api:v1", "port": 8080}
            ]
        },
        "data": {"backend": "clickhouse"},
        "secrets": {"mode": "inline"},
        "ingress": {}
    })
}

fn handoff_ec2() -> serde_json::Value {
    serde_json::json!({
        "version": "v1",
        "mode": "external",
        "compute_engine": "ec2",
        "project_name": "test-ec2",
        "runtime": {
            "ec2": {
                "public_ip": "54.123.45.67"
            }
        },
        "services": {
            "rpc_proxy": {"internal_url": "http://erpc:4000"}
        },
        "data": {"backend": "clickhouse"},
        "secrets": {"mode": "inline"},
        "ingress": {}
    })
}

// ===========================================================================
// Test environment
// ===========================================================================

struct TestEnv {
    _temp: TempDir,
    bin_dir: PathBuf,
    project_dir: PathBuf,
    state_dir: PathBuf,
    mock_deployer: bool,
}

impl TestEnv {
    fn new() -> Self {
        let temp = TempDir::new().expect("create tempdir");
        let bin_dir = temp.path().join("bin");
        let project_dir = temp.path().join("project");
        let state_dir = temp.path().join("state");

        fs::create_dir_all(&bin_dir).unwrap();
        fs::create_dir_all(&project_dir).unwrap();
        fs::create_dir_all(&state_dir).unwrap();

        // Write all mock scripts
        for (name, content) in [
            ("terraform", MOCK_TERRAFORM),
            ("kubectl", MOCK_KUBECTL),
            ("ssh", MOCK_SSH),
        ] {
            let path = bin_dir.join(name);
            fs::write(&path, content).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        Self {
            _temp: temp,
            bin_dir,
            project_dir,
            state_dir,
            mock_deployer: false,
        }
    }

    fn with_mock_deployer(mut self) -> Self {
        let path = self.bin_dir.join("mock_deployer.sh");
        fs::write(&path, MOCK_DEPLOYER).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        self.mock_deployer = true;
        self
    }

    /// Create the config files the deploy command requires before invoking the
    /// deployer: config/erpc.yaml, config/rindexer.yaml, config/abis/.
    fn with_deploy_configs(self) -> Self {
        let config_dir = self.project_dir.join("config");
        fs::create_dir_all(config_dir.join("abis")).unwrap();
        fs::write(
            config_dir.join("erpc.yaml"),
            "server:\n  port: 4000\n",
        )
        .unwrap();
        // rindexer.yaml is already created by with_config, but ensure it exists
        if !config_dir.join("rindexer.yaml").exists() {
            fs::write(
                config_dir.join("rindexer.yaml"),
                "name: test\nnetworks: []\ncontracts: []\n",
            )
            .unwrap();
        }
        self
    }

    fn with_config(self, config: &str) -> Self {
        fs::write(self.project_dir.join("evm-cloud.toml"), config).unwrap();
        fs::create_dir_all(self.project_dir.join(".evm-cloud")).unwrap();
        fs::create_dir_all(self.project_dir.join("config")).unwrap();
        fs::write(
            self.project_dir.join("config/rindexer.yaml"),
            "name: test\nnetworks: []\ncontracts: []\n",
        )
        .unwrap();
        self
    }

    fn with_handoff(self, handoff: serde_json::Value) -> Self {
        let value = handoff.to_string();

        // terraform output -json workload_handoff
        fs::write(
            self.state_dir.join("output_workload_handoff.json"),
            &value,
        )
        .unwrap();

        // terraform output -json (full)
        let full = serde_json::json!({ "workload_handoff": { "value": handoff } });
        fs::write(self.state_dir.join("output_full.json"), full.to_string()).unwrap();

        self
    }

    fn cmd(&self) -> Command {
        let original_path = std::env::var("PATH").unwrap_or_default();
        let mock_path = format!("{}:{}", self.bin_dir.display(), original_path);

        let mut cmd = Command::cargo_bin("evm-cloud").expect("binary not found");
        cmd.env("PATH", mock_path);
        cmd.env("MOCK_TF_STATE_DIR", &self.state_dir);
        cmd.current_dir(&self.project_dir);

        if self.mock_deployer {
            cmd.env(
                "EVM_CLOUD_DEPLOYER_OVERRIDE",
                self.bin_dir.join("mock_deployer.sh"),
            );
            // TMPDIR is in the sanitized env whitelist — the mock deployer
            // writes logs here so we can inspect them after the run.
            cmd.env("TMPDIR", &self.state_dir);
        }

        cmd
    }

    /// Read the kubectl invocation log. Each line is a full command.
    fn kubectl_log(&self) -> Vec<String> {
        let path = self.state_dir.join("kubectl.log");
        if !path.exists() {
            return vec![];
        }
        fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|l| l.to_string())
            .collect()
    }

    /// Read the ssh invocation log.
    #[allow(dead_code)]
    fn ssh_log(&self) -> Vec<String> {
        let path = self.state_dir.join("ssh.log");
        if !path.exists() {
            return vec![];
        }
        fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|l| l.to_string())
            .collect()
    }

    /// Read the terraform invocation log.
    fn terraform_log(&self) -> Vec<String> {
        let path = self.state_dir.join("terraform.log");
        if !path.exists() {
            return vec![];
        }
        fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|l| l.to_string())
            .collect()
    }

    /// Read the mock deployer invocation log (written to TMPDIR/deployer.log).
    fn deployer_log(&self) -> Vec<String> {
        let path = self.state_dir.join("deployer.log");
        if !path.exists() {
            return vec![];
        }
        fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|l| l.to_string())
            .collect()
    }

    /// Read the mock deployer env log (written to TMPDIR/deployer_env.log).
    fn deployer_env_log(&self) -> String {
        let path = self.state_dir.join("deployer_env.log");
        if !path.exists() {
            return String::new();
        }
        fs::read_to_string(&path).unwrap()
    }
}

// ===========================================================================
// Tier 1: CLI bootstrap — binary boots, arg parsing works
// ===========================================================================

#[test]
fn t1_help_output() {
    TestEnv::new()
        .cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Deploy EVM blockchain"));
}

#[test]
fn t1_version_output() {
    TestEnv::new()
        .cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn t1_templates_list_shows_registry() {
    TestEnv::new()
        .cmd()
        .args(["templates", "list"])
        .assert()
        .success()
        .stdout(
            // Verify actual template names from the registry
            predicate::str::contains("uniswap-v4")
                .or(predicate::str::contains("aave"))
                .or(predicate::str::contains("erc20")),
        );
}

#[test]
fn t1_logs_help_shows_flags() {
    TestEnv::new()
        .cmd()
        .args(["logs", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("--follow")
                .and(predicate::str::contains("--tail"))
                .and(predicate::str::contains("--list")),
        );
}

#[test]
fn t1_unknown_command_fails() {
    TestEnv::new()
        .cmd()
        .arg("nonexistent-command")
        .assert()
        .failure();
}

// ===========================================================================
// Tier 2: Service discovery — verify output content & formatting
// ===========================================================================

#[test]
fn t2_logs_list_discovers_all_services_from_handoff() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    env.cmd()
        .args(["logs", "--list"])
        .assert()
        .success()
        .stdout(
            // All services from handoff: 2 indexer instances + erpc + custom + monitoring + static
            predicate::str::contains("indexer")
                .and(predicate::str::contains("backfill"))
                .and(predicate::str::contains("erpc"))
                .and(predicate::str::contains("api"))
                .and(predicate::str::contains("grafana"))
                .and(predicate::str::contains("prometheus"))
                .and(predicate::str::contains("clickhouse"))
                .and(predicate::str::contains("caddy")),
        );
}

#[test]
fn t2_logs_list_shows_correct_target_names() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    env.cmd()
        .args(["logs", "--list"])
        .assert()
        .success()
        .stdout(
            // Full Helm release names as targets
            predicate::str::contains("test-project-indexer")
                .and(predicate::str::contains("test-project-backfill"))
                .and(predicate::str::contains("test-project-erpc"))
                .and(predicate::str::contains("test-project-api")),
        );
}

#[test]
fn t2_logs_list_shows_engine_type() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    env.cmd()
        .args(["logs", "--list"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("k8s").and(predicate::str::contains("k3s")),
        );
}

#[test]
fn t2_logs_no_args_shows_service_table() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    env.cmd()
        .arg("logs")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("test-project")
                .and(predicate::str::contains("Service"))
                .and(predicate::str::contains("Target"))
                .and(predicate::str::contains("Engine"))
                .and(predicate::str::contains("Tip:")),
        );
}

#[test]
fn t2_logs_docker_compose_filters_k8s_only_services() {
    let env = TestEnv::new()
        .with_config(CONFIG_DOCKER_COMPOSE)
        .with_handoff(handoff_docker_compose());

    let output = env
        .cmd()
        .args(["logs", "--list"])
        .output()
        .expect("run logs --list");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Docker Compose should show compose-available services
    assert!(stdout.contains("erpc"), "should show erpc");
    assert!(stdout.contains("compose"), "should show compose engine");

    // Should NOT show custom services (K8s-only)
    assert!(
        !stdout.contains("test-compose-api"),
        "custom service 'api' should be filtered out on docker_compose"
    );
}

#[test]
fn t2_logs_unknown_service_lists_available() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    env.cmd()
        .args(["logs", "nonexistent-service"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("unknown service")
                .and(predicate::str::contains("indexer"))
                .and(predicate::str::contains("erpc"))
                .and(predicate::str::contains("api")),
        );
}

#[test]
fn t2_logs_no_config_shows_preflight_error() {
    TestEnv::new()
        .cmd()
        .args(["logs", "--list"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("evm-cloud.toml")
                .or(predicate::str::contains("main.tf"))
                .or(predicate::str::contains("project")),
        );
}

// ===========================================================================
// Tier 3: Command construction — verify the exact kubectl/SSH commands built
// ===========================================================================

#[test]
fn t3_logs_indexer_builds_correct_kubectl_command() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    let _ = env.cmd().args(["logs", "indexer"]).output().unwrap();

    let log = env.kubectl_log();
    assert!(!log.is_empty(), "kubectl should have been invoked");

    let cmd = &log[0];
    // Verify label selector uses app.kubernetes.io/instance (not name)
    assert!(
        cmd.contains("app.kubernetes.io/instance=test-project-indexer"),
        "should use instance label for indexer, got: {cmd}"
    );
    // Verify namespace
    assert!(
        cmd.contains("-n test-project"),
        "should use project namespace, got: {cmd}"
    );
    // Verify standard flags
    assert!(
        cmd.contains("--all-containers=true"),
        "should pass --all-containers, got: {cmd}"
    );
    assert!(
        cmd.contains("--prefix"),
        "should pass --prefix, got: {cmd}"
    );
    assert!(
        cmd.contains("--max-log-requests=20"),
        "should pass --max-log-requests, got: {cmd}"
    );
}

#[test]
fn t3_logs_custom_service_builds_correct_kubectl_command() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    let _ = env.cmd().args(["logs", "api"]).output().unwrap();

    let log = env.kubectl_log();
    assert!(!log.is_empty(), "kubectl should have been invoked");

    let cmd = &log[0];
    assert!(
        cmd.contains("app.kubernetes.io/instance=test-project-api"),
        "should use instance label for custom service, got: {cmd}"
    );
    assert!(
        cmd.contains("-n test-project"),
        "should use project namespace, got: {cmd}"
    );
}

#[test]
fn t3_logs_backfill_instance_builds_correct_kubectl_command() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    let _ = env.cmd().args(["logs", "backfill"]).output().unwrap();

    let log = env.kubectl_log();
    assert!(!log.is_empty(), "kubectl should have been invoked");

    let cmd = &log[0];
    assert!(
        cmd.contains("app.kubernetes.io/instance=test-project-backfill"),
        "should target backfill instance, got: {cmd}"
    );
}

#[test]
fn t3_logs_monitoring_uses_monitoring_namespace() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    let _ = env.cmd().args(["logs", "grafana"]).output().unwrap();

    let log = env.kubectl_log();
    assert!(!log.is_empty(), "kubectl should have been invoked");

    let cmd = &log[0];
    assert!(
        cmd.contains("-n monitoring"),
        "grafana should use monitoring namespace, got: {cmd}"
    );
    assert!(
        cmd.contains("app.kubernetes.io/name=grafana"),
        "should use name label for monitoring, got: {cmd}"
    );
}

#[test]
fn t3_logs_follow_flag_passes_through() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    let _ = env
        .cmd()
        .args(["logs", "indexer", "-f"])
        .output()
        .unwrap();

    let log = env.kubectl_log();
    assert!(!log.is_empty(), "kubectl should have been invoked");

    let cmd = &log[0];
    assert!(cmd.contains("-f"), "should pass -f flag, got: {cmd}");
}

#[test]
fn t3_logs_tail_flag_passes_through() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    let _ = env
        .cmd()
        .args(["logs", "erpc", "--tail", "50"])
        .output()
        .unwrap();

    let log = env.kubectl_log();
    assert!(!log.is_empty(), "kubectl should have been invoked");

    let cmd = &log[0];
    assert!(
        cmd.contains("--tail 50"),
        "should pass --tail 50, got: {cmd}"
    );
}

#[test]
fn t3_logs_full_name_resolves_same_as_short() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    let _ = env
        .cmd()
        .args(["logs", "test-project-api"])
        .output()
        .unwrap();

    let log = env.kubectl_log();
    assert!(!log.is_empty(), "kubectl should have been invoked");

    let cmd = &log[0];
    assert!(
        cmd.contains("app.kubernetes.io/instance=test-project-api"),
        "full name should resolve to same target, got: {cmd}"
    );
}

#[test]
fn t3_logs_custom_service_on_compose_rejects() {
    let env = TestEnv::new()
        .with_config(CONFIG_DOCKER_COMPOSE)
        .with_handoff(handoff_docker_compose());

    env.cmd()
        .args(["logs", "api"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("K3s/EKS"));
}

#[test]
fn t3_status_loads_handoff_and_shows_project() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    let output = env.cmd().arg("status").output().expect("run status");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Handoff loaded — project name visible
    assert!(
        combined.contains("test-project"),
        "should show project name from handoff, got: {combined}"
    );
    // No terraform errors
    assert!(
        !combined.contains("terraform not found"),
        "mock terraform should be found, got: {combined}"
    );

    // Terraform was invoked to read state
    let tf_log = env.terraform_log();
    assert!(
        tf_log.iter().any(|l| l.contains("output -json")),
        "terraform output should have been called, log: {tf_log:?}"
    );
}

#[test]
fn t3_status_without_state_fails_with_message() {
    let env = TestEnv::new().with_config(CONFIG_K3S);

    env.cmd()
        .arg("status")
        .assert()
        .failure()
        .stderr(predicate::str::is_empty().not());
}

#[test]
fn t3_deploy_dry_run_calls_terraform_plan() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    let _ = env
        .cmd()
        .args(["deploy", "--dry-run"])
        .output()
        .expect("run deploy --dry-run");

    // Verify terraform was called with plan (not apply)
    let tf_log = env.terraform_log();
    assert!(
        tf_log.iter().any(|l| l.contains("plan")),
        "deploy --dry-run should call terraform plan, log: {tf_log:?}"
    );
    assert!(
        !tf_log.iter().any(|l| l.contains("apply")),
        "deploy --dry-run should NOT call terraform apply, log: {tf_log:?}"
    );
}

// ===========================================================================
// Tier 4: Config validation — verify that invalid configs produce clear errors
// ===========================================================================

#[test]
fn t4_config_invalid_schema_version_rejected() {
    let env = TestEnv::new();
    fs::write(
        env.project_dir.join("evm-cloud.toml"),
        r#"schema_version = 99

[project]
name = "bad-version"

[compute]
engine = "k3s"

[database]
mode = "single"
provider = "bare_metal"

[indexer]
config_path = "config/rindexer.yaml"
chains = ["ethereum"]

[rpc]
endpoints = { ethereum = "https://rpc.example.com" }

[ingress]
mode = "none"

[secrets]
mode = "local"
"#,
    )
    .unwrap();

    // deploy --dry-run goes through easy_mode which calls config::loader::load()
    // triggering full schema validation including schema_version check
    env.cmd()
        .args(["deploy", "--dry-run"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("schema_version")
                .and(predicate::str::contains("unsupported")),
        );
}

#[test]
fn t4_config_missing_indexer_section_rejected() {
    let env = TestEnv::new();
    // Write config missing the required [indexer] section
    fs::write(
        env.project_dir.join("evm-cloud.toml"),
        r#"schema_version = 1

[project]
name = "no-indexer"

[compute]
engine = "k3s"

[database]
mode = "single"
provider = "bare_metal"

[rpc]
endpoints = { ethereum = "https://rpc.example.com" }

[ingress]
mode = "none"

[secrets]
mode = "local"
"#,
    )
    .unwrap();

    // deploy --dry-run triggers config::loader::load() which requires all sections
    env.cmd()
        .args(["deploy", "--dry-run"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("indexer"));
}

#[test]
fn t4_config_invalid_compute_engine_rejected() {
    let env = TestEnv::new();
    fs::write(
        env.project_dir.join("evm-cloud.toml"),
        r#"schema_version = 1

[project]
name = "bad-engine"

[compute]
engine = "invalid_engine"

[database]
mode = "single"
provider = "bare_metal"

[indexer]
config_path = "config/rindexer.yaml"
chains = ["ethereum"]

[rpc]
endpoints = { ethereum = "https://rpc.example.com" }

[ingress]
mode = "none"

[secrets]
mode = "local"
"#,
    )
    .unwrap();

    // deploy --dry-run triggers config parsing where serde rejects unknown enum variant
    env.cmd()
        .args(["deploy", "--dry-run"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("engine")
                .or(predicate::str::contains("invalid_engine"))
                .or(predicate::str::contains("unknown variant")),
        );
}

// ===========================================================================
// Tier 5: Templates — apply, invalid template, missing --chains
// ===========================================================================

#[test]
fn t5_templates_apply_generates_config_files() {
    let env = TestEnv::new();

    let output = env
        .cmd()
        .args([
            "templates",
            "apply",
            "erc20-transfers",
            "--chains",
            "ethereum",
            "--var",
            "token_address=0x0000000000000000000000000000000000000001",
            "--dir",
            env.project_dir.to_str().unwrap(),
        ])
        .output()
        .expect("run templates apply");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    assert!(
        output.status.success(),
        "templates apply should succeed, got: {combined}"
    );

    // Verify that key config files were generated
    assert!(
        env.project_dir.join("config/rindexer.yaml").exists(),
        "config/rindexer.yaml should be generated"
    );
    assert!(
        env.project_dir.join("config/erpc.yaml").exists(),
        "config/erpc.yaml should be generated"
    );
    assert!(
        env.project_dir.join("evm-cloud.toml").exists(),
        "evm-cloud.toml should be generated"
    );
    assert!(
        env.project_dir
            .join(".evm-cloud/template-lock.toml")
            .exists(),
        ".evm-cloud/template-lock.toml should be generated"
    );
}

#[test]
fn t5_templates_apply_invalid_template_rejected() {
    let env = TestEnv::new();

    env.cmd()
        .args([
            "templates",
            "apply",
            "nonexistent-protocol",
            "--chains",
            "ethereum",
            "--dir",
            env.project_dir.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("not found")
                .and(predicate::str::contains("nonexistent-protocol")),
        );
}

#[test]
fn t5_templates_apply_without_chains_rejected() {
    let env = TestEnv::new();

    env.cmd()
        .args([
            "templates",
            "apply",
            "erc20-transfers",
            "--dir",
            env.project_dir.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--chains"));
}

// ===========================================================================
// Tier 6: Preflight routing — mode detection and ambiguity
// ===========================================================================

#[test]
fn t6_power_mode_detection_no_toml() {
    let env = TestEnv::new();

    // Create main.tf + versions.tf (explicit Terraform root) — no evm-cloud.toml
    fs::write(env.project_dir.join("main.tf"), "terraform {}\n").unwrap();
    fs::write(
        env.project_dir.join("versions.tf"),
        "terraform { required_version = \">= 1.5\" }\n",
    )
    .unwrap();

    let output = env
        .cmd()
        .args(["logs", "--list"])
        .output()
        .expect("run logs --list");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // logs passes allow_raw_terraform=true, so it should NOT fail at preflight
    // with "no evm-cloud.toml" — it should route to RawTerraform mode.
    // It may fail later (e.g. missing handoff), but NOT with NoProjectDetected.
    assert!(
        !stderr.contains("no evm-cloud.toml or"),
        "should not fail with NoProjectDetected for explicit TF root, got: {stderr}"
    );
}

#[test]
fn t6_ambiguous_mode_both_toml_and_tf_root() {
    let env = TestEnv::new();

    // Create BOTH evm-cloud.toml AND main.tf + versions.tf — should be ambiguous
    fs::write(
        env.project_dir.join("evm-cloud.toml"),
        "schema_version = 1\n[project]\nname = \"test\"\n",
    )
    .unwrap();
    fs::write(env.project_dir.join("main.tf"), "terraform {}\n").unwrap();
    fs::write(
        env.project_dir.join("versions.tf"),
        "terraform { required_version = \">= 1.5\" }\n",
    )
    .unwrap();

    env.cmd()
        .args(["logs", "--list"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("cannot determine project mode")
                .or(predicate::str::contains("ambiguous")),
        );
}

// ===========================================================================
// Tier 7: Status output + Deploy orchestration + Env + Error paths
// ===========================================================================

// ── 7a: Status command — output verification ──

#[test]
fn t7_status_shows_compute_engine() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    let output = env.cmd().arg("status").arg("--json").output().expect("run status --json");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // The handoff has compute_engine=k3s, status should surface it
    assert!(
        combined.contains("k3s"),
        "status output should contain the compute engine 'k3s', got: {combined}"
    );
}

#[test]
fn t7_status_calls_terraform_output() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    let _ = env.cmd().arg("status").arg("--json").output().expect("run status");

    let tf_log = env.terraform_log();
    assert!(
        !tf_log.is_empty(),
        "terraform should have been invoked"
    );
    assert!(
        tf_log.iter().any(|l| l.contains("output") && l.contains("-json")),
        "terraform should have been called with 'output -json', log: {tf_log:?}"
    );
}

// ── 7b: Deploy command — orchestration verification ──

#[test]
fn t7_deploy_dry_run_calls_init_before_plan() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    let _ = env
        .cmd()
        .args(["deploy", "--dry-run"])
        .output()
        .expect("run deploy --dry-run");

    let tf_log = env.terraform_log();

    // Find the positions of init and plan in the log
    let init_pos = tf_log.iter().position(|l| l.contains("init"));
    let plan_pos = tf_log.iter().position(|l| l.contains("plan"));

    assert!(
        init_pos.is_some(),
        "deploy should call terraform init, log: {tf_log:?}"
    );
    assert!(
        plan_pos.is_some(),
        "deploy --dry-run should call terraform plan, log: {tf_log:?}"
    );
    assert!(
        init_pos.unwrap() < plan_pos.unwrap(),
        "terraform init (pos {:?}) must come before plan (pos {:?}), log: {tf_log:?}",
        init_pos, plan_pos
    );
}

#[test]
fn t7_deploy_no_flags_non_interactive_fails() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    // Without --dry-run or --auto-approve, in a non-interactive (piped) context
    // the CLI should refuse to run
    env.cmd()
        .args(["deploy"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("non-interactive")
                .or(predicate::str::contains("--auto-approve")),
        );
}

#[test]
fn t7_deploy_only_invalid_phase_rejected() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    // --only with an invalid value should be rejected by clap
    env.cmd()
        .args(["deploy", "--only", "invalid_phase"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("invalid value")
                .or(predicate::str::contains("invalid_phase"))
                .or(predicate::str::contains("possible values")),
        );
}

// ── 7c: Env commands ──

#[test]
fn t7_env_list_no_envs() {
    let env = TestEnv::new().with_config(CONFIG_K3S);

    // env list with no envs/ directory should succeed and mention no environments
    let output = env
        .cmd()
        .args(["env", "list"])
        .output()
        .expect("run env list");

    assert!(
        output.status.success(),
        "env list should succeed with no envs, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No environments") || stderr.contains("no environments") || stderr.contains("env add"),
        "should mention no environments or how to add one, got: {stderr}"
    );
}

#[test]
fn t7_env_add_duplicate_rejects() {
    let env = TestEnv::new().with_config(CONFIG_K3S);

    // env add requires a [state] section in the config. Write a config with state.
    fs::write(
        env.project_dir.join("evm-cloud.toml"),
        r#"schema_version = 1

[project]
name = "test-project"

[compute]
engine = "k3s"

[database]
mode = "single"
provider = "bare_metal"

[indexer]
config_path = "config/rindexer.yaml"
chains = ["ethereum"]

[rpc]
endpoints = { ethereum = "https://rpc.example.com" }

[ingress]
mode = "none"

[secrets]
mode = "local"

[state]
backend = "s3"
bucket = "test-bucket"
region = "us-east-1"
dynamodb_table = "test-lock"
"#,
    )
    .unwrap();

    // First add: create the env with --yes to skip interactive prompts
    let first = env
        .cmd()
        .args(["env", "add", "staging", "--yes"])
        .output()
        .expect("first env add");

    assert!(
        first.status.success(),
        "first env add should succeed, stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );

    // Verify the envs/staging/ directory was created
    assert!(
        env.project_dir.join("envs/staging").is_dir(),
        "envs/staging/ should exist after env add"
    );

    // Second add: should fail because staging already exists
    env.cmd()
        .args(["env", "add", "staging", "--yes"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("already exists"),
        );
}

// ── 7d: Handoff error paths ──

#[test]
fn t7_corrupt_handoff_json_shows_error() {
    let env = TestEnv::new().with_config(CONFIG_K3S);

    // Write malformed JSON to the handoff output files
    fs::write(
        env.state_dir.join("output_workload_handoff.json"),
        "{ this is not valid json!!!",
    )
    .unwrap();
    fs::write(
        env.state_dir.join("output_full.json"),
        "{ also broken }}}",
    )
    .unwrap();

    let output = env.cmd().arg("status").arg("--json").output().expect("run status");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "status with corrupt handoff should fail"
    );
    assert!(
        stderr.contains("parse")
            || stderr.contains("invalid")
            || stderr.contains("JSON")
            || stderr.contains("json")
            || stderr.contains("expected")
            || stderr.contains("syntax"),
        "error should mention parsing/invalid JSON, got: {stderr}"
    );
}

#[test]
fn t7_wrong_handoff_version_shows_error() {
    let env = TestEnv::new().with_config(CONFIG_K3S);

    // Write a valid JSON handoff but with an unsupported version
    let bad_handoff = serde_json::json!({
        "version": "v99",
        "mode": "external",
        "compute_engine": "k3s",
        "project_name": "test-project",
        "runtime": {
            "k3s": {
                "kubeconfig_base64": "ZmFrZQ==",
                "host_ip": "10.0.0.1"
            }
        },
        "services": {},
        "data": {},
        "secrets": {},
        "ingress": {}
    });

    fs::write(
        env.state_dir.join("output_workload_handoff.json"),
        bad_handoff.to_string(),
    )
    .unwrap();

    let full = serde_json::json!({ "workload_handoff": { "value": bad_handoff } });
    fs::write(
        env.state_dir.join("output_full.json"),
        full.to_string(),
    )
    .unwrap();

    let output = env.cmd().arg("status").arg("--json").output().expect("run status");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "status with wrong handoff version should fail"
    );
    assert!(
        stderr.contains("version") || stderr.contains("v99") || stderr.contains("unsupported"),
        "error should mention version mismatch, got: {stderr}"
    );
}

// ===========================================================================
// Tier 8: Deploy orchestration + flag validation
// ===========================================================================

// ── 8a: Flag validation (clap-level — no terraform needed) ──

#[test]
fn t8_deploy_only_app_with_dry_run_conflicts() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    env.cmd()
        .args(["deploy", "--only", "app", "--dry-run"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("incompatible with --only app")
                .or(predicate::str::contains("conflict")),
        );
}

#[test]
fn t8_deploy_only_app_with_auto_approve_conflicts() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    env.cmd()
        .args(["deploy", "--only", "app", "--auto-approve"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("incompatible with --only app")
                .or(predicate::str::contains("conflict")),
        );
}

#[test]
fn t8_deploy_only_app_with_tf_args_conflicts() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    env.cmd()
        .args(["deploy", "--only", "app", "--tf-args=-target=module.x"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("incompatible with --only app")
                .or(predicate::str::contains("conflict")),
        );
}

// ── 8b: Phase selection via terraform log ──

#[test]
fn t8_deploy_dry_run_writes_no_apply() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    let _ = env
        .cmd()
        .args(["deploy", "--dry-run"])
        .output()
        .expect("run deploy --dry-run");

    let tf_log = env.terraform_log();
    assert!(
        tf_log.iter().any(|l| l.contains("plan")),
        "deploy --dry-run should call terraform plan, log: {tf_log:?}"
    );
    assert!(
        !tf_log.iter().any(|l| l.contains("apply")),
        "deploy --dry-run should NOT call terraform apply, log: {tf_log:?}"
    );
}

#[test]
fn t8_deploy_only_infra_dry_run_no_deployer() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    let _ = env
        .cmd()
        .args(["deploy", "--only", "infra", "--dry-run"])
        .output()
        .expect("run deploy --only infra --dry-run");

    let tf_log = env.terraform_log();
    assert!(
        tf_log.iter().any(|l| l.contains("plan")),
        "deploy --only infra --dry-run should call terraform plan, log: {tf_log:?}"
    );

    let kubectl_log = env.kubectl_log();
    assert!(
        kubectl_log.is_empty(),
        "deploy --only infra should NOT invoke kubectl (no deployer), got: {kubectl_log:?}"
    );
}

#[test]
fn t8_deploy_auto_approve_calls_apply() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    // auto-approve triggers apply; deployer phase may fail (no bundled script) — that's OK
    let _ = env
        .cmd()
        .args(["deploy", "--auto-approve"])
        .output()
        .expect("run deploy --auto-approve");

    let tf_log = env.terraform_log();
    assert!(
        tf_log.iter().any(|l| l.contains("init")),
        "deploy --auto-approve should call terraform init, log: {tf_log:?}"
    );
    assert!(
        tf_log.iter().any(|l| l.contains("apply")),
        "deploy --auto-approve should call terraform apply, log: {tf_log:?}"
    );
}

// ── 8c: Terraform command sequence ──

#[test]
fn t8_deploy_dry_run_init_then_plan_sequence() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    let _ = env
        .cmd()
        .args(["deploy", "--dry-run"])
        .output()
        .expect("run deploy --dry-run");

    let tf_log = env.terraform_log();

    let version_pos = tf_log.iter().position(|l| l.contains("version -json"));
    let init_pos = tf_log.iter().position(|l| l.contains("init"));
    let plan_pos = tf_log.iter().position(|l| l.contains("plan"));

    assert!(
        version_pos.is_some(),
        "deploy should call terraform version -json, log: {tf_log:?}"
    );
    assert!(
        init_pos.is_some(),
        "deploy should call terraform init, log: {tf_log:?}"
    );
    assert!(
        plan_pos.is_some(),
        "deploy --dry-run should call terraform plan, log: {tf_log:?}"
    );

    assert!(
        version_pos.unwrap() < init_pos.unwrap(),
        "terraform version (pos {:?}) must come before init (pos {:?}), log: {tf_log:?}",
        version_pos, init_pos
    );
    assert!(
        init_pos.unwrap() < plan_pos.unwrap(),
        "terraform init (pos {:?}) must come before plan (pos {:?}), log: {tf_log:?}",
        init_pos, plan_pos
    );
}

// ── 8d: Non-interactive rejection ──

#[test]
fn t8_deploy_requires_approval_in_non_interactive() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    // Without --dry-run or --auto-approve, piped (non-interactive) context should fail
    env.cmd()
        .args(["deploy"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("non-interactive")
                .or(predicate::str::contains("--auto-approve")),
        );
}

// ===========================================================================
// Tier 9: Lock mechanics + error paths
// ===========================================================================

// ── 9a: Deploy lock mechanics ──

#[test]
fn t9_deploy_lock_stale_lock_recovered() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    // Write a stale lock file with a PID that is almost certainly not running
    let lock_path = env.project_dir.join(".evm-cloud-deploy.lock");
    fs::write(&lock_path, r#"{"pid":999999,"started_at":1000000000}"#).unwrap();

    // deploy --auto-approve should auto-recover the stale lock (PID 999999 is dead)
    let output = env
        .cmd()
        .args(["deploy", "--auto-approve"])
        .output()
        .expect("run deploy --auto-approve");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // The command should NOT fail with "lock busy" — stale lock is auto-recovered
    assert!(
        !stderr.contains("lock busy") && !stderr.contains("DeployLockBusy"),
        "stale lock should be auto-recovered, not rejected as busy, got: {stderr}"
    );

    // Terraform log should show activity beyond just version check
    let tf_log = env.terraform_log();
    assert!(
        tf_log
            .iter()
            .any(|l| l.contains("init") || l.contains("plan") || l.contains("apply")),
        "terraform should have been invoked (past lock stage), log: {tf_log:?}"
    );
}

#[test]
fn t9_deploy_lock_file_cleaned_up_after_deploy() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    let lock_path = env.project_dir.join(".evm-cloud-deploy.lock");

    // Verify no lock file exists before deploy
    assert!(
        !lock_path.exists(),
        "lock file should not exist before deploy"
    );

    // Run deploy --auto-approve (may fail at deployer stage, that's fine)
    let _ = env
        .cmd()
        .args(["deploy", "--auto-approve"])
        .output()
        .expect("run deploy --auto-approve");

    // After command completes, lock file should be cleaned up (Drop impl)
    assert!(
        !lock_path.exists(),
        "lock file should be cleaned up after deploy completes (Drop guard)"
    );
}

#[test]
fn t9_deploy_lock_env_namespaced() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    let default_lock = env.project_dir.join(".evm-cloud-deploy.lock");

    // Run a default (non-env) deploy
    let _ = env
        .cmd()
        .args(["deploy", "--auto-approve"])
        .output()
        .expect("run deploy --auto-approve");

    // The default lock should be cleaned up, proving it was created as
    // .evm-cloud-deploy.lock (not an env-namespaced variant)
    assert!(
        !default_lock.exists(),
        "default lock file should be cleaned up after deploy"
    );

    // Verify no env-namespaced lock files were left behind
    let entries: Vec<_> = fs::read_dir(&env.project_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(".evm-cloud-deploy")
        })
        .collect();
    assert!(
        entries.is_empty(),
        "no deploy lock files should remain after deploy, found: {entries:?}"
    );
}

// ── 9b: Error paths ──

#[test]
fn t9_deploy_handoff_version_mismatch_error() {
    let env = TestEnv::new().with_config(CONFIG_K3S);

    // Write a handoff with an unsupported version
    let bad_handoff = serde_json::json!({
        "version": "v99",
        "mode": "external",
        "compute_engine": "k3s",
        "project_name": "test-project",
        "runtime": {
            "k3s": {
                "kubeconfig_base64": "ZmFrZQ==",
                "host_ip": "10.0.0.1"
            }
        },
        "services": {},
        "data": {},
        "secrets": {},
        "ingress": {}
    });

    // Write the bad handoff to mock terraform output
    fs::write(
        env.state_dir.join("output_workload_handoff.json"),
        bad_handoff.to_string(),
    )
    .unwrap();

    let full = serde_json::json!({ "workload_handoff": { "value": bad_handoff } });
    fs::write(env.state_dir.join("output_full.json"), full.to_string()).unwrap();

    // deploy --auto-approve should fail with a version-related error
    let output = env
        .cmd()
        .args(["deploy", "--auto-approve"])
        .output()
        .expect("run deploy --auto-approve");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "deploy with bad handoff version should fail"
    );
    assert!(
        stderr.contains("version") || stderr.contains("v99") || stderr.contains("unsupported"),
        "error should mention version issue, got: {stderr}"
    );
}

#[test]
fn t9_status_corrupt_terraform_output() {
    let env = TestEnv::new().with_config(CONFIG_K3S);

    // Write `null` — valid JSON but not a usable handoff object
    fs::write(
        env.state_dir.join("output_workload_handoff.json"),
        "null",
    )
    .unwrap();

    let full = serde_json::json!({ "workload_handoff": { "value": null } });
    fs::write(env.state_dir.join("output_full.json"), full.to_string()).unwrap();

    let output = env
        .cmd()
        .args(["status", "--json"])
        .output()
        .expect("run status");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should fail with a parsing/handoff error, not a crash/panic
    assert!(
        !output.status.success(),
        "status with null handoff should fail gracefully"
    );
    assert!(
        stderr.contains("parse")
            || stderr.contains("invalid")
            || stderr.contains("handoff")
            || stderr.contains("JSON")
            || stderr.contains("json")
            || stderr.contains("expected")
            || stderr.contains("null"),
        "error should mention parsing issue, got: {stderr}"
    );
}

#[test]
fn t9_deploy_flag_only_infra_skips_deployer() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    let _ = env
        .cmd()
        .args(["deploy", "--only", "infra", "--dry-run"])
        .output()
        .expect("run deploy --only infra --dry-run");

    // kubectl should never be invoked (deployer was skipped)
    let kubectl_log = env.kubectl_log();
    assert!(
        kubectl_log.is_empty(),
        "kubectl should NOT be invoked when --only infra, got: {kubectl_log:?}"
    );

    // Terraform should have been called (infra phase ran)
    let tf_log = env.terraform_log();
    assert!(
        !tf_log.is_empty(),
        "terraform should have been invoked for infra phase"
    );
    assert!(
        tf_log.iter().any(|l| l.contains("plan")),
        "terraform plan should have been called, log: {tf_log:?}"
    );
}

#[test]
fn t9_deploy_auto_approve_gets_past_confirmation() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    let _ = env
        .cmd()
        .args(["deploy", "--auto-approve"])
        .output()
        .expect("run deploy --auto-approve");

    // Terraform log should contain 'apply' — proving it didn't stop at
    // the confirmation prompt (which would happen without --auto-approve
    // in a non-interactive context)
    let tf_log = env.terraform_log();
    assert!(
        tf_log.iter().any(|l| l.contains("apply")),
        "terraform apply should have been called with --auto-approve, log: {tf_log:?}"
    );
}

// ===========================================================================
// Tier 11: Bare metal SSH resolution
// ===========================================================================

#[test]
fn t11_ec2_logs_ssh_targets_public_ip() {
    let env = TestEnv::new()
        .with_config(CONFIG_EC2)
        .with_handoff(handoff_ec2());

    let _ = env
        .cmd()
        .args(["logs", "erpc", "--tail", "10"])
        .output()
        .expect("run logs erpc");

    let ssh_log = env.ssh_log();
    assert!(
        !ssh_log.is_empty(),
        "ssh should have been invoked for EC2 logs"
    );
    assert!(
        ssh_log.iter().any(|l| l.contains("54.123.45.67")),
        "ssh should target EC2 public_ip 54.123.45.67, got: {ssh_log:?}"
    );
}

#[test]
fn t11_compose_logs_ssh_targets_bare_metal_host() {
    let env = TestEnv::new()
        .with_config(CONFIG_DOCKER_COMPOSE)
        .with_handoff(handoff_docker_compose());

    let _ = env
        .cmd()
        .args(["logs", "erpc", "--tail", "10"])
        .output()
        .expect("run logs erpc");

    let ssh_log = env.ssh_log();
    assert!(
        !ssh_log.is_empty(),
        "ssh should have been invoked for docker_compose logs"
    );
    assert!(
        ssh_log.iter().any(|l| l.contains("10.0.0.2")),
        "ssh should target bare_metal host_address 10.0.0.2, got: {ssh_log:?}"
    );
}

#[test]
fn t11_ec2_logs_ssh_uses_default_user() {
    let env = TestEnv::new()
        .with_config(CONFIG_EC2)
        .with_handoff(handoff_ec2());

    let _ = env
        .cmd()
        .args(["logs", "erpc", "--tail", "10"])
        .output()
        .expect("run logs erpc");

    let ssh_log = env.ssh_log();
    assert!(
        !ssh_log.is_empty(),
        "ssh should have been invoked for EC2 logs"
    );
    // EC2 default SSH user is "ec2-user" (from post_deploy::ssh_user_for)
    assert!(
        ssh_log.iter().any(|l| l.contains("ec2-user")),
        "ssh should use default EC2 user 'ec2-user', got: {ssh_log:?}"
    );
}

// ===========================================================================
// Tier 11: Destroy command
// ===========================================================================

#[test]
fn t11_destroy_without_auto_approve_fails() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    // destroy without --yes or --auto-approve in non-interactive (piped) context should fail
    env.cmd()
        .args(["destroy"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("auto-approve")
                .or(predicate::str::contains("--yes"))
                .or(predicate::str::contains("non-interactive"))
                .or(predicate::str::contains("confirm")),
        );
}

#[test]
fn t11_destroy_help_shows_flags() {
    TestEnv::new()
        .cmd()
        .args(["destroy", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--auto-approve"));
}

#[test]
fn t11_destroy_auto_approve_calls_terraform_destroy() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    // --yes --auto-approve should get past safety checks and invoke terraform destroy
    let _ = env
        .cmd()
        .args(["destroy", "--yes", "--auto-approve"])
        .output()
        .expect("run destroy --yes --auto-approve");

    let tf_log = env.terraform_log();
    assert!(
        tf_log.iter().any(|l| l.contains("destroy")),
        "terraform destroy should have been called, log: {tf_log:?}"
    );
}

// ===========================================================================
// Tier 10: SSH + Docker Compose command construction
// ===========================================================================

#[test]
fn t10_compose_logs_erpc_builds_ssh_command() {
    let env = TestEnv::new()
        .with_config(CONFIG_DOCKER_COMPOSE)
        .with_handoff(handoff_docker_compose());

    let _ = env.cmd().args(["logs", "erpc"]).output().unwrap();

    let log = env.ssh_log();
    assert!(!log.is_empty(), "ssh should have been invoked");

    let cmd = &log[0];
    assert!(
        cmd.contains("docker compose -f /opt/evm-cloud/docker-compose.yml logs erpc"),
        "should contain docker compose logs erpc, got: {cmd}"
    );
    assert!(
        cmd.contains("--tail 100"),
        "should contain default --tail 100, got: {cmd}"
    );
    assert!(
        cmd.contains("ubuntu@10.0.0.2"),
        "should target ubuntu@bare_metal host, got: {cmd}"
    );
}

#[test]
fn t10_compose_logs_indexer_maps_to_rindexer() {
    let env = TestEnv::new()
        .with_config(CONFIG_DOCKER_COMPOSE)
        .with_handoff(handoff_docker_compose());

    let _ = env.cmd().args(["logs", "indexer"]).output().unwrap();

    let log = env.ssh_log();
    assert!(!log.is_empty(), "ssh should have been invoked");

    let cmd = &log[0];
    assert!(
        cmd.contains("logs rindexer"),
        "indexer should map to compose name 'rindexer', got: {cmd}"
    );
}

#[test]
fn t10_compose_logs_follow_flag_in_ssh() {
    let env = TestEnv::new()
        .with_config(CONFIG_DOCKER_COMPOSE)
        .with_handoff(handoff_docker_compose());

    let _ = env
        .cmd()
        .args(["logs", "erpc", "-f"])
        .output()
        .unwrap();

    let log = env.ssh_log();
    assert!(!log.is_empty(), "ssh should have been invoked");

    let cmd = &log[0];
    assert!(
        cmd.contains("docker compose -f /opt/evm-cloud/docker-compose.yml logs erpc"),
        "should contain docker compose logs command, got: {cmd}"
    );
    assert!(
        cmd.contains("-f"),
        "should contain -f follow flag, got: {cmd}"
    );
}

#[test]
fn t10_compose_logs_custom_tail() {
    let env = TestEnv::new()
        .with_config(CONFIG_DOCKER_COMPOSE)
        .with_handoff(handoff_docker_compose());

    let _ = env
        .cmd()
        .args(["logs", "erpc", "--tail", "50"])
        .output()
        .unwrap();

    let log = env.ssh_log();
    assert!(!log.is_empty(), "ssh should have been invoked");

    let cmd = &log[0];
    assert!(
        cmd.contains("--tail 50"),
        "should contain --tail 50, got: {cmd}"
    );
}

#[test]
fn t10_compose_logs_caddy() {
    let env = TestEnv::new()
        .with_config(CONFIG_DOCKER_COMPOSE)
        .with_handoff(handoff_docker_compose());

    let _ = env.cmd().args(["logs", "caddy"]).output().unwrap();

    let log = env.ssh_log();
    assert!(!log.is_empty(), "ssh should have been invoked");

    let cmd = &log[0];
    assert!(
        cmd.contains("logs caddy"),
        "should contain 'logs caddy', got: {cmd}"
    );
}

#[test]
fn t10_compose_logs_clickhouse() {
    let env = TestEnv::new()
        .with_config(CONFIG_DOCKER_COMPOSE)
        .with_handoff(handoff_docker_compose());

    let _ = env.cmd().args(["logs", "clickhouse"]).output().unwrap();

    let log = env.ssh_log();
    assert!(!log.is_empty(), "ssh should have been invoked");

    let cmd = &log[0];
    assert!(
        cmd.contains("logs clickhouse"),
        "should contain 'logs clickhouse', got: {cmd}"
    );
}

#[test]
fn t10_compose_logs_custom_service_rejected() {
    let env = TestEnv::new()
        .with_config(CONFIG_DOCKER_COMPOSE)
        .with_handoff(handoff_docker_compose());

    env.cmd()
        .args(["logs", "api"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("K3s/EKS"));

    // SSH should never have been invoked
    let log = env.ssh_log();
    assert!(
        log.is_empty(),
        "ssh should NOT be invoked for rejected custom service, got: {log:?}"
    );
}

#[test]
fn t10_compose_logs_list_shows_compose_targets() {
    let env = TestEnv::new()
        .with_config(CONFIG_DOCKER_COMPOSE)
        .with_handoff(handoff_docker_compose());

    let output = env
        .cmd()
        .args(["logs", "--list"])
        .output()
        .expect("run logs --list");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should show "compose" engine
    assert!(
        stdout.contains("compose"),
        "should show compose engine type, got: {stdout}"
    );

    // Should show compose service names
    assert!(
        stdout.contains("rindexer"),
        "should show rindexer compose target, got: {stdout}"
    );
    assert!(
        stdout.contains("erpc"),
        "should show erpc compose target, got: {stdout}"
    );
    assert!(
        stdout.contains("caddy"),
        "should show caddy compose target, got: {stdout}"
    );
    assert!(
        stdout.contains("clickhouse"),
        "should show clickhouse compose target, got: {stdout}"
    );
    assert!(
        stdout.contains("postgres"),
        "should show postgres compose target, got: {stdout}"
    );

    // Should NOT show K8s release names for custom services
    assert!(
        !stdout.contains("test-compose-api"),
        "should not show K8s-only custom service 'api', got: {stdout}"
    );
}

// ===========================================================================
// Tier 12: --only app + EKS engine
// ===========================================================================

const CONFIG_EKS: &str = r#"schema_version = 1

[project]
name = "test-eks"

[compute]
engine = "eks"

[database]
mode = "single"
provider = "bare_metal"

[indexer]
config_path = "config/rindexer.yaml"
chains = ["ethereum"]

[rpc]
endpoints = { ethereum = "https://rpc.example.com" }

[ingress]
mode = "none"

[secrets]
mode = "local"
"#;

fn handoff_eks() -> serde_json::Value {
    serde_json::json!({
        "version": "v1",
        "mode": "external",
        "compute_engine": "eks",
        "project_name": "test-eks",
        "runtime": {
            "eks": { "cluster_name": "test-cluster" }
        },
        "services": {},
        "data": {},
        "secrets": {},
        "ingress": {}
    })
}

#[test]
fn t12_deploy_only_app_loads_from_state() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s());

    // --only app skips terraform plan/apply; loads handoff from state instead.
    // The deployer will fail (no real scripts), but we can verify terraform log.
    let _ = env
        .cmd()
        .args(["deploy", "--only", "app"])
        .output()
        .expect("run deploy --only app");

    let tf_log = env.terraform_log();

    // Should load handoff from state via `terraform output -json`
    assert!(
        tf_log.iter().any(|l| l.contains("output -json")),
        "deploy --only app should call terraform output to load handoff from state, log: {tf_log:?}"
    );

    // Should NOT call plan or apply (infra phase skipped)
    assert!(
        !tf_log.iter().any(|l| l.contains("plan")),
        "deploy --only app should NOT call terraform plan, log: {tf_log:?}"
    );
    assert!(
        !tf_log.iter().any(|l| l.contains("apply")),
        "deploy --only app should NOT call terraform apply, log: {tf_log:?}"
    );
}

#[test]
fn t12_deploy_only_app_no_state_fails() {
    let env = TestEnv::new().with_config(CONFIG_K3S);
    // No handoff written — no terraform state to load from.

    env.cmd()
        .args(["deploy", "--only", "app"])
        .assert()
        .failure()
        .stderr(predicate::str::is_empty().not());
}

#[test]
fn t12_eks_deploy_unsupported_engine() {
    let env = TestEnv::new()
        .with_config(CONFIG_EKS)
        .with_handoff(handoff_eks());

    // deploy --auto-approve: terraform apply succeeds (mock), then deployer
    // dispatch hits DeployerUnsupportedEngine for EKS.
    let output = env
        .cmd()
        .args(["deploy", "--auto-approve"])
        .output()
        .expect("run deploy --auto-approve with EKS");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "EKS deploy should fail at deployer dispatch"
    );
    assert!(
        stderr.contains("unsupported") || stderr.contains("eks"),
        "error should mention unsupported engine or eks, got: {stderr}"
    );
}

// ===========================================================================
// Tier 13: Mock deployer — full deploy pipeline
// ===========================================================================

#[test]
fn t13_deploy_writes_valid_handoff_to_deployer() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s())
        .with_deploy_configs()
        .with_mock_deployer();

    let output = env
        .cmd()
        .args(["deploy", "--auto-approve"])
        .output()
        .expect("run deploy --auto-approve with mock deployer");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "deploy with mock deployer should succeed, stderr: {stderr}"
    );

    // Deployer was invoked
    let log = env.deployer_log();
    assert!(
        !log.is_empty(),
        "deployer.log should exist and have content"
    );

    // First line should be the deployer invocation with the handoff path
    let first = &log[0];
    assert!(
        first.starts_with("deployer "),
        "deployer log should start with 'deployer ', got: {first}"
    );

    // Read the handoff copy saved by the mock deployer to TMPDIR
    let handoff_copy = env.state_dir.join("deployer_handoff.json");
    assert!(
        handoff_copy.exists(),
        "deployer should have copied handoff to deployer_handoff.json"
    );
    let handoff_content = fs::read_to_string(&handoff_copy)
        .expect("handoff copy should be readable");
    let handoff: serde_json::Value = serde_json::from_str(&handoff_content)
        .expect("handoff should be valid JSON");

    assert_eq!(
        handoff["project_name"], "test-project",
        "handoff should contain project_name"
    );
    assert_eq!(
        handoff["compute_engine"], "k3s",
        "handoff should contain compute_engine"
    );
    assert!(
        handoff["services"].is_object(),
        "handoff should contain services object"
    );
}

#[test]
fn t13_deploy_sanitizes_env_for_deployer() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s())
        .with_deploy_configs()
        .with_mock_deployer();

    let output = env
        .cmd()
        .env("DATABASE_PASSWORD", "secret_value")
        .args(["deploy", "--auto-approve"])
        .output()
        .expect("run deploy --auto-approve with secret env");

    assert!(
        output.status.success(),
        "deploy should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let env_log = env.deployer_env_log();
    assert!(
        !env_log.is_empty(),
        "deployer_env.log should exist and have content"
    );

    // DATABASE_PASSWORD should NOT be in the sanitized env
    assert!(
        !env_log.contains("DATABASE_PASSWORD"),
        "DATABASE_PASSWORD should be stripped by env sanitization, got env log:\n{env_log}"
    );

    // PATH should be present (it's in the whitelist)
    assert!(
        env_log.contains("PATH="),
        "PATH should be present in sanitized env, got env log:\n{env_log}"
    );
}

#[test]
fn t13_deploy_streams_formatted_output() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s())
        .with_deploy_configs()
        .with_mock_deployer();

    let output = env
        .cmd()
        .args(["deploy", "--auto-approve"])
        .output()
        .expect("run deploy --auto-approve");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "deploy should succeed, stderr: {stderr}"
    );

    // format_deploy_line maps "Cluster reachable." -> "k3s cluster reachable"
    assert!(
        stderr.contains("k3s cluster reachable"),
        "stderr should contain formatted 'k3s cluster reachable', got: {stderr}"
    );

    // "ESO is ready." -> "ESO is ready"
    assert!(
        stderr.contains("ESO is ready"),
        "stderr should contain formatted 'ESO is ready', got: {stderr}"
    );

    // "Deploying eRPC (test-erpc)..." -> "eRPC:" with "test-erpc"
    assert!(
        stderr.contains("eRPC"),
        "stderr should contain formatted eRPC line, got: {stderr}"
    );

    // "Deploying rindexer instance (test-indexer)..." -> "rindexer #1"
    assert!(
        stderr.contains("rindexer #1"),
        "stderr should contain formatted 'rindexer #1', got: {stderr}"
    );

    // "All workloads deployed successfully." is suppressed by format_deploy_line
    assert!(
        !stderr.contains("All workloads deployed successfully"),
        "raw 'All workloads deployed successfully' should be suppressed, got: {stderr}"
    );
}

#[test]
fn t13_deploy_success_exits_zero() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s())
        .with_deploy_configs()
        .with_mock_deployer();

    env.cmd()
        .args(["deploy", "--auto-approve"])
        .assert()
        .success();
}

#[test]
fn t13_deploy_failure_shows_recovery_hint() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s())
        .with_deploy_configs()
        .with_mock_deployer();

    // Write signal file to make mock deployer exit with code 1
    fs::write(env.state_dir.join("deployer_exit_code"), "1").unwrap();

    let output = env
        .cmd()
        .args(["deploy", "--auto-approve"])
        .output()
        .expect("run deploy --auto-approve (failure)");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "deploy should fail when deployer exits 1"
    );

    // Recovery hint: "Retry deployer only: evm-cloud deploy --only app"
    assert!(
        stderr.contains("--only app"),
        "stderr should contain recovery hint with '--only app', got: {stderr}"
    );
}

#[test]
fn t13_deploy_passthrough_args_reach_deployer() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s())
        .with_deploy_configs()
        .with_mock_deployer();

    let output = env
        .cmd()
        .args(["deploy", "--auto-approve", "--", "--helm-timeout=600s"])
        .output()
        .expect("run deploy --auto-approve with passthrough args");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "deploy with passthrough args should succeed, stderr: {stderr}"
    );

    let log = env.deployer_log();
    assert!(
        !log.is_empty(),
        "deployer.log should exist"
    );

    let first = &log[0];
    assert!(
        first.contains("--helm-timeout=600s"),
        "passthrough arg should reach deployer, got: {first}"
    );
}

// ===========================================================================
// Tier 14: Mock deployer — advanced scenarios
// ===========================================================================

#[test]
fn t14_deploy_lock_cleaned_up_after_success() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s())
        .with_deploy_configs()
        .with_mock_deployer();

    let lock_path = env.project_dir.join(".evm-cloud-deploy.lock");

    // Deployer exits 0 (default) -> deploy succeeds.
    let output = env
        .cmd()
        .args(["deploy", "--auto-approve"])
        .output()
        .expect("run deploy --auto-approve with mock deployer");

    assert!(
        output.status.success(),
        "deploy with mock deployer (exit 0) should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // The deployer was invoked (proving the lock didn't block execution).
    let dlog = env.deployer_log();
    assert!(
        !dlog.is_empty(),
        "deployer should have been invoked, log is empty"
    );

    // After deploy completes, the lock file must be cleaned up (Drop guard).
    assert!(
        !lock_path.exists(),
        "lock file should be cleaned up after successful deploy"
    );
}

#[test]
fn t14_deploy_only_app_invokes_deployer_without_apply() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s())
        .with_deploy_configs()
        .with_mock_deployer();

    // --only app skips terraform plan/apply, but should still invoke the deployer.
    // Note: --only app is incompatible with --auto-approve (flag conflict).
    let _ = env
        .cmd()
        .args(["deploy", "--only", "app"])
        .output()
        .expect("run deploy --only app with mock deployer");

    // Terraform should NOT have plan or apply (infra phase skipped).
    let tf_log = env.terraform_log();
    assert!(
        !tf_log.iter().any(|l| l.contains("plan")),
        "deploy --only app should NOT call terraform plan, log: {tf_log:?}"
    );
    assert!(
        !tf_log.iter().any(|l| l.contains("apply")),
        "deploy --only app should NOT call terraform apply, log: {tf_log:?}"
    );

    // The deployer WAS invoked.
    let dlog = env.deployer_log();
    assert!(
        !dlog.is_empty(),
        "deployer should have been invoked for --only app, log is empty"
    );
}

#[test]
fn t14_deploy_nonzero_exit_propagates_as_failure() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s())
        .with_deploy_configs()
        .with_mock_deployer();

    // Configure mock deployer to exit with code 42.
    fs::write(env.state_dir.join("deployer_exit_code"), "42").unwrap();

    let output = env
        .cmd()
        .args(["deploy", "--auto-approve"])
        .output()
        .expect("run deploy --auto-approve with failing mock deployer");

    assert!(
        !output.status.success(),
        "deploy should fail when deployer exits non-zero"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("deployer") || stderr.contains("failed") || stderr.contains("exit"),
        "stderr should mention deployer failure, got: {stderr}"
    );
}

#[test]
fn t14_deploy_handoff_contains_expected_fields() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s())
        .with_deploy_configs()
        .with_mock_deployer();

    let output = env
        .cmd()
        .args(["deploy", "--auto-approve"])
        .output()
        .expect("run deploy --auto-approve with mock deployer");

    assert!(
        output.status.success(),
        "deploy should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Read the handoff copy saved by the mock deployer to TMPDIR.
    let handoff_copy = env.state_dir.join("deployer_handoff.json");
    assert!(
        handoff_copy.exists(),
        "deployer should have copied handoff to deployer_handoff.json"
    );
    let handoff_content =
        fs::read_to_string(&handoff_copy).expect("handoff copy should be readable");
    let handoff: serde_json::Value =
        serde_json::from_str(&handoff_content).expect("handoff should be valid JSON");

    // Verify expected fields.
    assert_eq!(
        handoff["version"], "v1",
        "handoff version should be v1, got: {}",
        handoff["version"]
    );
    assert_eq!(
        handoff["compute_engine"], "k3s",
        "handoff compute_engine should be k3s, got: {}",
        handoff["compute_engine"]
    );
    assert_eq!(
        handoff["project_name"], "test-project",
        "handoff project_name should be test-project, got: {}",
        handoff["project_name"]
    );
    assert!(
        handoff["services"]["rpc_proxy"].is_object(),
        "handoff should contain services.rpc_proxy, got: {}",
        handoff["services"]
    );
}

#[test]
fn t14_deploy_mock_deployer_receives_handoff_as_first_arg() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s())
        .with_deploy_configs()
        .with_mock_deployer();

    let output = env
        .cmd()
        .args(["deploy", "--auto-approve"])
        .output()
        .expect("run deploy --auto-approve with mock deployer");

    assert!(
        output.status.success(),
        "deploy should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let dlog = env.deployer_log();
    assert!(!dlog.is_empty(), "deployer log should not be empty");

    // First line should start with "deployer /" — confirming the handoff
    // path (an absolute path) was passed as the first argument.
    assert!(
        dlog[0].starts_with("deployer /"),
        "deployer log first line should start with 'deployer /' (handoff as first arg), got: {}",
        dlog[0]
    );
}

#[test]
fn t14_deploy_success_prints_no_recovery_hint() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s())
        .with_deploy_configs()
        .with_mock_deployer();

    // Mock deployer exits 0 (default) — full deploy succeeds.
    let output = env
        .cmd()
        .args(["deploy", "--auto-approve"])
        .output()
        .expect("run deploy --auto-approve with mock deployer");

    assert!(
        output.status.success(),
        "deploy should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);

    // On success, the recovery hint ("Retry deployer only") should NOT appear.
    assert!(
        !stderr.contains("only app"),
        "successful deploy should not show '--only app' recovery hint, stderr: {stderr}"
    );
    assert!(
        !stderr.contains("Retry deployer"),
        "successful deploy should not show 'Retry deployer' hint, stderr: {stderr}"
    );
}

// ===========================================================================
// Tier 15: Deploy pipeline gap closure
// ===========================================================================

#[test]
fn t15_deploy_timeout_kills_deployer() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s())
        .with_deploy_configs()
        .with_mock_deployer();

    // Write a slow deployer that sleeps 30s (overrides the normal mock)
    let slow_script = "#!/bin/bash\nsleep 30\nexit 0\n";
    let slow_path = env.state_dir.join("slow_deployer.sh");
    fs::write(&slow_path, slow_script).unwrap();
    fs::set_permissions(&slow_path, fs::Permissions::from_mode(0o755)).unwrap();

    let output = env
        .cmd()
        .env("EVM_CLOUD_DEPLOYER_OVERRIDE", &slow_path)
        .args(["deploy", "--auto-approve", "--deploy-timeout", "2"])
        .output()
        .expect("run deploy with timeout");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "deploy with slow deployer + 2s timeout should fail"
    );
    assert!(
        stderr.contains("timed out"),
        "should mention 'timed out', got stderr: {stderr}"
    );
}

#[test]
fn t15_deploy_config_bundle_created() {
    let env = TestEnv::new()
        .with_config(CONFIG_K3S)
        .with_handoff(handoff_k3s())
        .with_mock_deployer();

    // Don't call with_deploy_configs() — put config files at project root
    // (not inside config/) to trigger the config-bundle path in
    // ensure_config_dir(). Deliberately omit abis/ at the root so
    // config_dir_ready(project_root) returns false and the bundle is created.
    fs::write(
        env.project_dir.join("erpc.yaml"),
        "server:\n  httpPort: 4000\n",
    )
    .unwrap();
    fs::write(
        env.project_dir.join("rindexer.yaml"),
        "name: test\ncontracts: []\n",
    )
    .unwrap();

    let output = env
        .cmd()
        .args(["deploy", "--auto-approve"])
        .output()
        .expect("run deploy");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "deploy should succeed with bundled config, stderr: {stderr}, stdout: {stdout}"
    );

    // The config-bundle directory should have been created
    let bundle_dir = env.project_dir.join(".evm-cloud/config-bundle");
    assert!(
        bundle_dir.exists(),
        "config-bundle dir should be created at {}",
        bundle_dir.display()
    );

    // The deployer log should contain --config-dir pointing to the bundle
    let dlog = env.deployer_log();
    assert!(
        dlog.iter().any(|l| l.contains("--config-dir")),
        "deployer should receive --config-dir, log: {dlog:?}"
    );
}

#[test]
fn t15_compose_deploy_generates_env_file() {
    let env = TestEnv::new()
        .with_config(CONFIG_DOCKER_COMPOSE)
        .with_handoff(handoff_docker_compose())
        .with_deploy_configs()
        .with_mock_deployer();

    // Create secrets.auto.tfvars with clickhouse vars so generate_env_file
    // has data to write.
    fs::write(
        env.project_dir.join("secrets.auto.tfvars"),
        "indexer_clickhouse_url = \"http://ch:8123\"\nindexer_clickhouse_password = \"secret\"\n",
    )
    .unwrap();

    let output = env
        .cmd()
        .args(["deploy", "--auto-approve"])
        .output()
        .expect("run deploy");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "compose deploy should succeed, stderr: {stderr}"
    );

    // .env should be generated in the config dir
    let env_file = env.project_dir.join("config/.env");
    assert!(
        env_file.exists(),
        ".env should be generated at {}",
        env_file.display()
    );

    let content = fs::read_to_string(&env_file).unwrap();
    assert!(
        content.contains("CLICKHOUSE_URL=http://ch:8123"),
        ".env should contain CLICKHOUSE_URL, got: {content}"
    );
    assert!(
        content.contains("CLICKHOUSE_PASSWORD=secret"),
        ".env should contain CLICKHOUSE_PASSWORD, got: {content}"
    );

    // Deployer should have been invoked
    let dlog = env.deployer_log();
    assert!(!dlog.is_empty(), "deployer should have been invoked");
}

#[test]
fn t15_compose_deploy_injects_ssh_credentials() {
    let env = TestEnv::new()
        .with_config(CONFIG_DOCKER_COMPOSE)
        .with_handoff(handoff_docker_compose())
        .with_deploy_configs()
        .with_mock_deployer();

    // Create secrets.auto.tfvars with SSH vars
    fs::write(
        env.project_dir.join("secrets.auto.tfvars"),
        "ssh_private_key_path = \"/home/user/.ssh/id_rsa\"\nbare_metal_ssh_user = \"deploy\"\nbare_metal_ssh_port = \"2222\"\n",
    )
    .unwrap();

    let output = env
        .cmd()
        .args(["deploy", "--auto-approve"])
        .output()
        .expect("run deploy");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "compose deploy should succeed, stderr: {stderr}"
    );

    // Deployer should have received --ssh-key, --user, --port via auto-injection
    let dlog = env.deployer_log();
    assert!(!dlog.is_empty(), "deployer should have been invoked");

    let deployer_line = &dlog[0];
    assert!(
        deployer_line.contains("--ssh-key") && deployer_line.contains("/home/user/.ssh/id_rsa"),
        "deployer should receive --ssh-key, got: {deployer_line}"
    );
    assert!(
        deployer_line.contains("--user") && deployer_line.contains("deploy"),
        "deployer should receive --user, got: {deployer_line}"
    );
    assert!(
        deployer_line.contains("--port") && deployer_line.contains("2222"),
        "deployer should receive --port, got: {deployer_line}"
    );
}
