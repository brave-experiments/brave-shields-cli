import Table from 'cli-table3';
import { sortedStringify } from './json.js';
import type { JsonObject, JsonValue } from './json.js';
import { PortError } from './errors.js';

export type OutputFormat = 'json' | 'table';

/** Mirror of Rust's `OutputFormat::from_str` (validated in app code, not the parser). */
export function outputFormatFromStr(s: string): OutputFormat {
  if (s === 'json') return 'json';
  if (s === 'table') return 'table';
  throw new PortError(`unknown format '${s}'. Valid: json, table`);
}

/**
 * Build a table the way comfy-table's default (Dynamic arrangement) does.
 * Minor rendering drift vs comfy-table is explicitly allowed by the port plan.
 */
function newTable(headers: string[]): Table.Table {
  return new Table({ head: headers });
}

/** Output body for the `get` command (the caller prints it to stdout). */
export function renderGet(
  domain: string,
  profile: string,
  settings: Record<string, string>,
  format: OutputFormat,
): string {
  if (format === 'json') {
    const output: JsonValue = { domain, profile, settings };
    return sortedStringify(output);
  }
  const table = newTable(['Setting', 'Value']);
  for (const [k, v] of Object.entries(settings)) {
    table.push([k, v]);
  }
  return `Domain: ${domain}  Profile: ${profile}\n${table.toString()}`;
}

/** A single domain's overrides for the `list` command. */
export interface DomainOverrides {
  domain: string;
  overrides: Record<string, string>;
}

/** Output body for the `list` command (the caller prints it to stdout). */
export function renderList(entries: DomainOverrides[], format: OutputFormat): string {
  if (format === 'json') {
    const output: JsonValue = entries.map((e) => ({
      domain: e.domain,
      overrides: e.overrides,
    }));
    return sortedStringify(output);
  }
  if (entries.length === 0) {
    return 'No per-site shield overrides found.';
  }
  const table = newTable(['Domain', 'Overrides']);
  for (const entry of entries) {
    const overridesStr = Object.entries(entry.overrides)
      .map(([k, v]) => `${k}=${v}`)
      .join(', ');
    table.push([entry.domain, overridesStr]);
  }
  return table.toString();
}

export interface ProfileInfo {
  dirName: string;
  displayName: string;
}

/** Output body for the `profiles` command (the caller prints it to stdout). */
export function renderProfiles(profiles: ProfileInfo[], format: OutputFormat): string {
  if (format === 'json') {
    const output: JsonValue = profiles.map((p) => ({
      dir: p.dirName,
      name: p.displayName,
    }));
    return sortedStringify(output);
  }
  if (profiles.length === 0) {
    return 'No profiles found.';
  }
  const table = newTable(['Name', 'Directory']);
  for (const p of profiles) {
    table.push([p.displayName, p.dirName]);
  }
  return table.toString();
}

/** Output body for the `filters list` command (the caller prints it to stdout). */
export function renderFilters(filters: string[], format: OutputFormat): string {
  if (format === 'json') {
    const output: JsonObject = { custom_filters: filters };
    return sortedStringify(output);
  }
  if (filters.length === 0) {
    return 'No custom filter rules.';
  }
  const table = newTable(['Rule']);
  for (const rule of filters) {
    table.push([rule]);
  }
  return table.toString();
}

/** Diagnostic message for the `set` command (the caller prints it to stderr). */
export function renderSet(domain: string, setting: string, value: string): string {
  return `Set ${setting} = ${value} for ${domain}`;
}

/** Diagnostic message for the `reset` command (the caller prints it to stderr). */
export function renderReset(domain: string, setting?: string): string {
  if (setting !== undefined) {
    return `Reset ${setting} for ${domain}`;
  }
  return `Reset all shield settings for ${domain}`;
}