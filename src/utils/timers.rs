//! Port of `lib/utils/timers.js`: records named timing spans and writes a
//! `<logpath>timing.json` file when `--timing` is enabled.

use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct TimingRecord {
    pub name: String,
    pub start: u64,
    pub duration: u64,
}

pub struct Timers {
    path: Option<std::path::PathBuf>,
    timing: bool,
    started: Instant,
    list: BTreeMap<String, TimingRecord>,
}

impl Default for Timers {
    fn default() -> Self {
        Self {
            path: None,
            timing: false,
            started: Instant::now(),
            list: BTreeMap::new(),
        }
    }
}

impl Timers {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(&mut self, path: impl Into<std::path::PathBuf>, timing: bool) {
        self.path = Some(path.into());
        self.timing = timing;
    }

    pub fn started(&self) -> Instant {
        self.started
    }

    /// Runs `f`, recording how long it took under `name`.
    pub fn start<T>(&mut self, name: &str, f: impl FnOnce() -> T) -> T {
        let start = self.started.elapsed().as_millis() as u64;
        let begin = Instant::now();
        let result = f();
        let duration = begin.elapsed().as_millis() as u64;
        self.list.insert(
            name.to_string(),
            TimingRecord {
                name: name.to_string(),
                start,
                duration,
            },
        );
        result
    }

    pub fn add(&mut self, name: &str, start: u64, duration: u64) {
        self.list.insert(
            name.to_string(),
            TimingRecord {
                name: name.to_string(),
                start,
                duration,
            },
        );
    }

    pub fn finish(&mut self, id: &str, command: &[String], logfiles: &[String], version: &str) {
        if !self.timing {
            self.off();
            return;
        }
        let mut data = Map::new();
        data.insert("id".into(), json!(id));
        data.insert("command".into(), json!(command));
        data.insert("logfiles".into(), json!(logfiles));
        data.insert("version".into(), json!(version));
        data.insert("elapsed".into(), json!(self.started.elapsed().as_millis() as u64));
        data.insert(
            "timers".into(),
            Value::Array(
                self.list
                    .values()
                    .map(|t| {
                        json!({
                            "name": t.name,
                            "start": t.start,
                            "duration": t.duration,
                        })
                    })
                    .collect(),
            ),
        );
        if let (Some(path), Ok(text)) = (&self.path, serde_json::to_string(&data)) {
            let _ = std::fs::write(path.join("timing.json"), text);
        }
        self.off();
    }

    /// Clears recorded timers. Called from `Npm::unload`.
    pub fn off(&mut self) {
        self.list.clear();
    }
}
