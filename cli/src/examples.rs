use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use crate::error::{CliError, Result};

const EXAMPLES_DIR: &str = "examples";
const BARE_EXAMPLE_SENTINEL: &str = "__LIST__";
const EXAMPLES_REPO_OWNER: &str = "ExoMonk";
const EXAMPLES_REPO_NAME: &str = "evm-cloud";

#[derive(Debug, Clone)]
pub(crate) struct ExampleSpec {
    pub(crate) canonical: String,
    pub(crate) aliases: Vec<String>,
    pub(crate) path: PathBuf,
}

#[derive(Debug, Clone)]
pub(crate) struct BootstrapResult {
    pub(crate) canonical: String,
    pub(crate) wrote_power_metadata: bool,
}

pub(crate) fn bare_example_sentinel() -> &'static str {
    BARE_EXAMPLE_SENTINEL
}

pub(crate) fn list_examples_from_cwd() -> Result<Vec<ExampleSpec>> {
    let cwd = std::env::current_dir().map_err(|source| CliError::Io {
        source,
        path: PathBuf::from("."),
    })?;
    let repo_root = resolve_examples_repo_root(&cwd)?;
    discover_examples(&repo_root)
}

pub(crate) fn bootstrap_example_to_dir(
    requested: &str,
    destination_dir: &Path,
    force: bool,
) -> Result<BootstrapResult> {
    let cwd = std::env::current_dir().map_err(|source| CliError::Io {
        source,
        path: PathBuf::from("."),
    })?;
    let repo_root = resolve_examples_repo_root(&cwd)?;
    let examples = discover_examples(&repo_root)?;
    let selected = resolve_example(requested, &examples)?;

    fs::create_dir_all(destination_dir).map_err(|source| CliError::Io {
        source,
        path: destination_dir.to_path_buf(),
    })?;

    let source_root = selected
        .path
        .canonicalize()
        .map_err(|source| CliError::Io {
            source,
            path: selected.path.clone(),
        })?;

    let source_files = collect_source_files(&source_root, &source_root)?;

    for relative in &source_files {
        if is_excluded_path(relative) {
            continue;
        }
        if relative.file_name().and_then(|n| n.to_str()) == Some(".gitignore") {
            continue; // .gitignore is merged, not overwritten
        }
        let target_path = destination_dir.join(relative);
        if target_path.exists() && !force {
            return Err(CliError::InitFileExists { path: target_path });
        }
    }

    if force {
        backup_existing_for_example(destination_dir, &source_files)?;
    }

    for relative in source_files {
        if is_excluded_path(&relative) {
            continue;
        }

        let source_path = source_root.join(&relative);
        let target_path = destination_dir.join(&relative);

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|source| CliError::Io {
                source,
                path: parent.to_path_buf(),
            })?;
        }

        fs::copy(&source_path, &target_path).map_err(|source| CliError::Io {
            source,
            path: source_path.clone(),
        })?;
    }

    rewrite_module_source(destination_dir)?;

    let wrote_power_metadata =
        ensure_power_mode_metadata(destination_dir, &selected.canonical, force)?;

    Ok(BootstrapResult {
        canonical: selected.canonical,
        wrote_power_metadata,
    })
}

fn resolve_examples_repo_root(start: &Path) -> Result<PathBuf> {
    match find_repo_root(start) {
        Ok(repo_root) => Ok(repo_root),
        Err(CliError::ExampleRepoRootNotFound { .. }) => fetch_examples_repo_root_from_github(),
        Err(other) => Err(other),
    }
}

