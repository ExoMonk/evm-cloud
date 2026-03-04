use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::error::{CliError, Result};

pub(crate) fn wait_for_http(url: &str, timeout_secs: u64) -> Result<()> {
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);

    while std::time::Instant::now() < deadline {
        let status = Command::new("curl")
            .args(["-sf", "--max-time", "2", url])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        if matches!(status, Ok(s) if s.success()) {
            return Ok(());
        }
        thread::sleep(Duration::from_secs(2));
    }

    Err(CliError::HealthCheckTimeout {
        service: url.to_string(),
        url: url.to_string(),
    })
}

pub(crate) fn wait_for_anvil(timeout_secs: u64) -> Result<()> {
    let url = "http://localhost:8545";
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);

    while std::time::Instant::now() < deadline {
        let output = Command::new("curl")
            .args([
                "-sf",
                "--max-time",
                "2",
                url,
                "-X",
                "POST",
                "-H",
                "Content-Type: application/json",
                "-d",
                r#"{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}"#,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output();

        if let Ok(o) = output {
            if o.status.success() {
                return Ok(());
            }
        }
        thread::sleep(Duration::from_secs(2));
    }

    Err(CliError::HealthCheckTimeout {
        service: "Anvil".into(),
        url: url.into(),
    })
}

pub(crate) fn probe_chain_id(rpc_url: &str) -> Result<u64> {
    let output = Command::new("curl")
        .args([
            "-sf",
            "--max-time",
            "10",
            rpc_url,
            "-X",
            "POST",
            "-H",
            "Content-Type: application/json",
            "-d",
            r#"{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}"#,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| CliError::ChainIdProbeFailed {
            url: rpc_url.into(),
            details: e.to_string(),
        })?;

    if !output.status.success() {
        return Err(CliError::ChainIdProbeFailed {
            url: rpc_url.into(),
            details: "RPC request failed".into(),
        });
    }

    let body = String::from_utf8_lossy(&output.stdout);
    // Parse {"jsonrpc":"2.0","id":1,"result":"0x1"}
    let parsed: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| CliError::ChainIdProbeFailed {
            url: rpc_url.into(),
            details: format!("invalid JSON: {e}"),
        })?;

    let hex = parsed["result"]
        .as_str()
        .ok_or_else(|| CliError::ChainIdProbeFailed {
            url: rpc_url.into(),
            details: "missing result field".into(),
        })?;

    let hex = hex.strip_prefix("0x").unwrap_or(hex);
    u64::from_str_radix(hex, 16).map_err(|e| CliError::ChainIdProbeFailed {
        url: rpc_url.into(),
        details: format!("invalid hex chain ID: {e}"),
    })
}
