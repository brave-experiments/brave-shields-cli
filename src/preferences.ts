import * as nodePath from 'node:path';
import psList from 'ps-list';
import { PortError } from './errors.js';
import type { Channel } from './platform.js';
import { channelDisplay } from './platform.js';
import {
  atomicWriteJsonFile,
  getPointer,
  getPointerMut,
  lockedReadModifyWriteJson,
  readJsonFileSync,
} from './json-io.js';
import type { JsonObject, JsonValue } from './json.js';

/** The path within the Preferences JSON where shield exceptions live. */
const EXCEPTIONS_PATH: readonly string[] = ['profile', 'content_settings', 'exceptions'];

/** Read and parse a Preferences JSON file. */
export function readPreferences(target: string): JsonValue {
  return readJsonFileSync(target, 'Preferences');
}

/** Navigate to the exceptions object, returning null if any intermediate key is missing. */
export function getExceptions(prefs: JsonValue): JsonObject | null {
  return getPointer(prefs, EXCEPTIONS_PATH);
}

/** Navigate to the exceptions object, creating intermediate objects as needed. */
export function getExceptionsMut(prefs: JsonValue): JsonObject {
  return getPointerMut(prefs, EXCEPTIONS_PATH);
}

/** Get a specific exception type (e.g. "braveShields") from the exceptions object. */
export function getExceptionEntries(
  prefs: JsonValue,
  key: string,
): JsonObject | null {
  const exceptions = getExceptions(prefs);
  if (exceptions === null) return null;
  const entries = exceptions[key];
  if (entries === null || typeof entries !== 'object' || Array.isArray(entries)) {
    return null;
  }
  return entries as JsonObject;
}

/** Get a mutable exception type object, creating it if missing. */
export function getExceptionEntriesMut(prefs: JsonValue, key: string): JsonObject {
  const exceptions = getExceptionsMut(prefs);
  const entries = exceptions[key];
  if (entries === null || typeof entries !== 'object' || Array.isArray(entries)) {
    exceptions[key] = {};
  }
  return exceptions[key] as JsonObject;
}

/** Write preferences to disk atomically, with backup and verification. */
export function writePreferences(target: string, prefs: JsonValue): void {
  atomicWriteJsonFile(target, 'Preferences', prefs);
}

/**
 * Open the Preferences file with an exclusive lock, read-modify-write, then
 * release the lock.
 *
 * Note: the lock is held on the original file descriptor. After atomic
 * rename, the locked fd points to the old inode. A concurrent process
 * opening the file after the rename would not see the lock. This is
 * acceptable for a single-user CLI tool but would not be safe under
 * concurrent writers.
 */
export function lockedReadModifyWrite(
  target: string,
  modify: (prefs: JsonValue) => void,
): void {
  lockedReadModifyWriteJson(target, 'Preferences', modify);
}

/** Process names for each Brave channel per platform. */
function braveProcessNames(channel: Channel): readonly string[] {
  switch (process.platform) {
    case 'darwin':
      switch (channel) {
        case 'release':
          return ['Brave Browser'];
        case 'beta':
          return ['Brave Browser Beta'];
        case 'nightly':
          return ['Brave Browser Nightly'];
        case 'dev':
          return ['Brave Browser Development'];
      }
      break;
    case 'linux':
      switch (channel) {
        case 'release':
          return ['brave-browser'];
        case 'beta':
          return ['brave-browser-beta'];
        case 'nightly':
          return ['brave-browser-nightly'];
        case 'dev':
          return ['brave-browser-dev'];
      }
      break;
    default:
      // Unsupported platform: no process names, never "running"
      // (matches Rust #[cfg] behavior).
      return [];
  }
  return [];
}

/**
 * Check if a specific Brave channel is currently running.
 * Throws by default; pass `force: true` to downgrade to a warning
 * (returned as `warning` for the CLI layer to print to stderr).
 */
export async function checkBraveNotRunning(
  channel: Channel,
  force: boolean,
): Promise<{ warning?: string }> {
  const expectedNames = braveProcessNames(channel);
  const processes = await psList();
  let braveRunning = false;
  for (const p of processes) {
    for (const expected of expectedNames) {
      if (p.name === expected) {
        braveRunning = true;
        break;
      }
    }
  }
  if (braveRunning) {
    if (force) {
      return {
        warning: `warning: Brave ${channelDisplay(channel)} appears to be running. Changes may be overwritten by the browser.`,
      };
    }
    throw new PortError(
      `Brave ${channelDisplay(channel)} is running. Quit the browser first, or use --force to write anyway.`,
    );
  }
  return {};
}

export function preferencesPath(braveDir: string, profileDir: string): string {
  return nodePath.join(braveDir, profileDir, 'Preferences');
}