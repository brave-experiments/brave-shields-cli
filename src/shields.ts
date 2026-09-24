import { PortError } from './errors.js';
import type { JsonValue } from './json.js';

/** All shield setting types we support (CLI names). */
export type ShieldSetting =
  | 'shields'
  | 'ads'
  | 'fingerprinting'
  | 'https-upgrade'
  | 'scripts';

export const ALL_SETTINGS: readonly ShieldSetting[] = [
  'shields',
  'ads',
  'fingerprinting',
  'https-upgrade',
  'scripts',
];

const VALID_SETTING_NAMES = 'shields, ads, fingerprinting, https-upgrade, scripts';

/** JSON keys in Preferences exceptions that this setting reads/writes. */
const JSON_KEYS: Record<ShieldSetting, readonly string[]> = {
  shields: ['braveShields'],
  ads: ['shieldsAds', 'trackers', 'cosmeticFilteringV2'],
  fingerprinting: ['fingerprintingV2'],
  'https-upgrade': ['httpsUpgrades'],
  scripts: ['javascript'],
};

/** The primary JSON key used for reading the setting value. */
const PRIMARY_JSON_KEY: Record<ShieldSetting, string> = {
  shields: 'braveShields',
  ads: 'shieldsAds',
  fingerprinting: 'fingerprintingV2',
  'https-upgrade': 'httpsUpgrades',
  scripts: 'javascript',
};

/** Valid CLI values for this setting. */
const VALID_VALUES: Record<ShieldSetting, readonly string[]> = {
  shields: ['on', 'off'],
  ads: ['standard', 'aggressive', 'disabled'],
  fingerprinting: ['standard', 'aggressive', 'disabled'],
  'https-upgrade': ['standard', 'strict', 'disabled'],
  scripts: ['allow', 'block'],
};

export function jsonKeys(setting: ShieldSetting): readonly string[] {
  return JSON_KEYS[setting];
}

export function primaryJsonKey(setting: ShieldSetting): string {
  return PRIMARY_JSON_KEY[setting];
}

export function validValues(setting: ShieldSetting): readonly string[] {
  return VALID_VALUES[setting];
}

/** Parse a CLI setting name. */
export function shieldSettingFromCli(s: string): ShieldSetting {
  switch (s) {
    case 'shields':
    case 'ads':
    case 'fingerprinting':
    case 'https-upgrade':
    case 'scripts':
      return s;
    default:
      throw new PortError(
        `unknown setting '${s}'. Valid settings: ${VALID_SETTING_NAMES}`,
      );
  }
}

/** Validate that a domain contains only safe characters. */
function validateDomain(domain: string): void {
  if (domain.length === 0) {
    throw new PortError('domain must not be empty');
  }
  if (!/^[A-Za-z0-9._-]*$/.test(domain)) {
    throw new PortError(
      `invalid domain '${domain}': only ASCII alphanumeric characters, dots, hyphens, and underscores are allowed`,
    );
  }
}

/** Format a domain into the pattern key used in Preferences exceptions. */
export function domainPattern(domain: string): string {
  validateDomain(domain);
  return `${domain},*`;
}

/**
 * Resolve the Chromium content settings pattern from user input.
 * If `rawPattern` is true, the input is used as-is (must not be empty).
 * Otherwise, it is treated as a domain and converted to "domain,*".
 */
export function resolvePattern(input: string, rawPattern: boolean): string {
  if (rawPattern) {
    if (input.length === 0) {
      throw new PortError('pattern must not be empty');
    }
    return input;
  }
  return domainPattern(input);
}

/** Extract the domain from a pattern key like "example.com,*". */
export function domainFromPattern(pattern: string): string | null {
  return pattern.endsWith(',*') ? pattern.slice(0, -2) : null;
}

function simpleEntry(setting: number, lastModified: string): JsonValue {
  return { last_modified: lastModified, setting };
}

function cosmeticEntry(cosmeticVal: number, lastModified: string): JsonValue {
  return {
    last_modified: lastModified,
    setting: { cosmeticFilteringV2: cosmeticVal },
  };
}

/**
 * Convert a CLI value string to the stored JSON entries for a setting.
 * Returns an array of [jsonKey, entryValue] pairs.
 */
