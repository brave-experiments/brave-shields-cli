import { renderList } from '../output.js';
import type { DomainOverrides, OutputFormat } from '../output.js';
import { readPreferences, getExceptionEntries } from '../preferences.js';
import {
  ALL_SETTINGS,
  domainFromPattern,
  fromStored,
  primaryJsonKey,
  readCosmeticValue,
  readSettingValue,
} from '../shields.js';
import type { ShieldSetting } from '../shields.js';
import type { JsonValue } from '../json.js';

export function listRun(prefsPath: string, format: OutputFormat): string {
  const prefs = readPreferences(prefsPath);

  // Collect all overrides keyed by domain
  const domainMap: Record<string, Record<string, string>> = {};
  for (const setting of ALL_SETTINGS) {
    collectSettingOverrides(prefs, setting, domainMap);
  }

  // BTreeMap iteration order parity: sorted domains and sorted setting names.
  const entries: DomainOverrides[] = Object.keys(domainMap)
    .sort()
    .map((domain) => {
      const overrides: Record<string, string> = {};
      for (const key of Object.keys(domainMap[domain]!).sort()) {
        overrides[key] = domainMap[domain]![key]!;
      }
      return { domain, overrides };
    });

  return renderList(entries, format);
}

function collectSettingOverrides(
  prefs: JsonValue,
  setting: ShieldSetting,
  domainMap: Record<string, Record<string, string>>,
): void {
  const primaryKey = primaryJsonKey(setting);
  const entries = getExceptionEntries(prefs, primaryKey);
  if (entries === null) return;

  for (const [pattern, entry] of Object.entries(entries)) {
    const domain = domainFromPattern(pattern) ?? pattern;

    const primaryValue = readSettingValue(entry);
    if (primaryValue === null) continue;

    let cosmeticValue: number | null = null;
    if (setting === 'ads') {
      const cosmeticEntries = getExceptionEntries(prefs, 'cosmeticFilteringV2');
      const cosmeticEntry =
        cosmeticEntries === null ? undefined : cosmeticEntries[pattern];
      if (cosmeticEntry !== undefined) {
        cosmeticValue = readCosmeticValue(cosmeticEntry);
      }
    }

    try {
      const cliValue = fromStored(setting, primaryValue, cosmeticValue);
      if (domainMap[domain] === undefined) domainMap[domain] = {};
      domainMap[domain]![setting] = cliValue;
    } catch {
      // from_stored error → skip (matches `if let Ok`)
    }
  }
}