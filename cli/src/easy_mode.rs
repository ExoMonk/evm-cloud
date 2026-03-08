use std::path::{Path, PathBuf};

use crate::codegen::scaffold::{self, TfbackendResult};
use crate::codegen::tfvars;
use crate::codegen::ScaffoldResult;
use crate::config::loader;
use crate::config::schema::StateConfig;
use crate::error::Result;
use crate::output::{self, ColorMode};

pub(crate) fn prepare_workspace(
    project_root: &Path,
    color: ColorMode,
) -> Result<(PathBuf, ScaffoldResult)> {
    prepare_workspace_inner(project_root, Some(color))
}

pub(crate) fn prepare_workspace_quiet(
    project_root: &Path,
) -> Result<(PathBuf, ScaffoldResult)> {
    prepare_workspace_inner(project_root, None)
}

fn prepare_workspace_inner(
    project_root: &Path,
    color: Option<ColorMode>,
) -> Result<(PathBuf, ScaffoldResult)> {
    let config_path = project_root.join("evm-cloud.toml");
    let mut config = loader::load(&config_path)?;

    if let Some(ref mut state) = config.state {
        state.resolve_defaults(&config.project.name);
    }

    if let Some(c) = color {
        output::castle("Loaded evm-cloud.toml", c);
    }

    tfvars::generate_tfvars(&config, project_root)?;
    let main_tf_result = scaffold::generate_main_tf(&config, project_root)?;
    let tfbackend_result = scaffold::generate_tfbackend(&config, project_root)?;
    scaffold::generate_variables_tf(&config, project_root)?;
    scaffold::generate_outputs_tf(project_root)?;

    // Composite change detection:
    // - main.tf backend type change → BackendChanged (hard stop)
    // - tfbackend values changed → BackendChanged (needs -reconfigure)
    // - Both first-write → Written
    // - Both unchanged → Unchanged
    let scaffold_result = match (&main_tf_result, &tfbackend_result) {
        (ScaffoldResult::BackendChanged, _) => ScaffoldResult::BackendChanged,
        (_, TfbackendResult::Changed(_)) => ScaffoldResult::BackendChanged,
        (ScaffoldResult::Written, _) => ScaffoldResult::Written,
        (ScaffoldResult::Unchanged, TfbackendResult::Written(_)) => ScaffoldResult::Written,
        _ => ScaffoldResult::Unchanged,
    };

    if let Some(c) = color {
        output::success("Generated .evm-cloud terraform bridge files", c);
    }

    // State backend warnings — always print regardless of quiet mode (safety gates).
    if let Some(ref state) = config.state {
        emit_state_warnings(state, scaffold_result);
    }

    Ok((project_root.join(".evm-cloud"), scaffold_result))
}

/// Handle `BackendChanged` result: backup state, print warning, return error.
/// Callers that should hard-stop (apply, destroy, deploy) use this.
pub(crate) fn handle_backend_changed(project_root: &Path) -> crate::error::CliError {
    match scaffold::backup_state_file(project_root) {
        Ok(Some(backup)) => {
            eprintln!(
                "     ℹ State backup saved to: {}",
                backup.display()
            );
        }
        Ok(None) => {}
        Err(err) => {
            eprintln!("     ❌ Failed to backup state file: {err}");
            eprintln!("       Resolve this before proceeding with backend migration.");
        }
    }

    eprintln!();
    eprintln!("     ⚠ Backend configuration changed.");
    eprintln!("       Your state data is NOT automatically migrated.");
    eprintln!();
    eprintln!("       To reconfigure (start fresh, existing state in old backend remains):");
    eprintln!("         terraform -chdir=.evm-cloud init -reconfigure");
    eprintln!();
    eprintln!("       To migrate existing state to the new backend:");
    eprintln!("         terraform -chdir=.evm-cloud init -migrate-state");

    crate::error::CliError::BackendChanged
}

/// Handle `BackendChanged` for init: warn but don't error (init always runs terraform init).
/// Writes the new main.tf after warning so terraform init picks up the new backend.
pub(crate) fn warn_backend_changed(project_root: &Path) -> Result<()> {
    match scaffold::backup_state_file(project_root) {
        Ok(Some(backup)) => {
            eprintln!(
                "     ℹ State backup saved to: {}",
                backup.display()
            );
        }
        Ok(None) => {}
        Err(err) => {
            eprintln!("     ⚠ Failed to backup state file: {err}");
        }
    }

    eprintln!("     ⚠ Backend configuration changed. Will run terraform init -reconfigure.");
    eprintln!("       To migrate existing state instead, run manually:");
    eprintln!("         terraform -chdir=.evm-cloud init -migrate-state");

    // Commit the new main.tf and tfbackend so terraform init picks up the new backend config.
    let config_path = project_root.join("evm-cloud.toml");
    let mut config = loader::load(&config_path)?;
    if let Some(ref mut state) = config.state {
        state.resolve_defaults(&config.project.name);
    }
    scaffold::commit_main_tf(&config, project_root)?;
    scaffold::commit_tfbackend(&config, project_root)
}

fn emit_state_warnings(state: &StateConfig, scaffold_result: ScaffoldResult) {
    // Encrypt warning fires every run — security gate.
    if state.is_encrypt_disabled() {
        eprintln!(
            "     ⚠ State encryption is disabled. State files may contain sensitive outputs (API keys, passwords)."
        );
        eprintln!(
            "       Set `encrypt = true` in [state] to enable server-side encryption."
        );
    }

    // Bootstrap notes only on first write — avoid noise on every subsequent run.
    if scaffold_result != ScaffoldResult::Written {
        return;
    }

    match state {
        StateConfig::S3 { .. } => {
            eprintln!(
                "     ℹ Remote state configured (S3). Ensure the bucket and DynamoDB table exist before running terraform init."
            );
        }
        StateConfig::Gcs { .. } => {
            eprintln!(
                "     ℹ Remote state configured (GCS). Ensure the bucket exists before running terraform init."
            );
        }
    }
}
