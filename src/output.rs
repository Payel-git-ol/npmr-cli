//! Port of the `output` half of `proc-log` plus the small display helpers
//! npm wraps around it (`standard`, `error`, flush on exit).

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static META: AtomicBool = AtomicBool::new(false);
static PENDING: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Marks output that belongs to machine readable results (`--json`).
pub fn set_meta(enabled: bool) {
    META.store(enabled, Ordering::Relaxed);
}

pub fn meta() -> bool {
    META.load(Ordering::Relaxed)
}

/// Queues a line of output. When json/meta mode is on the queued lines are
/// joined and written as a single block on flush, matching npm's behaviour of
/// not interleaving log lines with json results.
pub fn standard(line: impl Into<String>) {
    let line = line.into();
    if meta() {
        if let Ok(mut pending) = PENDING.lock() {
            pending.push(line);
            return;
        }
    }
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{line}");
}

pub fn error(line: impl Into<String>) {
    let mut err = std::io::stderr().lock();
    let _ = writeln!(err, "{}", line.into());
}

/// Writes any buffered meta output and clears the buffer.
pub fn flush() {
    let drained = PENDING.lock().map(|mut p| std::mem::take(&mut *p));
    if let Ok(lines) = drained {
        if lines.is_empty() {
            return;
        }
        let joined = lines.join("\n");
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{joined}");
    }
}
