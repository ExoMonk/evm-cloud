use std::fs;
use std::path::Path;

use crate::error::{CliError, Result};

pub(crate) fn auto_var_file_arg(
    terraform_dir: &Path,
    passthrough_args: &[String],
) -> Result<Option<String>> {
    if passthrough_args.iter().any(|arg| {
        arg == "-var-file"
            || arg == "--var-file"
            || arg.starts_with("-var-file=")
            || arg.starts_with("--var-file=")
    }) {
        return Ok(None);
    }

    let mut candidates = fs::read_dir(terraform_dir)
        .map_err(|source| CliError::Io {
            source,
            path: terraform_dir.to_path_buf(),
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|source| CliError::Io {
            source,
            path: terraform_dir.to_path_buf(),
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
            !name.ends_with(".auto.tfvars")
                && !name.ends_with(".tfvars.example")
                && !name.eq_ignore_ascii_case("terraform.tfvars")
        })
        .collect::<Vec<_>>();

    candidates.sort();

    let Some(selected) = candidates.into_iter().next() else {
        return Ok(None);
    };

    Ok(Some(format!("-var-file={}", selected.display())))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::auto_var_file_arg;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let base = std::env::temp_dir().join(format!(
            "evm-cloud-tfvars-tests-{}-{}-{}",
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

    fn write(path: &std::path::Path, content: &str) {
        fs::write(path, content).expect("write file");
    }

    #[test]
    fn picks_single_non_auto_tfvars_file() {
        let dir = temp_dir("single");
        write(&dir.join("minimal_k3.tfvars"), "project_name = \"x\"\n");

        let arg = auto_var_file_arg(&dir, &[])
            .expect("resolve var file")
            .expect("must auto select var file");

        assert!(arg.contains("-var-file="));
        assert!(arg.contains("minimal_k3.tfvars"));
    }

    #[test]
    fn does_not_override_explicit_var_file() {
        let dir = temp_dir("explicit");
        write(&dir.join("minimal_k3.tfvars"), "project_name = \"x\"\n");

        let arg = auto_var_file_arg(&dir, &["-var-file=custom.tfvars".to_string()])
            .expect("resolve var file");

        assert!(arg.is_none());
    }
}
