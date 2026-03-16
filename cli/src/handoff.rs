use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::config::schema::ComputeEngine;
use crate::deployer::Action;
use crate::error::{CliError, Result};

/// Deserialize a value that may be a string or an integer into `Option<String>`.
/// Terraform outputs numeric values like port as integers in JSON.
fn deserialize_string_or_number<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(Value::Number(n)) => Ok(Some(n.to_string())),
        Some(other) => Err(serde::de::Error::custom(format!(
            "expected string or number, got {other}"
        ))),
    }
}

// NOTE: changes to this struct require updating cli/tests/e2e.rs handoff fixtures
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct WorkloadHandoff {
    pub(crate) version: String,
    pub(crate) mode: String,
    pub(crate) compute_engine: ComputeEngine,
    pub(crate) project_name: String,
    pub(crate) runtime: Runtime,
    pub(crate) services: HandoffServices,
    pub(crate) data: HandoffData,
    pub(crate) secrets: HandoffSecrets,
    pub(crate) ingress: HandoffIngress,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct HandoffData {
    #[serde(default)]
    pub(crate) backend: Option<String>,
    #[serde(default)]
    pub(crate) clickhouse: Option<ClickhouseData>,
    #[serde(default)]
    pub(crate) postgres: Option<PostgresData>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct ClickhouseData {
    #[serde(default)]
    pub(crate) url: Option<String>,
    #[serde(default)]
    pub(crate) password: Option<String>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct PostgresData {
    #[serde(default)]
    pub(crate) host: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_or_number")]
    pub(crate) port: Option<String>,
    #[serde(default)]
    pub(crate) db_name: Option<String>,
    #[serde(default)]
    pub(crate) url: Option<String>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct HandoffServices {
    #[serde(default)]
    pub(crate) rpc_proxy: Option<RpcProxyService>,
    #[serde(default)]
    pub(crate) monitoring: Option<MonitoringService>,
    #[serde(default)]
    pub(crate) indexer: Option<IndexerService>,
    #[serde(default)]
    pub(crate) custom_services: Option<Vec<CustomServiceEntry>>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct RpcProxyService {
    #[serde(default)]
    pub(crate) internal_url: Option<String>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct MonitoringService {
    #[serde(default)]
    pub(crate) grafana_hostname: Option<String>,
    #[serde(default)]
    pub(crate) grafana_admin_password_secret_name: Option<String>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct IndexerService {
    #[serde(default)]
    pub(crate) service_name: Option<String>,
    #[serde(default)]
    pub(crate) instances: Option<Vec<IndexerInstance>>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct IndexerInstance {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) config_key: Option<String>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct CustomServiceEntry {
    pub(crate) name: String,
    pub(crate) image: String,
    #[serde(default, deserialize_with = "deserialize_string_or_number")]
    pub(crate) port: Option<String>,
    #[serde(default)]
    pub(crate) health_path: Option<String>,
    #[serde(default)]
    pub(crate) replicas: Option<u32>,
    #[serde(default)]
    pub(crate) cpu_request: Option<String>,
    #[serde(default)]
    pub(crate) memory_request: Option<String>,
    #[serde(default)]
    pub(crate) cpu_limit: Option<String>,
    #[serde(default)]
    pub(crate) memory_limit: Option<String>,
    #[serde(default)]
    pub(crate) env: Option<HashMap<String, String>>,
    #[serde(default)]
    pub(crate) secret_env: Option<HashMap<String, String>>,
    #[serde(default)]
    pub(crate) ingress_hostname: Option<String>,
    #[serde(default)]
    pub(crate) ingress_path: Option<String>,
    #[serde(default)]
    pub(crate) node_role: Option<String>,
    #[serde(default)]
    pub(crate) tolerations: Option<Vec<TolerationEntry>>,
    #[serde(default)]
    pub(crate) enable_egress: Option<bool>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct TolerationEntry {
    pub(crate) key: String,
    #[serde(default)]
    pub(crate) operator: Option<String>,
    #[serde(default)]
    pub(crate) value: Option<String>,
    #[serde(default)]
    pub(crate) effect: Option<String>,
    #[serde(default)]
    pub(crate) toleration_seconds: Option<u64>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct HandoffSecrets {
    #[serde(default)]
    pub(crate) mode: Option<String>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct HandoffIngress {
    #[serde(default)]
    pub(crate) erpc_hostname: Option<String>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct Runtime {
    #[serde(default)]
    pub(crate) ec2: Option<Ec2Runtime>,
    #[serde(default)]
    pub(crate) k3s: Option<K3sRuntime>,
    #[serde(default)]
    pub(crate) bare_metal: Option<BareMetalRuntime>,
    #[serde(default)]
    pub(crate) eks: Option<EksRuntime>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct Ec2Runtime {
    #[serde(default)]
    pub(crate) public_ip: Option<String>,
    #[serde(default)]
    pub(crate) cloudwatch_log_group: Option<String>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct EksRuntime {
    #[serde(default)]
    pub(crate) cluster_name: Option<String>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct K3sRuntime {
    #[serde(default)]
    pub(crate) host_ip: Option<String>,
    #[serde(default)]
    pub(crate) kubeconfig_base64: Option<String>,
    #[serde(default)]
    pub(crate) node_name: Option<String>,
    #[serde(default)]
    pub(crate) worker_nodes: Vec<Value>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct BareMetalRuntime {
    #[serde(default)]
    pub(crate) host_address: Option<String>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

pub(crate) fn parse_handoff_value(value: Value) -> Result<WorkloadHandoff> {
    if !value.is_object() {
        return Err(CliError::HandoffInvalid {
            field: "workload_handoff".to_string(),
            details: "expected JSON object".to_string(),
        });
    }

    let parsed: WorkloadHandoff =
        serde_json::from_value(value).map_err(|err| CliError::HandoffInvalid {
            field: "workload_handoff".to_string(),
            details: err.to_string(),
        })?;

    if parsed.version != "v1" {
        return Err(CliError::HandoffVersionUnsupported {
            found: parsed.version.clone(),
            expected: "v1".to_string(),
        });
    }

    Ok(parsed)
}

pub(crate) fn parse_from_full_output(output: Value, module_name: &str) -> Result<WorkloadHandoff> {
    let module_value = output
        .get("module")
        .and_then(|m| m.get(module_name))
        .and_then(|m| m.get("workload_handoff"))
        .and_then(|m| m.get("value"))
        .cloned();

    if let Some(value) = module_value {
        return parse_handoff_value(value);
    }

    let root_value = output
        .get("workload_handoff")
        .and_then(|m| m.get("value"))
        .cloned();

    if let Some(value) = root_value {
        return parse_handoff_value(value);
    }

    Err(CliError::HandoffMissing {
        module: module_name.to_string(),
    })
}

/// Load a `WorkloadHandoff` from Terraform state.
///
/// Tries `terraform output -json workload_handoff` first, then falls back to
/// parsing the full output for a module-scoped handoff at `module_name`.
pub(crate) fn load_from_state(
    runner: &crate::terraform::TerraformRunner,
    terraform_dir: &std::path::Path,
    module_name: &str,
) -> Result<WorkloadHandoff> {
    match runner.output_named_json(terraform_dir, "workload_handoff") {
        Ok(value) => parse_handoff_value(value),
        Err(CliError::TerraformOutputMissing { .. }) => {
            let full_output = runner.output_json(terraform_dir)?;
            parse_from_full_output(full_output, module_name)
        }
        Err(err) => Err(err),
    }
}

/// Try to load a `WorkloadHandoff`, returning `None` instead of an error on failure.
pub(crate) fn try_load_from_state(
    runner: &crate::terraform::TerraformRunner,
    terraform_dir: &std::path::Path,
    module_name: &str,
) -> Option<WorkloadHandoff> {
    load_from_state(runner, terraform_dir, module_name).ok()
}

pub(crate) fn validate_for_action(
    handoff: &WorkloadHandoff,
    action: Action,
    passthrough_args: &[String],
) -> Result<()> {
    if handoff.project_name.trim().is_empty() {
        return Err(CliError::HandoffInvalid {
            field: "project_name".to_string(),
            details: "must not be empty".to_string(),
        });
    }

    if handoff.mode.trim().is_empty() {
        return Err(CliError::HandoffInvalid {
            field: "mode".to_string(),
            details: "must not be empty".to_string(),
        });
    }

    match handoff.compute_engine {
        ComputeEngine::K3s => {
            if handoff.mode != "external" {
                return Err(CliError::HandoffInvalid {
                    field: "mode".to_string(),
                    details: "k3s deploy requires mode=external".to_string(),
                });
            }

            let missing = handoff
                .runtime
                .k3s
                .as_ref()
                .and_then(|k| k.kubeconfig_base64.as_ref())
                .map(|v| v.trim().is_empty())
                .unwrap_or(true);

            if missing {
                return Err(CliError::HandoffInvalid {
                    field: "runtime.k3s.kubeconfig_base64".to_string(),
                    details: "required for k3s deployer".to_string(),
                });
            }
        }
        ComputeEngine::Ec2 | ComputeEngine::DockerCompose => {
            if matches!(action, Action::Deploy) {
                let has_runtime_host = handoff
                    .runtime
                    .ec2
                    .as_ref()
                    .and_then(|ec2| ec2.public_ip.as_ref())
                    .map(|v| !v.trim().is_empty())
                    .unwrap_or(false)
                    || handoff
                        .runtime
                        .bare_metal
                        .as_ref()
                        .and_then(|bm| bm.host_address.as_ref())
                        .map(|v| !v.trim().is_empty())
                        .unwrap_or(false);

                let has_override = passthrough_args
                    .iter()
                    .any(|arg| arg == "--host" || arg.starts_with("--host="));

                if !has_runtime_host && !has_override {
                    return Err(CliError::HandoffInvalid {
                        field: "runtime.ec2.public_ip|runtime.bare_metal.host_address|--host"
                            .to_string(),
                        details: "compose deploy requires resolvable host".to_string(),
                    });
                }
            }
        }
        ComputeEngine::Eks => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{parse_from_full_output, parse_handoff_value};

    fn sample_handoff() -> serde_json::Value {
        json!({
            "version": "v1",
            "mode": "external",
            "compute_engine": "k3s",
            "project_name": "demo",
            "runtime": {
                "ec2": null,
                "eks": null,
                "bare_metal": null,
                "k3s": {
                    "kubeconfig_base64": "abc123"
                }
            },
            "services": {},
            "data": {},
            "secrets": {},
            "ingress": {},
            "artifacts": {
                "config_channel": "helm"
            }
        })
    }

    #[test]
    fn parses_named_output_shape() {
        let handoff = parse_handoff_value(sample_handoff()).expect("must parse");
        assert_eq!(handoff.version, "v1");
        assert!(handoff.extra.contains_key("artifacts"));
    }

    #[test]
    fn parses_module_key_from_full_output() {
        let full = json!({
            "module": {
                "custom_stack": {
                    "workload_handoff": {
                        "value": sample_handoff()
                    }
                }
            }
        });

        let handoff =
            parse_from_full_output(full, "custom_stack").expect("must parse from module path");
        assert_eq!(
            handoff.compute_engine,
            crate::config::schema::ComputeEngine::K3s
        );
    }

    #[test]
    fn parses_custom_services() {
        let mut value = sample_handoff();
        value["services"]["custom_services"] = json!([{
            "name": "my-api",
            "image": "ghcr.io/example/api:v1",
            "port": 3000,
            "health_path": "/health",
            "replicas": 1,
            "env": { "FOO": "bar" },
            "secret_env": { "API_KEY": "secret" },
            "node_role": "worker",
            "tolerations": [{ "key": "dedicated", "value": "api", "effect": "NoSchedule" }],
            "enable_egress": false
        }]);

        let handoff = parse_handoff_value(value).expect("must parse");
        let custom = handoff
            .services
            .custom_services
            .expect("custom_services must be Some");
        assert_eq!(custom.len(), 1);
        assert_eq!(custom[0].name, "my-api");
        assert_eq!(custom[0].image, "ghcr.io/example/api:v1");
        assert_eq!(custom[0].port.as_deref(), Some("3000"));
        assert_eq!(custom[0].health_path.as_deref(), Some("/health"));
        assert_eq!(custom[0].replicas, Some(1));
        assert_eq!(custom[0].enable_egress, Some(false));
        assert_eq!(
            custom[0].env.as_ref().and_then(|m| m.get("FOO")).map(|s| s.as_str()),
            Some("bar")
        );
        assert_eq!(custom[0].node_role.as_deref(), Some("worker"));
        let tolerations = custom[0].tolerations.as_ref().expect("tolerations must be Some");
        assert_eq!(tolerations.len(), 1);
        assert_eq!(tolerations[0].key, "dedicated");
        assert_eq!(tolerations[0].effect.as_deref(), Some("NoSchedule"));
    }

    #[test]
    fn parses_empty_custom_services() {
        let mut value = sample_handoff();
        value["services"]["custom_services"] = json!([]);

        let handoff = parse_handoff_value(value).expect("must parse");
        let custom = handoff.services.custom_services.expect("must be Some");
        assert!(custom.is_empty());
    }

    #[test]
    fn parses_null_custom_services() {
        let mut value = sample_handoff();
        value["services"]["custom_services"] = json!(null);
        let handoff = parse_handoff_value(value).expect("must parse");
        assert!(handoff.services.custom_services.is_none());
    }

    #[test]
    fn parses_absent_custom_services() {
        let value = sample_handoff();
        let handoff = parse_handoff_value(value).expect("must parse");
        assert!(handoff.services.custom_services.is_none());
    }

    #[test]
    fn parses_indexer_instances() {
        let mut value = sample_handoff();
        value["services"]["indexer"] = json!({
            "service_name": "demo-indexer",
            "instances": [
                {"name": "indexer", "config_key": "default"},
                {"name": "backfill", "config_key": "backfill"}
            ]
        });

        let handoff = parse_handoff_value(value).expect("must parse");
        let indexer = handoff.services.indexer.expect("indexer must be Some");
        assert_eq!(indexer.service_name.as_deref(), Some("demo-indexer"));
        let instances = indexer.instances.expect("instances must be Some");
        assert_eq!(instances.len(), 2);
        assert_eq!(instances[0].name, "indexer");
        assert_eq!(instances[0].config_key.as_deref(), Some("default"));
        assert_eq!(instances[1].name, "backfill");
    }

    #[test]
    fn parses_indexer_without_instances() {
        let mut value = sample_handoff();
        value["services"]["indexer"] = json!({
            "service_name": "demo-indexer"
        });

        let handoff = parse_handoff_value(value).expect("must parse");
        let indexer = handoff.services.indexer.expect("indexer must be Some");
        assert_eq!(indexer.service_name.as_deref(), Some("demo-indexer"));
        assert!(indexer.instances.is_none());
    }

    #[test]
    fn parses_absent_indexer() {
        let value = sample_handoff();
        let handoff = parse_handoff_value(value).expect("must parse");
        assert!(handoff.services.indexer.is_none());
    }

    #[test]
    fn rejects_unsupported_version() {
        let mut value = sample_handoff();
        value["version"] = json!("v2");

        let err = parse_handoff_value(value).expect_err("must fail");
        let rendered = err.to_string();
        assert!(rendered.contains("unsupported handoff version"));
    }
}
