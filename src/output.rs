//! Port of the `output` half of `proc-log` plus the small display helpers
//! npm wraps around it (`standard`, `error`, flush on exit).

use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

static META_FLAG: AtomicBool = AtomicBool::new(false);
static PENDING_LINES: Mutex<Vec<String>> = Mutex::new(Vec::new());
static STATUS: AtomicI32 = AtomicI32::new(0);

/// Sets `process.exitCode`, the way commands that swallow an error do.
pub fn set_exit_code(code: i32) {
    STATUS.store(code, Ordering::Relaxed);
}

pub fn exit_code() -> i32 {
    STATUS.load(Ordering::Relaxed)
}

/// Marks output that belongs to machine readable results (`--json`).
pub fn set_meta(enabled: bool) {
    META_FLAG.store(enabled, Ordering::Relaxed);
}

pub fn meta() -> bool {
    META_FLAG.load(Ordering::Relaxed)
}

/// Queues a line of output. When json/meta mode is on the queued lines are
/// joined and written as a single block on flush, matching npm's behaviour of
/// not interleaving log lines with json results.
pub fn standard(line: impl Into<String>) {
    let line = line.into();
    if meta() {
        if let Ok(mut pending) = PENDING_LINES.lock() {
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
    let drained = PENDING_LINES.lock().map(|mut p| std::mem::take(&mut *p));
    if let Ok(lines) = drained {
        if lines.is_empty() {
            return;
        }
        let joined = lines.join("\n");
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{joined}");
    }
}
