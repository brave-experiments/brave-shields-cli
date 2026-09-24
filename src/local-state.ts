import * as nodePath from 'node:path';
import {
  atomicWriteJsonFile,
  lockedReadModifyWriteJson,
  readJsonFileSync,
} from './json-io.js';
import type { JsonValue } from './json.js';

/** Read and parse the Local State file from the Brave data directory. */
export function readLocalState(braveDir: string): JsonValue {
  const target = nodePath.join(braveDir, 'Local State');
  return readJsonFileSync(target, 'Local State');
}

/** Extract custom filter rules from Local State as a list of non-empty lines. */
export function getCustomFilters(localState: JsonValue): string[] {
  let customFilters: string | null = null;
  if (
    localState !== null &&
    typeof localState === 'object' &&
    !Array.isArray(localState)
  ) {
    const brave = localState['brave'];
    if (brave !== null && typeof brave === 'object' && !Array.isArray(brave)) {
      const adBlock = brave['ad_block'];
      if (
        adBlock !== null &&
        typeof adBlock === 'object' &&
        !Array.isArray(adBlock)
      ) {
        const cf = adBlock['custom_filters'];
        if (typeof customFilters !== 'string' && typeof cf === 'string') {
          customFilters = cf;
        }
      }
    }
  }
  if (customFilters === null) customFilters = '';
  return customFilters
    .split('\n')
    .map((line) => (line.endsWith('\r') ? line.slice(0, -1) : line))
    .filter((line) => line.trim().length > 0);
}

/** Write Local State to disk atomically, with backup and verification. */
export function writeLocalState(target: string, state: JsonValue): void {
  atomicWriteJsonFile(target, 'Local State', state);
}

/**
 * Open the Local State file with an exclusive lock, read-modify-write, then
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
  modify: (state: JsonValue) => void,
): void {
  lockedReadModifyWriteJson(target, 'Local State', modify);
}

export function localStatePath(braveDir: string): string {
  return nodePath.join(braveDir, 'Local State');
}