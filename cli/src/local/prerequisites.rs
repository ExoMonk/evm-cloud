use std::net::TcpListener;
use std::process::{Command, Stdio};

use crate::error::{CliError, Result};
use crate::output::{self, ColorMode};

const REQUIRED_TOOLS: &[&str] = &["kind", "kubectl", "helm", "docker"];

pub(crate) fn check_all(
    profile: super::Profile,
    check_ports: bool,
    color: ColorMode,
) -> Result<()> {
    check_tools(color)?;
    check_docker_running()?;
    check_docker_memory(profile, color);
    if check_ports {
        check_port_conflicts()?;
    }
    Ok(())
}

fn check_tools(_color: ColorMode) -> Result<()> {
    for tool in REQUIRED_TOOLS {
        if which::which(tool).is_err() {
            return Err(CliError::PrerequisiteNotFound {
                tool: tool.to_string(),
            });
        }
    }
    Ok(())
}

fn check_docker_running() -> Result<()> {
    let status = Command::new("docker")
        .args(["info"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    match status {
        Ok(s) if s.success() => Ok(()),
        _ => Err(CliError::DockerNotRunning),
    }
}

fn check_docker_memory(profile: super::Profile, color: ColorMode) {
    let output = Command::new("docker")
        .args(["info", "--format", "{{.MemTotal}}"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();

    let mem_bytes: u64 = match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .trim()
            .parse()
            .unwrap_or(0),
        Err(_) => return,
    };

    let mem_gb = mem_bytes / 1_073_741_824;
    let min_gb = match profile {
        super::Profile::Heavy => 8,
        super::Profile::Default => 4,
    };

    if mem_gb < min_gb {
        output::warn(
            &format!(
                "Docker has {mem_gb}GB memory. Recommended: {min_gb}GB+ for '{profile}' profile. \
                 Increase in Docker Desktop > Settings > Resources > Memory.",
                profile = match profile {
                    super::Profile::Default => "default",
                    super::Profile::Heavy => "heavy",
                },
            ),
            color,
        );
    }
}

fn check_port_conflicts() -> Result<()> {
    let ports = [8545u16, 4000, 8123, 18080];
    for port in ports {
        if TcpListener::bind(("127.0.0.1", port)).is_err() {
            return Err(CliError::PortConflict { port });
        }
    }
    Ok(())
}