fn fetch_examples_repo_root_from_github() -> Result<PathBuf> {
    let temp_root = std::env::temp_dir().join(format!(
        "evm-cloud-examples-fetch-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|err| CliError::SystemClock(err.to_string()))?
            .as_nanos()
    ));

    fs::create_dir_all(&temp_root).map_err(|source| CliError::Io {
        source,
        path: temp_root.clone(),
    })?;

    let archive_path = temp_root.join("repo.tar.gz");
    let mut last_error: Option<String> = None;
    let mut selected_url: Option<String> = None;
    for url in remote_archive_urls_for_current_version() {
        match download_archive(&url, &archive_path) {
            Ok(()) => {
                selected_url = Some(url);
                break;
            }
            Err(err) => {
                last_error = Some(err.to_string());
            }
        }
    }

    if selected_url.is_none() {
        return Err(CliError::ExampleFetchFailed {
            details: last_error.unwrap_or_else(|| "unknown download error".to_string()),
        });
    }

    let extract_root = temp_root.join("extract");
    fs::create_dir_all(&extract_root).map_err(|source| CliError::Io {
        source,
        path: extract_root.clone(),
    })?;

    extract_archive(&archive_path, &extract_root)?;

    let mut children = fs::read_dir(&extract_root)
        .map_err(|source| CliError::Io {
            source,
            path: extract_root.clone(),
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|source| CliError::Io {
            source,
            path: extract_root.clone(),
        })?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    children.sort();

    let repo_root = children
        .into_iter()
        .next()
        .ok_or_else(|| CliError::ExampleArchiveInvalid {
            details: "downloaded GitHub archive is missing repository root directory".to_string(),
        })?;

    if !repo_root.join(EXAMPLES_DIR).is_dir() {
        return Err(CliError::ExampleArchiveInvalid {
            details: format!(
                "downloaded repository missing `{EXAMPLES_DIR}` directory: {}",
                repo_root.display()
            ),
        });
    }

    Ok(repo_root)
}

fn remote_archive_urls_for_current_version() -> Vec<String> {
    let tag = format!("v{}", env!("CARGO_PKG_VERSION"));
    vec![
        format!(
            "https://codeload.github.com/{}/{}/tar.gz/refs/tags/{}",
            EXAMPLES_REPO_OWNER, EXAMPLES_REPO_NAME, tag
        ),
        format!(
            "https://codeload.github.com/{}/{}/tar.gz/refs/heads/main",
            EXAMPLES_REPO_OWNER, EXAMPLES_REPO_NAME
        ),
    ]
}

fn download_archive(url: &str, destination: &Path) -> Result<()> {
    let output = Command::new("curl")
        .arg("-fLsS")
        .arg(url)
        .arg("-o")
        .arg(destination)
        .output()
        .map_err(|source| CliError::Io {
            source,
            path: PathBuf::from("curl"),
        })?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(CliError::ExampleFetchFailed {
        details: format!("download from {url}: {}", stderr.trim()),
    })
}

fn extract_archive(archive_path: &Path, extract_root: &Path) -> Result<()> {
    let output = Command::new("tar")
        .arg("-xzf")
        .arg(archive_path)
        .arg("-C")
        .arg(extract_root)
        .output()
        .map_err(|source| CliError::Io {
            source,
            path: PathBuf::from("tar"),
        })?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(CliError::ExampleArchiveInvalid {
        details: format!("extract {}: {}", archive_path.display(), stderr.trim()),
    })
}

fn ensure_power_mode_metadata(
    destination_dir: &Path,
    example_name: &str,
    force: bool,
) -> Result<bool> {
    let marker_path = destination_dir.join(".evm-cloud").join("mode");
    let toml_path = destination_dir.join("evm-cloud.toml");

    if marker_path.exists() && !force {
        return Err(CliError::InitFileExists { path: marker_path });
    }

    if toml_path.exists() && !force {
        return Err(CliError::InitFileExists { path: toml_path });
    }

    if let Some(parent) = marker_path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::Io {
            source,
            path: parent.to_path_buf(),
        })?;
    }

    fs::write(&marker_path, "power\n").map_err(|source| CliError::Io {
        source,
        path: marker_path,
    })?;

    let metadata = derive_power_metadata(destination_dir, example_name)?;
    let content = format!(
        "schema_version = 1\n\n[project]\nname = \"{}\"\nregion = \"{}\"\n\n[compute]\nengine = \"{}\"\ninstance_type = \"{}\"\n\n[database]\nmode = \"{}\"\nprovider = \"{}\"\n\n[indexer]\nconfig_path = \"{}\"\nchains = [\"ethereum\"]\n\n[rpc]\nendpoints = {{ ethereum = \"https://ethereum-rpc.publicnode.com\" }}\n\n[ingress]\nmode = \"none\"\n\n[secrets]\nmode = \"provider\"\n",
        metadata.project_name,
        metadata.region,
        metadata.compute_engine,
        metadata.instance_type,
        metadata.database_mode,
        metadata.database_provider,
        metadata.indexer_config_path,
    );

    fs::write(&toml_path, content).map_err(|source| CliError::Io {
        source,
        path: toml_path,
    })?;

    Ok(true)
}

