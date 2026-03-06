use std::path::{Path, PathBuf};

use crate::codegen::scaffold;
use crate::codegen::tfvars;
use crate::config::loader;
use crate::error::Result;
use crate::output::{self, ColorMode};

pub(crate) fn prepare_workspace(project_root: &Path, color: ColorMode) -> Result<PathBuf> {
    prepare_workspace_inner(project_root, Some(color))
}

pub(crate) fn prepare_workspace_quiet(project_root: &Path) -> Result<PathBuf> {
    prepare_workspace_inner(project_root, None)
}

fn prepare_workspace_inner(project_root: &Path, color: Option<ColorMode>) -> Result<PathBuf> {
    let config_path = project_root.join("evm-cloud.toml");
    let config = loader::load(&config_path)?;

    if let Some(c) = color {
        output::castle("Loaded evm-cloud.toml", c);
    }

    tfvars::generate_tfvars(&config, project_root)?;
    scaffold::generate_main_tf(&config, project_root)?;
    scaffold::generate_variables_tf(&config, project_root)?;
    scaffold::generate_outputs_tf(project_root)?;

    if let Some(c) = color {
        output::success("Generated .evm-cloud terraform bridge files", c);
    }

    Ok(project_root.join(".evm-cloud"))
}
