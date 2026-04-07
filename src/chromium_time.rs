use anyhow::{Context, Result};
use chrono::Utc;

/// Chromium epoch is 1601-01-01 00:00:00 UTC.
/// Chromium timestamps are microseconds since that epoch.
/// The offset between Unix epoch (1970-01-01) and Chromium epoch is
/// 11644473600 seconds.
const UNIX_TO_CHROMIUM_EPOCH_OFFSET_US: i64 = 11_644_473_600 * 1_000_000;

/// Generate a Chromium timestamp string for the current time.
/// Stored as a string of digits in the Preferences JSON.
pub fn now() -> Result<String> {
    let unix_us = Utc::now().timestamp_micros();
    let chromium_us = unix_us
        .checked_add(UNIX_TO_CHROMIUM_EPOCH_OFFSET_US)
        .context("Chromium timestamp overflow")?;
    Ok(chromium_us.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_is_reasonable() {
        let ts: i64 = now().unwrap().parse().unwrap();
        // Post-2024 in Chromium epoch: 2024-01-01 ~= 13_348_876_800_000_000
        assert!(ts > 13_348_876_800_000_000);
        // Pre-2100 in Chromium epoch: 2100-01-01 ~= 15_745_036_800_000_000
        assert!(ts < 15_745_036_800_000_000);
    }

    #[test]
    fn test_timestamp_format() {
        let ts = now().unwrap();
        assert!(ts.chars().all(|c| c.is_ascii_digit()));
        assert!(!ts.is_empty());
    }
}
