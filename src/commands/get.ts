import { renderGet } from '../output.js';
import type { OutputFormat } from '../output.js';
import { readPreferences, getExceptionEntries } from '../preferences.js';
import {
  ALL_SETTINGS,
  fromStored,
  primaryJsonKey,
  readCosmeticValue,
  readSettingValue,
  resolvePattern,
} from '../shields.js';
import type { ShieldSetting } from '../shields.js';
import type { JsonValue } from '../json.js';

export function getRun(
  prefsPath: string,
  domain: string,
  rawPattern: boolean,
  profileName: string,
  format: OutputFormat,
): string {
  const prefs = readPreferences(prefsPath);
  const pattern = resolvePattern(domain, rawPattern);
  const settings: Record<string, string> = {};
  for (const setting of ALL_SETTINGS) {
    settings[setting] = readSettingValueForPattern(prefs, setting, pattern);
  }
  // Sort keys (BTreeMap iteration order parity, e.g. for table output).
  const sorted: Record<string, string> = {};
  for (const key of Object.keys(settings).sort()) {
    sorted[key] = settings[key]!;
  }
  return renderGet(domain, profileName, sorted, format);
}

function readSettingValueForPattern(
  prefs: JsonValue,
  setting: ShieldSetting,
  pattern: string,
): string {
  const primaryKey = primaryJsonKey(setting);
  const entries = getExceptionEntries(prefs, primaryKey);
  if (entries === null) return 'default';
  const entry = entries[pattern];
  if (entry === undefined) return 'default';

  const primaryValue = readSettingValue(entry);
  if (primaryValue === null) return 'default';

  // For ads, also read cosmeticFilteringV2
  let cosmeticValue: number | null = null;
  if (setting === 'ads') {
    const cosmeticEntries = getExceptionEntries(prefs, 'cosmeticFilteringV2');
    const cosmeticEntry = cosmeticEntries === null ? undefined : cosmeticEntries[pattern];
    if (cosmeticEntry !== undefined) {
      cosmeticValue = readCosmeticValue(cosmeticEntry);
    }
  }

  try {
    return fromStored(setting, primaryValue, cosmeticValue);
  } catch {
    return 'unknown';
  }
}