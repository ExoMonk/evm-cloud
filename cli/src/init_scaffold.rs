use std::fs;
use std::path::{Path, PathBuf};

use crate::config::loader;
use crate::error::{CliError, Result};
use crate::init_answers::{IndexerConfigStrategy, InitAnswers, InitMode};
use crate::init_templates;
use crate::output::{self, ColorMode};

const MODE_MARKER_REL: &str = ".evm-cloud/mode";

pub(crate) fn scaffold_project(project_root: &Path, answers: &InitAnswers, force: bool, color: ColorMode) -> Result<()> {
    fs::create_dir_all(project_root).map_err(|source| CliError::Io {
        source,
        path: project_root.to_path_buf(),
    })?;

    let managed_files = managed_files(answers);

    for rel in &managed_files {
        let path = project_root.join(rel);
        if path.exists() && !force {
            return Err(CliError::InitFileExists { path });
        }
    }

    if force {
        backup_existing_managed(project_root, &managed_files)?;
    }

    let toml_path = project_root.join("evm-cloud.toml");
    write_atomic(&toml_path, &init_templates::render_evm_cloud_toml(answers))?;

    match &answers.indexer_config {
        IndexerConfigStrategy::Generate => {
            write_atomic(
                &project_root.join("config").join("rindexer.yaml"),
                &init_templates::render_rindexer_yaml(answers),
            )?;
        }
        IndexerConfigStrategy::Existing(path) => {
            let resolved = if path.is_absolute() {
                path.clone()
            } else {
                project_root.join(path)
            };
            if !resolved.exists() {
                return Err(CliError::ConfigValidation {
                    field: "indexer_config.path".to_string(),
                    message: format!("path does not exist: {}", resolved.display()),
                });
            }
        }
    }

    if answers.generate_erpc_config {
        write_atomic(
            &project_root.join("config").join("erpc.yaml"),
            &init_templates::render_erpc_yaml(answers),
        )?;
    }

    write_atomic(
        &project_root
            .join("config")
            .join("abis")
            .join("ERC20.json"),
        init_templates::erc20_abi_json(),
    )?;

    let secrets_content = init_templates::render_secrets_example(answers);
    write_atomic(
        &project_root.join("secrets.auto.tfvars.example"),
        &secrets_content,
    )?;

    match answers.mode {
        InitMode::Easy => {
            // Easy mode: secrets live inside .evm-cloud/ (terraform runs there)
            write_atomic(
                &project_root.join(".evm-cloud").join("secrets.auto.tfvars"),
                &secrets_content,
            )?;
        }
        InitMode::Power => {
            // Power mode: secrets live at root (terraform runs there)
            write_atomic(
                &project_root.join("secrets.auto.tfvars"),
                &secrets_content,
            )?;
        }
    }

    if matches!(answers.mode, InitMode::Power) {
        write_atomic(&project_root.join("versions.tf"), &init_templates::render_versions_tf())?;
        write_atomic(&project_root.join("main.tf"), &init_templates::render_main_tf(answers))?;
        write_atomic(&project_root.join("variables.tf"), &init_templates::render_variables_tf(answers))?;
        write_atomic(&project_root.join("outputs.tf"), &init_templates::render_outputs_tf())?;
    }

    write_atomic(
        &project_root.join(MODE_MARKER_REL),
        &format!("{}\n", answers.mode.as_str()),
    )?;

    update_gitignore(project_root, answers.mode)?;
    let _ = loader::load(&toml_path)?;

    output::success("Scaffolded project files", color);
    Ok(())
}

fn managed_files(answers: &InitAnswers) -> Vec<&'static str> {
    let mut files = vec!["evm-cloud.toml", "secrets.auto.tfvars.example", ".gitignore", MODE_MARKER_REL];
    if matches!(answers.indexer_config, IndexerConfigStrategy::Generate) {
        files.push("config/rindexer.yaml");
    }
    if answers.generate_erpc_config {
        files.push("config/erpc.yaml");
    }
    files.push("config/abis/.gitkeep");
    if matches!(answers.mode, InitMode::Power) {
        files.extend(["versions.tf", "main.tf", "variables.tf", "outputs.tf"]);
    }
    files
}

fn update_gitignore(project_root: &Path, mode: InitMode) -> Result<()> {
    let gitignore_path = project_root.join(".gitignore");
    let mut lines = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path)
            .map_err(|source| CliError::Io {
                source,
                path: gitignore_path.clone(),
            })?
            .lines()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    match mode {
        InitMode::Easy => {
            ensure_line(&mut lines, ".evm-cloud/");
            ensure_line(&mut lines, "terraform.auto.tfvars.json");
        }
        InitMode::Power => {
            ensure_line(&mut lines, "secrets.auto.tfvars");
            ensure_line(&mut lines, ".terraform/");
            ensure_line(&mut lines, "*.tfstate*");
            ensure_line(&mut lines, ".evm-cloud/");
        }
    }

    let content = format!("{}\n", lines.join("\n"));
    write_atomic(&gitignore_path, &content)
}

fn ensure_line(lines: &mut Vec<String>, line: &str) {
    if !lines.iter().any(|existing| existing.trim() == line) {
        lines.push(line.to_string());
    }
}

fn backup_existing_managed(project_root: &Path, managed_files: &[&str]) -> Result<()> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| CliError::Message(format!("system clock error: {err}")))?
        .as_secs();

    let backup_root = project_root.join(".evm-cloud").join("backups").join(timestamp.to_string());

    for rel in managed_files {
        let source_path = project_root.join(rel);
        if !source_path.exists() {
            continue;
        }

        let target = backup_root.join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|source| CliError::Io {
                source,
                path: parent.to_path_buf(),
            })?;
        }

        if source_path.is_dir() {
            continue;
        }

        fs::copy(&source_path, &target).map_err(|source| CliError::Io {
            source,
            path: source_path.clone(),
        })?;
    }

    Ok(())
}

fn write_atomic(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::Io {
            source,
            path: parent.to_path_buf(),
        })?;
    }

    let temp_path = temp_path_for(path);
    fs::write(&temp_path, content).map_err(|source| CliError::Io {
        source,
        path: temp_path.clone(),
    })?;

    fs::rename(&temp_path, path).map_err(|source| CliError::Io {
        source,
        path: path.to_path_buf(),
    })
}

fn temp_path_for(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("tmp")
        .to_string();
    name.push_str(".tmp");

    path.parent()
        .unwrap_or_else(|| Path::new("."))
        .join(name)
}
