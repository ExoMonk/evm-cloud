use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::deployer::Action;
use crate::error::{CliError, Result};

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct WorkloadHandoff {
    pub(crate) version: String,
    pub(crate) mode: String,
    pub(crate) compute_engine: String,
    pub(crate) project_name: String,
    pub(crate) runtime: Runtime,
    pub(crate) services: Value,
    pub(crate) data: Value,
    pub(crate) secrets: Value,
    pub(crate) ingress: Value,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct Runtime {
    pub(crate) ec2: Option<Ec2Runtime>,
    pub(crate) k3s: Option<K3sRuntime>,
    pub(crate) bare_metal: Option<BareMetalRuntime>,
    pub(crate) eks: Option<EksRuntime>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct Ec2Runtime {
    pub(crate) public_ip: Option<String>,
    pub(crate) cloudwatch_log_group: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct EksRuntime {
    pub(crate) cluster_name: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct K3sRuntime {
    pub(crate) host_ip: Option<String>,
    pub(crate) kubeconfig_base64: Option<String>,
    pub(crate) node_name: Option<String>,
    #[serde(default)]
    pub(crate) worker_nodes: Vec<Value>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct BareMetalRuntime {
    pub(crate) host_address: Option<String>,
}

pub(crate) fn parse_handoff_value(value: Value) -> Result<WorkloadHandoff> {
    if !value.is_object() {
        return Err(CliError::HandoffInvalid {
            field: "workload_handoff".to_string(),
            details: "expected JSON object".to_string(),
        });
    }

    let parsed: WorkloadHandoff = serde_json::from_value(value).map_err(|err| CliError::HandoffInvalid {
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

pub(crate) fn validate_for_action(handoff: &WorkloadHandoff, action: Action, passthrough_args: &[String]) -> Result<()> {
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

    match handoff.compute_engine.as_str() {
        "k3s" => {
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
        "ec2" | "docker_compose" => {
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
                        field: "runtime.ec2.public_ip|runtime.bare_metal.host_address|--host".to_string(),
                        details: "compose deploy requires resolvable host".to_string(),
                    });
                }
            }
        }
        "eks" => {}
        other => {
            return Err(CliError::DeployerUnsupportedEngine {
                compute_engine: other.to_string(),
            })
        }
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

        let handoff = parse_from_full_output(full, "custom_stack").expect("must parse from module path");
        assert_eq!(handoff.compute_engine, "k3s");
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
