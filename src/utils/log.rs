//! Replacement for the `proc-log` package. A process wide logger plus the
//! `output` helpers used by commands to write to stdout/stderr.

use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

/// Log levels, ordered from most to least verbose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Silly = 0,
    Verbose = 1,
    Info = 2,
    Timeline = 3,
    Http = 4,
    Notice = 5,
    Warn = 6,
    Error = 7,
    Silent = 8,
}

impl Level {
    pub fn from_str_lossy(s: &str) -> Option<Level> {
        Some(match s.to_ascii_lowercase().as_str() {
            "silly" => Level::Silly,
            "verbose" => Level::Verbose,
            "info" => Level::Info,
            "timeline" => Level::Timeline,
            "http" => Level::Http,
            "notice" => Level::Notice,
            "warn" => Level::Warn,
            "error" => Level::Error,
            "silent" => Level::Silent,
            _ => return None,
        })
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Level::Silly => "silly",
            Level::Verbose => "verbose",
            Level::Info => "info",
            Level::Timeline => "timeline",
            Level::Http => "http",
            Level::Notice => "notice",
            Level::Warn => "warn",
            Level::Error => "error",
            Level::Silent => "silent",
        }
    }
}

pub type LogHandler = Box<dyn Fn(Level, &str, &str) + Send + Sync>;

static LEVEL: AtomicU8 = AtomicU8::new(Level::Notice as u8);
static COLOR: AtomicBool = AtomicBool::new(false);
static HANDLER: OnceLock<Mutex<Option<LogHandler>>> = OnceLock::new();

pub fn set_level(level: Level) {
    LEVEL.store(level as u8, Ordering::Relaxed);
}

pub fn level() -> Level {
    match LEVEL.load(Ordering::Relaxed) {
        0 => Level::Silly,
        1 => Level::Verbose,
        2 => Level::Info,
        3 => Level::Timeline,
        4 => Level::Http,
        5 => Level::Notice,
        6 => Level::Warn,
        7 => Level::Error,
        _ => Level::Silent,
    }
}

pub fn set_color(enabled: bool) {
    COLOR.store(enabled, Ordering::Relaxed);
}

pub fn color() -> bool {
    COLOR.load(Ordering::Relaxed)
}

/// Installs the sink that receives every message passing the level filter.
/// The CLI installs a file logger here; tests leave it unset.
pub fn set_handler(handler: LogHandler) {
    let slot = HANDLER.get_or_init(|| Mutex::new(None));
    *slot.lock().expect("log handler lock poisoned") = Some(handler);
}

fn emit(level: Level, prefix: &str, body: &str) {
    if level < self::level() {
        return;
    }
    let slot = HANDLER.get_or_init(|| Mutex::new(None));
    if let Some(handler) = slot.lock().expect("log handler lock poisoned").as_ref() {
        handler(level, prefix, body);
        return;
    }
    // Without a handler npm still prints warnings and errors to stderr so the
    // user is never left without feedback.
    if level >= Level::Warn {
        let style = if color() { "33" } else { "0" };
        eprintln!(
            "\x1b[{}mnpm \x1b[{}m{} \x1b[0m{}",
            if color() { "33" } else { "0" },
            style,
            level.as_str(),
            body
        );
    }
}

pub fn log(level: Level, prefix: &str, parts: &[&dyn std::fmt::Display]) {
    let body = parts
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    emit(level, prefix, &body);
}

macro_rules! log_macros {
    ($($name:ident => $level:ident),* $(,)?) => {
        $(
            pub fn $name(prefix: &str, parts: &[&dyn std::fmt::Display]) {
                log(Level::$level, prefix, parts)
            }
        )*
    };
}

log_macros! {
    silly => Silly,
    verbose => Verbose,
    info => Info,
    timeline => Timeline,
    http => Http,
    notice => Notice,
    warn => Warn,
    error => Error,
}

/// Immediate stderr write, never filtered by log level.
pub fn output_error(msg: &str) {
    let mut err = std::io::stderr().lock();
    let _ = writeln!(err, "{msg}");
}

/// Immediate stdout write, never filtered; this is what `npm` prints as
/// results (as opposed to log lines).
pub fn standard(msg: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{msg}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_ord_is_silly_lowest() {
        assert!(Level::Silly < Level::Info);
        assert!(Level::Error < Level::Silent);
    }

    #[test]
    fn parses_levels() {
        assert_eq!(Level::from_str_lossy("WARN"), Some(Level::Warn));
        assert_eq!(Level::from_str_lossy("nope"), None);
    }
}
