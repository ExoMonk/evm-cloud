use clap::Args;

use crate::error::Result;
use crate::output::{self, ColorMode};

#[derive(Args)]
pub(crate) struct DeployArgs {}

pub(crate) fn run(_args: DeployArgs, color: ColorMode) -> Result<()> {
    output::warn("deploy command not yet implemented", color);
    Ok(())
}