#[derive(Debug, Clone)]
struct PowerMetadata {
    project_name: String,
    region: String,
    compute_engine: String,
    instance_type: String,
    database_mode: String,
    database_provider: String,
    indexer_config_path: String,
}

fn derive_power_metadata(destination_dir: &Path, example_name: &str) -> Result<PowerMetadata> {
    let mut metadata = PowerMetadata {
        project_name: example_name.replace('_', "-"),
        region: "us-east-1".to_string(),
        compute_engine: infer_compute_engine_from_name(example_name).to_string(),
        instance_type: default_instance_type(infer_compute_engine_from_name(example_name))
            .to_string(),
        database_mode: "self_hosted".to_string(),
        database_provider: "aws".to_string(),
        indexer_config_path: if destination_dir.join("config/rindexer.yaml").is_file() {
            "config/rindexer.yaml".to_string()
        } else {
            "rindexer.yaml".to_string()
        },
    };

    if let Some(tfvars_path) = primary_tfvars_file(destination_dir)? {
        let raw = fs::read_to_string(&tfvars_path).map_err(|source| CliError::Io {
            source,
            path: tfvars_path.clone(),
        })?;

        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            let Some((key, value)) = trimmed.split_once('=') else {
                continue;
            };

            let key = key.trim();
            let value = value.trim();
            let string_value = parse_tfvars_string(value);

            match key {
                "project_name" => {
                    if let Some(parsed) = string_value {
                        metadata.project_name = parsed;
                    }
                }
                "aws_region" => {
                    if let Some(parsed) = string_value {
                        metadata.region = parsed;
                    }
                }
                "compute_engine" => {
                    if let Some(parsed) = string_value {
                        metadata.compute_engine = parsed;
                    }
                }
                "ec2_instance_type" => {
                    if let Some(parsed) = string_value {
                        metadata.instance_type = parsed;
                    }
                }
                "k3s_instance_type" => {
                    if let Some(parsed) = string_value {
                        metadata.instance_type = parsed;
                    }
                    if metadata.compute_engine == "ec2" {
                        metadata.compute_engine = "k3s".to_string();
                    }
                }
                "database_mode" => {
                    if let Some(parsed) = string_value {
                        metadata.database_mode = parsed;
                    }
                }
                _ => {}
            }
        }
    }

    if metadata.compute_engine == "ec2" && example_name.contains("k3s") {
        metadata.compute_engine = "k3s".to_string();
        metadata.instance_type = "t3.small".to_string();
    }

    Ok(metadata)
}

fn infer_compute_engine_from_name(example_name: &str) -> &'static str {
    let normalized = example_name.to_ascii_lowercase();
    if normalized.contains("eks") {
        "eks"
    } else if normalized.contains("k3s") {
        "k3s"
    } else if normalized.contains("baremetal") {
        "docker_compose"
    } else {
        "ec2"
    }
}

fn default_instance_type(compute_engine: &str) -> &'static str {
    match compute_engine {
        "k3s" => "t3.small",
        _ => "t3.micro",
    }
}

fn primary_tfvars_file(root: &Path) -> Result<Option<PathBuf>> {
    let mut candidates = fs::read_dir(root)
        .map_err(|source| CliError::Io {
            source,
            path: root.to_path_buf(),
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|source| CliError::Io {
            source,
            path: root.to_path_buf(),
        })?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .map(|ext| ext.eq_ignore_ascii_case("tfvars"))
                    .unwrap_or(false)
        })
        .filter(|path| {
            let name = path
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or_default();
            !name.ends_with(".auto.tfvars") && !name.ends_with(".tfvars.example")
        })
        .collect::<Vec<_>>();

    candidates.sort();
    Ok(candidates.into_iter().next())
}

