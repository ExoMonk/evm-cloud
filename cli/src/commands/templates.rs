use std::collections::BTreeMap;
use std::path::PathBuf;

use clap::{Args, Subcommand};

use crate::error::{CliError, Result};
use crate::output::{self, ColorMode};
use crate::templates;
use crate::templates::registry;

#[derive(Args)]
pub(crate) struct TemplatesArgs {
    #[command(subcommand)]
    pub(crate) command: TemplatesCommand,
}

#[derive(Subcommand)]
pub(crate) enum TemplatesCommand {
    /// List available templates from the registry
    List {
        /// Force refresh the registry cache
        #[arg(long)]
        refresh: bool,
        /// Filter templates by chain support
        #[arg(long)]
        chain: Option<String>,
        /// Custom registry URL (default: GitHub)
        #[arg(long, env = "EVM_CLOUD_REGISTRY_URL")]
        registry_url: Option<String>,
    },
    /// Apply a protocol template to set up config/ with indexer, ABI, and analytics files
    Apply {
        /// Template name (e.g. uniswap-v4, aave-v3)
        name: String,
        /// Chains to deploy on (comma-separated)
        #[arg(long, value_delimiter = ',')]
        chains: Vec<String>,
        /// Template variable override (repeatable: --var key=value)
        #[arg(long = "var", value_name = "KEY=VALUE")]
        vars: Vec<String>,
        /// Custom template registry URL
        #[arg(long, env = "EVM_CLOUD_REGISTRY_URL")]
        registry_url: Option<String>,
        /// Overwrite existing files
        #[arg(long)]
        force: bool,
        /// Target directory (default: current directory)
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,
    },
}

pub(crate) fn run(args: TemplatesArgs, color: ColorMode) -> Result<()> {
    match args.command {
        TemplatesCommand::List {
            refresh,
            chain,
            registry_url,
        } => run_list(refresh, chain.as_deref(), registry_url.as_deref(), color),
        TemplatesCommand::Apply {
            name,
            chains,
            vars,
            registry_url,
            force,
            dir,
        } => run_apply(
            &name,
            &chains,
            &vars,
            registry_url.as_deref(),
            force,
            &dir,
            color,
        ),
    }
}

fn run_list(
    refresh: bool,
    chain: Option<&str>,
    registry_url: Option<&str>,
    color: ColorMode,
) -> Result<()> {
    let registry = registry::fetch_registry(refresh, registry_url)?;

    let templates: Vec<_> = registry
        .templates
        .iter()
        .filter(|t| match chain {
            Some(c) => t.chains.iter().any(|tc| tc == c),
            None => true,
        })
        .collect();

    if templates.is_empty() {
        if let Some(c) = chain {
            output::info(
                &format!("No templates found supporting chain '{c}'."),
                color,
            );
        } else {
            output::info("No templates found in registry.", color);
        }
        return Ok(());
    }

    output::headline("Available Templates", color);
    println!();
    println!(
        "{:<22} {:<10} {:<30} {}",
        "NAME", "VERSION", "CHAINS", "DESCRIPTION"
    );
    println!("{}", "-".repeat(90));

    for t in &templates {
        let chains_str = t.chains.join(", ");
        let chains_display = if chains_str.len() > 28 {
            format!("{}...", &chains_str[..25])
        } else {
            chains_str
        };

        println!(
            "{:<22} {:<10} {:<30} {}",
            t.name, t.version, chains_display, t.description
        );
    }

    println!();
    output::info(
        &format!(
            "{} template(s) available. Use `evm-cloud templates apply <name> --chains <chain>` to scaffold.",
            templates.len()
        ),
        color,
    );

    Ok(())
}

