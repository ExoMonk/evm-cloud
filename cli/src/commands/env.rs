use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};

use crate::codegen;
use crate::config::loader;
use crate::config::schema::StateConfig;
use crate::env::{resolve_env_state, validate_env_name};
use crate::error::{CliError, Result};
use crate::output::{self, ColorMode};

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

#[derive(Subcommand)]
pub(crate) enum EnvCommand {
    /// Create a new environment
    Add(AddArgs),
    /// List all environments
    List(ListArgs),
    /// Remove an environment directory (does NOT destroy infrastructure)
    Remove(RemoveArgs),
}

#[derive(Args)]
pub(crate) struct AddArgs {
    /// Environment name (alphanumeric + hyphens, 1-32 chars)
    name: String,

    #[arg(short = 'd', long, default_value = ".")]
    dir: PathBuf,

    /// Copy tfvars from an existing environment
    #[arg(long)]
    copy_from: Option<String>,

    /// Also copy secrets.auto.tfvars from the source environment
    #[arg(long)]
    include_secrets: bool,

    /// Skip confirmation prompts
    #[arg(long, short = 'y')]
    yes: bool,
}

#[derive(Args)]
pub(crate) struct ListArgs {
    #[arg(short = 'd', long, default_value = ".")]
    dir: PathBuf,
}

#[derive(Args)]
pub(crate) struct RemoveArgs {
    /// Environment name to remove
    name: String,

    #[arg(short = 'd', long, default_value = ".")]
    dir: PathBuf,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    yes: bool,
}

// ---------------------------------------------------------------------------
// Dispatcher
// ---------------------------------------------------------------------------

pub(crate) fn run(cmd: EnvCommand, color: ColorMode) -> Result<()> {
    match cmd {
        EnvCommand::Add(args) => run_add(args, color),
        EnvCommand::List(args) => run_list(args, color),
        EnvCommand::Remove(args) => run_remove(args, color),
    }
}

// ---------------------------------------------------------------------------
// env add
// ---------------------------------------------------------------------------

