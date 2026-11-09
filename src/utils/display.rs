//! Port of `lib/utils/display.js`. Handles all log/output rendering: level
//! filtering, colors, prefixing, notice de-duplication and buffering until
//! `load` has run.

use crate::utils::log::Level;
use std::collections::HashSet;
use std::io::{IsTerminal, Write};
use std::sync::Mutex;

/// Named foreground colors npm uses. Never use background colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BlueBright,
    CyanBright,
    MagentaBright,
}

impl Color {
    fn ansi_fg(self) -> &'static str {
        match self {
            Color::Red => "31",
            Color::Green => "32",
            Color::Yellow => "33",
            Color::Blue => "34",
            Color::Magenta => "35",
            Color::Cyan => "36",
            Color::White => "37",
            Color::BlueBright => "94",
            Color::CyanBright => "96",
            Color::MagentaBright => "95",
        }
    }
}

/// Colors used per log level, mirroring `COLOR_PALETTE`.
pub fn palette_for(level: Level) -> Color {
    match level {
        Level::Silly => Color::Blue,
        Level::Verbose => Color::Blue,
        Level::Info => Color::Cyan,
        Level::Http => Color::Green,
        Level::Timeline => Color::MagentaBright,
        Level::Notice => Color::CyanBright,
        Level::Warn => Color::Yellow,
        Level::Error => Color::Red,
        Level::Silent => Color::White,
    }
}

pub fn paint(text: &str, color: Color, enabled: bool) -> String {
    if enabled {
        format!("\x1b[{}m{}\x1b[0m", color.ansi_fg(), text)
    } else {
        text.to_string()
    }
}

pub fn bold(text: &str, enabled: bool) -> String {
    if enabled {
        format!("\x1b[1m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

pub fn underline(text: &str, enabled: bool) -> String {
    if enabled {
        format!("\x1b[4m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

#[derive(Debug, Clone)]
pub struct DisplayOptions {
    pub loglevel: Level,
    pub stdout_color: bool,
    pub stderr_color: bool,
    pub timing: bool,
    pub unicode: bool,
    pub progress: bool,
    pub json: bool,
    pub heading: String,
}

impl Default for DisplayOptions {
    fn default() -> Self {
        Self {
            loglevel: Level::Notice,
            stdout_color: false,
            stderr_color: false,
            timing: false,
            unicode: true,
            progress: true,
            json: false,
            heading: "npm".into(),
        }
    }
}

struct State {
    opts: DisplayOptions,
    buffering: bool,
    log_buffer: Vec<(Level, String, String)>,
    output_buffer: Vec<(bool, String)>,
    seen_notices: HashSet<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            opts: DisplayOptions::default(),
            buffering: true,
            log_buffer: Vec::new(),
            output_buffer: Vec::new(),
            seen_notices: HashSet::new(),
        }
    }
}

pub struct Display {
    state: Mutex<State>,
}

impl Default for Display {
    fn default() -> Self {
        Self::new()
    }
}

impl Display {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State::default()),
        }
    }

    /// Switches from buffering to live rendering. Buffered log lines are
    /// flushed in order at this point.
    pub fn load(&self, opts: DisplayOptions) {
        let mut state = self.state.lock().expect("display lock");
        state.opts = opts;
        state.buffering = false;
        let buffered = std::mem::take(&mut state.log_buffer);
        for (level, prefix, body) in buffered {
            Self::write_log(&state, level, &prefix, &body);
        }
    }

    pub fn off(&self) {
        let mut state = self.state.lock().expect("display lock");
        state.log_buffer.clear();
        state.output_buffer.clear();
        state.seen_notices.clear();
    }

    pub fn log(&self, level: Level, prefix: &str, body: &str) {
        let mut state = self.state.lock().expect("display lock");
        if state.buffering {
            state.log_buffer.push((level, prefix.to_string(), body.to_string()));
            return;
        }
        if level == Level::Notice && state.opts.loglevel < Level::Verbose {
            let notice_key = format!("{prefix}\u{0}{body}");
            if !state.seen_notices.insert(notice_key) {
                return;
            }
        }
        Self::write_log(&state, level, prefix, body);
    }

    pub fn standard(&self, body: &str) {
        let mut state = self.state.lock().expect("display lock");
        if state.buffering {
            state.output_buffer.push((true, body.to_string()));
            return;
        }
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{body}");
    }

    pub fn error(&self, body: &str) {
        let mut state = self.state.lock().expect("display lock");
        if state.buffering {
            state.output_buffer.push((false, body.to_string()));
            return;
        }
        let mut err = std::io::stderr().lock();
        let _ = writeln!(err, "{body}");
    }

    /// Emits buffered stdout/stderr lines; called from `Npm::finish`.
    pub fn flush(&self) {
        let mut state = self.state.lock().expect("display lock");
        let buffered = std::mem::take(&mut state.output_buffer);
        state.buffering = false;
        for (to_stdout, body) in buffered {
            if to_stdout {
                let mut out = std::io::stdout().lock();
                let _ = writeln!(out, "{body}");
            } else {
                let mut err = std::io::stderr().lock();
                let _ = writeln!(err, "{body}");
            }
        }
    }

    fn write_log(state: &State, level: Level, title: &str, body: &str) {
        if !Self::should_show(state, level) {
            return;
        }
        let color = state.opts.stderr_color;
        let heading = bold(&state.opts.heading, color);
        let level_text = paint(level.as_str(), palette_for(level), color);
        let title_text = if title.is_empty() {
            String::new()
        } else {
            format!("{} ", paint(title, Color::BlueBright, color))
        };
        let mut err = std::io::stderr().lock();
        let _ = writeln!(err, "{heading} {level_text} {title_text}{body}");
    }

    fn should_show(state: &State, level: Level) -> bool {
        if level == Level::Timeline {
            return state.opts.timing && state.opts.loglevel != Level::Silent;
        }
        level >= state.opts.loglevel
    }

    /// Whether output should be colored based on tty detection and
    /// `--color` / `--no-color`.
    pub fn color_supported(&self, stdout: bool) -> bool {
        if stdout {
            std::io::stdout().is_terminal()
        } else {
            std::io::stderr().is_terminal()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silly_hidden_at_notice() {
        let display = Display::new();
        display.load(DisplayOptions::default());
        assert!(!Display::should_show(
            &display.state.lock().unwrap(),
            Level::Silly
        ));
    }

    #[test]
    fn paint_adds_ansi_when_enabled() {
        assert_eq!(paint("x", Color::Red, true), "\x1b[31mx\x1b[0m");
        assert_eq!(paint("x", Color::Red, false), "x");
    }
}