fn run_apply(
    template_name: &str,
    chains: &[String],
    var_args: &[String],
    registry_url: Option<&str>,
    force: bool,
    dir: &PathBuf,
    color: ColorMode,
) -> Result<()> {
    let started = std::time::Instant::now();

    if chains.is_empty() {
        return Err(CliError::FlagConflict {
            message: "--chains is required with `templates apply`".to_string(),
        });
    }

    output::headline(
        &format!(
            "🏰 ⚒️ Applying template `{template_name}` in {}",
            dir.display()
        ),
        color,
    );

    // 1. Fetch registry
    let registry = templates::registry::fetch_registry(false, registry_url)?;

    // 2. Resolve template
    let entry = templates::resolve::resolve_template(template_name, &registry)?;
    output::subline(
        &format!(
            "📦 Found template `{}` v{}",
            entry.name, entry.version
        ),
        color,
    );

    // 3. Fetch/cache template package
    let template_dir = templates::resolve::fetch_template_package(entry, force)?;
    output::subline("📥 Template package ready", color);

    // 4. Parse template.toml
    let manifest_path = template_dir.join("template.toml");
    let manifest_content =
        std::fs::read_to_string(&manifest_path).map_err(|source| CliError::Io {
            source,
            path: manifest_path.clone(),
        })?;
    let manifest_file: templates::types::TemplateManifestFile =
        toml::from_str(&manifest_content).map_err(|e| CliError::ConfigParse {
            path: manifest_path.clone(),
            details: e.to_string(),
        })?;
    let manifest = &manifest_file.template;

    // 5. Validate requested chains are supported
    let supported_chains: Vec<String> = manifest.chains.keys().cloned().collect();
    for chain in chains {
        if !manifest.chains.contains_key(chain) {
            return Err(CliError::TemplateChainNotSupported {
                template: manifest.name.clone(),
                chain: chain.clone(),
                supported: supported_chains.clone(),
            });
        }
    }

    // 6. Check min_evm_cloud_version if set
    if let Some(ref min_version) = manifest.min_evm_cloud_version {
        let current = env!("CARGO_PKG_VERSION");
        if current < min_version.as_str() {
            output::warn(
                &format!(
                    "Template requires evm-cloud >= {min_version}, current is {current}. Proceeding anyway."
                ),
                color,
            );
        }
    }

    // 7. Parse --var strings into BTreeMap
    let mut user_vars: BTreeMap<String, String> = BTreeMap::new();
    for var_str in var_args {
        let (key, value) = var_str.split_once('=').ok_or_else(|| CliError::InvalidArg {
            arg: var_str.clone(),
            details: "expected KEY=VALUE format for --var".to_string(),
        })?;
        user_vars.insert(key.to_string(), value.to_string());
    }

    // 8. Create output directories
    let config_dir = dir.join("config");
    std::fs::create_dir_all(&config_dir).map_err(|source| CliError::Io {
        source,
        path: config_dir.clone(),
    })?;
    std::fs::create_dir_all(dir).map_err(|source| CliError::Io {
        source,
        path: dir.clone(),
    })?;

    // 9. Render template with selective routing:
    //    - rindexer.yaml, abis/ → config/
    //    - clickhouse/, grafana/, README.md → project root
    let written = templates::render::render_template(
        &template_dir,
        manifest,
        chains,
        &user_vars,
        &config_dir,
        Some(dir),
    )?;

    for path in &written {
        output::checkline(&format!("  {}", path.display()), color);
    }

    // 10. Generate config/erpc.yaml
    let erpc_path = config_dir.join("erpc.yaml");
    if !erpc_path.exists() || force {
        let chain_ids: Vec<u64> = chains
            .iter()
            .filter_map(|c| templates::chains::chain_id(c))
            .collect();
        let erpc_content = templates::render::render_erpc_from_chains(&chain_ids);
        crate::codegen::write_atomic(&erpc_path, &erpc_content)?;
        output::checkline("  config/erpc.yaml", color);
    } else {
        output::warn("config/erpc.yaml already exists, skipping (use --force to overwrite)", color);
    }

    // 11. Write template-lock.toml
    let mut resolved_vars: BTreeMap<String, toml::Value> = BTreeMap::new();
    for (k, v) in &user_vars {
        resolved_vars.insert(k.clone(), toml::Value::String(v.clone()));
    }
    for (var_name, var_def) in &manifest.variables {
        if !resolved_vars.contains_key(var_name) {
            if let Some(default) = &var_def.default {
                resolved_vars.insert(var_name.clone(), default.clone());
            }
        }
    }

    let lock_entry = templates::types::TemplateLockEntry {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        chains: chains.to_vec(),
        init_date: chrono_date_today(),
        variables: resolved_vars,
    };

    let lock_path = dir.join(".evm-cloud").join("template-lock.toml");
    let lock = if lock_path.exists() && !force {
        let existing_content =
            std::fs::read_to_string(&lock_path).map_err(|source| CliError::Io {
                source,
                path: lock_path.clone(),
            })?;
        let mut existing: templates::types::TemplateLock =
            toml::from_str(&existing_content).unwrap_or(templates::types::TemplateLock {
                templates: Vec::new(),
            });
        existing.templates.push(lock_entry);
        existing
    } else {
        templates::types::TemplateLock {
            templates: vec![lock_entry],
        }
    };

    let lock_content =
        toml::to_string_pretty(&lock).unwrap_or_else(|_| "# failed to serialize lock\n".to_string());
    crate::codegen::write_atomic(&lock_path, &lock_content)?;
    output::checkline("  .evm-cloud/template-lock.toml", color);

    // 12. Create a minimal evm-cloud.toml if one doesn't exist
    let toml_path = dir.join("evm-cloud.toml");
    if !toml_path.exists() {
        let chain_list = chains
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect::<Vec<_>>()
            .join(", ");

        let rpc_endpoints = chains
            .iter()
            .map(|c| format!("{c} = \"https://{c}-rpc.publicnode.com\""))
            .collect::<Vec<_>>()
            .join(", ");

        let toml_content = format!(
            "schema_version = 1\n\n\
             [project]\n\
             name = \"{template_name}\"\n\n\
             [indexer]\n\
             config_path = \"config/rindexer.yaml\"\n\
             erpc_config_path = \"config/erpc.yaml\"\n\
             chains = [{chain_list}]\n\n\
             [rpc]\n\
             endpoints = {{ {rpc_endpoints} }}\n"
        );

        crate::codegen::write_atomic(&toml_path, &toml_content)?;
        output::checkline("  evm-cloud.toml", color);
    }

    output::headline(
        &format!(
            "🏰 ✅ Template applied - {}",
            output::duration_human(started.elapsed())
        ),
        color,
    );

    output::info(
        "Run `evm-cloud init` to set up infrastructure, or `evm-cloud deploy` to deploy.",
        color,
    );

    Ok(())
}

/// Returns today's date as YYYY-MM-DD without pulling in the `chrono` crate.
fn chrono_date_today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let days = secs / 86400;
    let mut year = 1970i64;
    let mut remaining_days = days as i64;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let month_days = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1u32;
    for &md in &month_days {
        if remaining_days < md {
            break;
        }
        remaining_days -= md;
        month += 1;
    }

    let day = remaining_days + 1;
    format!("{year:04}-{month:02}-{day:02}")
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}