fn parse_tfvars_string(raw: &str) -> Option<String> {
    let without_comment = raw.split('#').next()?.trim();
    if without_comment.starts_with('"') {
        let mut chars = without_comment.chars();
        chars.next()?;
        let remainder = chars.collect::<String>();
        let end = remainder.find('"')?;
        return Some(remainder[..end].to_string());
    }
    None
}

fn find_repo_root(start: &Path) -> Result<PathBuf> {
    let mut cursor = Some(start);
    while let Some(path) = cursor {
        let examples_path = path.join(EXAMPLES_DIR);
        if examples_path.is_dir() {
            return Ok(path.to_path_buf());
        }
        cursor = path.parent();
    }

    Err(CliError::ExampleRepoRootNotFound {
        start: start.display().to_string(),
    })
}

fn discover_examples(repo_root: &Path) -> Result<Vec<ExampleSpec>> {
    let examples_root = repo_root.join(EXAMPLES_DIR);
    let entries = fs::read_dir(&examples_root).map_err(|source| CliError::Io {
        source,
        path: examples_root.clone(),
    })?;

    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| CliError::Io {
            source,
            path: examples_root.clone(),
        })?;
        let file_type = entry.file_type().map_err(|source| CliError::Io {
            source,
            path: entry.path(),
        })?;

        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }

        let path = entry.path();
        if is_valid_example_dir(&path)? {
            names.push(name);
        }
    }

    names.sort();

    let alias_map = alias_map();
    let mut by_alias: BTreeMap<String, String> = BTreeMap::new();
    for (alias, canonical) in &alias_map {
        if let Some(existing) = by_alias.get(alias) {
            if existing != canonical {
                return Err(CliError::ExampleAliasCollision {
                    alias: alias.clone(),
                    first: existing.clone(),
                    second: canonical.clone(),
                });
            }
        } else {
            by_alias.insert(alias.clone(), canonical.clone());
        }
    }

    let mut specs = Vec::new();
    for canonical in names {
        let aliases = alias_map
            .iter()
            .filter_map(|(alias, target)| (target == &canonical).then_some(alias.clone()))
            .collect::<Vec<_>>();

        specs.push(ExampleSpec {
            canonical: canonical.clone(),
            aliases,
            path: examples_root.join(canonical),
        });
    }

    Ok(specs)
}

fn resolve_example(requested: &str, specs: &[ExampleSpec]) -> Result<ExampleSpec> {
    let normalized = requested.trim().to_ascii_lowercase();

    if let Some(exact) = specs
        .iter()
        .find(|spec| spec.canonical.eq_ignore_ascii_case(&normalized))
    {
        return Ok(exact.clone());
    }

    let aliases = alias_map();
    if let Some(target) = aliases.get(&normalized) {
        if let Some(spec) = specs.iter().find(|spec| spec.canonical == *target) {
            return Ok(spec.clone());
        }
    }

    let mut available = specs
        .iter()
        .map(|spec| {
            if spec.aliases.is_empty() {
                spec.canonical.clone()
            } else {
                format!("{} ({})", spec.canonical, spec.aliases.join(","))
            }
        })
        .collect::<Vec<_>>();
    available.sort();

    Err(CliError::ExampleNotFound {
        requested: requested.to_string(),
        available,
    })
}

fn is_valid_example_dir(path: &Path) -> Result<bool> {
    let entries = fs::read_dir(path).map_err(|source| CliError::Io {
        source,
        path: path.to_path_buf(),
    })?;

    let mut has_tf = false;
    let mut has_setup_signal = false;

    for entry in entries {
        let entry = entry.map_err(|source| CliError::Io {
            source,
            path: path.to_path_buf(),
        })?;
        let child = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if child.is_file()
            && child
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("tf"))
                .unwrap_or(false)
        {
            has_tf = true;
        }

        if name == "README.md" || name.ends_with(".tfvars") || name.ends_with(".tfvars.example") {
            has_setup_signal = true;
        }
    }

    Ok(has_tf && has_setup_signal)
}

