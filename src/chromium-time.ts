/**
 * Chromium epoch is 1601-01-01 00:00:00 UTC.
 * Chromium timestamps are microseconds since that epoch.
 * The offset between Unix epoch (1970-01-01) and Chromium epoch is
 * 11644473600 seconds.
 */
const UNIX_TO_CHROMIUM_EPOCH_OFFSET_US = 11_644_473_600n * 1_000_000n;

/**
 * Generate a Chromium timestamp string for the current time.
 * Stored as a string of digits in the Preferences JSON.
 *
 * Values are microseconds since 1601 (~1.3e16), beyond Number.MAX_SAFE_INTEGER,
 * so all math is BigInt. Never use floating-point math here.
 */
export function chromiumTimeNow(): string {
  const unixUs = BigInt(Date.now()) * 1000n;
  const chromiumUs = unixUs + UNIX_TO_CHROMIUM_EPOCH_OFFSET_US;
  return chromiumUs.toString();
}