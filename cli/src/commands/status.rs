use clap::Args;

use crate::error::Result;
use crate::output::{self, ColorMode};

#[derive(Args)]
pub(crate) struct StatusArgs {}

pub(crate) fn run(_args: StatusArgs, color: ColorMode) -> Result<()> {
    output::warn("status command not yet implemented", color);
    Ok(())
}