fn is_excluded_path(relative: &Path) -> bool {
    let name = relative
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or_default();

    if name == ".terraform.lock.hcl" {
        return true;
    }

    if name == "terraform.tfstate"
        || name.ends_with(".tfstate")
        || name.ends_with(".tfstate.backup")
        || name.contains(".tfstate.")
    {
        return true;
    }

    if name.ends_with(".auto.tfvars") || name.ends_with(".auto.tfvars.json") {
        return true;
    }

    relative.components().any(|component| {
        matches!(component, Component::Normal(part) if part == ".terraform" || part == ".git")
    })
}

/// Rewrite `source = "../.."` (relative dev path) to the published GitHub module source
/// in all `.tf` files under the destination directory.
fn rewrite_module_source(dir: &Path) -> Result<()> {
    let module_source = crate::module_source();
    let entries = fs::read_dir(dir).map_err(|source| CliError::Io {
        source,
        path: dir.to_path_buf(),
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| CliError::Io {
            source,
            path: dir.to_path_buf(),
        })?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("tf") {
            let content = fs::read_to_string(&path).map_err(|source| CliError::Io {
                source,
                path: path.clone(),
            })?;
            if content.contains("source = \"../..\"") {
                let updated = content.replace(
                    "source = \"../..\"",
                    &format!("source = \"{module_source}\""),
                );
                fs::write(&path, updated).map_err(|source| CliError::Io {
                    source,
                    path: path.clone(),
                })?;
            }
        }
    }

    Ok(())
}

fn collect_source_files(root: &Path, cursor: &Path) -> Result<Vec<PathBuf>> {
    let mut entries = fs::read_dir(cursor)
        .map_err(|source| CliError::Io {
            source,
            path: cursor.to_path_buf(),
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|source| CliError::Io {
            source,
            path: cursor.to_path_buf(),
        })?;

    entries.sort_by_key(|entry| entry.file_name());

    let mut files = Vec::new();
    for entry in entries {
        let file_type = entry.file_type().map_err(|source| CliError::Io {
            source,
            path: entry.path(),
        })?;

        let path = entry.path();
        if file_type.is_symlink() {
            return Err(CliError::ExampleSymlinkUnsupported { path });
        }

        let canonical = path.canonicalize().map_err(|source| CliError::Io {
            source,
            path: path.clone(),
        })?;
        if !canonical.starts_with(root) {
            return Err(CliError::ExamplePathEscape { path });
        }

        if file_type.is_dir() {
            files.extend(collect_source_files(root, &path)?);
            continue;
        }

        if file_type.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| CliError::ExamplePathEscape { path: path.clone() })?;
            files.push(relative.to_path_buf());
        }
    }

    Ok(files)
}

fn backup_existing_for_example(destination_dir: &Path, source_files: &[PathBuf]) -> Result<()> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| CliError::SystemClock(err.to_string()))?
        .as_secs();

    let backup_root = destination_dir
        .join(".evm-cloud")
        .join("backups")
        .join(timestamp.to_string())
        .join("example-bootstrap");

    for relative in source_files {
        if is_excluded_path(relative) {
            continue;
        }

        let source = destination_dir.join(relative);
        if !source.exists() || source.is_dir() {
            continue;
        }

        let target = backup_root.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|source| CliError::Io {
                source,
                path: parent.to_path_buf(),
            })?;
        }

        fs::copy(&source, &target).map_err(|source_err| CliError::Io {
            source: source_err,
            path: source.clone(),
        })?;
    }

    Ok(())
}

