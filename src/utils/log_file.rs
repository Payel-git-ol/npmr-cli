//! Port of `lib/utils/log-file.js`. Buffers early log events, then writes
//! them to `debug-<n>.log` files under the configured log path, rotating
//! after a fixed number of events and pruning old files on load.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::utils::log::Level;

const MAX_LOGS_PER_FILE: usize = 50_000;
const MAX_FILES_PER_PROCESS: usize = 5;

enum Sink {
    /// Writes that arrive before `load` knows where the log path lives.
    Buffered(Vec<(Level, String)>),
    File(LogFile),
    Ended,
}

struct LogFile {
    path: PathBuf,
    file: File,
    events: usize,
}

pub struct LogFiles {
    sink: Sink,
    path: Option<PathBuf>,
    logs_max: usize,
    timing: bool,
    files: Vec<PathBuf>,
    total: usize,
    enabled: bool,
}

impl Default for LogFiles {
    fn default() -> Self {
        Self::new()
    }
}

impl LogFiles {
    pub fn new() -> Self {
        Self {
            sink: Sink::Buffered(Vec::new()),
            path: None,
            logs_max: 10,
            timing: false,
            files: Vec::new(),
            total: 0,
            enabled: true,
        }
    }

    pub fn off(&mut self) {
        self.enabled = false;
        self.sink = Sink::Ended;
    }

    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    /// Points the logger at `path` and flushes any buffered events. Returns
    /// the log files kept after pruning.
    pub fn load(
        &mut self,
        command: Option<&str>,
        path: PathBuf,
        logs_max: usize,
        timing: bool,
    ) -> Vec<PathBuf> {
        if command == Some("completion") || !self.enabled {
            return Vec::new();
        }
        self.path = Some(path);
        self.logs_max = logs_max;
        self.timing = timing;

        let buffered = match std::mem::replace(&mut self.sink, Sink::Ended) {
            Sink::Buffered(items) => items,
            other => {
                self.sink = other;
                return Vec::new();
            }
        };

        if logs_max > 0 {
            if let Some(file) = self.open_log_file() {
                self.sink = Sink::File(file);
                for (level, body) in buffered {
                    self.write_event(level, &body);
                }
            }
        }
        self.clean_logs()
    }

    /// Records a log event. Synchronous on purpose: npm also logs from the
    /// process exit path, where async writes would be dropped.
    pub fn log(&mut self, level: Level, title: &str, body: &str) {
        if !self.enabled {
            return;
        }
        let value = if title.is_empty() {
            body.to_string()
        } else {
            format!("{title} {body}")
        };
        if let Sink::Buffered(items) = &mut self.sink {
            items.push((level, value));
            return;
        }
        self.write_event(level, &value);
    }

    fn write_event(&mut self, level: Level, body: &str) {
        if level == Level::Timeline && !self.timing {
            return;
        }
        let line = format!("{} {} {}", self.total, level.as_str(), body);
        self.total += 1;

        let need_rotate = match &self.sink {
            Sink::File(f) => f.events >= MAX_LOGS_PER_FILE,
            _ => return,
        };
        if need_rotate {
            if self.files.len() >= MAX_FILES_PER_PROCESS {
                self.off();
                return;
            }
            if let Some(file) = self.open_log_file() {
                self.sink = Sink::File(file);
                if let Sink::File(f) = &mut self.sink {
                    let _ = writeln!(f.file, "{line}");
                    f.events += 1;
                }
            }
            return;
        }
        if let Sink::File(f) = &mut self.sink {
            let _ = writeln!(f.file, "{line}");
            f.events += 1;
        }
    }

    fn log_file_path(&self, count: Option<usize>) -> PathBuf {
        let base = self.path.clone().unwrap_or_default();
        let suffix = match count {
            Some(c) => format!("debug-{c:02}.log"),
            None => "debug-.log".to_string(),
        };
        base.with_file_name(suffix)
    }

    fn open_log_file(&mut self) -> Option<LogFile> {
        let count = self.files.len();
        let path = self.log_file_path(Some(count));
        match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(file) => {
                self.files.push(path.clone());
                Some(LogFile {
                    path,
                    file,
                    events: 0,
                })
            }
            Err(e) => {
                crate::utils::log::verbose(
                    "logfile",
                    &[&format!("could not be created: {e}")],
                );
                None
            }
        }
    }

    fn clean_logs(&self) -> Vec<PathBuf> {
        let Some(dir) = self.path.as_deref().and_then(Path::parent) else {
            return Vec::new();
        };
        let mut candidates: Vec<PathBuf> = match std::fs::read_dir(dir) {
            Ok(entries) => entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with("debug-") && n.ends_with(".log"))
                        .unwrap_or(false)
                })
                .filter(|p| !self.files.contains(p))
                .collect(),
            Err(_) => return Vec::new(),
        };
        candidates.sort();
        let to_delete = candidates.len().saturating_sub(self.logs_max);
        for file in candidates.drain(..to_delete) {
            let _ = std::fs::remove_file(file);
        }
        self.files.clone()
    }
}
