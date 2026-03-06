use std::io::IsTerminal;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

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

fn pretty_enabled(mode: ColorMode) -> bool {
    color_enabled(mode) && std::io::stderr().is_terminal()
}

fn paint(msg: &str, code: &str, mode: ColorMode) -> String {
    if color_enabled(mode) {
        format!("\x1b[{code}m{msg}\x1b[0m")
    } else {
        msg.to_string()
    }
}

fn paint_bold_blue(msg: &str, mode: ColorMode) -> String {
    if color_enabled(mode) {
        format!("\x1b[1;34m{msg}\x1b[0m")
    } else {
        msg.to_string()
    }
}

fn paint_bold_red(msg: &str, mode: ColorMode) -> String {
    if color_enabled(mode) {
        format!("\x1b[1;31m{msg}\x1b[0m")
    } else {
        msg.to_string()
    }
}

fn with_prefix(msg: &str, pretty_prefix: &str, plain_prefix: &str, mode: ColorMode) -> String {
    if pretty_enabled(mode) {
        format!("     {pretty_prefix} {msg}")
    } else {
        format!("{plain_prefix}: {msg}")
    }
}

pub(crate) fn headline(msg: &str, mode: ColorMode) {
    eprintln!("{}", paint_bold_blue(msg, mode));
}

pub(crate) fn headline_red(msg: &str, mode: ColorMode) {
    eprintln!("{}", paint_bold_red(msg, mode));
}

pub(crate) fn subline(msg: &str, mode: ColorMode) {
    if pretty_enabled(mode) {
        eprintln!("     {msg}");
    } else {
        eprintln!("{}", msg);
    }
}

pub(crate) fn checkline(msg: &str, mode: ColorMode) {
    if pretty_enabled(mode) {
        eprintln!("     ✓ {msg}");
    } else {
        eprintln!("OK: {msg}");
    }
}

pub(crate) fn duration_human(duration: std::time::Duration) -> String {
    let total = duration.as_secs();
    let mins = total / 60;
    let secs = total % 60;
    if mins > 0 {
        format!("{}m {}s", mins, secs)
    } else {
        format!("{}s", secs)
    }
}

pub(crate) fn confirmline(prompt: &str, mode: ColorMode) -> std::io::Result<bool> {
    if pretty_enabled(mode) {
        eprint!("     ✔ {prompt} (y/N) · ");
    } else {
        eprint!("{prompt} (y/N): ");
    }
    std::io::stderr().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let normalized = input.trim().to_ascii_lowercase();

    Ok(matches!(normalized.as_str(), "y" | "yes"))
}

pub(crate) fn with_terraforming<T, E, F>(mode: ColorMode, action: F) -> std::result::Result<T, E>
where
    F: FnOnce() -> std::result::Result<T, E>,
{
    if !pretty_enabled(mode) {
        return action();
    }

    let running = Arc::new(AtomicBool::new(true));
    let signal = Arc::clone(&running);
    let painter_mode = mode;

    let handle = std::thread::spawn(move || {
        let dots = [".", "..", "...", "...."];
        let mut idx = 0usize;
        let start = std::time::Instant::now();
        while signal.load(Ordering::Relaxed) {
            let elapsed = start.elapsed().as_secs();
            let time_suffix = if elapsed >= 5 {
                let mins = elapsed / 60;
                let secs = elapsed % 60;
                if mins > 0 {
                    format!(" ({}m {}s)", mins, secs)
                } else {
                    format!(" ({}s)", secs)
                }
            } else {
                String::new()
            };
            let frame = format!(
                "     🔄 Terraforming{:<4}{}",
                dots[idx % dots.len()],
                time_suffix
            );
            let painted = paint(&frame, "36", painter_mode);
            eprint!("\r\x1b[2K{painted}");
            let _ = std::io::stderr().flush();
            idx += 1;
            std::thread::sleep(Duration::from_millis(140));
        }
        eprint!("\r\x1b[2K");
        let _ = std::io::stderr().flush();
    });

    let result = action();
    running.store(false, Ordering::Relaxed);
    let _ = handle.join();
    result
}

pub(crate) fn success(msg: &str, mode: ColorMode) {
    let decorated = with_prefix(msg, "🎉", "OK", mode);
    eprintln!("{}", paint(&decorated, "32", mode));
}

pub(crate) fn castle(msg: &str, mode: ColorMode) {
    info(&with_prefix(msg, "🏰", "INFO", mode), mode);
}

pub(crate) fn info(msg: &str, mode: ColorMode) {
    eprintln!("{}", paint(msg, "36", mode));
}

pub(crate) fn warn(msg: &str, mode: ColorMode) {
    let decorated = with_prefix(msg, "🚧", "WARN", mode);
    eprintln!("{}", paint(&decorated, "33", mode));
}

pub(crate) fn error(msg: &str, mode: ColorMode) {
    let decorated = with_prefix(msg, "❌", "ERROR", mode);
    eprintln!("{}", paint(&decorated, "31", mode));
}

pub(crate) fn status_line(name: &str, icon: &str, status: &str, detail: &str, mode: ColorMode) {
    if pretty_enabled(mode) {
        eprintln!("  {icon} {name:<14} {status:<10} {detail}");
    } else {
        eprintln!("{name}: {status} {detail}");
    }
}

pub(crate) fn hint_line(msg: &str, mode: ColorMode) {
    if pretty_enabled(mode) {
        eprintln!(
            "{}",
            paint(&format!("                    → {msg}"), "33", mode)
        );
    } else {
        eprintln!("  HINT: {msg}");
    }
}

pub(crate) fn section_line(title: &str, mode: ColorMode) {
    if pretty_enabled(mode) {
        eprintln!();
        eprintln!("  {title}");
        eprintln!("  {}", "─".repeat(56));
    } else {
        eprintln!();
        eprintln!("[{title}]");
    }
}

pub(crate) fn with_spinner<T, E, F>(
    label: &str,
    mode: ColorMode,
    action: F,
) -> std::result::Result<T, E>
where
    F: FnOnce() -> std::result::Result<T, E>,
{
    if !pretty_enabled(mode) {
        return action();
    }

    let running = Arc::new(AtomicBool::new(true));
    let signal = Arc::clone(&running);
    let painter_mode = mode;
    let label = label.to_string();

    let handle = std::thread::spawn(move || {
        let dots = [".", "..", "...", "...."];
        let mut idx = 0usize;
        while signal.load(Ordering::Relaxed) {
            let frame = format!("     🔄 {}{:<4}", label, dots[idx % dots.len()]);
            let painted = paint(&frame, "36", painter_mode);
            eprint!("\r{painted}");
            let _ = std::io::stderr().flush();
            idx += 1;
            std::thread::sleep(Duration::from_millis(140));
        }
        eprint!("\r\x1b[2K");
        let _ = std::io::stderr().flush();
    });

    let result = action();
    running.store(false, Ordering::Relaxed);
    let _ = handle.join();
    result
}