fn alias_map() -> BTreeMap<String, String> {
    [
        ("ec2_rds", "minimal_aws_rds"),
        ("ec2_clickhouse", "minimal_aws_byo_clickhouse"),
        ("external_ec2", "minimal_aws_external_ec2_byo"),
        ("k3s_clickhouse", "minimal_aws_k3s_byo_clickhouse"),
        ("eks_clickhouse", "aws_eks_BYO_clickhouse"),
        ("k3s_cloudflare", "aws_k3s_cloudflare_ingress"),
        ("baremetal_clickhouse", "baremetal_byo_clickhouse"),
        ("baremetal_k3s", "baremetal_k3s_byo_db"),
        ("k3s_multi_clickhouse", "prod_aws_k3s_multi_byo_clickhouse"),
    ]
    .into_iter()
    .map(|(alias, canonical)| (alias.to_string(), canonical.to_string()))
    .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::{
        derive_power_metadata, discover_examples, ensure_power_mode_metadata, is_excluded_path,
        remote_archive_urls_for_current_version,
    };

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let base = std::env::temp_dir().join(format!(
            "evm-cloud-examples-tests-{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock before unix epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("create temp dir");
        base
    }

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dir");
        }
        fs::write(path, content).expect("write file");
    }

    #[test]
    fn discovery_includes_valid_examples_only() {
        let root = temp_dir("discovery");
        let examples = root.join("examples");
        fs::create_dir_all(&examples).expect("create examples root");

        let valid = examples.join("minimal_aws_rds");
        fs::create_dir_all(&valid).expect("create valid example");
        write(&valid.join("main.tf"), "terraform {}\n");
        write(&valid.join("README.md"), "# test\n");

        let invalid = examples.join("broken");
        fs::create_dir_all(&invalid).expect("create invalid example");
        write(&invalid.join("main.tf"), "terraform {}\n");

        let discovered = discover_examples(&root).expect("discover examples");
        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].canonical, "minimal_aws_rds");
    }

    #[test]
    fn excludes_runtime_files() {
        assert!(is_excluded_path(Path::new(".terraform/providers/file")));
        assert!(is_excluded_path(Path::new("terraform.tfstate")));
        assert!(is_excluded_path(Path::new("terraform.tfstate.backup")));
        assert!(is_excluded_path(Path::new(
            "terraform.tfstate.1772475822.backup"
        )));
        assert!(is_excluded_path(Path::new("secrets.auto.tfvars")));
        assert!(is_excluded_path(Path::new("dev.auto.tfvars.json")));
        assert!(is_excluded_path(Path::new(".git/config")));
        assert!(!is_excluded_path(Path::new("main.tf")));
    }

    #[test]
    fn writes_power_mode_metadata_files() {
        let dir = temp_dir("power-metadata");
        let wrote =
            ensure_power_mode_metadata(&dir, "minimal_aws_rds", false).expect("write metadata");
        assert!(wrote);

        let mode = fs::read_to_string(dir.join(".evm-cloud/mode")).expect("read mode");
        assert_eq!(mode.trim(), "power");

        let toml = fs::read_to_string(dir.join("evm-cloud.toml")).expect("read toml");
        assert!(toml.contains("schema_version = 1"));
        assert!(toml.contains("name = \"minimal-aws-rds\""));
    }

    #[test]
    fn metadata_write_rejects_existing_without_force() {
        let dir = temp_dir("power-metadata-collision");
        write(&dir.join("evm-cloud.toml"), "schema_version = 1\n");

        let err = ensure_power_mode_metadata(&dir, "minimal_aws_rds", false)
            .expect_err("must fail when metadata exists");
        assert!(err.to_string().contains("managed init file already exists"));
    }

    #[test]
    fn derives_power_metadata_from_tfvars() {
        let dir = temp_dir("derive-power-metadata");
        write(
            &dir.join("minimal_k3.tfvars"),
            "project_name = \"evm-cloud-k3s\"\naws_region = \"us-west-2\"\nk3s_instance_type = \"c6i.large\"\n",
        );
        write(&dir.join("config/rindexer.yaml"), "name: test\n");

        let metadata =
            derive_power_metadata(&dir, "minimal_aws_k3s_byo_clickhouse").expect("derive metadata");

        assert_eq!(metadata.project_name, "evm-cloud-k3s");
        assert_eq!(metadata.region, "us-west-2");
        assert_eq!(metadata.compute_engine, "k3s");
        assert_eq!(metadata.instance_type, "c6i.large");
        assert_eq!(metadata.indexer_config_path, "config/rindexer.yaml");
    }

    #[test]
    fn remote_archive_urls_are_tag_then_main() {
        let urls = remote_archive_urls_for_current_version();
        assert_eq!(urls.len(), 2);
        assert!(urls[0].contains("/refs/tags/v"));
        assert!(urls[1].ends_with("/refs/heads/main"));
    }
}
