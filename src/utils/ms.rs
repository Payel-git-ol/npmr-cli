//! Port of `lib/utils/timers.js` duration formatting used by reify output.

use std::time::Duration;

/// Formats a duration the way the `ms` package does (`19.157s`, `1m 3s`).
pub fn ms(duration: Duration) -> String {
    let total = duration.as_millis();
    if total < 1000 {
        format!("{total}ms")
    } else if total < 60_000 {
        let secs = duration.as_secs_f64();
        format!("{secs:.3}s")
    } else if total < 3_600_000 {
        let minutes = total / 60_000;
        let seconds = (total % 60_000) / 1000;
        if seconds == 0 {
            format!("{minutes}m")
        } else {
            format!("{minutes}m {seconds}s")
        }
    } else if total < 86_400_000 {
        let hours = total / 3_600_000;
        let minutes = (total % 3_600_000) / 60_000;
        if minutes == 0 {
            format!("{hours}h")
        } else {
            format!("{hours}h {minutes}m")
        }
    } else {
        let days = total / 86_400_000;
        let hours = (total % 86_400_000) / 3_600_000;
        if hours == 0 {
            format!("{days}d")
        } else {
            format!("{days}d {hours}h")
        }
    }
}
