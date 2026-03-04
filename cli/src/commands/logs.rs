use clap::Args;

use crate::error::Result;
use crate::output::{self, ColorMode};

#[derive(Args)]
pub(crate) struct LogsArgs {}

pub(crate) fn run(_args: LogsArgs, color: ColorMode) -> Result<()> {
    output::warn("logs command not yet implemented", color);
    Ok(())
}
