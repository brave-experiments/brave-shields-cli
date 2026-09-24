import * as nodePath from 'node:path';
import { PortError } from './errors.js';
import { readLocalState } from './local-state.js';
import { getPointer } from './json-io.js';
import type { JsonObject, JsonValue } from './json.js';

/** Information about a Brave profile. */
export interface ProfileInfo {
  dirName: string;
  displayName: string;
}

function infoCache(localState: JsonValue): JsonObject {
  const cache = getPointer(localState, ['profile', 'info_cache']);
  if (cache === null) {
    throw new PortError('Local State missing profile.info_cache');
  }
  return cache;
}

function displayNameOf(info: JsonValue): string {
  if (info !== null && typeof info === 'object' && !Array.isArray(info)) {
    const name = info['name'];
    if (typeof name === 'string') return name;
  }
  return '';
}

/** List all profiles from the Local State info_cache. */
export function listProfiles(braveDir: string): ProfileInfo[] {
  const localState = readLocalState(braveDir);
  const cache = infoCache(localState);

  // Rust's BTreeMap iterates in sorted key order; match it.
  const profiles: ProfileInfo[] = [];
  for (const dirName of Object.keys(cache).sort()) {
    const info = cache[dirName]!;
    const displayName = displayNameOf(info) || dirName;
    profiles.push({ dirName, displayName });
  }
  return profiles;
}

/**
 * Resolve the profile directory name from an optional user-specified name.
 * If no name is given, uses `profile.last_used` from Local State.
 * Accepts either a display name (e.g. "Work") or dir name (e.g. "Profile 1").
 */
export function resolveProfile(
  braveDir: string,
  profileArg?: string,
): ProfileInfo {
  const localState = readLocalState(braveDir);

  if (profileArg === undefined) {
    // Use last_used profile
    let lastUsed: string | null = null;
    if (
      localState !== null &&
      typeof localState === 'object' &&
      !Array.isArray(localState)
    ) {
      const profile = localState['profile'];
      if (
        profile !== null &&
        typeof profile === 'object' &&
        !Array.isArray(profile) &&
        typeof profile['last_used'] === 'string'
      ) {
        lastUsed = profile['last_used'];
      }
    }
    if (lastUsed === null) {
      throw new PortError('Local State missing profile.last_used');
    }

    // Note: if last_used contained / or ~, the JSON pointer would be
    // incorrect per RFC 6901. In practice Chromium profile dir names are
    // always simple strings like "Default" or "Profile 1".
    const infoEntry = getPointer(localState, ['profile', 'info_cache', lastUsed]);
    const displayName =
      infoEntry !== null
        ? displayNameOf(infoEntry) || lastUsed
        : lastUsed;

    return { dirName: lastUsed, displayName };
  }

  const name = profileArg;
  const cache = infoCache(localState);

  // First try: exact match on dir name
  const directInfo = cache[name];
  if (directInfo !== undefined) {
    const displayName = displayNameOf(directInfo) || name;
    return { dirName: name, displayName };
  }

  // Second try: match on display name
  for (const [dirName, info] of Object.entries(cache)) {
    const display = displayNameOf(info);
    if (display !== '' && display === name) {
      return { dirName, displayName: display };
    }
  }

  const available = Object.keys(cache)
    .sort()
    .map((dir) => {
      const display = displayNameOf(cache[dir]!) || dir;
      return `'${display}' (${dir})`;
    })
    .join(', ');
  throw new PortError(
    `profile '${name}' not found. Available profiles: ${available}`,
  );
}

/** Get the full path to the Preferences file for a resolved profile. */
export function preferencesPath(braveDir: string, profile: ProfileInfo): string {
  return nodePath.join(braveDir, profile.dirName, 'Preferences');
}