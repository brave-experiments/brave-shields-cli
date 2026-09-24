import { PortError } from '../errors.js';
import { describeIoError } from '../fsutil.js';
import * as fs from 'node:fs';
import {
  getCustomFilters,
  localStatePath,
  lockedReadModifyWrite,
  readLocalState,
} from '../local-state.js';
import { renderFilters } from '../output.js';
import type { OutputFormat } from '../output.js';
import type { RunResult } from '../output.js';
import { checkBraveNotRunning } from '../preferences.js';
import type { Channel } from '../platform.js';
import { getPointerMut } from '../json-io.js';
import type { JsonValue } from '../json.js';

function withContext<T>(fn: () => T, context: string): T {
  try {
    return fn();
  } catch (err) {
    throw new PortError(`${context}: ${describeIoError(err)}`);
  }
}

export function filtersList(braveDir: string, format: OutputFormat): string {
  const localState = readLocalState(braveDir);
  return renderFilters(getCustomFilters(localState), format);
}

export async function filtersAdd(
  braveDir: string,
  filter: string,
  channel: Channel,
  force: boolean,
): Promise<RunResult> {
  const trimmed = filter.trim();
  if (trimmed.length === 0) {
    throw new PortError('filter rule must not be empty');
  }

  const lsPath = localStatePath(braveDir);
  const { warning } = await checkBraveNotRunning(channel, force);

  withContext(() => {
    lockedReadModifyWrite(lsPath, (state) => {
      const filtersStr = pointerString(state, ['brave', 'ad_block', 'custom_filters']);

      // Check for duplicate (trimmed line comparison)
      const alreadyExists = filtersStr
        .split('\n')
        .some((line) => line.trim() === trimmed);
      if (alreadyExists) return;

      const newFilters =
        filtersStr.length === 0 ? trimmed : `${filtersStr}\n${trimmed}`;

      // Ensure brave.ad_block exists (auto-vivify, like Rust's IndexMut).
      getPointerMut(state, ['brave', 'ad_block'])['custom_filters'] = newFilters;
    });
  }, 'failed to add custom filter');

  return { warning, message: `Added custom filter: ${trimmed}` };
}

export async function filtersRemove(
  braveDir: string,
  filter: string,
  channel: Channel,
  force: boolean,
): Promise<RunResult> {
  const trimmed = filter.trim();
  if (trimmed.length === 0) {
    throw new PortError('filter rule must not be empty');
  }

  const lsPath = localStatePath(braveDir);
  const { warning } = await checkBraveNotRunning(channel, force);

  let found = false;
  withContext(() => {
    lockedReadModifyWrite(lsPath, (state) => {
      const filtersStr = pointerString(state, ['brave', 'ad_block', 'custom_filters']);

      const newFilters = filtersStr
        .split('\n')
        .filter((line) => {
          if (line.trim() === trimmed) {
            found = true;
            return false;
          }
          return true;
        });

      if (found) {
        getPointerMut(state, ['brave', 'ad_block'])['custom_filters'] =
          newFilters.join('\n');
      }
    });
  }, 'failed to remove custom filter');

  return {
    warning,
    message: found
      ? `Removed custom filter: ${trimmed}`
      : `Filter not found: ${trimmed}`,
  };
}

export async function filtersLoad(
  braveDir: string,
  filePath: string,
  channel: Channel,
  force: boolean,
): Promise<RunResult> {
  let content: string;
  try {
    content = fs.readFileSync(filePath, 'utf8');
  } catch (err) {
    throw new PortError(`failed to read filter file: ${filePath}: ${describeIoError(err)}`);
  }
  const rules = content
    .split('\n')
    .map((line) => (line.endsWith('\r') ? line.slice(0, -1) : line))
    .filter((line) => line.trim().length > 0);

  const lsPath = localStatePath(braveDir);
  const { warning } = await checkBraveNotRunning(channel, force);

  writeAllFilters(lsPath, rules.join('\n'));

  return {
    warning,
    message: `Loaded ${rules.length} filter rules from ${filePath}`,
  };
}

/** Write an arbitrary string as the full set of custom filters. */
export function writeAllFilters(lsPath: string, filters: string): void {
  withContext(() => {
    lockedReadModifyWrite(lsPath, (state) => {
      getPointerMut(state, ['brave', 'ad_block'])['custom_filters'] = filters;
    });
  }, 'failed to write custom filters');
}

export async function filtersClear(
  braveDir: string,
  channel: Channel,
  force: boolean,
): Promise<RunResult> {
  const lsPath = localStatePath(braveDir);
  const { warning } = await checkBraveNotRunning(channel, force);

  withContext(() => {
    lockedReadModifyWrite(lsPath, (state) => {
      if (pointerExists(state, ['brave', 'ad_block', 'custom_filters'])) {
        getPointerMut(state, ['brave', 'ad_block'])['custom_filters'] = '';
      }
    });
  }, 'failed to clear custom filters');

  return { warning, message: 'Cleared all custom filters' };
}

// --- helpers -------------------------------------------------------------

function pointerString(value: JsonValue, keys: readonly string[]): string {
  let cur = value;
  for (const key of keys) {
    if (cur === null || typeof cur !== 'object' || Array.isArray(cur)) return '';
    const next = cur[key];
    if (next === undefined) return '';
    cur = next;
  }
  return typeof cur === 'string' ? cur : '';
}

function pointerExists(value: JsonValue, keys: readonly string[]): boolean {
  let cur = value;
  for (const key of keys) {
    if (cur === null || typeof cur !== 'object' || Array.isArray(cur)) return false;
    const next = cur[key];
    if (next === undefined) return false;
    cur = next;
  }
  return true;
}