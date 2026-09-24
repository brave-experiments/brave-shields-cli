import { constants, flockSync } from 'fs-ext';
import * as crypto from 'node:crypto';
import * as fs from 'node:fs';
import * as nodePath from 'node:path';
import { PortError } from './errors.js';
import { describeIoError } from './fsutil.js';
import { sortedStringify } from './json.js';
import type { JsonObject, JsonValue } from './json.js';

/**
 * Shared JSON file persistence pipeline used by both the Preferences and
 * Local State layers. Replicates the Rust safety pipeline:
 *   create `<file>.bak` only if absent -> temp file -> rename -> read-back
 *   parse verification.
 */

function readJsonFileSync(target: string, label: string): JsonValue {
  let content: string;
  try {
    content = fs.readFileSync(target, 'utf8');
  } catch (err) {
    throw new PortError(`failed to read ${label} at ${target}: ${describeIoError(err)}`);
  }
  try {
    return JSON.parse(content) as JsonValue;
  } catch (err) {
    throw new PortError(`failed to parse ${label} as JSON: ${describeIoError(err)}`);
  }
}

/**
 * Write a JSON file atomically:
 * - `content` is serialized with sorted keys and 2-space indent
 *   (serde_json `to_string_pretty` on a BTreeMap-backed document).
 * - Creates a `<label>.bak` backup in the same directory only if the target
 *   exists and no backup exists yet.
 * - Writes to a temp file in the same directory (so `rename` is atomic on
 *   POSIX), then renames it over the target.
 * - Re-reads and parses the file after writing to verify valid JSON.
 */
function atomicWriteJsonFile(target: string, label: string, value: JsonValue): void {
  const content = sortedStringify(value);
  const dir = nodePath.dirname(target);
  if (dir.length === 0) {
    throw new PortError(`${label} path has no parent directory`);
  }

  // Create a backup of the current file if it exists and no backup exists yet.
  const bakPath = nodePath.join(dir, `${label}.bak`);
  if (fs.existsSync(target) && !fs.existsSync(bakPath)) {
    try {
      fs.copyFileSync(target, bakPath);
    } catch (err) {
      throw new PortError(`failed to create backup at ${bakPath}: ${describeIoError(err)}`);
    }
  }

  // Temp file in the same directory as the target (rename atomicity).
  const tmpPath = nodePath.join(
    dir,
    `.${nodePath.basename(target)}.tmp-${process.pid}-${crypto.randomBytes(6).toString('hex')}`,
  );
  let tmpFd: number;
  try {
    tmpFd = fs.openSync(tmpPath, 'wx');
  } catch {
    throw new PortError('failed to create temp file for atomic write');
  }
  try {
    try {
      fs.writeFileSync(tmpFd, content);
    } catch (err) {
      throw new PortError(`failed to write temp file at ${tmpPath}: ${describeIoError(err)}`);
    }
    fs.closeSync(tmpFd);
    tmpFd = -1;
    try {
      fs.renameSync(tmpPath, target);
    } catch (err) {
      throw new PortError(`failed to persist temp file to ${target}: ${describeIoError(err)}`);
    }
  } finally {
    if (tmpFd !== -1) {
      try {
        fs.closeSync(tmpFd);
      } catch {
        // ignore
      }
    }
    try {
      fs.unlinkSync(tmpPath);
    } catch {
      // already renamed away
    }
  }

  // Post-write verification — read back and parse to confirm valid JSON.
  let readback: string;
  try {
    readback = fs.readFileSync(target, 'utf8');
  } catch (err) {
    throw new PortError(
      `post-write verification: failed to read back ${target}: ${describeIoError(err)}`,
    );
  }
  try {
    JSON.parse(readback);
  } catch {
    throw new PortError('post-write verification: file is not valid JSON after write');
  }
}

/**
 * Open the file with an exclusive lock (flock), read-modify-write, then
 * release the lock.
 *
 * Note: the lock is held on the original file descriptor. After atomic
 * rename, the locked fd points to the old inode. A concurrent process
 * opening the file after the rename would not see the lock. This is
 * acceptable for a single-user CLI tool but would not be safe under
 * concurrent writers. (Same caveat as the Rust implementation.)
 */
function lockedReadModifyWriteJson(
  target: string,
  label: string,
  modify: (value: JsonValue) => void,
): void {
  // Ensure the file exists so we can lock it.
  if (!fs.existsSync(target)) {
    throw new PortError(`${label} file not found at ${target}`);
  }

  let fd: number;
  try {
    fd = fs.openSync(target, 'r');
  } catch (err) {
    throw new PortError(`failed to open ${target} for locking: ${describeIoError(err)}`);
  }

  try {
    try {
      flockSync(fd, constants.LOCK_EX);
    } catch (err) {
      throw new PortError(
        `failed to acquire exclusive lock on ${target}: ${describeIoError(err)}`,
      );
    }

    // Read via the filesystem (not the locked fd) to get full content reliably.
    let content: string;
    try {
      content = fs.readFileSync(target, 'utf8');
    } catch (err) {
      throw new PortError(describeIoError(err));
    }

    let value: JsonValue;
    try {
      value = JSON.parse(content) as JsonValue;
    } catch (err) {
      throw new PortError(`failed to parse ${label} as JSON: ${describeIoError(err)}`);
    }

    modify(value);

    atomicWriteJsonFile(target, label, value);
  } finally {
    try {
      flockSync(fd, constants.LOCK_UN);
    } catch {
      // ignore unlock errors
    }
    fs.closeSync(fd);
  }
}

/** Navigate an object path, returning the deepest object or null if any key is missing. */
export function getPointer(value: JsonValue, keys: readonly string[]): JsonObject | null {
  let current = value;
  for (const key of keys) {
    if (current === null || typeof current !== 'object' || Array.isArray(current)) {
      return null;
    }
    const next = current[key];
    if (next === undefined) return null;
    current = next;
  }
  return current !== null && typeof current === 'object' && !Array.isArray(current)
    ? current
    : null;
}

/** Navigate to an object path, creating intermediate objects as needed. */
export function getPointerMut(value: JsonValue, keys: readonly string[]): JsonObject {
  let current = value as JsonObject;
  for (const key of keys) {
    const next = current[key];
    if (next === null || typeof next !== 'object' || Array.isArray(next)) {
      current[key] = {};
    }
    current = current[key] as JsonObject;
  }
  return current;
}

export { atomicWriteJsonFile, lockedReadModifyWriteJson, readJsonFileSync };