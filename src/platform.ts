import { PortError } from './errors.js';
import * as fs from 'node:fs';
import * as path from 'node:path';

/** Brave browser release channel. */
export type Channel = 'release' | 'beta' | 'nightly' | 'dev';

export const DEFAULT_CHANNEL: Channel = 'release';

/** Display name used in messages (matches Rust's `Display` impl). */
export function channelDisplay(channel: Channel): string {
  switch (channel) {
    case 'release':
      return 'Release';
    case 'beta':
      return 'Beta';
    case 'nightly':
      return 'Nightly';
    case 'dev':
      return 'Development';
  }
}

/** Parse a channel from its clap-style CLI value; null if unrecognized. */
export function parseChannel(s: string): Channel | null {
  switch (s) {
    case 'release':
    case 'beta':
    case 'nightly':
    case 'dev':
      return s;
    default:
      return null;
  }
}

/** Directory name suffix under BraveSoftware/ for each channel. */
function dirName(channel: Channel): string {
  switch (channel) {
    case 'release':
      return 'Brave-Browser';
    case 'beta':
      return 'Brave-Browser-Beta';
    case 'nightly':
      return 'Brave-Browser-Nightly';
    case 'dev':
      return 'Brave-Browser-Development';
  }
}

function braveBaseDir(home: string): string {
  switch (process.platform) {
    case 'darwin':
      // macOS: ~/Library/Application Support/BraveSoftware
      return path.join(home, 'Library', 'Application Support', 'BraveSoftware');
    case 'linux':
      // Linux: ~/.config/BraveSoftware
      return path.join(home, '.config', 'BraveSoftware');
    default:
      throw new PortError(
        'unsupported platform; use --brave-dir to specify the path manually',
      );
  }
}

/** Return the Brave Browser data directory for a given channel. */
export function braveDirForChannel(channel: Channel): string {
  const home = homeDir();
  return path.join(braveBaseDir(home), dirName(channel));
}

/**
 * Resolve the Brave data directory from --brave-dir override, --channel, or default.
 *
 * Precedence: --brave-dir > --channel > default (Release).
 * Returns the resolved directory plus an informational message that the CLI
 * layer must print to stderr so the user knows which browser is targeted
 * (library modules never print).
 */
export function resolveBraveDir(
  overrideDir: string | undefined,
  channel: Channel,
): { path: string; note?: string } {
  if (overrideDir !== undefined) {
    if (!isDirectory(overrideDir)) {
      throw new PortError(`specified --brave-dir does not exist: ${overrideDir}`);
    }
    return {
      path: overrideDir,
      note: `Using custom Brave directory: ${overrideDir}`,
    };
  }
  const p = braveDirForChannel(channel);
  if (!isDirectory(p)) {
    const display = channelDisplay(channel);
    throw new PortError(
      `Brave ${display} data directory not found at ${p}. Is Brave ${display} installed?`,
    );
  }
  return { path: p, note: `Using Brave ${channelDisplay(channel)} (${p})` };
}

function homeDir(): string {
  // Read at call time, never at import time.
  const home = process.env['HOME'];
  if (home === undefined || home.length === 0) {
    throw new PortError('could not determine home directory');
  }
  return home;
}

function isDirectory(p: string): boolean {
  try {
    return fs.statSync(p).isDirectory();
  } catch {
    return false;
  }
}