export function toStored(
  setting: ShieldSetting,
  cliValue: string,
  lastModified: string,
): Array<[string, JsonValue]> {
  switch (setting) {
    case 'shields': {
      let stored: number;
      if (cliValue === 'on') stored = 1;
      else if (cliValue === 'off') stored = 2;
      else
        throw new PortError(
          `invalid value '${cliValue}' for shields. Valid: on, off`,
        );
      return [['braveShields', simpleEntry(stored, lastModified)]];
    }
    case 'fingerprinting': {
      let stored: number;
      if (cliValue === 'standard') stored = 3;
      else if (cliValue === 'aggressive') stored = 2;
      else if (cliValue === 'disabled') stored = 1;
      else
        throw new PortError(
          `invalid value '${cliValue}' for fingerprinting. Valid: standard, aggressive, disabled`,
        );
      return [['fingerprintingV2', simpleEntry(stored, lastModified)]];
    }
    case 'https-upgrade': {
      let stored: number;
      if (cliValue === 'standard') stored = 3;
      else if (cliValue === 'strict') stored = 2;
      else if (cliValue === 'disabled') stored = 1;
      else
        throw new PortError(
          `invalid value '${cliValue}' for https-upgrade. Valid: standard, strict, disabled`,
        );
      return [['httpsUpgrades', simpleEntry(stored, lastModified)]];
    }
    case 'scripts': {
      let stored: number;
      if (cliValue === 'allow') stored = 1;
      else if (cliValue === 'block') stored = 2;
      else
        throw new PortError(
          `invalid value '${cliValue}' for scripts. Valid: allow, block`,
        );
      return [['javascript', simpleEntry(stored, lastModified)]];
    }
    case 'ads': {
      let adsVal: number;
      let cosmeticVal: number;
      if (cliValue === 'standard') [adsVal, cosmeticVal] = [2, 2];
      else if (cliValue === 'aggressive') [adsVal, cosmeticVal] = [2, 1];
      else if (cliValue === 'disabled') [adsVal, cosmeticVal] = [1, 0];
      else
        throw new PortError(
          `invalid value '${cliValue}' for ads. Valid: standard, aggressive, disabled`,
        );
      return [
        ['shieldsAds', simpleEntry(adsVal, lastModified)],
        ['trackers', simpleEntry(adsVal, lastModified)],
        ['cosmeticFilteringV2', cosmeticEntry(cosmeticVal, lastModified)],
      ];
    }
  }
}

/** Convert stored JSON values back to a CLI value string. */
export function fromStored(
  setting: ShieldSetting,
  primaryValue: number,
  cosmeticValue?: number | null,
): string {
  switch (setting) {
    case 'shields':
      switch (primaryValue) {
        case 1:
          return 'on';
        case 2:
          return 'off';
        default:
          throw new PortError(`unknown stored value ${primaryValue} for braveShields`);
      }
    case 'fingerprinting':
      switch (primaryValue) {
        case 3:
          return 'standard';
        case 2:
          return 'aggressive';
        case 1:
          return 'disabled';
        default:
          throw new PortError(`unknown stored value ${primaryValue} for fingerprintingV2`);
      }
    case 'https-upgrade':
      switch (primaryValue) {
        case 3:
          return 'standard';
        case 2:
          return 'strict';
        case 1:
          return 'disabled';
        default:
          throw new PortError(`unknown stored value ${primaryValue} for httpsUpgrades`);
      }
    case 'scripts':
      switch (primaryValue) {
        case 1:
          return 'allow';
        case 2:
          return 'block';
        default:
          throw new PortError(`unknown stored value ${primaryValue} for javascript`);
      }
    case 'ads': {
      const cosmetic = cosmeticValue ?? -1;
      if (primaryValue === 2 && cosmetic === 2) return 'standard';
      if (primaryValue === 2 && cosmetic === 1) return 'aggressive';
      if (primaryValue === 1 && cosmetic === 0) return 'disabled';
      if (primaryValue === 1) return 'disabled';
      if (primaryValue === 2) return 'standard';
      throw new PortError(
        `unknown stored values shieldsAds=${primaryValue}, cosmeticFilteringV2=${cosmetic} for ads`,
      );
    }
  }
}

/** Read the stored integer "setting" value from an exception entry. */
export function readSettingValue(entry: JsonValue): number | null {
  if (entry !== null && typeof entry === 'object' && !Array.isArray(entry)) {
    const setting = entry['setting'];
    if (typeof setting === 'number') return setting;
  }
  return null;
}

/**
 * Read the cosmetic filtering value from a cosmeticFilteringV2 exception entry.
 * The setting is stored as {"setting": {"cosmeticFilteringV2": N}}.
 */
export function readCosmeticValue(entry: JsonValue): number | null {
  if (entry !== null && typeof entry === 'object' && !Array.isArray(entry)) {
    const setting = entry['setting'];
    if (
      setting !== null &&
      typeof setting === 'object' &&
      !Array.isArray(setting) &&
      typeof setting['cosmeticFilteringV2'] === 'number'
    ) {
      return setting['cosmeticFilteringV2'];
    }
  }
  return null;
}