fn run_add(args: AddArgs, color: ColorMode) -> Result<()> {
    validate_env_name(&args.name)?;

    let project_root = fs::canonicalize(&args.dir).map_err(|source| CliError::Io {
        source,
        path: args.dir.clone(),
    })?;

    let config_path = project_root.join("evm-cloud.toml");
    let mut config = loader::load(&config_path)?;

    let project_name = config.project.name.clone();

    // Resolve state defaults so key/prefix are populated.
    if let Some(ref mut state) = config.state {
        state.resolve_defaults(&project_name);
    }

    let base_state = config
        .state
        .as_ref()
        .ok_or_else(|| CliError::ConfigValidation {
            field: "state".into(),
            message:
                "multi-env requires a [state] section in evm-cloud.toml for remote state backend"
                    .into(),
        })?;

    let envs_dir = project_root.join("envs");
    let is_easy_mode = project_root.join(".evm-cloud").is_dir();

    // -------------------------------------------------------------------
    // First-time migration: envs/ doesn't exist yet
    // -------------------------------------------------------------------
    if !envs_dir.is_dir() {
        // Phase A: Validate preconditions
        let default_env_name = if args.yes {
            "default".to_string()
        } else if std::io::stdin().is_terminal() {
            prompt_default_env_name(color)?
        } else {
            "default".to_string()
        };
        validate_env_name(&default_env_name)?;

        let single_env = default_env_name == args.name;

        // Phase B: Create directories and migrate files.
        // If any step fails, clean up the entire envs/ directory to avoid
        // leaving a partially-migrated state that blocks future attempts.
        let migration_result = if single_env {
            // Single-env conversion: just move existing files into envs/<name>/
            migrate_single_env(
                &envs_dir,
                &args.name,
                &project_root,
                &project_name,
                base_state,
                is_easy_mode,
            )
        } else {
            migrate_to_multi_env(
                &envs_dir,
                &default_env_name,
                &args.name,
                &project_root,
                &project_name,
                base_state,
                is_easy_mode,
                args.include_secrets,
                args.copy_from.is_some(),
            )
        };

        match migration_result {
            Ok(()) => {}
            Err(err) => {
                // Roll back: remove the envs/ directory we just created.
                let _ = fs::remove_dir_all(&envs_dir);
                return Err(err);
            }
        }

        // Remove old tfbackend from original location (only after migration succeeded).
        if let Err(e) = remove_old_tfbackend(&project_root, &project_name, base_state, is_easy_mode)
        {
            output::warn(&format!("Failed to remove old tfbackend: {e}"), color);
        }

        if single_env {
            output::success(
                &format!("Converted existing project to env envs/{}/", args.name),
                color,
            );
        } else {
            output::success(
                &format!(
                    "Migrated existing config to envs/{default_env_name}/ and created envs/{name}/",
                    name = args.name
                ),
                color,
            );
        }

        eprintln!();
        eprintln!("     Note: Your existing Terraform state key has NOT been migrated.");
        let migrated_env = if single_env {
            &args.name
        } else {
            &default_env_name
        };
        eprintln!("     The env now uses key: {project_name}/{migrated_env}/terraform.tfstate");
        eprintln!(
            "     You may need to run `terraform init -migrate-state` in envs/{migrated_env}/"
        );
    } else {
        // -------------------------------------------------------------------
        // envs/ already exists — just create new env
        // -------------------------------------------------------------------
        let new_dir = envs_dir.join(&args.name);
        if new_dir.exists() {
            return Err(CliError::ConfigValidation {
                field: "env".into(),
                message: format!("environment \"{}\" already exists", args.name),
            });
        }
        fs::create_dir_all(&new_dir).map_err(|source| CliError::Io {
            source,
            path: new_dir.clone(),
        })?;

        // Generate tfbackend with namespaced key.
        let new_state = resolve_env_state(base_state, &project_name, &args.name);
        let backend_file = new_dir.join(new_state.tfbackend_filename(&project_name));
        codegen::write_atomic(&backend_file, &new_state.render_tfbackend())?;

        output::success(&format!("Created environment envs/{}/", args.name), color);
    }

    // Handle --copy-from (overrides the default copy in migration path).
    if let Some(ref source_env) = args.copy_from {
        let source_dir = envs_dir.join(source_env);
        if !source_dir.is_dir() {
            return Err(CliError::ConfigValidation {
                field: "env".into(),
                message: format!("source environment \"{source_env}\" not found in envs/"),
            });
        }
        let target_dir = envs_dir.join(&args.name);
        copy_tfvars_between_envs(&source_dir, &target_dir, args.include_secrets)?;
        output::info(
            &format!(
                "Copied tfvars from envs/{source_env}/ to envs/{}/",
                args.name
            ),
            color,
        );
    }

    // Generate skeleton terraform.auto.tfvars if it doesn't exist yet.
    let target_dir = envs_dir.join(&args.name);
    let tfvars_path = target_dir.join("terraform.auto.tfvars");
    if !tfvars_path.exists() {
        codegen::write_atomic(
            &tfvars_path,
            "# Environment-specific variable overrides\n# See evm-cloud.toml for base configuration\n",
        )?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// env list
// ---------------------------------------------------------------------------

fn run_list(args: ListArgs, color: ColorMode) -> Result<()> {
    let project_root = fs::canonicalize(&args.dir).map_err(|source| CliError::Io {
        source,
        path: args.dir.clone(),
    })?;

    let envs_dir = project_root.join("envs");
    if !envs_dir.is_dir() {
        output::info(
            "No environments configured. Use `evm-cloud env add <name>` to create one.",
            color,
        );
        return Ok(());
    }

    let mut envs: Vec<(String, bool)> = Vec::new();

    let entries = fs::read_dir(&envs_dir).map_err(|source| CliError::Io {
        source,
        path: envs_dir.clone(),
    })?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();

        // Check if terraform has been initialized in this env.
        let initialized = path.join(".terraform").is_dir();
        envs.push((name, initialized));
    }

    envs.sort_by(|a, b| a.0.cmp(&b.0));

    if envs.is_empty() {
        output::info(
            "envs/ directory exists but contains no environments.",
            color,
        );
        return Ok(());
    }

    output::info("Environments:", color);
    eprintln!();
    eprintln!("  {:<24} {:<10} PATH", "NAME", "INIT");
    eprintln!("  {:<24} {:<10} ----", "----", "----");
    for (name, initialized) in &envs {
        let init_str = if *initialized { "yes" } else { "no" };
        let rel_path = format!("envs/{name}/");
        eprintln!("  {:<24} {:<10} {}", name, init_str, rel_path);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// env remove
// ---------------------------------------------------------------------------

fn run_remove(args: RemoveArgs, color: ColorMode) -> Result<()> {
    validate_env_name(&args.name)?;

    let project_root = fs::canonicalize(&args.dir).map_err(|source| CliError::Io {
        source,
        path: args.dir.clone(),
    })?;

    let env_dir = project_root.join("envs").join(&args.name);
    if !env_dir.is_dir() {
        return Err(CliError::ConfigValidation {
            field: "env".into(),
            message: format!("environment \"{}\" not found in envs/", args.name),
        });
    }

    // Check for active deploy lock on this env.
    let lock_path = project_root.join(format!(".evm-cloud-deploy-{}.lock", args.name));
    if lock_path.exists() {
        return Err(CliError::ConfigValidation {
            field: "env".into(),
            message: format!(
                "environment \"{}\" has an active deploy lock at {}. Wait for it to complete or remove the lock file manually.",
                args.name,
                lock_path.display()
            ),
        });
    }

    // Warn about infrastructure.
    output::warn(
        &format!(
            "This will delete the local envs/{name}/ directory. It does NOT destroy deployed infrastructure.",
            name = args.name
        ),
        color,
    );
    eprintln!(
        "     If you have deployed resources, run `evm-cloud destroy --env {name}` first.",
        name = args.name
    );

    if !args.yes {
        if !std::io::stdin().is_terminal() {
            return Err(CliError::ConfigValidation {
                field: "env".into(),
                message: "non-interactive mode requires --yes to confirm removal".into(),
            });
        }
        if !confirm_removal(&args.name, color)? {
            output::info("Aborted.", color);
            return Ok(());
        }
    }

    fs::remove_dir_all(&env_dir).map_err(|source| CliError::Io {
        source,
        path: env_dir,
    })?;

    output::success(&format!("Removed environment envs/{}/", args.name), color);

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn prompt_default_env_name(color: ColorMode) -> Result<String> {
    output::info(
        "First env migration: what should the EXISTING environment be called?",
        color,
    );
    eprint!("     Environment name [default]: ");
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .map_err(|e| CliError::PromptFailed(e.to_string()))?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        Ok("default".to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

fn confirm_removal(name: &str, _color: ColorMode) -> Result<bool> {
    eprint!("     Remove envs/{name}/? [y/N]: ");
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .map_err(|e| CliError::PromptFailed(e.to_string()))?;
    Ok(matches!(input.trim(), "y" | "Y" | "yes" | "YES"))
}

/// Perform the first-time migration from single-env to multi-env layout.
/// Creates envs/<default>/ and envs/<new>/, copies files, generates backend configs.
/// Caller is responsible for cleanup on failure (remove envs/ dir).
#[allow(clippy::too_many_arguments)]
fn migrate_to_multi_env(
    envs_dir: &Path,
    default_env_name: &str,
    new_env_name: &str,
    project_root: &Path,
    project_name: &str,
    base_state: &StateConfig,
    is_easy_mode: bool,
    include_secrets: bool,
    has_copy_from: bool,
) -> Result<()> {
    let default_dir = envs_dir.join(default_env_name);
    let new_dir = envs_dir.join(new_env_name);

    fs::create_dir_all(&default_dir).map_err(|source| CliError::Io {
        source,
        path: default_dir.clone(),
    })?;
    fs::create_dir_all(&new_dir).map_err(|source| CliError::Io {
        source,
        path: new_dir.clone(),
    })?;

    // Copy existing tfbackend + tfvars into default env dir.
    copy_existing_env_files(project_root, &default_dir, is_easy_mode)?;

    // Generate tfbackend for default env (with namespaced key).
    let default_state = resolve_env_state(base_state, project_name, default_env_name);
    let default_backend_file = default_dir.join(default_state.tfbackend_filename(project_name));
    codegen::write_atomic(&default_backend_file, &default_state.render_tfbackend())?;

    // Generate tfbackend for new env.
    let new_state = resolve_env_state(base_state, project_name, new_env_name);
    let new_backend_file = new_dir.join(new_state.tfbackend_filename(project_name));
    codegen::write_atomic(&new_backend_file, &new_state.render_tfbackend())?;

    // Copy tfvars from default to new if --copy-from not specified.
    if !has_copy_from {
        copy_tfvars_between_envs(&default_dir, &new_dir, include_secrets)?;
    }

    Ok(())
}

/// Perform a single-env conversion: move existing files into envs/<name>/ without
/// creating a second environment. Used when the user names their existing env the
/// same as the new env in `env add`.
fn migrate_single_env(
    envs_dir: &Path,
    env_name: &str,
    project_root: &Path,
    project_name: &str,
    base_state: &StateConfig,
    is_easy_mode: bool,
) -> Result<()> {
    let env_dir = envs_dir.join(env_name);
    fs::create_dir_all(&env_dir).map_err(|source| CliError::Io {
        source,
        path: env_dir.clone(),
    })?;

    // Copy existing tfbackend + tfvars into the env dir.
    copy_existing_env_files(project_root, &env_dir, is_easy_mode)?;

    // Generate tfbackend with namespaced key.
    let env_state = resolve_env_state(base_state, project_name, env_name);
    let backend_file = env_dir.join(env_state.tfbackend_filename(project_name));
    codegen::write_atomic(&backend_file, &env_state.render_tfbackend())?;

    Ok(())
}

/// Copy tfbackend and tfvars files from the original project location into an env dir.
fn copy_existing_env_files(project_root: &Path, env_dir: &Path, is_easy_mode: bool) -> Result<()> {
    let source_dir = if is_easy_mode {
        project_root.join(".evm-cloud")
    } else {
        project_root.to_path_buf()
    };

    // Copy any .tfbackend files.
    if let Ok(entries) = fs::read_dir(&source_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("tfbackend") {
                let dest = env_dir.join(entry.file_name());
                fs::copy(&path, &dest).map_err(|source| CliError::Io {
                    source,
                    path: path.clone(),
                })?;
            }
        }
    }

    // Copy secrets.auto.tfvars if it exists.
    let secrets_src = source_dir.join("secrets.auto.tfvars");
    if secrets_src.exists() {
        let secrets_dest = env_dir.join("secrets.auto.tfvars");
        fs::copy(&secrets_src, &secrets_dest).map_err(|source| CliError::Io {
            source,
            path: secrets_src,
        })?;
    }

    // Copy terraform.auto.tfvars.json if it exists (easy mode).
    if is_easy_mode {
        let tfvars_src = source_dir.join("terraform.auto.tfvars.json");
        if tfvars_src.exists() {
            let tfvars_dest = env_dir.join("terraform.auto.tfvars.json");
            fs::copy(&tfvars_src, &tfvars_dest).map_err(|source| CliError::Io {
                source,
                path: tfvars_src,
            })?;
        }
    }

    Ok(())
}

/// Copy tfvars (and optionally secrets) from one env dir to another.
fn copy_tfvars_between_envs(source: &Path, target: &Path, include_secrets: bool) -> Result<()> {
    // Copy terraform.auto.tfvars.json
    let tfvars_json = source.join("terraform.auto.tfvars.json");
    if tfvars_json.exists() {
        fs::copy(&tfvars_json, target.join("terraform.auto.tfvars.json")).map_err(
            |source_err| CliError::Io {
                source: source_err,
                path: tfvars_json.clone(),
            },
        )?;
    }

    // Copy terraform.auto.tfvars
    let tfvars = source.join("terraform.auto.tfvars");
    if tfvars.exists() {
        fs::copy(&tfvars, target.join("terraform.auto.tfvars")).map_err(|source_err| {
            CliError::Io {
                source: source_err,
                path: tfvars.clone(),
            }
        })?;
    }

    if include_secrets {
        let secrets = source.join("secrets.auto.tfvars");
        if secrets.exists() {
            fs::copy(&secrets, target.join("secrets.auto.tfvars")).map_err(|source_err| {
                CliError::Io {
                    source: source_err,
                    path: secrets.clone(),
                }
            })?;
        }
    }

    Ok(())
}

/// Remove old tfbackend from original location after migration.
fn remove_old_tfbackend(
    project_root: &Path,
    project_name: &str,
    state: &StateConfig,
    is_easy_mode: bool,
) -> Result<()> {
    let source_dir = if is_easy_mode {
        project_root.join(".evm-cloud")
    } else {
        project_root.to_path_buf()
    };

    let filename = state.tfbackend_filename(project_name);
    let old_path = source_dir.join(&filename);
    if old_path.exists() {
        fs::remove_file(&old_path).map_err(|source| CliError::Io {
            source,
            path: old_path,
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_env_state_s3_namespaces_key() {
        let base: StateConfig = toml::from_str(
            r#"
backend = "s3"
bucket = "my-bucket"
dynamodb_table = "my-lock"
region = "us-east-1"
key = "myproject/terraform.tfstate"
"#,
        )
        .expect("parse");
        let resolved = resolve_env_state(&base, "myproject", "staging");
        match resolved {
            StateConfig::S3 { key, .. } => {
                assert_eq!(key.as_deref(), Some("myproject/staging/terraform.tfstate"));
            }
            _ => panic!("expected S3"),
        }
    }

    #[test]
    fn resolve_env_state_gcs_namespaces_prefix() {
        let base: StateConfig = toml::from_str(
            r#"
backend = "gcs"
bucket = "my-bucket"
region = "us-central1"
prefix = "myproject"
"#,
        )
        .expect("parse");
        let resolved = resolve_env_state(&base, "myproject", "production");
        match resolved {
            StateConfig::Gcs { prefix, .. } => {
                assert_eq!(prefix.as_deref(), Some("myproject/production"));
            }
            _ => panic!("expected Gcs"),
        }
    }
}
