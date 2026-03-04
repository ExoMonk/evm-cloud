use std::io::IsTerminal;

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub(crate) enum ColorMode {
    Auto,
    Always,
    Never,
}

fn color_enabled(mode: ColorMode) -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }

    match mode {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => std::io::stderr().is_terminal(),
    }
}

fn paint(msg: &str, code: &str, mode: ColorMode) -> String {
    if color_enabled(mode) {
        format!("\x1b[{code}m{msg}\x1b[0m")
    } else {
        msg.to_string()
    }
}

pub(crate) fn info(msg: &str, mode: ColorMode) {
    eprintln!("{}", paint(msg, "36", mode));
}

pub(crate) fn warn(msg: &str, mode: ColorMode) {
    eprintln!("{}", paint(msg, "33", mode));
}

pub(crate) fn error(msg: &str, mode: ColorMode) {
    eprintln!("{}", paint(msg, "31", mode));
}
