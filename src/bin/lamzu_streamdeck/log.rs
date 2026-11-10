//! File logging for the plugin.
//!
//! Stream Deck plugins have no console, so diagnostics go to
//! `%TEMP%\lamzu-streamdeck.log`. The file is rotated to `.log.old` once it
//! exceeds [`MAX_LOG_BYTES`] so it can't grow without bound.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_LOG_BYTES: u64 = 1024 * 1024;

pub fn log_path() -> PathBuf {
    std::env::temp_dir().join("lamzu-streamdeck.log")
}

/// Append a line to the log file. Errors are ignored: logging must never
/// take the plugin down.
pub fn log(msg: impl AsRef<str>) {
    let path = log_path();

    if fs::metadata(&path).is_ok_and(|m| m.len() > MAX_LOG_BYTES) {
        let _ = fs::rename(&path, path.with_extension("log.old"));
    }

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "[{}] {}", timestamp(), msg.as_ref());
    }
}

/// Route panics to the log file so crashes leave a trace.
pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log(format!("PANIC: {info}"));
        default_hook(info);
    }));
}

/// Current UTC time formatted as `YYYY-MM-DD HH:MM:SS`.
fn timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (y, m, d) = civil_from_days(days as i64);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Convert days since 1970-01-01 to a (year, month, day) civil date.
/// Algorithm from Howard Hinnant's `chrono`-compatible date library.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::civil_from_days;

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
        assert_eq!(civil_from_days(20_766), (2026, 11, 9));
    }
}
