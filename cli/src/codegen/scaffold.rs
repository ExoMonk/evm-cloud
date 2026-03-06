use std::path::Path;

use crate::codegen::manifest::{GenerationMode, ResolvedConfig};
use crate::codegen::write_atomic;
use crate::config::schema::EvmCloudConfig;
use crate::error::Result;

const GENERATED_DIR: &str = ".evm-cloud";

pub(crate) fn generate_main_tf(config: &EvmCloudConfig, project_root: &Path) -> Result<()> {
    let module_source = crate::module_source();
    let resolved = ResolvedConfig::from_evm_config(config);
    let module_body =
        super::manifest::render_module_args(&resolved, GenerationMode::Easy, &module_source);

    let v = crate::terraform::REQUIRED_VERSION_CONSTRAINT;
    let contents = format!(
        "terraform {{\n  required_version = \"{v}\"\n}}\n\nmodule \"evm_cloud\" {{\n{module_body}\n}}\n"
    );

    let path = project_root.join(GENERATED_DIR).join("main.tf");
    write_atomic(&path, &contents)
}

pub(crate) fn generate_variables_tf(config: &EvmCloudConfig, project_root: &Path) -> Result<()> {
    let resolved = ResolvedConfig::from_evm_config(config);
    let contents = super::manifest::render_variables_tf(&resolved, GenerationMode::Easy);

    let path = project_root.join(GENERATED_DIR).join("variables.tf");
    write_atomic(&path, &contents)
}

pub(crate) fn generate_outputs_tf(project_root: &Path) -> Result<()> {
    let contents = r#"output "workload_handoff" {
  value     = module.evm_cloud.workload_handoff
  sensitive = true
}
"#;

    let path = project_root.join(GENERATED_DIR).join("outputs.tf");
    write_atomic(&path, contents)
}
