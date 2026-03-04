use std::path::{Path, PathBuf};

use crate::codegen::scaffold;
use crate::codegen::tfvars;
use crate::config::loader;
use crate::error::Result;
use crate::output::{self, ColorMode};

pub(crate) fn prepare_workspace(project_root: &Path, color: ColorMode) -> Result<PathBuf> {
    let config_path = project_root.join("evm-cloud.toml");
    let config = loader::load(&config_path)?;

    output::info("Loaded evm-cloud.toml (schema_version = 1)", color);
    tfvars::generate_tfvars(&config, project_root)?;
    scaffold::generate_main_tf(&config, project_root)?;
    scaffold::generate_outputs_tf(project_root)?;
    output::info("Generated .evm-cloud terraform bridge files", color);

    Ok(project_root.join(".evm-cloud"))
}
